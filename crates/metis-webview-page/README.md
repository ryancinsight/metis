# metis-webview-page

The Windows form and permission probe run in Rust on WebAssembly.
Moirai owns the WebView2 message bridge and browser permission requests.
The form sends a JSON object to the native host and reads the decimal audit
identifier without passing it through a JavaScript number.

## Build

Regenerate the packaged module and its JavaScript loader from the workspace
lockfile, then compare both files with an independent build. The generator
uses Rust's size optimization for the final WebAssembly page crate.

```sh
python scripts/webview_page.py build
python scripts/webview_page.py check
```

The Windows host maps the package at `https://metis.invalid/`. The generated
loader requests `page_bg.wasm` from that package. The native target exposes
the presentation logic tests and has no runtime browser dependency.
