# ADR 0019: Raster display command

Status: Accepted

Date: 2026-09-09

Driver: [METIS-ASSETS-001](../../backlog.md#METIS-ASSETS-001).

## Context

The software display list previously covered rectangles, borders and bitmap
text. The RITK viewer migration needs a host-independent way to present pixels
decoded by an owning Atlas format provider. The command must preserve painter
order, bounded memory and the framebuffer's existing alpha contract without
moving DICOM or image-format parsing into `metis-ui-lang`.

`DisplayCommand` is a public enum. Adding an image variant changes the match
surface for exhaustive downstream matches, so the change has major SemVer
impact even though it is additive in capability. Release metadata remains at
the current workspace version until an explicitly authorized release increment.

## Decision

Add `RasterImage`, `ImagePlacement` and `ImageSampling::Nearest` to
`metis-ui-lang`. `RasterImage::new` validates nonzero dimensions, coordinate
limits, the shared 16,777,216-pixel storage bound and an exact row-major pixel
count, then retains the immutable pixels in `Arc<[Color]>`. A placement validates
an in-bounds positive source crop and positive destination dimensions; its
coordinates may extend off-screen for clipping.

`DisplayCommand::DrawImage` carries the validated placement. Rendering clips
the destination against the framebuffer before iteration, maps each visible
destination pixel to a source texel with nearest-neighbor integer arithmetic and
calls the framebuffer's source-over compositor. The draw loop performs no
allocation and uses only the validated image storage. The enum is marked
`non_exhaustive` so later command families do not repeat the same downstream
exhaustiveness break.

Image-format decoding, orientation metadata, transfer-syntax policy, fonts and
media controls remain upstream/provider or browser-host responsibilities. RITK
continues to own DICOM parsing and medical-display semantics; it supplies
validated pixels to this boundary when the viewer migration reaches it.

## Alternatives

Keeping images in a second display list would lose painter order or require a
second renderer contract. Adding a raw byte or path command would move hostile
format parsing and filesystem authority into the presentation crate. Rendering
only in the browser would leave the native software surface unable to host the
same viewer. A silent fallback to a rectangle would hide unsupported image data
and violate the typed rendering contract.

## Threat model and limits

Image dimensions and pixel counts are checked before storage is accepted, and a
source crop cannot address outside the validated image. Destination clipping
prevents off-screen coordinates from driving unbounded loops. Source colors are
copied by value into the existing alpha compositor; no path, network, process or
filesystem authority is introduced. The image type does not decode untrusted
bytes, so format parser hardening and orientation tests remain with the
provider that owns those bytes.

## Verification

`metis-ui-lang` tests cover invalid dimensions and storage, invalid crops and
destinations, asymmetric nearest-neighbor scaling, clipping, painter order and
alpha-over-background values. `examples/image.rs` renders a 3×2 fixture into a
240×180 framebuffer, asserts the source colors and untouched background, and
writes SVG/BMP artifacts under `output/`. The reviewed SVG is committed in
`docs/manual/images/`, and the visual gate compares it exactly beside the
existing form captures. The public API
check passes with `cargo semver-checks check-release --workspace
--baseline-rev HEAD^ --release-type major`; a default release check correctly
classifies the enum addition as major.
