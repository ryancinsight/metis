# 0043 — Rounded rectangle paint semantics

Status: Accepted

Date: 2026-09-22

Driver: `METIS-RASTER-ROUND-001`.

## Context

[ADR 0013](0013-strict-style-contract.md) rejects `border-radius` because the
software renderer has no paint semantics for it. The rejection was correct: a
silently dropped declaration is worse than a typed error. It is not, however, a
destination. Every surface the software renderer paints is a hard-cornered
rectangle, and an application that wants a rounded card must either move to the
browser host or abandon the style.

The renderer already owns clipped span filling for square rectangles. A rounded
rectangle differs from it only at the four corner arcs, and a border differs
from a fill only by an inset inner shape.

## Decision

`metis-platform` gains a validating `CornerRadius` and paints rounded
rectangles through one scanline routine.

- `CornerRadius::clamped` bounds a requested radius to half the shorter side,
  so opposite corners cannot overlap and the straight edge between them keeps a
  nonnegative extent. A nonpositive request or an empty rectangle is
  `CornerRadius::SQUARE`.
- `fill_rect` and `draw_rect_outline` take the radius as a parameter rather
  than gaining rounded siblings. `CornerRadius::SQUARE` takes the existing span
  path unchanged, and a test asserts square output stays bit-identical.
- One routine composites the area inside an outer shape and outside an optional
  inner shape. A fill supplies no inner shape; a border supplies the outer
  shape inset by the border width, so the border follows the same arc as the
  fill it encloses.
- Horizontal coverage is exact within each sampled row; the vertical direction
  is integrated over sixteen subsamples. Straight edges land on integer
  coordinates, so only the corner arcs carry fractional coverage. Coverage
  scales the source alpha, so a fully covered pixel composites exactly as an
  unrounded fill of the same color.
- Three column classes bound the work per row: columns every subsample covers
  completely are filled as a span, columns the inner shape covers completely
  contribute nothing and are skipped, and only the transition bands take the
  per-pixel path. Per-pixel cost is therefore proportional to the radius, not
  to the width of the shape.

Layout and the style contract were split into a follow-up increment so the
primitive could land free of a contended region and stay independently
verifiable. `METIS-RASTER-ROUND-002`
completed that: `DisplayCommand::FillRect` and `DrawBorder` carry the radius,
layout clamps the authored value against the final rectangle, and
[ADR 0013](0013-strict-style-contract.md) admits the declaration.

## Alternatives

Adding `FillRoundedRect` and `DrawRoundedBorder` commands beside the existing
pair was rejected: it forks one concept into two command families and two paint
paths that would drift, and a square radius is the same shape with a zero arc.

Painting unantialiased rounded corners was rejected because the stair-stepped
result is worse than the square corner it replaces at the sizes the renderer
targets.

Supersampling the whole shape was rejected because it costs work proportional
to the area for a result that differs from exact horizontal coverage only at
the arcs.

Shipping the primitive together with the layout and style wiring was rejected
for this increment because the display-command file is under another agent's
live edits; the lease protocol takes the disjoint remainder first.

## Consequences

The renderer can paint rounded cards, buttons and badges. Square output is
unchanged, so existing captures and pixel oracles hold. A rounded card stack
costs about 12.7 times its square counterpart and remains about seven times
faster than the square path before the span-fill change, at roughly one percent
of a 60 Hz frame budget.

An authored stylesheet reaches the capability since the follow-up increment.
The committed visual demonstration still waits for a surface that adopts it:
the `image` example's artifact is compared against a golden snapshot, and
growing that snapshot to hold a demonstration is not worth the tracked bytes
when the form captures will show it once the authored markup rounds.
