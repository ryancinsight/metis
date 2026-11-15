# ADR 0029: Pixel-grid image orientation

Status: Accepted

Date: 2026-09-12

Driver: [METIS-GRAPHICS-001](../../backlog.md#METIS-GRAPHICS-001)

Revision 2026-09-20: [METIS-ASSETS-001](../../backlog.md#METIS-ASSETS-001)
extends bounded native PNG admission to JPEG and EXIF orientation. The byte
boundary is `RasterImage::decode`; capability-checked file admission is
`RasterImage::load`. These replace `decode_png` and `load_png` with unchanged
argument and result types; `cargo-semver-checks` confirms the removals are a
major API change, and release versioning remains a separate authorized action.
Browser decoding remains browser-owned. RITK already implements sequential and
lossless JPEG primitives and its file readers use an independent image decoder,
so the shared JPEG and EXIF implementation moves to `consus-raster` under
[Consus ADR 0004](../../../consus/docs/adr/0004-raster-codecs.md). This avoids a
RITK-to-Metis-to-RITK repository cycle and preserves one format provider.
Both consumer suites must pass before the duplicate implementations count as
removed.

Revision 2026-09-21: Consus extends Huffman/arithmetic JPEG process and
precision coverage and owns shared display quantization; Apollo owns the DCT
used by reconstruction.

Encoded EXIF orientation is normalized once into immutable raster storage, so
the returned width and height describe the oriented pixel grid and presentation
uses identity unless the caller requests an additional transform. Metadata is
neither omitted nor applied twice, and no EXIF camera metadata enters the
framebuffer or platform host.

The independent orientation oracle uses the six row-major cells `A B / C D /
E F`. [CIPA DC-X010-2017, Table 3](https://cipa.jp/std/documents/e/DC-X010-2017.pdf)
defines the corresponding row/column origins:

| EXIF values | Returned rows, in value order | Dimensions |
| --- | --- | --- |
| 1 to 4 | AB / CD / EF; BA / DC / FE; FE / DC / BA; EF / CD / AB | 2 by 3 |
| 5 to 8 | ACE / BDF; ECA / FDB; FDB / ECA; BDF / ACE | 3 by 2 |

PNG admission checks the complete chunk envelope before decompression, then
uses strict CRC and Adler checks and finishes through IEND. `png` 0.18.1's
reader discards remaining compressed bytes after producing its pixel extent,
so a bounded `flate2` pass additionally requires explicit zlib stream end,
full compressed-input consumption and the exact scanline byte count. The Adam7
extent sums the seven nonempty pass extents, including filter bytes, as
specified in [PNG 3 section 8.1](https://www.w3.org/TR/png-3/#8InterlaceMethods).
Static PNG supports samples up to eight bits and complete indexed palettes;
straight RGBA samples retain alpha, and EXIF orientation is read as for JPEG.
Color management and physical pixel spacing are separate contracts;
unsupported color profiles, animation and spacing fail closed.

JPEG reconstruction and strict entropy admission belong to the shared provider.
Terminal-marker checks alone do not establish complete scans, because
permissive decoders can fill missing entropy bits. The shared parser follows
[ITU-T T.81](https://www.w3.org/Graphics/JPEG/itu-t81.pdf): Annex E for scan
and restart structure, Annex F for sequential Huffman coding and Annex G for
progressive coding. During reconstruction it checks the declared scan's
coded-block count, amplitude and refinement bits, restart order and scan
progression. JPEG has no checksum, so changed bytes that form another valid
JPEG cannot be classified as corruption without an external integrity digest.
Arithmetic decoding follows Annex D, including its implicit zero bits after a
physical marker; not every entropy edit followed by a replacement EOI is
detectable truncation, while physical EOF and invalid scan structure still
fail. The premature-marker corpus remains a provider regression oracle, and
Metis contains no separate entropy validator, IDCT or JPEG color conversion.

The provider retains eight- and twelve-bit sequential/progressive DCT gray and
RGB samples plus two- through sixteen-bit single-component lossless gray
samples at their declared precision. Consus maps them into its opaque
eight-bit display contract through the one exact nearest full-range conversion
`(sample * 255 + max / 2) / max`, where `max = (1 << precision) - 1`. This is
display quantization, not clinical windowing or rescaling: samples are not
widened, normalized through a floating type or interpreted as modality values.
Metis assembles the returned pixels with alpha 255 and orients them once.

EXIF parsing checks the TIFF byte order, magic, field types/counts, external
value extents and directory references. Traversal uses a fixed sixteen-directory
budget and rejects cycles. Only the primary image's orientation changes the
returned pixels. Both byte orders and all eight orientation values have exact
asymmetric-grid tests. This is bounded image admission, not a general EXIF
metadata editor or TIFF image decoder.

`ImagePlacement::contain` fits the full image after its discrete orientation,
centers it in positive bounds and leaves letterboxing untouched. Integer
division floors only the non-limiting extent, with less than one pixel of
rounding loss; a zero result fails rather than stretching. Arbitrary affine
transforms are rejected because they need a different fit contract.

## Context

The raster placement contract scales and clips a validated source crop but
cannot preserve a quarter-turn or mirror without the producer allocating a
second pixel buffer. RITK orients clinically before handing a frame to Métis,
and other Atlas consumers share the need. The host keeps one pixel mapping
contract and never inspects DICOM or other format metadata.

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

This is a presentation operation only: RITK keeps DICOM parsing, voxel
geometry, clinical orientation policy and window/level decisions, and a
consumer may pass an already oriented frame or use this seam for format-neutral
pixel-grid presentation without adding DICOM knowledge to Métis.

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
device-loss recovery; width-aware stroke geometry is defined by
[ADR 0030](0030-bounded-polyline-strokes.md).

## Verification

The generic orientation suite renders one asymmetric 2×3 source through all
five transforms and asserts the exact row-major pixel order, including both
axis-swapping rotations. JPEG fixtures cover two-, seven-, twelve- and
sixteen-bit gray samples plus twelve-bit RGB samples against the integer
conversion formula; the asymmetric EXIF corpus covers all eight orientations.
The native image example presents five PNG transforms, all eight EXIF
orientations, precision conversion and arithmetic JPEG fixtures with pixel and
containment assertions. Focused `metis-ui-lang` nextest, strict formatting and
the full Metis gate apply; the manual links the visual artifact, and real DICOM
evidence stays in the RITK-owned workflow.
