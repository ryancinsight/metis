# metis-webview-page

The Windows form and permission probe run in Rust on WebAssembly.

## Build

Regenerate the packaged module and its JavaScript loader from the workspace
lockfile, then compare both files with an independent build.

```sh
python scripts/webview_page.py build
python scripts/webview_page.py check
```

The Windows host maps the package at `https://metis.invalid/`. The generated
loader requests `page_bg.wasm` from that package. The native target exposes
the presentation logic tests and has no runtime browser dependency.
