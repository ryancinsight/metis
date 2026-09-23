# 0045 — Flex alignment by post-layout redistribution

Status: Accepted

Date: 2026-09-22

Driver: [METIS-LAYOUT-ALIGN-001](../../backlog.md#METIS-LAYOUT-ALIGN-001).

Revision: 2026-09-23 — [METIS-FORM-LABELS-001](https://github.com/ryancinsight/metis/pull/374)
sizes the children of a column whose `align-items` is not `stretch` to their
content, as CSS flex layout does. Every automatic-width element had filled its
available width, so a column's children never left cross-axis free space and
`center`, `flex-start` and `flex-end` moved nothing; a centred button label
stayed at the left edge. Such a child now takes its max-content width — text
advances, a row's children and gaps, a column's widest child, plus padding and
borders — capped at the available width, and redistribution places it.
`stretch`, the default, and row containers are unchanged.

## Context

[ADR 0013](0013-strict-style-contract.md) rejects `justify-content` and
`align-items` for want of layout semantics. They are the last two declarations
it rejects: `border-radius` ([ADR 0043](0043-rounded-rectangle-paint.md)),
`font-weight` and the minimum sizes have since been admitted as the renderer
gained the semantics for each.

Alignment differs from everything admitted so far. A radius, a weight or a
minimum changes one box in place. Alignment redistributes the free space a
container leaves among its children, so it cannot be decided until every child
has been measured and the container's own extent is known.

The layout pass places each child as it measures it, and measuring a child
emits that child's paint commands. By the time free space is known, the
children have already painted at start-aligned positions.

## Decision

Alignment is applied after child layout by translating each child's emitted
commands.

- Child layout records, per child, the half-open range of commands it emitted
  and its extents on both axes. The range is contiguous because a child and its
  descendants paint before the next sibling starts.
- After the container's own rectangle is final — its height is known only then,
  since an automatic height is derived from the children — the content box
  gives the main and cross extents. Free space is the content extent minus what
  the children occupied, gaps included, floored at zero.
- Each keyword maps to a per-child offset along its axis, and the child's
  command range is translated by it. Translation moves every command kind a
  child can emit; `ImagePlacement` gains a crate-internal translation because
  its destination is private.
- `FlexStart` and `Stretch` produce zero offsets, so a container with no free
  space and a document that declares nothing lay out exactly as before. That is
  what keeps every existing capture byte-identical.

Translating painted output rather than deferring the paint keeps one layout
pass and one painter order. The alternative — measuring children without
emitting, then painting them a second time — would split every element's paint
into a measure path and a paint path that can disagree, which is the
duplicated-variant defect the renderer is organized to avoid.

Scope is the capability the style model already declares: four main-axis
distributions, four cross-axis alignments, single-line only. No wrapping, no
`space-around`/`space-evenly`, no per-item `align-self`; each would be its own
declaration with its own evidence.

## Alternatives

A two-pass measure-then-place layout was rejected for the duplication above.

Deferring children's commands into a per-child buffer and splicing them at
final positions was rejected: it allocates per child per frame on the layout
path and reorders nothing that translation does not already handle.

Resolving the container's main extent before child layout was rejected as
incomplete — it works when the main size is declared but not when it is
automatic, and the automatic case is where the two paths would drift.

## Consequences

Authored surfaces can centre and distribute content on the software path. Free
space is zero for every existing document, so captures and pixel oracles hold.
Layout gains one bounded allocation per element with children, holding one
record per child.
