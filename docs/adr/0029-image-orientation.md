# ADR 0029: Pixel-grid image orientation

Status: Accepted

Date: 2026-09-12

Driver: [METIS-GRAPHICS-001](../../backlog.md#METIS-GRAPHICS-001)

Revision 2026-09-19: [METIS-ASSETS-001](../../backlog.md#METIS-ASSETS-001)
adds bounded native PNG admission and orientation-aware contain placement.
The native loader composes `ScopedFileProvider` with the pure-Rust `png`
decoder. Atlas provider inspection found no format-neutral bounded decoder:
RITK's path-based medical readers convert into medical image storage and do
not preserve this RGBA contract. Importing that domain into the host would
violate the dependency direction. The registry dependency is confined to
native `metis-ui-lang`; browser decoding remains browser-owned.

PNG admission checks the complete chunk envelope before decompression, then
uses strict CRC and Adler checks and finishes through IEND. `png` 0.18.1's
reader deliberately discards remaining compressed bytes after producing its
pixel extent, so a bounded `flate2` pass additionally requires explicit zlib
stream end, full compressed-input consumption and the exact scanline byte
count. The Adam7 extent sums the seven nonempty pass extents, each with its
filter bytes, as specified in [PNG 3 section 8.1](https://www.w3.org/TR/png-3/#8InterlaceMethods).
Every compressed prefix, checksum corruption, excess output and trailing
compressed byte is rejected by executable regression fixtures. Encoded bytes,
dimensions, pixels and decoder/output buffers are bounded. Unsupported
metadata, including EXIF, animation, ICC profiles and physical pixel spacing,
fails closed rather than silently changing orientation, colors or aspect.
Samples are straight RGBA, without color-management conversion. The initial
admitted subset is static PNG with IHDR/PLTE/tRNS/IDAT/IEND chunks; indexed
images require all palette entries, preventing the codec's invalid-index
black substitution. JPEG and
metadata-bearing images require a separately verified contract.

`ImagePlacement::contain` fits the full image after its discrete orientation,
centers it in positive bounds and leaves letterboxing untouched. Integer
division floors only the non-limiting extent, with less than one pixel of
rounding loss; a zero result fails rather than stretching. Arbitrary affine
transforms are rejected by contain because they require a different fit
contract. Clinical orientation remains RITK-owned.

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
A filtered or tessellated affine transform would add interpolation, rounding
and geometry policy beyond the nearest-neighbor image placement contract; those
operations remain separate graphics increments with their own oracles.

## Threat model and limits

Image dimensions, crops and destinations are validated before mapping. Every
coordinate product stays within widened `i64` bounds derived from positive
`i32` extents, and clipping bounds the visited pixels by the framebuffer area.
The source storage is immutable and never interpreted as a path, file or
format payload. The operation does not establish filtered affine sampling,
arbitrary affine vector paths, GPU acceleration, browser vector parity or
device-loss recovery. Width-aware
stroke geometry is defined by [ADR 0030](0030-bounded-polyline-strokes.md).

## Verification

The generic orientation suite renders one asymmetric 2×3 source through all
five transforms and asserts the exact row-major pixel order, including both
axis-swapping rotations. The image example renders identity, clockwise and
normalized affine placements from the same source, asserts representative
pixels in each view, and regenerates the inspected SVG/BMP artifact. Focused
`metis-ui-lang` nextest, strict formatting and the full Metis gate provide the
remaining verification; the manual links the visual artifact and keeps real
DICOM evidence in the RITK-owned workflow.
