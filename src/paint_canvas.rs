#![cfg_attr(target_arch = "wasm32", allow(dead_code))]
//! Canvas 2D paint: word-wrap using real `measureText`, then draw (fixed typography, no page CSS).
//! Used when the library is built for non-WASM targets; WASM uses Blitz (`blitz_wasm`).
use crate::flow::FlowBlock;
use wasm_bindgen::JsValue;
use web_sys::CanvasRenderingContext2d;

const PAD_X: f64 = 18.0;
const PAD_Y: f64 = 18.0;
const TITLE_FONT: &str = "600 17px ui-sans-serif, system-ui, sans-serif";
const TITLE_GAP: f64 = 14.0;
const PAR_FONT: &str = "16px ui-sans-serif, system-ui, sans-serif";
const LI_FONT: &str = "16px ui-sans-serif, system-ui, sans-serif";
const PRE_FONT: &str = "14px ui-monospace, SFMono-Regular, Menlo, monospace";
const TEXT: &str = "#0e1116";
const ACCENT: &str = "#2c4aa3";
const RULE: &str = "#c9ced8";
const PAPER: &str = "#f5f5f2";

fn heading_style(level: u8) -> (&'static str, f64, f64) {
    match level {
        1 => ("700 28px ui-sans-serif, system-ui, sans-serif", 32.0, 14.0),
        2 => ("700 22px ui-sans-serif, system-ui, sans-serif", 26.0, 12.0),
        3 => ("650 19px ui-sans-serif, system-ui, sans-serif", 24.0, 10.0),
        4 => ("650 17px ui-sans-serif, system-ui, sans-serif", 22.0, 8.0),
        _ => ("600 16px ui-sans-serif, system-ui, sans-serif", 22.0, 8.0),
    }
}

pub fn measure_content_height(
    ctx: &CanvasRenderingContext2d,
    css_width: f64,
    doc_title: &str,
    blocks: &[FlowBlock],
) -> Result<f64, JsValue> {
    let inner_w = (css_width - PAD_X * 2.0).max(120.0);
    let mut y = PAD_Y;

    ctx.set_font(TITLE_FONT);
    let title_lines = wrap_hard_break(ctx, doc_title, inner_w)?;
    let title_lh = 22.0;
    y += title_lines.len() as f64 * title_lh;
    y += TITLE_GAP;

    for block in blocks {
        match block {
            FlowBlock::Heading { level, text } => {
                if text.is_empty() {
                    continue;
                }
                let (font, lh, gap) = heading_style(*level);
                ctx.set_font(font);
                let lines = wrap_flow(ctx, text, inner_w, true)?;
                y += lines.len() as f64 * lh + gap;
            }
            FlowBlock::Paragraph { text } => {
                ctx.set_font(PAR_FONT);
                let lh = 22.0;
                let lines = wrap_flow(ctx, text, inner_w, true)?;
                y += lines.len() as f64 * lh + 10.0;
            }
            FlowBlock::Pre { text } => {
                ctx.set_font(PRE_FONT);
                let lh = 20.0;
                for raw_line in text.split('\n') {
                    let lines = wrap_monospace_line(ctx, raw_line, inner_w, lh)?;
                    y += lines.len() as f64 * lh;
                }
                y += 10.0;
            }
            FlowBlock::ListItem { marker, text } => {
                ctx.set_font(LI_FONT);
                let lh = 22.0;
                let indent = 18.0;
                let mark_w = ctx.measure_text(marker)?.width();
                let lines = wrap_flow(ctx, text, inner_w - indent - mark_w, true)?;
                y += lines.len() as f64 * lh + 4.0;
            }
            FlowBlock::Rule => {
                y += 6.0 + 14.0;
            }
        }
    }

    Ok(y + PAD_Y)
}

pub fn fill_and_draw(
    ctx: &CanvasRenderingContext2d,
    css_width: f64,
    height: f64,
    doc_title: &str,
    blocks: &[FlowBlock],
) -> Result<(), JsValue> {
    ctx.set_fill_style_str(PAPER);
    ctx.fill_rect(0.0, 0.0, css_width, height);
    draw_content(ctx, css_width, doc_title, blocks)
}

