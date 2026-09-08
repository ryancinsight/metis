# ADR 0014: Rust-owned browser input controls

Status: Accepted

Date: 2026-09-07

Driver: [METIS-INPUT-001](../../backlog.md#METIS-INPUT-001)

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
`pointercancel`. A single active identifier is retained per mounted surface;
additional pointer-down events are rejected until the current capture releases.
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
one captured pointer drags a bounded CSS-pixel pan, ordinary wheel input pans,
and Ctrl+wheel zooms between 50% and 300%. Line and page units normalize to
fixed CSS-pixel scales; non-finite deltas are rejected without mutation. The
single-pointer path also handles a touch pointer as a drag; multi-touch and
pinch interpretation remain open.

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
`metis-web` passes native warning-denied Clippy, 10 native tests, and the WASM
compile and Clippy checks against Moirai
`d879779247c8cfc5870f62f99a5364cbbf2d3c58`. The authenticated browser trace at
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
bounded pan/zoom state and a live drag/scroll transform trace.

## Residuals

Drag/drop policy, multi-touch/pinch interpretation, IME, accessibility
technology and native-window input remain under the linked backlog items. This
increment does not claim cross-engine or native input parity.
