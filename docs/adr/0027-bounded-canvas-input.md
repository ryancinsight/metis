# ADR 0027: Bounded format-neutral canvas input

Status: Accepted

Date: 2026-09-11

Driver: [METIS-CANVAS-INPUT-001](../../backlog.md#METIS-CANVAS-INPUT-001).

## Context

The Métis canvas surface already presents consumer-owned RGBA frames, but a
viewer cannot replace its browser input path until pointer and wheel events
cross the same boundary. Browser callbacks are bursty and pointer capture is
cancelable, so an unbounded callback-to-application queue would make memory
usage depend on application scheduling. RITK also needs target-local
coordinates and a typed failure that cancels its active gesture.

DICOM parsing, study metadata, geometry, pixel interpretation and viewer state
are RITK responsibilities. A canvas provider must remain independent of those
formats and concepts.

## Decision

`metis-web` exposes a format-neutral `CanvasEvent` enum and a fixed-capacity
queue. `CanvasSurface::from_current_document_with_input` retains Moirai pointer
and wheel listeners, captures active pointers, records target-local CSS-pixel
coordinates, and releases captures on up, cancel, overflow or drop. Queue
overflow and provider failures clear pending events and return a typed error.

The consumer translates the batch into its own presentation events. RITK maps
browser wheel units to its declared host units, rejects non-finite values and
cancels the reducer gesture on a translation or application failure. No
filesystem, DICOM, image-format or clinical state enters Métis.

## Rejected alternative

Passing raw browser event objects to RITK would couple the consumer to a DOM
runtime, retain browser handles across frames and make queue and capture
lifetime implicit. A DICOM-aware canvas adapter would duplicate RITK's format
authority and is outside the shell boundary.

## Verification

Native queue tests cover ordering, capacity, overflow disposal and recovery
after a typed failure. The Metis WASM target checks listener/provider wiring.
RITK's browser adapter consumes the bounded batch, routes each orthogonal
canvas to its viewport reducer and preserves the existing DICOM-owned render
path. Browser-driver and physical pointer evidence remain part of the RITK
viewer acceptance item; compile-only evidence does not claim that coverage.
