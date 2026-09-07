# metis-web

The browser host mounts a Metis application into an existing HTML5 document.
Rust owns the captured form state and browser event transitions; the page owns
ordinary CSS and the document shell. Moirai owns the DOM handles and callback
lifetime so this crate does not import `web-sys` or a second browser runtime.

The current host deliberately reports a typed disconnected outcome when no
authorized backend bridge is configured. It does not calculate clinical output
in the downloaded WASM and does not treat a browser origin as backend authority.
The WebSocket bridge will close this boundary after the host-origin policy and
live backend service are available.

Build the WASM artifact and generated browser glue with:

```text
cargo install wasm-bindgen-cli --version 0.2.128 --locked --root output/wasm-bindgen-cli
python scripts/browser.py build
```

Serve `output/browser` from an HTTP origin and open `index.html`. The page
contains real editable controls; input changes update the Rust-owned state and
the submit action exposes the missing privileged bridge as an explicit result.
