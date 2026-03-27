# browserbrowserbowser

This demo was done because an LLM told me it couldn't be done.

**Rust-based browser demo.** Loads a browser inside your browser and renders it to a `<canvas>` via **WebGPU** — **[Blitz](https://github.com/DioxusLabs/blitz)** for HTML/CSS/layout, **[Vello](https://github.com/linebender/vello)** for paint, **WASM** in the tab (not an `<iframe>`).

**[Servo](https://github.com/servo/servo)** is an open-source **browser engine** in Rust. **Blitz** plugs in Servo’s **Stylo** CSS engine (through **`blitz-dom`**) for cascade and computed styles. We also use Servo’s **[html5ever](https://github.com/servo/html5ever)** parser to collect `<link rel=stylesheet>` and inline them before Blitz runs.

**[WASM support #160](https://github.com/DioxusLabs/blitz/issues/160)** is still open, so we **wire up** fetch, GPU readback, and **`putImageData`** ourselves — **Blitz** still provides **DOM**, **CSS**, **layout**, and the **Vello** scene.

https://github.com/user-attachments/assets/633ca687-14d6-4880-bf9b-c742c7c539ce

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

- WASM **`fetch`** loads HTML; **html5ever** (Servo parser, above) inlines linked CSS so **Blitz** sees full author styles.
- **Blitz** — **Stylo** via [**`blitz-dom`**](https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-dom) for cascade/computed styles; **[Taffy](https://github.com/DioxusLabs/taffy)** for layout; [**`blitz-paint`**](https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-paint) drives **Vello** on **wgpu** (WebGPU).
- GPU frame → CPU readback → **`CanvasRenderingContext2D.putImageData`** on a **`<canvas>`**.
- **Host** is thin JS + **`fetchAndPaint`**; no **`blitz-shell`**, no **`<iframe>`** renderer.
