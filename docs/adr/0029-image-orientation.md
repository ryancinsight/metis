# ADR 0029: Pixel-grid image orientation

Status: Accepted

Date: 2026-09-12

Driver: [METIS-GRAPHICS-001](../../backlog.md#METIS-GRAPHICS-001)

Revision 2026-09-20: [METIS-ASSETS-001](../../backlog.md#METIS-ASSETS-001)
extends bounded native PNG admission to JPEG and EXIF orientation. The byte
boundary is `RasterImage::decode`; capability-checked file admission is
`RasterImage::load`. These replace the PNG-specific entry points.
Callers migrate `decode_png` to `decode` and `load_png` to `load`; argument and
result types are unchanged. `cargo-semver-checks` confirms these removals require
a major API change. Release versioning remains a separate authorized action.
Browser decoding remains browser-owned. RITK already implements sequential and lossless
JPEG primitives; its file readers also use an independent image decoder.
The shared JPEG and EXIF implementation moves to `consus-raster` under
[Consus ADR 0004](../../../consus/docs/adr/0004-raster-codecs.md).
This avoids a RITK-to-Metis-to-RITK repository cycle and preserves one format
provider. Consus maps decoded integer samples into the display range; Metis
assembles opaque RGBA pixels and RITK retains clinical interpretation. The
migration must pass both consumer suites before the duplicate implementations
are considered removed.

Revision 2026-09-21: Consus [PR 80](https://github.com/ryancinsight/consus/pull/80)
extends Huffman/arithmetic JPEG process and precision coverage and owns shared
display quantization. Apollo [PR 526](https://github.com/ryancinsight/apollo/pull/526)
owns the DCT transform used by reconstruction. Consumer code retains only
presentation or clinical interpretation; no consumer duplicates these kernels.

Encoded EXIF orientation is normalized once into immutable raster storage.
The returned width and height describe the oriented pixel grid; presentation
therefore uses identity unless the caller requests an additional transform.
This avoids accidentally omitting metadata or applying it twice. No EXIF
camera metadata enters the framebuffer or platform host.

The independent orientation oracle uses the six row-major cells `A B / C D /
E F`. [CIPA DC-X010-2017, Table 3](https://cipa.jp/std/documents/e/DC-X010-2017.pdf)
defines the corresponding row/column origins:

| EXIF value | Returned rows | Dimensions |
| --- | --- | --- |
| 1 | AB / CD / EF | 2 by 3 |
| 2 | BA / DC / FE | 2 by 3 |
| 3 | FE / DC / BA | 2 by 3 |
| 4 | EF / CD / AB | 2 by 3 |
| 5 | ACE / BDF | 3 by 2 |
| 6 | ECA / FDB | 3 by 2 |
| 7 | FDB / ECA | 3 by 2 |
| 8 | BDF / ACE | 3 by 2 |

PNG admission checks the complete chunk envelope before decompression, then
uses strict CRC and Adler checks and finishes through IEND. `png` 0.18.1's
reader discards remaining compressed bytes after producing its pixel extent,
so a bounded `flate2` pass additionally requires explicit zlib stream end,
full compressed-input consumption and the exact scanline byte count. The
Adam7 extent sums the seven nonempty pass extents, including filter bytes,
as specified in [PNG 3 section 8.1](https://www.w3.org/TR/png-3/#8InterlaceMethods).
Static PNG supports samples up to eight bits and complete indexed palettes;
EXIF uses the same orientation interpretation as JPEG. Straight RGBA samples
retain alpha. Color management and physical pixel spacing are separate
contracts; unsupported color profiles, animation and spacing fail closed.

JPEG reconstruction and strict entropy admission belong to the shared provider.
Codec review found that terminal-marker checks alone do not establish complete
scans: permissive decoders can fill missing entropy bits. The shared parser follows
[ITU-T T.81](https://www.w3.org/Graphics/JPEG/itu-t81.pdf), Annex E for scan and
restart structure, Annex F for sequential Huffman coding, and Annex G for
progressive coding. It checks the declared scan's coded-block count, amplitude
and refinement bits, restart order and scan progression during reconstruction.
JPEG has no checksum: changed bytes that form another valid JPEG cannot be
classified as corruption without an external integrity digest. Arithmetic
decoding follows Annex D, including its implicit zero bits after a physical
marker; therefore, not every entropy edit followed by a replacement EOI is
detectable truncation. Physical EOF and invalid scan structure still fail.
The premature-marker corpus remains a provider regression oracle. Metis contains
no separate entropy validator, IDCT or JPEG color conversion after migration.

The provider retains eight- and twelve-bit sequential/progressive DCT gray and
RGB samples plus two- through sixteen-bit single-component lossless gray
samples at their declared precision. Consus maps byte or native-endian wide
samples into its opaque eight-bit display contract through the one exact
nearest full-range conversion `(sample * 255 + max / 2) / max`, where
`max = (1 << precision) - 1`. This is display quantization, not clinical
windowing or rescaling: encoded values are not widened, normalized through a
floating type, or interpreted as modality values. Metis assembles the returned
opaque pixels and applies the existing eight-orientation normalization once, so
orientation cannot apply twice and every returned JPEG pixel has alpha 255.

EXIF parsing checks the TIFF byte order, magic, field types/counts, external
value extents and directory references. Traversal uses a fixed sixteen-directory
budget and rejects cycles. Camera metadata is not exposed; only the primary
image's orientation changes the returned pixels. Both byte orders and all
eight orientation values have exact asymmetric-grid tests. This is bounded
image admission, not a general EXIF metadata editor or TIFF image decoder.

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
axis-swapping rotations. JPEG fixtures cover two-, seven-, twelve- and
sixteen-bit gray samples plus twelve-bit RGB samples against the integer
conversion formula; the existing asymmetric EXIF corpus covers all eight
orientations. The native image example presents five explicit PNG transforms,
all eight normalized EXIF orientations, precision conversion, and arithmetic
JPEG fixtures, with representative pixel and containment assertions. Focused
`metis-ui-lang` nextest, strict formatting and the full Metis gate provide the
remaining verification; the manual links the visual artifact and keeps real
DICOM evidence in the RITK-owned workflow.
