//! Block-level structure extracted from the Servo html5ever DOM (subset).
use markup5ever::local_name;
use markup5ever_rcdom::{Handle, NodeData};

#[derive(Debug)]
pub enum FlowBlock {
    Heading { level: u8, text: String },
    Paragraph { text: String },
    Pre { text: String },
    ListItem { marker: String, text: String },
    Rule,
}

pub fn extract_flow(root: &Handle) -> Vec<FlowBlock> {
    let mut out = Vec::new();
    extract_flow_inner(root, &mut out);
    out
}

fn extract_flow_inner(handle: &Handle, out: &mut Vec<FlowBlock>) {
    match &handle.data {
        NodeData::Text { contents } => {
            let t = contents.borrow();
            let t = t.trim();
            if !t.is_empty() {
                out.push(FlowBlock::Paragraph {
                    text: normalize_ws(t),
                });
            }
        }
        NodeData::Element { name, .. } => {
            if skip_local(&name.local) {
                return;
            }
            if name.local == local_name!("br") {
                out.push(FlowBlock::Paragraph {
                    text: "\n".into(),
                });
                return;
            }
            if name.local == local_name!("hr") {
                out.push(FlowBlock::Rule);
                return;
            }
            if name.local == local_name!("ul") {
                for c in handle.children.borrow().iter() {
                    if let NodeData::Element { name: n, .. } = &c.data {
                        if n.local == local_name!("li") {
                            push_li(out, "• ", c);
                        } else {
                            extract_flow_inner(c, out);
                        }
                    } else {
                        extract_flow_inner(c, out);
                    }
                }
                return;
            }
            if name.local == local_name!("ol") {
                let mut i = 1u32;
                for c in handle.children.borrow().iter() {
                    if let NodeData::Element { name: n, .. } = &c.data {
                        if n.local == local_name!("li") {
                            push_li(out, &format!("{i}. "), c);
                            i = i.saturating_add(1);
                        } else {
                            extract_flow_inner(c, out);
                        }
                    } else {
                        extract_flow_inner(c, out);
                    }
                }
                return;
            }
            if name.local == local_name!("li") {
                push_li(out, "• ", handle);
                return;
            }
            if let Some(level) = heading_level(&name.local) {
                let text = collect_visible_inline(handle);
                if !text.is_empty() {
                    out.push(FlowBlock::Heading { level, text });
                }
                return;
            }
            if name.local == local_name!("pre") {
                let text = collect_preformatted(handle);
                if !text.trim().is_empty() || text.contains('\n') {
                    out.push(FlowBlock::Pre { text });
                }
                return;
            }
            if is_block_local(&name.local) {
                if structure_block_child(handle) {
                    for c in handle.children.borrow().iter() {
                        extract_flow_inner(c, out);
                    }
                } else {
                    let text = collect_visible_inline(handle);
                    let text = normalize_ws_multiline(&text);
                    if !text.is_empty() {
                        out.push(FlowBlock::Paragraph { text });
                    }
                }
                return;
            }
            for c in handle.children.borrow().iter() {
                extract_flow_inner(c, out);
            }
        }
        _ => {}
    }
}

fn push_li(out: &mut Vec<FlowBlock>, marker: &str, li: &Handle) {
    let text = normalize_ws(&collect_visible_inline(li));
    if !text.is_empty() {
        out.push(FlowBlock::ListItem {
            marker: marker.to_string(),
            text,
        });
    }
}

fn heading_level(local: &markup5ever::LocalName) -> Option<u8> {
    if *local == local_name!("h1") {
        Some(1)
    } else if *local == local_name!("h2") {
        Some(2)
    } else if *local == local_name!("h3") {
        Some(3)
    } else if *local == local_name!("h4") {
        Some(4)
    } else if *local == local_name!("h5") {
        Some(5)
    } else if *local == local_name!("h6") {
        Some(6)
    } else {
        None
    }
}

fn skip_local(local: &markup5ever::LocalName) -> bool {
    *local == local_name!("script")
        || *local == local_name!("style")
        || *local == local_name!("noscript")
        || *local == local_name!("svg")
        || *local == local_name!("iframe")
}

fn is_block_local(local: &markup5ever::LocalName) -> bool {
    *local == local_name!("p")
        || *local == local_name!("div")
        || *local == local_name!("section")
        || *local == local_name!("article")
        || *local == local_name!("main")
        || *local == local_name!("nav")
        || *local == local_name!("header")
        || *local == local_name!("footer")
        || *local == local_name!("aside")
        || *local == local_name!("blockquote")
        || *local == local_name!("figure")
        || *local == local_name!("figcaption")
        || *local == local_name!("address")
        || *local == local_name!("body")
}

fn structure_block_child(handle: &Handle) -> bool {
    handle.children.borrow().iter().any(|c| {
        matches!(
            &c.data,
            NodeData::Element { name, .. }
                if is_block_local(&name.local)
                    || name.local == local_name!("ul")
                    || name.local == local_name!("ol")
                    || name.local == local_name!("li")
                    || name.local == local_name!("hr")
                    || heading_level(&name.local).is_some()
                    || name.local == local_name!("pre")
                    || name.local == local_name!("br")
        )
    })
}

fn collect_visible_inline(handle: &Handle) -> String {
    match &handle.data {
        NodeData::Text { contents } => contents.borrow().to_string(),
        NodeData::Element { name, .. } => {
            if skip_local(&name.local) {
                return String::new();
            }
            if name.local == local_name!("br") {
                return "\n".to_string();
            }
            handle
                .children
                .borrow()
                .iter()
                .map(collect_visible_inline)
                .collect::<String>()
        }
        _ => String::new(),
    }
}

fn collect_preformatted(handle: &Handle) -> String {
    match &handle.data {
        NodeData::Text { contents } => contents.borrow().to_string(),
        NodeData::Element { name, .. } => {
            if skip_local(&name.local) {
                return String::new();
            }
            if name.local == local_name!("br") {
                let inner: String = handle
                    .children
                    .borrow()
                    .iter()
                    .map(collect_preformatted)
                    .collect();
                return format!("\n{inner}");
            }
            handle
                .children
                .borrow()
                .iter()
                .map(collect_preformatted)
                .collect()
        }
        _ => String::new(),
    }
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_ws_multiline(s: &str) -> String {
    s.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}
