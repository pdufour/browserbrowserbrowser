//! Inner “browser”: WASM build uses **Dioxus Blitz** (Stylo + Taffy + Vello) to render HTML/CSS offscreen,
//! then draws RGBA into `<canvas>`. Requires WebGPU.

mod document_url;
#[cfg(target_arch = "wasm32")]
mod blitz_wasm;

use html5ever::driver::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever::local_name;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}

#[derive(Serialize)]
pub struct RenderedPage {
    pub url: String,
    pub title: String,
    pub text: String,
    pub links: Vec<String>,
}

#[derive(Serialize)]
pub struct PaintResult {
    pub url: String,
    pub title: String,
    pub links: Vec<String>,
    pub height_css_px: f64,
}

pub(crate) struct AnalyzedPage {
    pub page: RenderedPage,
}

#[wasm_bindgen(js_name = renderDocument)]
pub fn render_document(html: &str, page_url: &str) -> Result<JsValue, JsValue> {
    let analyzed = analyze_page(html, page_url).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&analyzed.page).map_err(|e| JsValue::from_str(&e.to_string()))
}

fn parse_rcdom(html: &str) -> Result<RcDom, String> {
    let dom = RcDom::default();
    parse_document(dom, html5ever::ParseOpts::default())
        .from_utf8()
        .read_from(&mut std::io::Cursor::new(html.as_bytes()))
        .map_err(|e| e.to_string())
}

fn analyze_page(html: &str, page_url: &str) -> Result<AnalyzedPage, String> {
    let dom = parse_rcdom(html)?;
    let root = dom.document.clone();

    let title = find_first_element(&root, |h| {
        matches!(
            &h.data,
            NodeData::Element { name, .. } if name.local == local_name!("title")
        )
    })
    .map(|h| collect_text(&h))
    .unwrap_or_default();
    let title = html_escape::decode_html_entities(&title).to_string();

    let text = find_first_element(&root, |h| {
        matches!(
            &h.data,
            NodeData::Element { name, .. } if name.local == local_name!("body")
        )
    })
    .map(|h| collect_visible_text(&h))
    .unwrap_or_else(|| collect_visible_text(&root));

    let mut links = Vec::new();
    collect_hrefs(&root, &mut links);
    let base = document_url::effective_base_url(page_url, &root);
    let links: Vec<String> = links
        .into_iter()
        .filter(|h| !h.starts_with('#') && h != "javascript:void(0)")
        .take(100)
        .filter_map(|h| {
            if let Some(ref b) = base {
                b.join(&h).ok().map(|u| u.to_string())
            } else {
                Some(h)
            }
        })
        .collect();

    let page = RenderedPage {
        url: page_url.to_string(),
        title: title.clone(),
        text: collapse_ws(&text),
        links,
    };

    Ok(AnalyzedPage { page })
}

pub(crate) fn canvas_context(canvas: &HtmlCanvasElement) -> Result<CanvasRenderingContext2d, JsValue> {
    canvas
        .get_context("2d")
        .map_err(|_| JsValue::from_str("canvas 2d"))?
        .ok_or_else(|| JsValue::from_str("2d unsupported"))?
        .dyn_into()
        .map_err(|_| JsValue::from_str("2d context"))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = paintDocumentHtml)]
pub async fn paint_document_html(
    canvas: &HtmlCanvasElement,
    html: &str,
    page_url: &str,
    css_width: f64,
    device_pixel_ratio: f64,
) -> Result<JsValue, JsValue> {
    let analyzed = analyze_page(html, page_url).map_err(|e| JsValue::from_str(&e))?;
    let out = blitz_wasm::paint_blitz_async(
        canvas,
        html,
        page_url,
        &analyzed.page,
        css_width,
        device_pixel_ratio,
    )
    .await?;
    serde_wasm_bindgen::to_value(&out).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[cfg(not(target_arch = "wasm32"))]
#[wasm_bindgen(js_name = paintDocumentHtml)]
pub async fn paint_document_html(
    _canvas: &HtmlCanvasElement,
    _html: &str,
    _page_url: &str,
    _css_width: f64,
    _device_pixel_ratio: f64,
) -> Result<JsValue, JsValue> {
    Err(JsValue::from_str("paintDocumentHtml is wasm-only"))
}

async fn fetch_text_with_cors(url: &str) -> Result<String, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(web_sys::RequestMode::Cors);

    let request = web_sys::Request::new_with_str_and_init(url, &opts)
        .map_err(|_| JsValue::from_str("invalid request"))?;

    let resp_val = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| JsValue::from_str("fetch failed"))?;

    let resp: web_sys::Response = resp_val
        .dyn_into()
        .map_err(|_| JsValue::from_str("bad response"))?;

    if !resp.ok() {
        return Err(JsValue::from_str(&format!("HTTP {}", resp.status())));
    }

    let text_val = JsFuture::from(resp.text().map_err(|_| JsValue::from_str("no body"))?)
        .await
        .map_err(|_| JsValue::from_str("read failed"))?;

    text_val
        .as_string()
        .ok_or_else(|| JsValue::from_str("empty body"))
}

