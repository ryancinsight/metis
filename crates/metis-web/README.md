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

The **DICOM file drop** surface consumes Moirai's bounded `DropFiles` capture.
Rust validates the copied display metadata again, caps the accepted batch at 64
files and reports names, media types, byte sizes and DICOM candidates in a
semantic status region. The first selected entry is read through Moirai's
browser-owned file handle into a fixed 132-byte buffer; the consumer classifies
the DICOM Part 10 marker and reports the byte-read state. Moirai bounds one
read to 1 MiB, while this workflow never requests more than the DICOM header.
No browser name becomes a filesystem path, and the browser file remains outside
Rust-owned persistent storage. RITK retains full DICOM parsing and study
decoding; a native or trusted browser host still supplies the authorization and
the subsequent viewer workflow.

The **Text and composition** surface consumes Moirai's bounded text snapshots.
The textarea keeps Unicode values in Rust-owned state, preserves browser
UTF-16 selection offsets and direction, and records `InputEvent` data,
operation type and composition state. Composition start, update, commit and
cancel transitions render separate status values. The policy bounds values and
metadata, leaves grapheme segmentation, bidi layout, clipboard/undo and native
IME production to the host contract, and exposes the selection and composition
state through semantic status elements and data attributes.

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
