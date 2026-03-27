//! Inner “browser”: **Servo html5ever** parses HTML; we extract structure and **paint** a readable
//! layout to a `<canvas>` using real `measureText` wrapping (not Servo/WebRender paint).
mod flow;
mod paint_canvas;

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

#[wasm_bindgen(js_name = renderDocument)]
pub fn render_document(html: &str, page_url: &str) -> Result<JsValue, JsValue> {
    let (page, _) = analyze_page(html, page_url).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&page).map_err(|e| JsValue::from_str(&e.to_string()))
}

fn parse_rcdom(html: &str) -> Result<RcDom, String> {
    let dom = RcDom::default();
    parse_document(dom, html5ever::ParseOpts::default())
        .from_utf8()
        .read_from(&mut std::io::Cursor::new(html.as_bytes()))
        .map_err(|e| e.to_string())
}

fn analyze_page(html: &str, page_url: &str) -> Result<(RenderedPage, Vec<flow::FlowBlock>), String> {
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
    let base = url::Url::parse(page_url).ok();
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

    let body = find_first_element(&root, |h| {
        matches!(
            &h.data,
            NodeData::Element { name, .. } if name.local == local_name!("body")
        )
    })
    .unwrap_or_else(|| root.clone());
    let blocks = flow::extract_flow(&body);

    let page = RenderedPage {
        url: page_url.to_string(),
        title: title.clone(),
        text: collapse_ws(&text),
        links,
    };

    Ok((page, blocks))
}

fn canvas_context(canvas: &HtmlCanvasElement) -> Result<CanvasRenderingContext2d, JsValue> {
    canvas
        .get_context("2d")
        .map_err(|_| JsValue::from_str("canvas 2d"))?
        .ok_or_else(|| JsValue::from_str("2d unsupported"))?
        .dyn_into()
        .map_err(|_| JsValue::from_str("2d context"))
}

/// Parse `html`, lay out with **`measureText`**, and paint to `canvas`. Returns metadata and content height (CSS px).
#[wasm_bindgen(js_name = paintDocumentHtml)]
pub fn paint_document_html(
    canvas: &HtmlCanvasElement,
    html: &str,
    page_url: &str,
    css_width: f64,
    device_pixel_ratio: f64,
) -> Result<JsValue, JsValue> {
    let dpr = device_pixel_ratio.max(1.0);
    let css_w = css_width.max(120.0);

    let (meta, blocks) = analyze_page(html, page_url).map_err(|e| JsValue::from_str(&e))?;

    canvas.set_width((css_w * dpr).round().max(1.0) as u32);
    canvas.set_height(1);

    let ctx = canvas_context(canvas)?;
    let _ = ctx.scale(dpr, dpr);

    let height = paint_canvas::measure_content_height(&ctx, css_w, &meta.title, &blocks)?;

    canvas.set_height((height * dpr).round().max(1.0) as u32);
    let ctx = canvas_context(canvas)?;
    let _ = ctx.scale(dpr, dpr);
    paint_canvas::fill_and_draw(&ctx, css_w, height, &meta.title, &blocks)?;

    let out = PaintResult {
        url: meta.url,
        title: meta.title,
        links: meta.links,
        height_css_px: height,
    };
    serde_wasm_bindgen::to_value(&out).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Fetch then [`paint_document_html`].
#[wasm_bindgen(js_name = fetchAndPaint)]
pub async fn fetch_and_paint(
    canvas: &HtmlCanvasElement,
    url: &str,
    css_width: f64,
    device_pixel_ratio: f64,
) -> Result<JsValue, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(web_sys::RequestMode::Cors);

    let request = web_sys::Request::new_with_str_and_init(url, &opts)
        .map_err(|_| JsValue::from_str("invalid URL or request"))?;

    let resp_val = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| {
            JsValue::from_str("network/CORS error — try another URL or a CORS-friendly page")
        })?;

    let resp: web_sys::Response = resp_val
        .dyn_into()
        .map_err(|_| JsValue::from_str("bad response"))?;

    if !resp.ok() {
        return Err(JsValue::from_str(&format!("HTTP {}", resp.status())));
    }

    let text_val = JsFuture::from(resp.text().map_err(|_| JsValue::from_str("no text body"))?)
        .await
        .map_err(|_| JsValue::from_str("read error"))?;

    let html = text_val
        .as_string()
        .ok_or_else(|| JsValue::from_str("empty body"))?;

    paint_document_html(canvas, &html, url, css_width, device_pixel_ratio)
}

/// Fetch a URL with the browser’s `fetch` and return [`RenderedPage`]
#[wasm_bindgen(js_name = fetchAndRender)]
pub async fn fetch_and_render(url: &str) -> Result<JsValue, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(web_sys::RequestMode::Cors);

    let request = web_sys::Request::new_with_str_and_init(url, &opts)
        .map_err(|_| JsValue::from_str("invalid URL or request"))?;

    let resp_val = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| JsValue::from_str("network/CORS error — try another URL or a CORS-friendly page"))?;

    let resp: web_sys::Response = resp_val
        .dyn_into()
        .map_err(|_| JsValue::from_str("bad response"))?;

    if !resp.ok() {
        return Err(JsValue::from_str(&format!("HTTP {}", resp.status())));
    }

    let text_val = JsFuture::from(resp.text().map_err(|_| JsValue::from_str("no text body"))?)
        .await
        .map_err(|_| JsValue::from_str("read error"))?;

    let html = text_val
        .as_string()
        .ok_or_else(|| JsValue::from_str("empty body"))?;

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

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
