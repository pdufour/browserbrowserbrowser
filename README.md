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

## Architecture

What happens in the tab:

- You open a **normal page** (HTML + JS). Nothing special about the host URL.
- That page loads **Rust compiled to WebAssembly**.
- WASM uses **`fetch`** to load HTML (often wrapped in a **CORS proxy** so the response is allowed).
- The bytes go through **[Blitz](https://github.com/DioxusLabs/blitz)** — **[Stylo](https://github.com/servo/servo)** for CSS, **[Taffy](https://github.com/DioxusLabs/taffy)** for layout.
- **Vector paint** is **[Vello](https://github.com/linebender/vello)** on **[wgpu](https://github.com/gfx-rs/wgpu)** (**WebGPU**).
- The frame is **read back** from the GPU and copied with **`CanvasRenderingContext2D.putImageData`** into a **`<canvas>`**.
- There is **no nested `<iframe>`** acting as the renderer. The “inner browser” is **this pipeline**, in the same tab.

Upstream & this repo:

- Blitz’s WASM direction is discussed in **[#160 — wasm support](https://github.com/DioxusLabs/blitz/issues/160)**.
- Blitz upstream targets **native shells** (full app, windowing). **We don’t use that path.**
- We depend only on the **library crates**: [**blitz-dom**](https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-dom), [**blitz-html**](https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-html), [**blitz-paint**](https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-paint), [**blitz-traits**](https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-traits).
- They’re built for **`wasm32-unknown-unknown`**. Our page supplies the **chrome** (fetch UI, canvas, `wasm-bindgen` glue).

### Workarounds (why there’s no “official” Blitz-in-WASM app yet)

- **Glue** — **`wasm-bindgen`** + a thin host; one export, **`fetchAndPaint`**.
- **HTTP + CSS** — WASM **`fetch`** for the document. **[html5ever](https://github.com/servo/html5ever)** finds stylesheets; we **fetch** them and **inject `<style>`** (proxy if CORS blocks). Fonts: **`load_resource`** with a **CORS-friendly** URL (e.g. Inter).
- **Surface** — **WebGPU texture → CPU readback → `putImageData`**. No **`blitz-shell`** window.

Treat Blitz as a **library renderer**; this repo owns **I/O** and **how pixels hit the canvas**.

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
