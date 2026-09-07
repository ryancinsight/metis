# Run the browser workbench

The browser workbench is the first executable HTML5/CSS host. The document and
CSS shell come from `examples/browser/index.html`; Rust owns the controls,
captured inputs and event transitions through `metis-web`. Moirai owns the DOM
handles and listener lifetimes. The page contains no backend key and does not
perform the privileged clinical calculation in downloaded WASM.

## Build and serve

Install the pinned `wasm-bindgen-cli` version matching the workspace's
`wasm-bindgen` dependency. The build script checks for exactly 0.2.128 and also
discovers this ignored local root, then run:

```text
cargo install wasm-bindgen-cli --version 0.2.128 --locked --root output/wasm-bindgen-cli
python scripts/browser.py build
python -m http.server 8080 --directory output/browser
```

Open `http://127.0.0.1:8080/` in a browser. A file URL is not accepted because
module and WASM loading require an HTTP origin.

## Exercise the real Rust state

Change the weight, concentration or dose fields. The result panel updates from
the Rust-owned `FormInputs` and the status returns to idle for each valid edit.
Enter a non-numeric value to observe the typed `ERR_NUMERIC_INSTABILITY`
display. Submit the form to observe `ERR_CONNECTION_CLOSED`: this is the
explicit result for a page without an authorized backend bridge, not a local
calculation or a fabricated success.

Capture the initial and edited states with the browser's native screenshot
tool. Record browser engine, viewport, device scale, font environment and the
WASM revision beside the images. These captures establish HTML/CSS execution,
and focusable controls. Moirai's provider owns listener teardown in code, but a
post-drop allocation trace is still required; the captures do not close the
live WebSocket backend, origin policy or cancellation requirements in V02/V12.

The browser host deliberately keeps those privileged operations separate. The
next bridge increment will connect `AsyncFrontendApp` to
`BrowserWebSocketTransport` only after the host-origin/session policy is
implemented and tested.
