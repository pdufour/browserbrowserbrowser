# browser-in-browser

Minimal **HTML/CSS → layout → pixels** demo: **[Blitz](https://github.com/DioxusLabs/blitz)** in **WebAssembly**, drawn to a **`<canvas>`** via **WebGPU** (Vello), not an `<iframe>`.

![Screenshot](https://github.com/user-attachments/assets/1a9e72f9-6312-4667-b270-63d6c16f8109)

## Requirements

- **Rust** (see [`rust-toolchain.toml`](rust-toolchain.toml); includes `wasm32-unknown-unknown`)
- **[wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)**
- A **WebGPU-capable** browser (Vello uses wgpu on the WebGPU backend)
- Pages you fetch must be **reachable from the browser under CORS** — the stock UI expects URLs wrapped in something like **`https://corsproxy.io/?https://…`**

## Build & run

From the repo root:

```bash
wasm-pack build --target web --out-dir web/pkg
cargo run --bin serve-web
```

Open [http://127.0.0.1:8080/](http://127.0.0.1:8080/) (or pass a port: `cargo run --bin serve-web -- 3000`). You need a local server so the WASM ES module loads; `file://` will not work.

## Architecture (one paragraph)

The **real** browser tab loads a normal page. That page runs **Rust compiled to WebAssembly**. The WASM module **fetches HTML** (via `fetch`, often through a CORS proxy), **parses and styles** it with **[Blitz](https://github.com/DioxusLabs/blitz)** ([Stylo](https://github.com/servo/servo) for CSS, [Taffy](https://github.com/DioxusLabs/taffy) for layout), **rasterizes** with **[Vello](https://github.com/linebender/vello)** on **[wgpu](https://github.com/gfx-rs/wgpu)** (WebGPU in the tab), reads back pixels, and draws them with **`CanvasRenderingContext2D.putImageData`** into a `<canvas>`. There is no nested `<iframe>` doing the page for you: the “inner browser” is this pipeline inside the same tab.

Blitz’s own WASM story is tracked in **[Request: wasm support #160](https://github.com/DioxusLabs/blitz/issues/160)**. Upstream is aimed at **native shells** (full app + windowing). This repo **does not wait for that**: it pulls in only the **library crates** ([**blitz-dom**](https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-dom), [**blitz-html**](https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-html), [**blitz-paint**](https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-paint), [**blitz-traits**](https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-traits)), links them for **`wasm32-unknown-unknown`**, and supplies the missing “browser chrome” itself.

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

1. **`fetch_and_paint`** (wasm-bindgen): `fetch` the document, **collect `<link rel=stylesheet>`** `href`s (html5ever), **fetch and inject** their text as `<style>` so Blitz sees author CSS, then call the painter.
2. **`paint_blitz_async`**: build an **`HtmlDocument`**, **resolve** layout for the viewport, record a **Vello `Scene`**, **render to a texture**, **copy to CPU**, **putImageData** on the canvas.

That is the whole idea: **network + engine in WASM, pixels on canvas**, still a normal web page.

| Path | Role |
| ---- | ---- |
| [`web/index.html`](web/index.html) | Host UI + loads `./pkg/inner_browser.js` |
| [`src/lib.rs`](src/lib.rs) | `fetchAndPaint` — fetch, stylesheet inlining, call paint |
| [`src/blitz_wasm.rs`](src/blitz_wasm.rs) | Blitz document, Vello scene, GPU readback, `putImageData` |
| [`src/bin/serve_web.rs`](src/bin/serve_web.rs) | Static server for `web/` |

## Repos (stack)

| Piece | Repository |
| ----- | ---------- |
| Blitz monorepo | [DioxusLabs/blitz](https://github.com/DioxusLabs/blitz) |
| GPU vector raster | [linebender/vello](https://github.com/linebender/vello) |
| Vello backend for paint trait | [DioxusLabs/anyrender](https://github.com/DioxusLabs/anyrender) (`anyrender_vello`) |
| WebGPU in Rust | [gfx-rs/wgpu](https://github.com/gfx-rs/wgpu) |
| HTML parsing (inlining helpers here) | [servo/html5ever](https://github.com/servo/html5ever) |

### Blitz crates used here (paths in the monorepo)

| Crate | `packages/…` |
| ----- | ------------- |
| **blitz-dom** | [packages/blitz-dom](https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-dom) |
| **blitz-html** | [packages/blitz-html](https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-html) |
| **blitz-paint** | [packages/blitz-paint](https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-paint) |
| **blitz-traits** | [packages/blitz-traits](https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-traits) |
