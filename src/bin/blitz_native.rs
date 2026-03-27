//! Native **Blitz** window (Stylo + layout + Vello) using DioxusLabs/blitz **master**.
//! WASM builds cannot link Blitz; use the `cdylib` + canvas path in the browser.
//!
//! ```text
//! cargo run --release --bin blitz-native -- path/to/page.html
//! ```

fn main() {
    let path = std::env::args_os()
        .nth(1)
        .expect("usage: blitz-native <file.html>");
    let html = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("read {}: {e}", path.to_string_lossy());
        std::process::exit(1);
    });
    blitz::launch_static_html(&html);
}
