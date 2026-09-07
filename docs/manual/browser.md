# Run the browser workbench

The browser workbench is the first executable HTML5/CSS host. The document,
external stylesheet and module bootstrap come from `examples/browser/`; Rust
owns the controls, captured inputs and event transitions through `metis-web`.
Moirai owns the DOM handles and listener lifetimes. The page contains no
backend key and does not perform the privileged clinical calculation in
downloaded WASM.

The page carries the same strict content-security policy as
`metis_core::HostPolicy`. The policy source is
`crates/metis-core/src/content_security_policy.txt`; the browser build checks
the HTML asset against that source before copying it. Scripts, styles,
connections and form actions are same-origin, the generated WASM loader is
permitted by `'wasm-unsafe-eval'`, and plugins are disabled. The bootstrap also
cancels cross-origin anchor navigation as a defense-in-depth check.

The `frame-ancestors 'none'` directive is present for a host that delivers the
policy as an HTTP response header. Browsers do not enforce `frame-ancestors`
from a document meta tag, so the current static workbench has no framing
enforcement until its native or service host supplies that header. A host must
also enforce origin, session and operating-system policy at its boundary;
downloaded page code is not an authority source.

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

The page's **Stop host** control calls the generated `metis_stop` export. The
Rust host drops its listener guards and replaces `#metis-app` with
`Metis browser host stopped.`. **Start host** calls `metis_start` again and
mounts fresh controls. This exercises listener teardown and remount through the
same WASM module; it does not establish a live backend, task allocation count,
or origin/session grant.

Capture the initial and edited states with the browser's native screenshot
tool. Record browser engine, viewport, device scale, font environment and the
WASM revision beside the images. These captures establish HTML/CSS execution,
and focusable controls. Moirai's provider owns listener teardown in code, but a
post-drop allocation trace is still required; the captures do not close the
live WebSocket backend, origin policy or task allocation requirements in V02/V12.

The browser host deliberately keeps those privileged operations separate. A
future bridge will connect `AsyncFrontendApp` to `BrowserWebSocketTransport`
through Moirai's cancellable local-task handle only after the host-origin and
session policy is implemented and tested.