fn draw_content(
    ctx: &CanvasRenderingContext2d,
    css_width: f64,
    doc_title: &str,
    blocks: &[FlowBlock],
) -> Result<(), JsValue> {
    let inner_w = (css_width - PAD_X * 2.0).max(120.0);
    let mut y = PAD_Y;

    ctx.set_font(TITLE_FONT);
    ctx.set_fill_style_str(ACCENT);
    let title_lines = wrap_hard_break(ctx, doc_title, inner_w)?;
    let title_lh = 22.0;
    for line in &title_lines {
        ctx.fill_text(line, PAD_X, y + title_lh)?;
        y += title_lh;
    }
    y += TITLE_GAP;
    ctx.set_fill_style_str(TEXT);

    for block in blocks {
        match block {
            FlowBlock::Heading { level, text } => {
                if text.is_empty() {
                    continue;
                }
                let (font, lh, gap) = heading_style(*level);
                ctx.set_font(font);
                let lines = wrap_flow(ctx, text, inner_w, true)?;
                for line in &lines {
                    ctx.fill_text(line, PAD_X, y + lh)?;
                    y += lh;
                }
                y += gap;
            }
            FlowBlock::Paragraph { text } => {
                ctx.set_font(PAR_FONT);
                let lh = 22.0;
                let lines = wrap_flow(ctx, text, inner_w, true)?;
                for line in &lines {
                    ctx.fill_text(line, PAD_X, y + lh)?;
                    y += lh;
                }
                y += 10.0;
            }
            FlowBlock::Pre { text } => {
                ctx.set_font(PRE_FONT);
                let lh = 20.0;
                for raw_line in text.split('\n') {
                    let lines = wrap_monospace_line(ctx, raw_line, inner_w, lh)?;
                    for line in &lines {
                        ctx.fill_text(line, PAD_X, y + lh)?;
                        y += lh;
                    }
                }
                y += 10.0;
            }
            FlowBlock::ListItem { marker, text } => {
                ctx.set_font(LI_FONT);
                let lh = 22.0;
                let indent = 18.0;
                let mark_w = ctx.measure_text(marker)?.width();
                let lines = wrap_flow(ctx, text, inner_w - indent - mark_w, true)?;
                let mut first = true;
                for line in &lines {
                    let x_text = PAD_X + indent + mark_w;
                    if first {
                        ctx.set_fill_style_str(ACCENT);
                        ctx.fill_text(marker, PAD_X, y + lh)?;
                        ctx.set_fill_style_str(TEXT);
                        first = false;
                    }
                    ctx.fill_text(line, x_text, y + lh)?;
                    y += lh;
                }
                y += 4.0;
            }
            FlowBlock::Rule => {
                y += 6.0;
                ctx.set_stroke_style_str(RULE);
                ctx.set_line_width(1.0);
                ctx.begin_path();
                ctx.move_to(PAD_X, y);
                ctx.line_to(PAD_X + inner_w, y);
                ctx.stroke();
                y += 14.0;
                ctx.set_stroke_style_str(TEXT);
            }
        }
    }
    Ok(())
}

fn wrap_hard_break(
    ctx: &CanvasRenderingContext2d,
    text: &str,
    max_w: f64,
) -> Result<Vec<String>, JsValue> {
    if text.is_empty() {
        return Ok(vec!["(no title)".into()]);
    }
    wrap_flow(ctx, text, max_w, true)
}

fn wrap_flow(
    ctx: &CanvasRenderingContext2d,
    text: &str,
    max_w: f64,
    wrap_words: bool,
) -> Result<Vec<String>, JsValue> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        if para.is_empty() {
            out.push(String::new());
            continue;
        }
        if !wrap_words {
            out.push(para.to_string());
            continue;
        }
        let words: Vec<&str> = para.split_whitespace().collect();
        let mut line = String::new();
        for w in words {
            let trial = if line.is_empty() {
                w.to_string()
            } else {
                format!("{line} {w}")
            };
            let wid = ctx.measure_text(&trial)?.width();
            if wid > max_w && !line.is_empty() {
                out.push(line);
                line = w.to_string();
            } else {
                line = trial;
            }
        }
        if !line.is_empty() {
            out.push(line);
        }
    }
    if out.is_empty() && !text.is_empty() {
        out.push(text.to_string());
    }
    Ok(out)
}

fn wrap_monospace_line(
    ctx: &CanvasRenderingContext2d,
    line: &str,
    max_w: f64,
    _lh: f64,
) -> Result<Vec<String>, JsValue> {
    if line.is_empty() {
        return Ok(vec![String::new()]);
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in line.chars() {
        let trial = format!("{cur}{ch}");
        let wid = ctx.measure_text(&trial)?.width();
        if wid > max_w && !cur.is_empty() {
            out.push(cur);
            cur = ch.to_string();
        } else {
            cur = trial;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    Ok(out)
}
