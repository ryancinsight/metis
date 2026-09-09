# ADR 0014: Rust-owned browser input controls

Status: Accepted

Date: 2026-09-07

Driver: [METIS-INPUT-001](../../backlog.md#METIS-INPUT-001)

Revision 2026-09-08: Moirai PR #289 merged at
`5c8a9e8be32ad6beac14ed263c2f11c3663b87cb` adds bounded browser file access.
The file-drop consumer now reads the complete accepted batch through
provider-owned browser `File` handles and exposes one owned handoff slot;
full dataset parsing remains with RITK.

Revision 2026-09-08: commit `16e14afaf3ee8c993e840a06a539dfd3bb0ff5bc`
updates the gesture policy to retain at most two captured pointer identifiers.
A second pointer establishes a finite pinch baseline; centroid movement pans
and distance changes zoom, while a third pointer-down is rejected. The provider
captures and releases each accepted identifier.

## Context

Metis must reuse ordinary HTML5 controls while keeping application state and
authority in Rust. The existing browser workbench had editable text and number
inputs, but no typed checkbox, radio, range or select state. Pointer transitions
also need a Rust-owned capture boundary so a drag can keep its target when the
pointer leaves the hit surface. Browser bindings must stay inside the Moirai
provider so a Metis consumer does not import `web-sys`.

## Decision

The browser workbench uses semantic HTML5 checkbox, radio, range and select inputs.
`metis-web` owns one `ControlState` alongside the form state. Moirai's
`WebElement` reads the browser checked property, input/select value property and
disabled state; it also sets disabled state for form controls. Rust event
listeners validate the value, update `ControlState`, and render the derived
status and metric text. Listener guards remain owned by the mounted application
and are dropped on stop/remount.

Display-unit, visibility, and result-detail controls change presentation
preferences only. The range control is a bounded scale preference and the
select chooses between a clinical summary and an audit sequence label; neither
changes the clinical response value. The service response, capability grants
and session identity remain authoritative outside the control model.

The workbench includes a bounded pointer-capture surface. Its Rust listeners
read the `pointerId` exposed by Moirai, call `set_pointer_capture`, verify
`has_pointer_capture`, and release the same identifier on `pointerup` or
`pointercancel`. At most two active identifiers are retained per mounted
surface; duplicate and third pointer-down events are rejected until an
accepted capture releases. Each accepted identifier is captured and verified
independently.
Each pointer transition also reads Moirai's copyable `PointerMetadata` snapshot
and renders the normalized device type, CSS-pixel coordinates, changed and
held buttons, modifier keys and primary-pointer marker. Captured `pointermove`
events update the same status so drag policy can consume one coherent input
record.

The pointer surface also listens for wheel events. Moirai converts the browser
event into a copyable `WheelMetadata` snapshot containing all three deltas,
their browser unit, viewport coordinates and modifier keys. Metis renders that
record and prevents the browser default action after the event kind is
validated. A Rust-owned `GestureViewport` then applies the application policy:
one captured pointer drags a bounded CSS-pixel pan, two captured pointers use
their centroid for bounded pan and their finite distance ratio for zoom,
ordinary wheel input pans, and Ctrl+wheel zooms between 50% and 300%. Line and
page units normalize to fixed CSS-pixel scales; non-finite deltas are rejected
without mutation. A zero-distance pair waits for a valid baseline, and a
third pointer is rejected. Touch pointers use the same bounded policy.

The workbench uses Moirai's `DropFiles` capture for the DICOM file-drop card.
Validated metadata remains bounded to 64 entries. An accepted drop retains the
provider-owned browser file handles only for the asynchronous read task; Metis
reads each file to its declared end, limits one file to 64 MiB and the batch to
256 MiB, classifies the first payload's Part 10 marker and renders `reading`,
`complete` or `failed` in semantic status attributes. A completed
`FileDropBatch` is available through the WASM-only `take_file_drop` handoff and
replaces an unconsumed batch. The provider bounds each read to 1 MiB, and
neither a browser name nor a filesystem path crosses into the consumer. A
consumer that needs ownership calls `FileDropBatch::into_files` followed by
`FileDropPayload::into_parts`; these moves preserve the byte allocations and
allow a decoder to borrow or retain them without a second file-content copy.
RITK remains the owner of dataset parsing, pixel decoding and study selection.

The workbench also uses a semantic textarea for text and composition. Metis
owns a bounded `TextState` that receives Moirai's UTF-16 selection snapshots,
`InputEvent` metadata and `CompositionEvent` transitions. The state validates
selection ranges against the current Unicode value and renders text,
composition and selection status without importing `web-sys`. Grapheme
segmentation, bidi shaping, clipboard/undo, fallback-font metrics and native
IME production remain host contracts.

## Alternatives

Duplicating a widget renderer would lose browser-native focus and keyboard
semantics. Adding egui, GPUI, Iced or Tauri would introduce a second ownership
model and would not provide drop-in HTML/CSS reuse. Keeping JavaScript state
would bypass the Rust state boundary. These alternatives do not satisfy the
current browser contract.

## Verification

`ControlState` tests cover default values, checked, radio and select transitions,
bounded scale parsing, invalid control input, numeric-field validation and
preservation of a successful response while presentation controls change.
`metis-web` passes native warning-denied Clippy, 21 native tests, and the WASM
compile and Clippy checks against Moirai
`0862716265d657b8069d5a47fd1e77ae26ddd006`. The authenticated browser trace at
1280×720 CSS pixels and device scale 1.25 selected the radio and checkbox with
pointer actions, moved the range twice with the keyboard and selected Audit
detail through the native select; the accessibility values, status text, focus
ring and `2.175000 mg/hr` result matched the model.

The same trace observed a disabled submit button before bridge readiness, an
enabled button after the authenticated handshake, a disabled button during a
four-second response delay, and an enabled button after the correlated result.
A disconnected workbench rejected activation of the disabled button and kept
its status unchanged.

The dialog increment consumes Moirai `WebElement::dialog_open`, `show_modal`,
`close_dialog` and `focus` from `8f02b8b7de6cf6361b519bd79759d8508568fbdb`.
The authenticated browser trace opened the native HTML dialog, verified its
status and capability text, closed it through the Rust listener, restored focus
to the opener on the `close` event, and exercised Escape dismissal.

The pointer increment consumes Moirai pointer APIs from merged revision
`5a5e4b1540eff39bc3f082c6907f0c82fa14dcc8`. The browser trace activated the
pointer surface, observed the provider-backed release status with pointer ID
`1`, and captured the rendered pointer surface and accessibility name at the
same viewport and device scale. The metadata increment consumes
`PointerMetadata` from merged Moirai revision
`a3c86cd183a18edc35db30f1d35e79fe80092df4`; its live browser trace records the
pointer type, coordinates, buttons, modifiers and primary marker. The wheel
increment consumes `WheelMetadata` from merged Moirai revision
`f634b3a802ec0355da22f111ed01067d2435c5cb`; an in-app browser scroll action
renders input-sensitive vertical and horizontal pixel deltas with the target
coordinates and modifier state. The gesture increment adds native-tested
bounded pan/zoom state and a live drag/scroll transform trace. The two-pointer
increment adds bounded capture slots, duplicate/third-pointer rejection,
centroid pan, distance-ratio zoom and zero-distance baseline handling; its
native policy tests and WASM checks are recorded in the verification artifact.

The text increment adds 21 native policy tests for UTF-16 coordinates,
selection bounds, input metadata and composition transitions. The browser
trace renders the labelled textarea, bounded value preview, semantic status
regions and focus state; CUA cannot provide trusted OS IME input or expose the
browser `isTrusted` flag.

The file-drop increment consumes `DropFiles` from Moirai revision
`5c8a9e8be32ad6beac14ed263c2f11c3663b87cb`. The native `metis-web` suite
covers the DICOM header classifier, payload budget edges, batch value ownership
and ownership-consuming allocation preservation; strict native Clippy, the WASM
check and WASM Clippy compile the provider-backed full-read path. CUA cannot
attach a trusted local file, so no live browser trace claims a byte read or
DICOM decode.

## Residuals

Grapheme/bidi layout, clipboard/undo, native IME, accessibility technology and
native-window input remain under the linked backlog items. CUA cannot provide
trusted physical touch or expose the browser `isTrusted` flag, so this
increment does not claim cross-engine or physical-input parity.
