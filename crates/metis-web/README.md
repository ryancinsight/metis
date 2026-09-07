# metis-web

The browser host mounts a Metis application into an existing HTML5 document.
Rust owns the captured form state and browser event transitions; the page owns
ordinary CSS and the document shell. Moirai owns the DOM handles and callback
lifetime so this crate does not import `web-sys` or a second browser runtime.

The host reports a typed disconnected outcome when no authorized backend bridge
is configured. When the page host supplies an endpoint, process identifier and
session principal, it connects `AsyncFrontendApp` to the Metis service over the
bounded Moirai WebSocket transport. The downloaded WASM never calculates
clinical output and never treats page configuration as backend authority; the
service validates the observed origin and trusted session before the upgrade.

Build the WASM artifact and generated browser glue with:

```text
cargo install wasm-bindgen-cli --version 0.2.128 --locked --root output/wasm-bindgen-cli
python scripts/browser.py build
```

Serve `output/browser` from an HTTP origin and open `index.html`. The page
contains real editable controls; input changes update the Rust-owned state.
Configure the optional hidden host fields or the query parameters described in
the user manual to enable the service bridge. The generated module also exports
`metis_stop`, which drops the browser task and every Rust-owned DOM listener
before replacing the root with a stopped message; `metis_start` mounts fresh
state, listeners and (when configured) a new bounded service task.
