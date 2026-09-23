# ADR 0030: Bounded polyline strokes

Status: Accepted

Date: 2026-09-13

Driver: [METIS-GRAPHICS-001](../../backlog.md#METIS-GRAPHICS-001)

## Context

RITK overlays need more than isolated one-pixel segments: measurement guides,
contours and application chrome use a width, endpoint treatment and vertex
join. The software framebuffer remains the deterministic presentation oracle
for native and WASM hosts, so the operation must preserve source-over alpha,
avoid per-segment overdraw, and bound work for off-screen or adversarial paths.
The contract is format-neutral; DICOM decoding and clinical geometry stay in
RITK.

## Decision

`metis-platform` exposes `StrokeWidth`, `LineCap`, `LineJoin` and
`draw_polyline`. Width is a nonzero pixel newtype. The renderer computes the
integer intersection of the path's width-derived envelope with the framebuffer
and classifies each candidate pixel against every segment and join before
blending it once. Butt, square and round endpoint caps are explicit. Miter
joins use a four-radius bound and fall back to bevel geometry when the bound is
exceeded; bevel and round joins remain available directly. A path stores at
most 4,096 vertices in a `DisplayCommand::DrawPolyline` command.

`metis-ui-lang::DisplayList::append_polyline` copies a caller-owned slice once
after validating the point bound. It preserves painter order and shares the
same rasterizer as direct platform calls. The existing one-pixel
`DisplayCommand::DrawLine` remains the separate segment primitive because its
integer Bresenham contract is useful when a caller explicitly needs a single
pixel centerline.

## Alternatives

Drawing each segment independently would blend shared translucent pixels more
than once and would make joins caller-dependent. A floating-point tessellator
with an allocated polygon list would add lifetime and clipping work before the
software oracle requires it. Replacing the existing one-pixel command would
break consumers that rely on its exact Bresenham pixels, so the width-aware
path is a distinct operation with its own validated style.

## Threat model and limits

Path coordinates and colors are application inputs. The point count is bounded
at the display-list boundary, coordinate differences are widened to `f64` only
after conversion from `i32`, and the scan is clipped before pixel testing.
There is no file, network, process or authority access. The implementation is
a deterministic software renderer; GPU acceleration, arbitrary affine vector
paths, browser vector parity, and device-loss recovery remain separate backlog
work.

## Verification

`metis-platform` tests cover zero-width rejection, square versus butt extent,
overlap-safe translucent blending and clipped rendering. `metis-ui-lang` tests
cover command style retention, painter execution and the 4,096-point bound.
The `image` example renders round/round and square/bevel frames around the
identity and quarter-turn image placements, with a normalized affine shear
below them; its inspected PNG is the visual component demonstration. Strict
Clippy, formatting and focused nextest runs are required before delivery.