async fn inline_stylesheets_for_blitz(html: &str, fetch_url: &str) -> String {
    let Ok(dom) = parse_rcdom(html) else {
        return html.to_string();
    };
    let root = dom.document.clone();
    let base = document_url::effective_base_url(fetch_url, &root);
    let mut hrefs = Vec::new();
    collect_stylesheet_hrefs(&root, &mut hrefs);
    hrefs.truncate(24);

    let mut css = String::new();
    for href in hrefs {
        let abs = if let Some(ref b) = base {
            if let Ok(u) = b.join(&href) {
                u.to_string()
            } else {
                continue;
            }
        } else {
            continue;
        };
        let fetch_css_url = document_url::subresource_fetch_url(fetch_url, &abs);
        if let Ok(txt) = fetch_text_with_cors(&fetch_css_url).await {
            css.push_str("\n/* ");
            css.push_str(&abs);
            css.push_str(" */\n");
            css.push_str(&txt);
        }
    }
    if css.is_empty() {
        return html.to_string();
    }

    let injected = format!("<style>{css}</style>");
    if html.contains("</head>") {
        html.replacen("</head>", &(injected + "</head>"), 1)
    } else {
        format!("{injected}{html}")
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = fetchAndPaint)]
pub async fn fetch_and_paint(
    canvas: &HtmlCanvasElement,
    url: &str,
    css_width: f64,
    device_pixel_ratio: f64,
) -> Result<JsValue, JsValue> {
    let html = fetch_text_with_cors(url).await?;
    let html = inline_stylesheets_for_blitz(&html, url).await;
    let analyzed = analyze_page(&html, url).map_err(|e| JsValue::from_str(&e))?;
    let out = blitz_wasm::paint_blitz_async(
        canvas,
        &html,
        url,
        &analyzed.page,
        css_width,
        device_pixel_ratio,
    )
    .await?;
    serde_wasm_bindgen::to_value(&out).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[cfg(not(target_arch = "wasm32"))]
#[wasm_bindgen(js_name = fetchAndPaint)]
pub async fn fetch_and_paint(
    _canvas: &HtmlCanvasElement,
    _url: &str,
    _css_width: f64,
    _device_pixel_ratio: f64,
) -> Result<JsValue, JsValue> {
    Err(JsValue::from_str("fetchAndPaint is wasm-only"))
}

#[wasm_bindgen(js_name = fetchAndRender)]
pub async fn fetch_and_render(url: &str) -> Result<JsValue, JsValue> {
    let html = fetch_text_with_cors(url).await?;
    render_document(&html, url)
}

fn find_first_element<F>(handle: &Handle, pred: F) -> Option<Handle>
where
    F: Copy + Fn(&Handle) -> bool,
{
    if pred(handle) {
        return Some(handle.clone());
    }
    for child in handle.children.borrow().iter() {
        if let Some(h) = find_first_element(child, pred) {
            return Some(h);
        }
    }
    None
}

fn collect_text(handle: &Handle) -> String {
    match &handle.data {
        NodeData::Text { contents } => contents.borrow().to_string(),
        _ => handle
            .children
            .borrow()
            .iter()
            .map(collect_text)
            .collect::<String>(),
    }
}

fn collect_visible_text(handle: &Handle) -> String {
    match &handle.data {
        NodeData::Text { contents } => contents.borrow().to_string(),
        NodeData::Element { name, .. } => {
            let skip = name.local == local_name!("script")
                || name.local == local_name!("style")
                || name.local == local_name!("noscript");
            if skip {
                return String::new();
            }
            let parts: Vec<String> = handle
                .children
                .borrow()
                .iter()
                .map(collect_visible_text)
                .filter(|s| !s.is_empty())
                .collect();
            parts.join(" ")
        }
        _ => handle
            .children
            .borrow()
            .iter()
            .map(collect_visible_text)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn collect_hrefs(handle: &Handle, out: &mut Vec<String>) {
    if let NodeData::Element { name, attrs, .. } = &handle.data {
        if name.local == local_name!("a") {
            for a in attrs.borrow().iter() {
                if a.name.local == local_name!("href") {
                    out.push(a.value.to_string());
                }
            }
        }
    }
    for child in handle.children.borrow().iter() {
        collect_hrefs(child, out);
    }
}

fn collect_stylesheet_hrefs(handle: &Handle, out: &mut Vec<String>) {
    if let NodeData::Element { name, attrs, .. } = &handle.data {
        if name.local == local_name!("link") {
            let attrs = attrs.borrow();
            let mut rel_ok = false;
            let mut href: Option<String> = None;
            for a in attrs.iter() {
                if a.name.local == local_name!("rel") {
                    rel_ok = a
                        .value
                        .to_ascii_lowercase()
                        .split_whitespace()
                        .any(|v| v == "stylesheet");
                } else if a.name.local == local_name!("href") {
                    href = Some(a.value.to_string());
                }
            }
            if rel_ok {
                if let Some(h) = href {
                    out.push(h);
                }
            }
        }
    }
    for child in handle.children.borrow().iter() {
        collect_stylesheet_hrefs(child, out);
    }
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
