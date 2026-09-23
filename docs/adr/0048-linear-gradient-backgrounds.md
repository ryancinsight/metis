# 0048 — Linear gradient backgrounds

Status: Accepted

Date: 2026-09-23

Driver: [METIS-RASTER-GRADIENT-001](../../backlog.md#METIS-RASTER-GRADIENT-001).

## Context

With rounded fills ([ADR 0043](0043-rounded-rectangle-paint.md)), shadows
([ADR 0046](0046-gaussian-box-shadows.md)) and typography
([ADR 0047](0047-truetype-text.md)) in place, every surface of the software
renderer is still one flat color. A directional ramp is the usual way to make a
header or a primary control read as lit, and CSS expresses it as
`linear-gradient()`, which the strict style contract
([ADR 0013](0013-strict-style-contract.md)) rejects as an invalid `background`
value.

CSS Images Level 3 fixes the semantics:

- §3.1: an angle of 0 points up and angles increase clockwise; `to top`,
  `to right`, `to bottom` and `to left` are 0, 90, 180 and 270 degrees, and
  the default is `to bottom`. The gradient line passes through the center of
  the gradient box, and its length is `abs(W sin A) + abs(H cos A)`, so the
  perpendiculars through its ends touch two opposite corners.
- §3.4.2: before the first stop the color is the first stop's, after the last
  it is the last stop's, and between stops colors interpolate linearly in
  premultiplied RGBA.
- §3.4.3: a missing first position becomes 0% and a missing last one 100%; a
  position below an earlier one rises to the largest earlier position; each
  run of unpositioned stops spreads evenly between its positioned neighbours.

## Decision

`metis-platform` gains `LinearGradient`, validated from an angle in degrees
and between two and `MAX_GRADIENT_STOPS` (eight) stops, and
`fill_gradient(fb, rect, radius, &gradient)`. `metis-ui-lang` admits
`background-image: linear-gradient(...) | none` and the gradient form of
`background`, and emits `DisplayCommand::FillGradient` above the background
color.

- Construction resolves stop positions once by the §3.4.3 fixup and stores
  colors premultiplied, with each span's reciprocal, so painting a pixel is a
  projection, a short scan of at most eight stops and one interpolation. The
  first stop is held apart from the rest, so an empty gradient is not
  representable and the color lookup has no panicking branch.
- The four axis angles map to exact unit vectors. A vertical gradient is then
  one color per row, and fully covered runs fill as spans exactly as a solid
  color does.
- The rounded-shape scanline is generic over a `Paint`: fully covered runs go
  to `Paint::fill_run` and partially covered pixels scale `Paint::color_at` by
  coverage. A solid `Color` is one paint and a placed gradient another, so
  corners antialias identically for both and a gradient whose stops share one
  color paints exactly what `fill_rect` paints.
- The gradient box is the border box, which `background-origin: border-box`
  would give in CSS; the CSS default is the padding box. The subset has no
  `background-origin`, a border paints over the background, and one box for
  color, gradient and radius keeps the three coincident.
- Channels round half up by adding one half and truncating. `f64::round`
  compiles to a library call on the baseline x86-64 target; the conversion
  agrees with it except for a fractional part within one ulp below one half.
- The `background` shorthand sets one layer and resets the other (CSS
  Backgrounds 3 §2.10): a hex color clears the gradient and a gradient clears
  the color. `background-color` and `background-image` set their own layer.
- Corner keywords, other angle units, color hints, length positions and
  repeating, radial and conic gradients are `ErrorCode::InvalidCssStyle`
  rather than approximated, as ADR 0013 requires.

The demo form lights its header with a 135-degree ramp and its command
controls with a top-to-bottom ramp in both themes. The theme tests hold every
stop of both fills to a 4.5:1 contrast for white labels (WCAG 2.2, criterion
1.4.3).

## Alternatives

A color lookup table per placed gradient — quantizing the fraction to a
power-of-two table sized from the gradient length — would make the diagonal
path a table read per pixel. It was not taken on this evidence: the direct
evaluation costs about 6.5 ns per pixel above a solid fill, the demo paints
under 100,000 gradient pixels per frame, and a table trades exactness against
the closed-form tests for a cost the frame does not show. It remains the next
step if gradient area grows.

Interpolating straight rather than premultiplied colors was rejected: it is
what §3.4.2 rules out, and it darkens a fade to transparent (a transparent to
red ramp would pass through `(128, 0, 0, 128)` instead of
`(255, 0, 0, 128)`).

Painting the gradient only through a new rectangle routine, separate from the
rounded scanline, was rejected: it would duplicate the corner coverage the
shape routine already derives, and the two would drift.

## Verification

`crates/metis-platform/src/rasterizer/gradient_tests.rs`:

- the fraction at every sampled pixel center equals a direct transcription of
  §3.1 within 1e-12, over 48 angles from -30 to 322.5 degrees and three box
  shapes, and the four corners always span exactly `[0, 1]`;
- the keyword angles and angles outside `[0, 360)` point as specified;
- the §3.4.3 fixup on six stop lists, including clamped positions followed by
  an unpositioned run;
- premultiplied interpolation, end clamping and a coincident-stop hard edge;
- vertical and horizontal fills equal to the closed form per row and column;
- a single-color gradient equal to `fill_rect` bitwise for opaque and
  translucent colors, radii 0, 5 and 12, five angles and clipped rectangles;
- translucent gradients equal to compositing each pixel's color over the
  surface; empty, off-surface and very large boxes.

Replacing the gradient length with the box diagonal fails the placement,
keyword and vertical closed-form tests. `style_tests.rs` covers the grammar,
the shorthand reset and each rejection; `layout_tests.rs` the command order
and translation under alignment.

The `fill/gradient_card_stack` benchmark paints eight 520 × 72 cards with the
header ramp on the 1280 × 800 surface. Pinned to one P-core it measured 3.96 ms
with library rounding and 2.13–2.16 ms after this decision's rounding and span
reciprocals, with `fill/card_stack`, `fill/rounded_card_stack` and
`fill/elevated_card_stack` unchanged within 2% in the same runs.

## Limits

One gradient layer per box; no radial, conic or repeating gradients, color
hints or length positions. Colors are interpolated per pixel without dithering,
so a ramp between close colors over a long distance shows 8-bit steps.
