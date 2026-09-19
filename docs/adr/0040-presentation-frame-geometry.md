# ADR 0040: Validated physical geometry at the canvas frame seam

Status: Accepted

Date: 2026-09-19

Driver: [METIS-PRESENTATION-GEOMETRY-001](../../backlog.md#METIS-PRESENTATION-GEOMETRY-001).

## Decision

`metis-web::CanvasFrame` remains a borrowed RGBA8 contract and gains an
optional validated `DisplaySpacing` value. A producer constructs the value only
through `DisplaySpacing::try_new`; `CanvasSurface::present` validates the
resulting physical aspect against the frame dimensions before the Moirai
provider upload. Pixel-only producers keep the default `None` value.

RITK owns the derivation from voxel geometry and clinical display state. Metis
does not parse DICOM, calculate slice spacing, or publish medical metadata. The
same contract is used by raster and explicit WebGPU surfaces, so renderer
selection cannot change geometry validation.

## Alternatives

- Keep spacing beside the frame in each consumer. Rejected because native,
  browser and Python consumers can then drift or upload a frame with malformed
  physical geometry.
- Put voxel or DICOM metadata in Metis. Rejected because it violates the
  format-neutral host boundary and couples the shell to RITK.
- Make spacing a required trait method. Rejected because pixel-space canvas
  consumers do not have a physical extent; the optional method preserves that
  valid use without a second trait.

## Invariants and verification

- Spacing components are finite and strictly positive.
- Frame dimensions remain validated by the existing Moirai canvas contract.
- The physical width-to-height ratio is finite and positive before upload.
- Existing pixel-only frame implementations retain identical dimensions and
  RGBA bytes.

Focused unit tests cover valid anisotropic ratios, invalid distances and empty
dimensions. Native and WASM package gates remain the delivery oracle. RITK
consumer integration carries the derived spacing into this seam in its next
co-evolution increment.
