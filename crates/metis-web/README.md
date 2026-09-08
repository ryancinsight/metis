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
After a successful submission the workbench receives and decodes the backend's
`clinical.result` event, verifies it matches the correlated response, and
renders its identifier and rates in the event status line.

The workbench also demonstrates Rust-owned checkbox, radio, range and native
HTML dialog controls. Their semantic browser state is read through Moirai's DOM
seam and rendered as bounded presentation preferences without changing the
authoritative response. Dialog open/close operations and focus restoration stay
inside that same seam. The pointer-capture surface reads each browser pointer
identifier through `WebEvent`, captures it on `pointerdown`, verifies the
capture, and releases it on `pointerup` or `pointercancel`; the capture state
is owned by the mounted listener set. The same events expose a Rust-owned
metadata snapshot with device type, CSS-pixel coordinates, button state,
modifier keys and the primary-pointer marker; `pointermove` renders the
snapshot while the surface owns the capture.
The same surface listens for browser wheel events through Moirai's
`WheelMetadata` snapshot and renders pixel/line/page deltas, viewport position
and modifier state without importing `web-sys`. The listener prevents the
browser default action after the provider has validated the event kind.
The Rust-owned gesture policy consumes those records as a bounded viewport:
one captured pointer drags a CSS-pixel pan, ordinary wheel input pans, and
Ctrl+wheel changes zoom between 50% and 300%. Line and page deltas are
normalized to fixed CSS-pixel units; non-finite deltas are rejected without
changing the viewport. The policy writes one CSS transform and exposes its
state through `gesture-status`. Single-pointer touch follows the same drag
path; multi-touch and pinch interpretation remain host work.

The **DICOM file drop** surface consumes Moirai's bounded `DropMetadata`
snapshot. Rust validates the copied display metadata again, caps the accepted
batch at 64 files and reports names, media types, byte sizes and DICOM
candidates in a semantic status region. The browser seam does not read file
bytes or turn a browser name into a filesystem path; a native or trusted host
must provide the byte-reading grant before a viewer can open a study.

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
state, listeners, a typed bounded workbench extension manifest and (when
configured) a new bounded service task.
