# browser-in-browser

![Screenshot](https://github.com/user-attachments/assets/1a9e72f9-6312-4667-b270-63d6c16f8109)

## Architecture (one paragraph)

The **real** browser tab loads a normal page. That page runs **Rust compiled to WebAssembly**. The WASM module **fetches HTML** (via `fetch`, often through a CORS proxy), **parses and styles** it with **[Blitz](https://github.com/DioxusLabs/blitz)** ([Stylo](https://github.com/servo/servo) for CSS, [Taffy](https://github.com/DioxusLabs/taffy) for layout), **rasterizes** with **[Vello](https://github.com/linebender/vello)** on **[wgpu](https://github.com/gfx-rs/wgpu)** (WebGPU in the tab), reads back pixels, and draws them with **`CanvasRenderingContext2D.putImageData`** into a `<canvas>`. There is no nested `<iframe>` doing the page for you: the “inner browser” is this pipeline inside the same tab.

Blitz’s own WASM story is tracked in **[Request: wasm support #160](https://github.com/DioxusLabs/blitz/issues/160)**. Upstream is aimed at **native shells** (full app + windowing). This repo **does not wait for that**: it pulls in only the **library crates** (`blitz-dom`, `blitz-html`, `blitz-paint`, …), links them for **`wasm32-unknown-unknown`**, and supplies the missing “browser chrome” itself.

### Workarounds in practice

| Gap (why #160 exists) | What we do instead |
| --------------------- | ------------------ |
| No turnkey Blitz-in-WASM product | Use **wasm-bindgen** + a tiny JS host; export **`fetchAndPaint`** only. |
| Navigation / net / fonts in a real shell | **`fetch` in WASM** for the main document; **html5ever** walks the tree and **inlines `<link rel=stylesheet>`** via more fetches (often need a **CORS proxy**); **`DummyNetProvider`** + explicit **`load_resource`** for a **CORS-hosted webfont** (e.g. Inter) so text isn’t invisible. |
| “Where do pixels go?” | **Vello → wgpu WebGPU texture → GPU readback → `putImageData`** on a 2D canvas (no native surface, no `blitz-shell`). |

So the workaround is: **treat Blitz as an embeddable renderer**, not as the full Dioxus/native demo app, and **own I/O + raster presentation** in the embedding page.

## Super simple flow

```
User enters URL → JS calls WASM → WASM fetch(html) → Blitz: DOM + style + layout + paint → RGBA → 2D canvas
```

### Host page (shape of the integration)

The page is tiny: initialize WASM, then hand the canvas and URL to one exported async function (see `web/index.html` and `src/lib.rs`).

```html
<script type="module">
  import init, { fetchAndPaint } from "./pkg/inner_browser.js";
  const canvas = document.getElementById("page");
  await init();
  await fetchAndPaint(canvas, url, cssWidthPx, devicePixelRatio);
</script>
```

### Rust side (shape of the work)

1. **`fetch_and_paint`** (wasm-bindgen): `fetch` the document, optionally **inline `<link rel=stylesheet>`** bodies so layout sees CSS, then call the painter.
2. **`paint_blitz_async`**: build an **`HtmlDocument`**, **resolve** layout for the viewport, record a **Vello `Scene`**, **render to a texture**, **copy to CPU**, **putImageData** on the canvas.

That is the whole idea: **network + engine in WASM, pixels on canvas**, still a normal web page.

## Repos (stack)

| Piece | Repository |
| ----- | ---------- |
| Layout / paint / DOM engine | [DioxusLabs/blitz](https://github.com/DioxusLabs/blitz) |
| GPU vector raster | [linebender/vello](https://github.com/linebender/vello) |
| WebGPU in Rust | [gfx-rs/wgpu](https://github.com/gfx-rs/wgpu) |
| HTML parsing (inlining helpers here) | [servo/html5ever](https://github.com/servo/html5ever) |
