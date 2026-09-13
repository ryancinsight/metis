# ADR 0032: Fixed-point native display scale

Status: Accepted

Date: 2026-09-13

Driver: [METIS-DESKTOP-001](../../backlog.md#METIS-DESKTOP-001)

## Context

The Windows provider reports `DpiChanged` events, but the application used to
log the value without applying it. Layout, bitmap text, and native hit testing
therefore shared a 96-DPI coordinate system even when a window moved to a
fractional display scale. The host must map authored dimensions and text to the
same physical framebuffer while keeping the native surface format-neutral.
DICOM decoding, clinical orientation, and viewer state remain in RITK.

## Decision

`metis-platform::DisplayScale` stores the physical-to-authored ratio in
thousandths. `from_dpi` converts the native integer DPI relative to 96 DPI;
zero and unrepresentable values return typed window errors. Coordinate and
extent mapping use checked fixed-point arithmetic with nearest-pixel rounding.

`metis-ui-lang::LayoutViewport` carries the physical viewport dimensions and
the validated scale. `compute_layout` consumes this value so explicit pixel
sizes, margins, padding, borders, gaps, automatic child extents, and bitmap
text all use one scale. Percent sizes resolve once against the physical
viewport. `DisplayCommand::DrawText` retains the scale used by the layout and
the rasterizer applies the same fixed-point mapping without an intermediate
image or floating-point coordinate state.

`FrontendApp` owns the current scale. The native adapter converts each
`DpiChanged` event, repaints the application, and uses the same scale while
deriving the submit hit rectangle. A failed repaint restores the previous
scale and framebuffer, preserving the last valid presentation.

## Alternatives

Quantizing every DPI to an integer multiplier loses 125% and other common
scales, making geometry and text drift from the host. Floating-point positions
would make pixel rounding dependent on operation order and would duplicate
conversion rules across layout and rasterization. Adding the mapping to RITK
would couple a reusable host to DICOM presentation semantics and leave other
Metis applications without the contract.

## Threat model and limits

Native DPI is an external capability value. The constructor rejects zero and
values outside the validated fixed-point representation; checked arithmetic
rejects layout overflow before a display list is emitted. Direct text drawing
clips out-of-range fixed-point extents without panicking. The scale is a
presentation capability, not a permission or authority grant.

The Windows provider and its injected DPI event are covered by this increment.
A physical monitor transition, native accessibility technology, installed IME,
OS permission enforcement, and macOS/Linux display providers still require
their host-specific V05 evidence. The contract remains independent of DICOM;
RITK supplies decoded frames through the format-neutral native host.

## Verification

Display-scale tests cover 96/120/144 DPI conversion, zero rejection,
fractional coordinate rounding, scaled geometry and text, percentage sizing
against the physical viewport, repaint state retention, and deterministic
extreme-scale clipping. Focused native nextest and warning-denied Clippy cover
`metis-platform`, `metis-ui-lang`, `metis-frontend`, and `metis-app`; all
existing layout, native lifecycle, IPC, and process-isolation tests remain in
the same suite. The manual and V05 record the distinction between this
software mapping evidence and a future physical monitor capture.
