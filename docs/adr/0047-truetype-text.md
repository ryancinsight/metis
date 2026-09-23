# 0047 — Antialiased TrueType text in the software renderer

Status: Accepted

Date: 2026-09-22

Driver: [METIS-TYPOGRAPHY-TRUETYPE-001](https://github.com/ryancinsight/metis/pull/372).

## Context

The software renderer drew text from an 8 × 16 monospace bitmap, scaled by a
whole number `max(1, font-size / 14)` times the display scale. Every size from
8 to 27 pixels rendered identically, so headings could not be larger than body
text without doubling, and the glyphs had neither proportional widths nor
antialiasing. After rounded surfaces and shadows, the text was the least
finished part of every screen.

No crate in the stack parses fonts or rasterizes outlines, and the pure-Rust
preference rules out binding a C rasterizer.

## Decision

`metis-platform` gains a `typeface` module that parses TrueType fonts and
draws antialiased text; the bitmap font is deleted.

- The parser reads `head`, `hhea`, `maxp`, `hmtx`, `loca`, `glyf` and a
  format 4 `cmap` through a bounds-checked reader, so a malformed font is a
  typed `TypefaceError`, never a panic. Composite glyphs place components
  through their 2 × 2 transforms, with depth bounded at 8 and at most 64
  placements per glyph so a hostile font cannot demand exponential work.
- Outlines flatten quadratics into chords within 1/256 pixel — the chord
  error of a quadratic spanning parameter length `h` is `|p₀ − 2p₁ + p₂|·h²/4`
  — and at most 2¹⁸ segments per outline; past that bound no further curve is
  walked. A glyph's coverage may span at most four million pixels.
- A single-component outline is rasterized by exact area accumulation: every
  segment is split at pixel row and column boundaries and each piece's signed
  area accumulated, so a prefix sum over each row gives the exact area the
  outline covers in every pixel. Accumulation adds overlapping areas, so it is
  exact only without overlapping contours; a test over every simple glyph of
  both faces proves the winding number never exceeds one there.
- An outline placed from several components — an `Å` whose ring overlaps its
  `A` — is rasterized by the nonzero winding rule on 64 rows per pixel with
  exact horizontal coverage, so overlaps count once. Its error is half a row
  per unit the coverage varies down the pixel.
- Glyphs sit at fractional pen positions and composite by coverage. A run's
  layout box is its advance, as in CSS: ink may overhang it by the glyphs'
  side bearings ('j' left of its pen, 'q' and combining marks past their
  advance), and drawing continues while the face's leftmost extent can still
  reach the surface.
- Atkinson Hyperlegible Regular and Bold, designed by the Braille Institute of
  America for legibility, are embedded under the SIL Open Font License 1.1.
  Static faces keep weight a choice of face rather than variable-font deltas.
- `TextStyle` carries a validated `TextSize` in device pixels per em; layout
  multiplies the authored CSS size by the display scale, measures runs by the
  sum of their `hmtx` advances and the `hhea` line height, and rounds extents
  up so a box holds what it measures. `DisplayCommand::DrawText` carries the
  style.

Hinting instructions and `GPOS` kerning are not interpreted; text is unhinted
and unkerned.

## Alternatives

A larger or multi-size bitmap font was rejected: it keeps whole-pixel scaling
and monospace advances, which are what made headings and labels look the same.

Binding a C rasterizer (FreeType) was rejected by the pure-Rust preference: its
code is invisible to the lint, test and Miri gates and brings a native build.

Inter, a variable font, was the other candidate; its bold weight needs `gvar`
deltas and the file is eight times larger.

## Verification

- Glyph ids, advances and outline areas for nine characters in each face,
  including the composite `é`, match fontTools 4.61.1; rasterized coverage
  totals match the analytic outline areas within the flattening bound
  (tolerance times the flattened perimeter) at three sizes and three
  fractional origins, and the darkness of drawn text equals those areas
  within that bound plus half a level per inked pixel.
- The composite placements `Å` and `Ç` match a point-sampled winding
  reference on the same sample rows within half a sample column per edge
  crossing; area accumulation, which the review measured up to 0.31 of a
  pixel too dark there, fails it.
- A font assembled byte by byte places components through a 2 × 2 shear, an
  x-and-y scale with a scaled offset, a single scale with byte arguments and a
  nested composite at the hand-derived bounds and |det|-scaled areas, and
  rejects a component past the glyph count and a cyclic composite.
- Ink stays within the declared glyph bounds for runs with left and right
  overhangs and a combining mark, and a glyph overhanging the right edge
  paints its visible part.
- Squares, random triangles in both windings and a counter-wound hole cover
  exactly their areas; flattened chords stay within the tolerance of densely
  sampled curves.
- Every glyph of both faces decodes; truncated and randomly mutated copies of
  the regular face fail with typed errors and never panic.
- Text stays inside its line box, and clipped runs paint exactly the visible
  part.
- Mutations that drop implied midpoints or shift a `cmap` delta fail the area
  and mapping tests; swapping the 2 × 2 matrix entries fails the placement
  test, and routing composites through accumulation fails the union test.

An independent review of the first delivered form found the overlap
overcounting, ink outside the advance box, work continuing past the segment
bound and right-edge glyphs dropped, and verified cmap, advances and outlines
for every glyph against fontTools; all findings are fixed above.

## Limits

No hinting, kerning, ligatures, bidirectional text or shaping; characters the
faces lack draw the missing-glyph box. Glyphs are rasterized on every draw; a
glyph cache is a separate measured increment. The nonzero rule tests every
segment on every sample row, so a composite at the largest size takes up to
78 ms against 6 ms for accumulation; an active-edge sweep would bound that
work, and until then composites at display sizes stay the costly case.
