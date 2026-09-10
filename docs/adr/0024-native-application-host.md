# ADR 0024: Native application host seam

Status: Accepted

Date: 2026-09-10

Driver: [METIS-RITK-HOST-001](../../backlog.md#METIS-RITK-HOST-001)

Related application migration: [RITK-SNAP-METIS-001](../../../ritk/backlog.md#RITK-SNAP-METIS-001)

## Context

`NativeSurface` already owns the safe Metis boundary around Moirai's
thread-affine Windows window, finite event wait and retained ARGB presentation.
Each application still had to reimplement the same loop for event batches,
terminal-window cleanup and repaint decisions. That duplication makes a future
RITK viewer cutover prone to losing input or frame lifecycle behavior, and it
would invite format-specific logic into the platform crate.

RITK owns DICOM scanning, decoding, volume geometry, medical display semantics
and viewer state. Metis owns the application host contract only. The host seam
therefore needs to carry an application-produced frame and the complete Moirai
`WindowEvent` values without knowing whether the frame came from a form, a
viewer or another domain.

## Decision

`metis-platform::native` exposes the generic `NativeApplication` trait,
`NativeFlow` outcome and `run_native_application` loop. An application returns
its current `Framebuffer` and applies each bounded event batch, including an
empty batch when the finite wait expires. The application decides input policy,
state transitions and when a repaint is needed. A resize event must be applied
to the returned framebuffer before the application requests that repaint.

The host creates one `NativeSurface`, presents the initial frame, waits with the
caller-supplied finite duration, forwards the batch, and presents only when the
application reports `repaint: true`. A `CloseRequested`, `Destroyed` or
`NativeFlow::Exit` result ends the loop. The host closes a live surface on an
orderly exit and never closes an already-destroyed surface. Provider failures
and application failures remain distinct variants of `NativeHostError<E>` with
their source chains intact.

The existing `metis-app --metis-native-window` form implements this contract.
Its text, IME, submit, focus, DPI and resize policy stays in the application
adapter; the platform crate supplies no form or medical behavior. RITK can
implement the same seam with its validated viewer frames and viewer events,
while its DICOM loaders and presentation semantics remain in RITK. The browser
and WebView2 surfaces keep their existing provider-specific contracts and do
not inherit a native blocking loop.

## Alternatives

Keeping one loop per application preserves the current behavior but duplicates
terminal-event and repaint logic at every consumer and leaves the RITK cutover
without a canonical host seam. Adding a GUI toolkit would introduce another
window/event owner and a second teardown model. Moving DICOM or viewer state
into Metis would invert ownership and make a presentation framework depend on a
medical file format. A dynamic application trait would add a vtable to the
frame/event path without a runtime type-erasure requirement; the generic trait
keeps the path statically dispatched.

## Threat model and limits

The host receives native events and frame dimensions from the Moirai provider.
Moirai bounds event storage, text composition and native dimensions before the
adapter sees them. `NativeApplication` is an application policy boundary, not
an authority broker: it grants no filesystem, network, process, DICOM or WebView
capability. A consumer must validate its own domain inputs before producing a
frame and must keep its event transition bounded.

This increment is Windows-only because `metis-platform::native` is gated to the
Moirai Windows provider. The loop does not close the cross-platform host gap,
browser frame presentation, accessibility, OS permissions or RITK viewer
migration; those remain separate backlog items.

## Verification

At delivery revision `9934eeb5c05dedfdcd9c6da3088458b6ebaa07fe`, the native
platform and application suites pass with warning-denied Clippy and nextest. A
deterministic host-driver test records the exact initial and resized pixel
vectors, changed frame dimensions, repaint dispatch and orderly close;
companion tests cover destroyed-surface handling without a second close and
surface destruction after an application error.
The host tests also create real hidden HWNDs and verify provider readiness,
empty-batch delivery, typed application errors and typed finite-wait errors.
Existing provider tests cover retained-frame validation and two-window
lifecycle behavior. The frame-level capture and event trace are recorded in the
[native manual](../manual/native.md#inspect-the-host-trace-and-frame). No DICOM
dependency, parser or viewer state enters Metis; RITK remains the format and
medical-display owner.
