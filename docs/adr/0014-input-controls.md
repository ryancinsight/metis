# ADR 0014: Rust-owned browser input controls

Status: Accepted

Date: 2026-09-07

Driver: [METIS-INPUT-001](../../backlog.md#METIS-INPUT-001)

## Context

Metis must reuse ordinary HTML5 controls while keeping application state and
authority in Rust. The existing browser workbench had editable text and number
inputs, but no typed checkbox, radio or range state. Browser bindings must stay
inside the Moirai provider so a Metis consumer does not import `web-sys`.

## Decision

The browser workbench uses semantic HTML5 checkbox, radio and range inputs.
`metis-web` owns one `ControlState` alongside the form state. Moirai's
`WebElement` reads the browser checked property and existing value property;
Rust event listeners validate the value, update `ControlState`, and render the
derived status and metric text. Listener guards remain owned by the mounted
application and are dropped on stop/remount.

Display-unit and visibility controls change presentation preferences only. The
range control is a bounded scale preference and does not change the clinical
response value. The service response, capability grants and session identity
remain authoritative outside the control model.

## Alternatives

Duplicating a widget renderer would lose browser-native focus and keyboard
semantics. Adding egui, GPUI, Iced or Tauri would introduce a second ownership
model and would not provide drop-in HTML/CSS reuse. Keeping JavaScript state
would bypass the Rust state boundary. These alternatives do not satisfy the
current browser contract.

## Verification

`ControlState` tests cover default values, checked and radio transitions,
bounded scale parsing, invalid control input, numeric-field validation and
preservation of a successful response while presentation controls change.
`metis-web` passes native warning-denied Clippy, 10 native tests, and the WASM
compile and Clippy checks against Moirai
`5b7708606d14bcbf62e10925604eb45ebc0ddb86`. The authenticated browser trace at
1280×720 CSS pixels and device scale 1.25 selected the radio and checkbox with
pointer actions, then moved the range twice with the keyboard; the accessibility
values, status text, focus ring and `2.175000 mg/hr` result matched the model.

## Residuals

Select/menu/dialog controls, pointer capture, drag/drop, wheel/touch/modifier
events, IME, accessibility technology and native-window input remain under the
linked backlog items. This increment does not claim cross-engine or native
input parity.
