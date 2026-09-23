# 0046 — Gaussian outer box shadows

Status: Accepted

Date: 2026-09-22

Driver: [METIS-RASTER-SHADOW-001](https://github.com/ryancinsight/metis/pull/370).

## Context

The software renderer paints rounded fills and borders
([ADR 0043](0043-rounded-rectangle-paint.md)) but every surface sits flat on
the one behind it: nothing separates a card from its page except a hairline
border. Elevation — a soft shadow under a raised surface — is the conventional
depth cue, and CSS expresses it as `box-shadow`, which the strict style contract
([ADR 0013](0013-strict-style-contract.md)) rejects as an unknown property.

CSS Backgrounds and Borders Level 3 fixes the semantics:

- §6.1.1: an outer shadow is cast as if the border box were opaque, with the
  same size and shape as the border box absent a spread distance, and it is
  clipped inside the border box.
- §6.1.2: the result must approximate, with each pixel within 5% of its
  expected value, a Gaussian blur whose standard deviation is half the blur
  radius.
- §6.1.3: outer shadows paint immediately below the element's background.

## Decision

`metis-platform` gains `BoxShadow` (offsets, blur, color) and
`draw_box_shadow(fb, border_box, radius, shadow)`; `metis-ui-lang` admits
`box-shadow` and emits `DisplayCommand::DrawShadow` below the background.

- The blur is a Gaussian of standard deviation `σ = blur / 2`, separable into a
  horizontal and a vertical pass. The kernel reaches `K = ⌈3.5σ⌉` pixels; the
  dropped tails hold `2Φ(−3.5) ≈ 4.7e-4`, below half an 8-bit level.
- Straight rows — every row outside the corner bands — span the full width, so
  their horizontal pass is a difference of the running sum of the kernel's
  taps, each tap the Gaussian's mass over one pixel. For a pixel-aligned edge
  that is exactly the continuous blur sampled at the pixel centre, and the
  straight rows in a vertical window collapse into one more running-sum
  interval.
- Rows crossing a corner arc are integrated as `S` slabs, each with the arc's
  extent at its centre, an exact horizontal profile (`Φ` differences seen from
  the pixel centre) and the Gaussian's exact mass over the slab vertically.
  Near the flat apex of a large arc the edge is locally horizontal and the
  slab puts it at its centre, so the error is first order: at most half the
  largest slab weight, `(2Φ(1/(2σS)) − 1) / 2`. `S` is the smallest power of
  two with `σS ≥ 8`, which bounds it at 2.49% of full scale, 6.35 levels, half
  the specification's 5%. Blurs of 16 pixels and up need one slab.
- Zero blur uses the same structure with the box profile and sixteen slabs,
  which is exactly the fill's coverage: the shadow equals `fill_rect` of the
  offset shape.
- Arc rows are kept in a ring of `min(2K + 1, 2r)` slots keyed by their ordinal
  among the arc rows. Top-band ordinals run straight into bottom-band ones, so
  the arc rows a window reaches are one contiguous run and never collide.
  Only columns an arc can reach are stored, and columns are processed in tiles
  of 1024, so scratch memory is bounded by `897 · 1024 · 8` bytes, 7.3 MB, for
  any surface. A square shadow allocates no ring.
- The shadow is clipped inside the border box: pixels the box covers are
  skipped and arc pixels it covers partly receive the uncovered share.
- `BoxShadow::MAX_BLUR` is 256 device pixels; layout reports a scaled blur past
  it as `ErrorCode::LayoutOverflow`.
- The style subset is one shadow: `none` or `<x> <y> [<blur>] <color>`, color
  first or last. `inset`, a spread distance and comma-separated lists are
  `ErrorCode::InvalidCssStyle`, not silently dropped, because the renderer has
  no semantics for them.
- `erfc` comes from `eunomia::FloatElement`, the stack's numeric vocabulary,
  already in the lock through Moirai; `f64::erf` is unstable on the pinned
  toolchain.

The specification's tolerance, "within 5% of its expected value", is read as
5% of the channel's full scale, 12.75 levels. Read as relative error it is
unsatisfiable by any 8-bit rasterizer: a pixel whose expected alpha is 0.4
levels must round to 0 or 1, a relative error of at least 100%.

Adding the shadow to `ComputedStyle` pushed `DomElement` past Clippy's
enum-variant size bound, so `DomNode::Element` now boxes its element. A text
node no longer reserves an element's computed style.

## Alternatives

Per-pixel analytic evaluation — exact along x, quadrature along y over the
whole window — was measured first. Eight-node Gauss–Legendre and midpoint
rules both left errors of 2–7% at small σ with large radii, because the arc
makes the integrand change over a fraction of a pixel.

Convolving the antialiased coverage mask with the discrete kernel on both axes
was the first delivered form. It is exact on straight edges, but coverage is
already an area average, so the arcs are blurred by an extra pixel-wide box on
each axis. An independent review measured 15.6 levels against the 13.25-level
bound at a one-pixel blur, the smallest the style subset accepts. Integrating
arc rows as slabs with exact profiles removes that box and measures 0.5 levels
at the same blur on a 12-pixel radius.

The slab count was first chosen for `σS ≥ 4` on a second-order error law
measured only on that radius. A second review pass showed the law is first
order near the flat apex of large arcs, where it reached 13.16 of the 13.25
permitted levels at a radius of 67 million pixels; the resolution was doubled
to halve the bound, and a large-radius test pins it.

A triple box blur, the common browser approximation, was rejected: it is not
exact on straight edges, and exactness there is what lets the straight-edge
test assert the closed form.

Blurring a full rasterized mask of the whole shadow rectangle was rejected: its
cost is proportional to the shadow's area times the kernel width, where the
region decomposition pays only near the arcs.

## Verification

`crates/metis-platform/src/rasterizer/shadow_tests.rs`:

- normal distribution values against Abramowitz and Stegun Table 26.1;
- kernel taps equal to the Gaussian mass per pixel and symmetric, each row's
  slabs summing to its tap, the running-sum interval equal to the direct sum,
  and `σS ≥ 8` for every blur;
- a straight edge equal to the continuous blur at every column within half a
  level plus the truncation bound;
- corners within the specification's 5% (plus half a level of rounding) of a
  continuous reference for blur 1, 2, 3, 4, 8 and 16, at every pixel for the
  three smallest;
- the apex region of arcs with radius 200, 1,000 and 5,000, where the edge crosses rows at fractional heights, within the derived
  first-order slab bound for blur 1, 2 and 4;
- zero blur equal to `fill_rect` of the offset shape outside the border box;
- the border-box clip, and mirrored corners within one level;
- the region decomposition within one level of a direct evaluation of every
  slab for every pixel, with Gaussian masses computed independently of the
  kernel's tables: a narrow shape, a shape of height `2r`, a wide shape, a
  shape partly off the surface, square corners, a one-pixel blur on an arc, a
  one-pixel crescent and a shape crossing the 1024-column tile boundary;
- extreme geometry — coordinates, offsets and widths at the `i32` limits,
  radius `i32::MAX`, blur 0, 1 and 256 — painting nothing when the blurred
  extent misses the surface and nothing when the border box covers it. This
  test found a conversion panic on columns past `i32::MAX`, fixed before
  delivery.

Mutating σ to `blur` fails four tests; forcing one slab per arc row fails the
corner test at a one-pixel blur. Layout tests cover command order, display
scaling, the blur bound and translation under alignment.

The `fill/elevated_card_stack` benchmark measures eight 520 × 72 cards with a
16-pixel blur on the 1280 × 800 surface at 2.49 ms on the pinned cores, with
the identical-code control `fill/rounded_card_stack` unchanged at 204 µs. The
first per-pixel formulation measured 11.9 ms and the coverage-mask form 3.49 ms.

## Limits

No `inset` shadows, spread distance or shadow lists. Blur is capped at 256
device pixels. A shadow with no interior columns — a pill or circle — convolves
every arc row across its full width; the review measured 2.17 s for a
2048-pixel pill at the maximum blur on the coverage-mask form, and the slab
form keeps that shape of cost.
