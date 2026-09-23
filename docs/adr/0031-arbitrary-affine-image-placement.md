# ADR 0031: Arbitrary affine image placement

Status: Accepted

Date: 2026-09-13

Driver: [METIS-GRAPHICS-001](../../backlog.md#METIS-GRAPHICS-001)

## Context

The format-neutral raster placement boundary already supports integer pixel-grid
orientation, but RITK overlays and other Atlas applications also need a bounded
way to shear, reflect and scale an image without allocating a transformed
source. The operation must remain a presentation primitive: DICOM parsing,
clinical orientation, voxel geometry and interpolation policy remain with RITK
or another asset provider.

## Decision

`metis-ui-lang` adds `AffineTransform`, carrying six private `f64` coefficients
for the normalized source-to-destination mapping
`x' = a*x + c*y + tx` and `y' = b*x + d*y + ty`. Construction rejects
non-finite coefficients, a singular linear part and an inverse that is not
finite. `ImageTransform::Affine` stores the validated value on an existing
`ImagePlacement`.

The software renderer clips the integer destination before visiting pixels. It
maps each destination pixel center through the affine inverse, rejects samples
outside the normalized crop, selects the nearest source texel and uses the
existing source-over compositor. The source remains in its shared immutable
`Arc`, and the draw loop allocates no intermediate image or geometry.

## Alternatives

Keeping affine mapping in every consumer duplicates clipping and sampling rules
and makes native, browser and software output diverge. Copying a transformed
image before placement adds memory traffic and breaks shared-frame ownership.
Restricting the API to pixel-grid turns would leave valid overlay transforms
unrepresentable. A floating-point polygon or filtered resampler would add an
unbounded geometry/interpolation contract that the current nearest-neighbor
presentation surface does not require.

## Threat model and limits

Coefficient values and placement rectangles are application inputs. The
constructor validates finiteness and invertibility before the value can enter a
placement. Rendering bounds work by clipping to the framebuffer and rejects
non-finite or out-of-range inverse samples. Destination coordinates remain the
validated integer rectangle used by the existing surface contract. The mapping
is nearest-neighbor only; it does not establish filtering, color management,
clinical orientation, GPU acceleration, device-loss recovery or browser vector
parity.

## Verification

`metis-ui-lang` tests render identity and reflection mappings, a scaled mapping
with clipped output, and all invalid coefficient classes. They assert exact
pixel values and preserve the source storage address. The `image` example
renders identity, quarter-turn and affine shear placements from one shared
3×2 source and emits the inspected BMP/PNG artifact. Focused strict Clippy,
formatting and nextest runs plus the full locked Metis gate provide the
revision-bound verification.
