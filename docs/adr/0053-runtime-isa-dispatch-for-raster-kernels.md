# 0053 — Runtime instruction-set dispatch for raster kernels

Status: Accepted

Date: 2026-09-25

Driver: METIS-PERF-001 (the diagonal gradient as the dominant full-repaint cost); this change's pull request.

## Context

Metis builds for the baseline x86-64 target, so `f64::mul_add` compiles to a
call into the C library, and loops built on it do not vectorize. Once a
diagonal gradient interpolates runs of columns without a stop search
(`rasterizer/gradient/span.rs`), those runs are straight-line code. On the
eight-card bench (`fill/gradient_card_stack`, 299,520 pixels) they took
1.81 ms at baseline and 1.02 ms when the whole bench was built with
`+avx2,+fma`.

Reaching that code generation at runtime needs a `#[target_feature]` scope,
and entering one from code without those features is `unsafe`.
`metis-platform` is `#![forbid(unsafe_code)]`. The stack map names Hermes as
the owner of CPU instruction-set dispatch. Its `hermes_simd::vectorize` runs a
`LaneKernel` inside the widest scope the host supports, and needs no `unsafe`
from the consumer.

## Decision

- `metis-platform` depends on `hermes-simd` (git plus version `0.7`, default
  features off, `std` on). Raster kernels whose speed depends on the
  instruction set enter through `vectorize`. They do not use their own
  trampolines or `core::arch` intrinsics.
- A kernel is a `LaneKernel<f64>` whose `call` runs the ordinary scalar body.
  The compiler vectorizes it inside each instruction set's scope. The body and
  the helpers that carry its loops are `#[inline(always)]`, each with a
  measured reason, because an outlined body compiles at baseline. Measured
  with `opaque_row` or `interpolate` left to the inliner, the bench fell back
  to 1.79–1.81 ms; small helpers such as `fraction` and `byte` inline without
  forcing.
- Output does not depend on the instruction set. Every operation in these
  kernels (`mul_add`, subtraction, multiplication, clamping, truncation) is
  correctly rounded on every backend, the C library's `fma` included. The
  bit-for-bit sweep in `span_tests.rs` runs on whichever backend the test host
  selects.

## Alternatives

- **Own `#[target_feature]` trampolines in `metis-platform`** would need
  `unsafe` in a crate that forbids it. They would also be a second
  instruction-set dispatcher in the stack, which is what Hermes exists to
  prevent.
- **A raised baseline** (`target-cpu` or `+fma` in the release profile) would
  drop every host without those features. Code-generation settings come last
  in the optimization order.
- **Memoizing gradient pixels** would help only repeat paints of the same
  geometry, not first paints or resizes.

## Consequences

- The dependency brings Hermes' five crates and, through `hermes-simd-core`,
  `rkyv` and its derive crates, which Hermes uses for zero-copy containers.
  `deny.toml` allows the Hermes git source.
- wasm32 builds select Hermes' scalar backend: same pixels, baseline speed.
- On the card-stack bench, the gradient went from 2.34 ms before the run
  kernel to 1.81 ms with it and to 1.38 ms with dispatch, on a host with AVX2
  and FMA. The remaining gap to a full `+fma` build is antialiased edge
  pixels, which still take the per-pixel path outside the scope.

## Verification

- `runs_match_the_per_pixel_colors_bit_for_bit` sweeps 37 angles, four stop
  sets and three placements. Whole rows must equal the per-pixel colors
  exactly, and every interpolated run is checked on its own.
- The frame-hash and gradient oracles pass unchanged.
- `cargo build --target wasm32-unknown-unknown -p metis-platform` passes, and
  so does `cargo deny check`.
