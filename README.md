# browser-in-browser

**Rust-based browser demo.** Loads a browser inside your browser and renders it to a `<canvas>` via **WebGPU** — **[Blitz](https://github.com/DioxusLabs/blitz)** for HTML/CSS/layout, **[Vello](https://github.com/linebender/vello)** for paint, **WASM** in the tab (not an `<iframe>`).

![Screenshot](https://github.com/user-attachments/assets/1a9e72f9-6312-4667-b270-63d6c16f8109)

## Run

- **Rust** — see [`rust-toolchain.toml`](rust-toolchain.toml) (`wasm32-unknown-unknown`).
- **[wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)**
- **WebGPU** browser (Vello → wgpu).
- **CORS** — fetched pages must be allowed; the stock URL box expects wrappers like `https://corsproxy.io/?https://…`.

```bash
wasm-pack build --target web --out-dir web/pkg
cargo run --bin serve-web
```

Then open [http://127.0.0.1:8080/](http://127.0.0.1:8080/) (not `file://`).

## How it works

- WASM **`fetch`** loads HTML; **[html5ever](https://github.com/servo/html5ever)** grabs linked stylesheets and inlines them so **[Blitz](https://github.com/DioxusLabs/blitz)** gets real author CSS.
- **Blitz** — **Stylo** (via **`blitz-dom`**) for cascade/computed styles, **Taffy** for layout; **`blitz-paint`** drives **Vello** on **wgpu** (WebGPU).
- GPU frame → CPU readback → **`CanvasRenderingContext2D.putImageData`** on a **`<canvas>`**.
- **Host** is thin JS + **`fetchAndPaint`**; no **`blitz-shell`**, no **`<iframe>`** renderer.
- Blitz-on-WASM upstream: **[#160](https://github.com/DioxusLabs/blitz/issues/160)**.
