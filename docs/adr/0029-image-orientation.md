# ADR 0029: Pixel-grid image orientation

Status: Accepted

Date: 2026-09-12

Driver: [METIS-GRAPHICS-001](../../backlog.md#METIS-GRAPHICS-001)

## Context

The format-neutral raster placement contract can scale and clip a validated
source crop, but it cannot preserve a quarter-turn or mirror operation without
the producer allocating a second pixel buffer. RITK currently applies its
clinical display orientation before handing a frame to Métis, while other
Atlas consumers can have the same need for a generic overlay or image surface.
The host must keep one pixel mapping contract and must not inspect DICOM or
other format metadata.

## Decision

`metis-ui-lang` adds `ImageTransform` with identity, horizontal flip, vertical
flip, clockwise quarter-turn and counter-clockwise quarter-turn variants.
`ImagePlacement::with_transform` stores the selected transform beside the
validated crop and destination. Existing placements remain identity placements.

Rendering dispatches once at the placement boundary to a zero-sized
`ImageMapper` strategy. Each monomorphized mapper computes nearest-neighbor
source coordinates in widened integer arithmetic; quarter-turns swap the
source extents before scaling. The loop clips the destination first, allocates
no intermediate image, and feeds every selected pixel to the existing
source-over compositor. The transform does not alter the source `Arc`, so
multiple views can share one decoded frame.

This is a presentation operation only. RITK continues to own DICOM parsing,
voxel geometry, clinical orientation policy and window/level decisions. A
consumer may pass an already oriented RITK frame or use this seam for a
format-neutral pixel-grid presentation; neither path adds DICOM knowledge to
Métis.

## Alternatives

Keeping orientation in each consumer duplicates pixel indexing and makes
browser, native and software output diverge. Copying and rotating the source
before every placement adds memory traffic and defeats shared-frame ownership.
An arbitrary floating-point affine transform would add interpolation,
rounding and unbounded tessellation policy before the current viewer contract
requires it; it remains a separate graphics increment with its own oracle.

## Threat model and limits

Image dimensions, crops and destinations are validated before mapping. Every
coordinate product stays within widened `i64` bounds derived from positive
`i32` extents, and clipping bounds the visited pixels by the framebuffer area.
The source storage is immutable and never interpreted as a path, file or
format payload. The operation does not establish arbitrary affine transforms,
stroke width/caps/joins, GPU acceleration, browser vector parity or device-loss
recovery.

## Verification

The generic orientation suite renders one asymmetric 2×3 source through all
five transforms and asserts the exact row-major pixel order, including both
axis-swapping rotations. The image example renders an identity placement and a
clockwise placement from the same source, asserts representative pixels in
both views, and regenerates the inspected SVG/BMP artifact. Focused
`metis-ui-lang` nextest, strict formatting and the full Metis gate provide the
remaining verification; the manual links the visual artifact and keeps real
DICOM evidence in the RITK-owned workflow.
