# ADR 0014: Rust-owned browser input controls

Status: Accepted

Date: 2026-09-07

Driver: [METIS-INPUT-001](../../backlog.md#METIS-INPUT-001)

## Context

Metis must reuse ordinary HTML5 controls while keeping application state and
authority in Rust. The existing browser workbench had editable text and number
inputs, but no typed checkbox, radio, range or select state. Browser bindings must stay
inside the Moirai provider so a Metis consumer does not import `web-sys`.

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

## Residuals

Menu/dialog controls, pointer capture, drag/drop, wheel/touch/modifier events,
IME, accessibility technology and native-window input remain under the linked
backlog items. This increment does not claim cross-engine or native
input parity.
