# metis-web

The browser host mounts a Metis application into an existing HTML5 document.
Rust owns the captured form state and browser event transitions; the page owns
ordinary CSS and the document shell. Moirai owns the DOM handles and callback
lifetime so this crate does not import `web-sys` or a second browser runtime.

## Format-neutral canvas

`metis_web::CanvasSurface` presents a consumer-owned borrowed RGBA8 frame through
Moirai's bounded HTML5 canvas provider. Consumers implement
`metis_web::CanvasFrame` on their existing presentation value; the host checks
the dimensions and exact byte length, then uploads the frame without retaining
the source allocation or interpreting its format. RITK uses this seam for
viewer pixels; DICOM parsing, geometry and display policy stay in RITK.

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
one captured pointer drags a CSS-pixel pan, two captured pointers pan by their
centroid and zoom by their finite distance ratio, ordinary wheel input pans,
and Ctrl+wheel changes zoom between 50% and 300%. Line and page deltas are
normalized to fixed CSS-pixel units; non-finite deltas are rejected without
changing the viewport. A zero-distance pair waits for a valid baseline and a
third pointer is rejected. The policy writes one CSS transform and exposes its
state through `gesture-status`; touch pointers use the same bounded policy.

The **file drop** surface consumes Moirai's bounded `DropFiles` capture. Rust
validates the copied display metadata again, caps the accepted batch at 64 files
and reports names, media types and byte sizes in a semantic status region. Each
accepted entry is read asynchronously through its Moirai browser-owned handle
into a [`FileDropBatch`]. One file is limited to 64 MiB and one batch to 256 MiB;
a 64 KiB continuation buffer keeps each `FileReader` turn bounded even though
Moirai permits a 1 MiB provider chunk. The handoff performs no format
classification. A trusted application polls the WASM-only `take_file_drop`
handoff, which transfers ownership and leaves one bounded slot for a later drop.
No browser name becomes a filesystem path, and stopping or remounting drops
unconsumed bytes. Consumers that need to retain the payload call
[`FileDropBatch::into_files`] and then [`FileDropPayload::into_parts`]; both
moves preserve the existing byte allocations. Format-specific parsing and study
decoding belong to the consuming application, such as RITK; its adapter can
borrow those bytes for a synchronous load or retain the buffers for a decode
job without copying the file contents.

The **Text and composition** surface consumes Moirai's bounded text snapshots.
The textarea keeps Unicode values in Rust-owned state, preserves browser
UTF-16 selection offsets and direction, and records `InputEvent` data,
operation type and composition state. Composition start, update, commit and
cancel transitions render separate status values. The policy bounds values and
metadata, rejects offsets inside a UTF-16 surrogate pair before changing state,
leaves grapheme segmentation, bidi layout, clipboard/undo and native IME
production to the host contract, and exposes the selection and composition
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

## Theme and application branding

`metis_web::Theme` is the browser host's bounded presentation contract. The
Rust-owned selector accepts `system`, `light`, `dark` and `high-contrast`;
`system` follows `prefers-color-scheme`. Each render writes the selected value
to `data-metis-theme` on the document body and the application root. The
external stylesheet maps that attribute to semantic `--metis-*` CSS variables,
so an application can replace its palette in its own same-origin stylesheet
without changing state, event handling or the authority boundary.

The starter page also loads `examples/browser/assets/metis-mark.png` as a local
same-origin resource and favicon. `scripts/browser.py build` copies nested
browser assets and fails if the declared mark is absent. Applications replace
that resource and the `.metis-mark` rule with their own project-owned artwork;
the browser host does not fetch icons, fonts or media from an external origin.
