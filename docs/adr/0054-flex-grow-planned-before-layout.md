# 0054 — Flex growth planned before layout

Status: Accepted

Date: 2026-09-27

Driver: `flex-grow`, the remaining single-line flex distribution after
[ADR 0045](0045-flex-alignment-redistribution.md) and its 2026-09-27 revision;
this change's pull request.

## Context

Layout paints each child as it measures it, and places the child with the size
it resolves at that moment. [ADR 0045](0045-flex-alignment-redistribution.md)
handles alignment after the fact by translating the commands a child has
already emitted. Translation moves a child. Growth resizes one: a grown child's
background, border and element rectangle change, and so does the free space its
own children align within. A translation cannot deliver that.

Before this change a row gave every automatic-width child all the width its
earlier siblings left, so the first such child filled the row and later
siblings got nothing. A label beside a text field could not leave the field the
remaining width.

## Decision

A container plans its children's main-axis sizes before laying them out, and
only when a child in flow declares a positive `flex-grow`.

- **Base sizes come from the intrinsic measures.**
  - In a row, a child's base size is the border-box width layout gives it with
    content sizing. That is its declared width, or else its max-content width,
    and the declared minimum applies. `border_box_width` computes it, and it is
    the same function `element()` uses to size every element.
  - In a column, the base size is `max_content_height`. That follows layout's
    height rule: a text run is one line tall, a column stacks its children and
    gaps, and a row takes its tallest child. Padding, borders, declared heights
    and minimums apply as layout applies them.
  - Heights do not depend on widths, because text runs do not wrap. So one
    linear measure suffices, and nested growing containers are never laid out
    twice.
- **Free space** is the container's content-box main extent minus the bases and
  gaps, floored at zero.
  - A row's extent is its resolved content width.
  - A column's extent is its declared height, or its content height, raised to
    its declared minimum. `min-height` is therefore enough for a body to grow
    between a header and a footer.
- **Shares follow the weights through floored running totals.** Each share is
  the difference of two cumulative floors, so the shares sum to exactly the
  space distributed. That space is all of the free space when the factors sum
  to at least one, and their fraction of it when they sum to less (CSS Flexbox 1
  §9.7).
- **Each child is laid out once, at its planned size.**
  - In a row, every child in flow is fixed at its base plus its share, so a
    non-growing sibling is content-sized rather than filling the row.
  - In a column, only growing children are fixed. The rest keep their natural
    height, which is their base.
- **`flex-grow` is held in thousandths** (`FlexGrow`), capped at 1000. Style
  therefore stays free of floating point and the share arithmetic stays exact.
  An out-of-range or malformed factor is a typed style error, like every other
  out-of-range value.

The subset stays single-line: no `flex-shrink`, `flex-basis` or wrapping. An
overfull container shares nothing and keeps its bases.

## Alternatives

**Measuring by laying each child out into a scratch display list.** The popover
path uses this for one element. Here it would cost a full layout per measure,
and nested growing columns would multiply it at each level up to the 64-level
depth limit.

**Growing a painted child in place.** This would mean re-settling its reserved
box slots, re-running its alignment and shifting later siblings. That repeats
half of `element()` against state it has already emitted.

**Relaying out the whole child list after a first measuring pass.** This
doubles layout for every growing container and has the same nesting blow-up.

The plan-first design adds one intrinsic measure per child and only for
containers that grow.

## Consequences

Rows gain CSS content sizing wherever a child grows, and a field takes the width
its label leaves. Columns fill a declared height or minimum. A document that
declares no `flex-grow` is not planned at all, so its layout, captures and pixel
oracles are unchanged.

The height measure is a second statement of layout's height rule, and a second
statement can drift. The regression oracle
`measured_heights_equal_laid_out_heights` compares it with laid-out heights.
It runs over a corpus that covers text, gaps, margins, minimums, percentages,
hidden children, nesting and growing children, at two display scales. A
deliberately introduced disagreement, dropping column gaps from the measure,
fails that oracle.
