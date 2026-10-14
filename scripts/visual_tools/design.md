# Exact visual comparison ownership

`contract.py` owns the existing image/file bounds and contextual `VisualError`.
`codec.py` owns the admitted BMP and PNG grammars. `difference.py` owns the exact
pixel comparison. `visual.py` owns file/provenance/baseline orchestration and
invokes those owners directly. No old codec, bound, error, or difference names
remain exported by the orchestration module.

For admitted dimensions `1 <= w <= 800`, `1 <= h <= 600`, let `A` and `B` be
equal-length pixel vectors of length `4wh`. Pixel `i` is the four-byte segment
`[4i, 4i+4)` in the existing BGRA order. Define
`C = { i | A[4i:4i+4] != B[4i:4i+4] }`. The result is:

- Count `|C|`, including a change confined to the alpha byte.
- Bounds `None` when `C` is empty; otherwise the minimal half-open rectangle
  `[min(i % w), min(i // w), max(i % w)+1, max(i // w)+1]` over `C`.
- A complete `4wh`-byte mask: `(0, 0, 255, 255)` for each member of `C`, and
  `(0, 0, 0, 255)` otherwise. Every output alpha byte remains 255.

A row is the concatenation of exactly `w` disjoint pixel segments. Equality of
its two byte segments therefore proves equality of every pixel in that row.
The production comparison uses this equality to discharge an equal row and
leaves its precomputed black mask unchanged. An unequal row still uses the
existing four-byte pixel comparisons; every differing segment increments the
count once and sets only its mask red byte. This preserves the prior sliceable
buffer admission and does not introduce alignment, contiguity, or native-word
format requirements. No sampling, tolerance, sparse-fixture assumption, or
test-specific branch participates.

Rows and pixels visit increasing `y` and `x`. The first unequal row supplies
the minimum changed `y`, the last unequal row supplies the maximum, and changed
pixels update the global `x` extrema. Equal rows require bulk byte equality;
the scalar comparison workload is `w` times the number of unequal rows instead
of `wh`. The worst case still inspects every pixel and preserves dense changes.
All dimension and pixel-length admission occurs before row selection.

The existing 22 codec/evidence cases retain all ten 800x600 capture states,
three mutation probes, four-channel checks, and adversarial baseline/provenance
failures. Added tests derive all subsets of small grids analytically and compare
the optimized result with an independent scalar reference over equal, dense,
alternating, and sparse images at the maximum dimensions. They compare count,
bounds, dimensions, and every mask byte. Source buffers remain unchanged.

The Rust CLI does not exist yet when this Python test gate runs, so invoking it
here would require a recursive build boundary before the committed test gate.
This increment optimizes the actual Python comparison using bulk byte equality;
it adds no interpreter dependency or compiler/cache prerequisite.

`verify.source_state` records all files beneath `scripts/` recursively, so the
new implementation/design leaves join its full source provenance. The isolated
bootstrap tests copy the actual `visual_tools` tree along with `visual.py`.
Renderer fixture identities retain their existing Rust/font input domain.
The Python suite keeps its 60-second bound and named progress; actual Windows
CI must establish the full gate and native runtime outcome.
