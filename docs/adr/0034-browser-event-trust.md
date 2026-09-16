# ADR 0034: Browser event trust in the canvas contract

Status: Accepted

Date: 2026-09-15

Driver: [METIS-INPUT-TRUST-001](../backlog.md#METIS-INPUT-TRUST-001)

Upstream decision: [Moirai ADR 0060](../../../moirai/docs/adr/0060-browser-event-trust.md)

## Context

Moirai owns the browser DOM boundary and captures the browser's
`Event.isTrusted` snapshot. Metis owns the format-neutral canvas event
contract, while RITK owns DICOM decoding, viewer state and application input
policy. Dropping the snapshot at the Metis boundary forces each consumer to
import `web-sys` or accept synthetic script events without a provenance value.

## Decision

`CanvasPointerEvent`, `CanvasWheelEvent` and `CanvasKeyboardEvent` carry the
Moirai trust snapshot as the typed [`CanvasEventTrust`](../../crates/metis-web/src/lib.rs)
value and expose `is_trusted` accessors. `CanvasSurface` copies the value when
it translates provider metadata. Metis does not reject the event: the contract
is format-neutral and some applications may intentionally accept
browser-generated events. An owning consumer such as RITK applies its own
policy before mutating viewer state.

The field is a value snapshot. It does not retain a DOM event, browser element
or callback and does not change native input, file handling or DICOM behavior.

## Alternatives rejected

1. Importing `web-sys` in Metis or RITK duplicates the DOM boundary and
   violates the provider ownership split.
2. Rejecting false values in Metis would make a format-neutral canvas contract
   encode one application's security policy.
3. Treating all events as trusted removes the browser provenance signal and
   prevents RITK from failing closed for synthetic input.

## Threat model and limits

Script-created browser events are untrusted and can otherwise reach a consumer
as ordinary pointer, wheel or keyboard values. RITK must reject false values
before changing clinical viewer state. A true browser value does not prove a
physical human, a secure browser session, an operating-system permission or
the authenticity of automation. Existing hosted trace checks remain
independent evidence and retain their WebKit and physical-input limits.

DICOM scanning, decoding, geometry and clinical presentation remain RITK-owned;
Metis only transports bounded canvas values.

## Verification

Metis unit tests construct both trust values and assert that the accessors
preserve them through the queue. The locked native and WASM checks, strict
Clippy and Rustdoc verify the public contract against the merged Moirai
provider. RITK adds consumer-level rejection tests and keeps the existing
real-image browser galleries as the visual oracle.
