# ADR 0020: Python presentation surface

Status: Accepted

Date: 2026-09-09

Driver: [METIS-PYTHON-002](../../backlog.md#METIS-PYTHON-002).

## Context

The first `metis-rs` wheel exposed validated clinical values but could not
compose a Rust-rendered frame. A Python consumer needs a small surface for
image inspection and application composition before a native window and event
loop can be bound. The presentation contract already owns bounded RGBA images,
rectangles, clipping and source-over alpha in `metis-ui-lang` and
`metis-platform`.

Python must not duplicate those rules or acquire filesystem, process, network,
window or DICOM authority. A wheel also needs one stable cross-platform API;
the Windows-only native host cannot be placed in the abi3 binding without
making unsupported targets appear available.

## Decision

Add `Rect`, `RasterImage` and `Canvas` to the existing `metis._metis` module.
`RasterImage` accepts exact row-major RGBA bytes and delegates dimension,
storage and channel validation to the Rust image type. `Rect` carries the
integer crop or destination geometry. `Canvas` owns a bounded Rust
`Framebuffer`, clears it, and submits validated nearest-neighbor image
placements through the existing image renderer; clipping and source-over alpha
therefore have one implementation. The draw operation performs no allocation
after validation.

`Canvas.to_rgba()` returns a cold-boundary bytes copy. The copy is explicit so
Python cannot retain an alias to Rust storage or mutate a frame during a host
present. The binding exposes no native window, event loop, DICOM decoder or
filesystem path. Those surfaces remain separate contracts: Moirai owns native
window mechanisms, and RITK owns DICOM parsing and medical-display semantics.

## Alternatives

Python-side Pillow or NumPy rendering would duplicate the Rust compositor,
add third-party runtime dependencies and create divergent alpha behavior. A
Python callback or second event loop would violate the existing host lifecycle
and make teardown unbounded. Binding `NativeSurface` directly would make a
Windows-only implementation look portable in an abi3 wheel and would bypass
the host authority boundary. Returning raw framebuffer storage would expose
mutable Rust memory across the language boundary.

## Threat model and limits

Image dimensions, pixel counts, rectangle bounds and output allocation are
bounded before drawing. The binding accepts bytes, not paths or URLs, so it
adds no filesystem or network authority. Invalid geometry returns a Python
`ValueError` carrying the stable Metis error code. The bytes copy prevents
post-render mutation through an alias. The contract does not validate image
file formats, transfer syntax, orientation or clinical display policy; those
checks remain with the provider that decodes the input.

## Verification

The Rust image suite covers RGBA channel conversion and byte-length errors.
The extracted-wheel pytest suite covers asymmetric scaling with clipping,
exact row-major output, alpha-over-white composition, invalid crops and bounded
dimension errors. The Python manual uses the same six-channel fixture and links
to the inspected software-renderer artifact. `cargo clippy --all-targets
-- -D warnings`, `cargo nextest`, the locked maturin wheel build and pytest
exercise the exact binding and its shared image implementation.
