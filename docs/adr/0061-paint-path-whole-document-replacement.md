# 0061 — Whole-document replacement for the native paint path

Status: Proposed

Date: 2026-10-11

Driver: METIS-COMPONENT-001. Answers open question 2 of ADR 0056: how a
`DomElement` reaches a `TrackedDocument` and the native paint path.

## Context

The native render path edits the presentation document in place. `FrontendApp`
changes it through `restyle`, `set_text` and `set_attribute`
(`crates/metis-frontend/src/presentation.rs`). Each of those moves
`TrackedDocument`'s revision only when the content changes
(`crates/metis-frontend/src/document.rs:65`). Two derived values key on the
revision:

- The focusable order is recomputed when the revision changes
  (`crates/metis-frontend/src/presentation/cache.rs:37-38`). Focus is reconciled
  against it on every render (`presentation.rs:135-136`), and held as an authored
  id (`crates/metis-frontend/src/focus.rs:42`).
- The layout is recomputed when the revision or the viewport changes
  (`cache.rs:61-63`).

Repaint does not key on the revision. It is the damage between the new display
list and the painted one (`presentation.rs:147-149`), as ADR 0051 requires.

The three edits cannot insert element children, and cannot remove one child while
keeping its siblings. `set_text_content` removes every
child of an element and leaves one text node (`crates/metis-ui-lang/src/dom.rs:110-115`).

`DomElement`, `DomNode` and `DomDocument` derive `PartialEq`
(`crates/metis-ui-lang/src/dom.rs:7,52,74`), so two whole documents compare
directly. `DomDocument::new` validates nothing (`dom.rs:83-85`).

The render path's documented contract is that a render of unchanged state
requests no memory beyond the new display list's command storage
(`presentation.rs:100-102`).

Recorded baseline (ADR 0051, revised 2026-09-24): a keystroke repaint of the
authored form takes 0.40 ms at 800×600 and 0.49 ms at 1600×1200 on a pinned
core, against 2.8 ms and 8.2 ms before damage limiting. These are the repaint
costs the new path must not regress. They do not measure this decision. The
frame budget is 60 Hz, 16.7 ms (1/60 s; ADR 0043:80 names the budget).

## Decision

A component's view is applied to the native paint document as a whole.

- `TrackedDocument` gains `replace(document: DomDocument) -> Edit`. It returns
  `Unchanged` and keeps the revision when the new document equals the current
  one, and returns `Changed` and advances the revision once otherwise. The three
  existing edits stay. No insert or remove edit is added.
- The candidate is laid out and its semantic projection is built before it is
  stored. A candidate that either refuses leaves the previous document in place.
  A candidate whose focus order is empty is refused too: `reconcile` already
  errors on that state (`focus.rs:70`), and the order is built from the document
  alone (`cache.rs:39`), so this check needs no viewport.
- Element identity is the authored `id`. Ids are never derived from tree
  position.
- A view hides a focusable control with the `hidden` or `aria-hidden` attribute,
  on the control or on an ancestor. Style alone does not hide it: `display: none`
  leaves the control in the focus order (`semantics.rs:127-130,162-163`;
  `focus.rs:157-161`). The command menu sets `aria-hidden` on its container, and
  its descendants inherit the hidden state (`presentation.rs:226`;
  `semantics.rs:129-130`).
- A focused control that is removed or hidden loses focus to `reconcile`
  (`focus.rs:61-70`), in this order: the `popover-anchor` of its nearest
  enclosing popover, if that anchor is focusable; `label-patient`
  (`INITIAL_FOCUS`, `focus.rs:17`); the first focusable control; otherwise a
  typed error. A removed control has no ancestors in the new tree, so the anchor
  step applies only to a hidden one (`focus.rs:173-177`).
- Full replacement is the adopted path. The measurement below tests it, and can
  overturn it.
- This decision covers the native paint path. The list projection (ADR 0056,
  open question 3) and the browser host (open question 4) are not decided here.

## Rejected alternatives

- Child insert and remove edits. Layout is keyed by the revision and the viewport
  (`cache.rs:61-63`), so any revision change recomputes the display list. Child-level edits
  therefore save only building the view and comparing it. Neither cost is measured
  yet, so this rejection is analytical. The edit set would also have to agree
  with the view's tree at every step.
- A keyed diff that derives those edits. It has the same cost structure as an
  edit set, plus a diff algorithm and an id rule that this decision already
  states.
- Unconditional replacement, which advances the revision on every update. It
  invalidates the focusable order and the layout (`cache.rs:38,61`) even when the
  view is unchanged. The equality check is one comparison of the tree.
- Painting a `DomElement` directly, without a `DomDocument`. It bypasses
  `RenderCache`, which holds the focusable order and the revision-keyed layout
  (`cache.rs:37-39,56-63`). It would also skip the focus reconcile that runs
  against that order (`presentation.rs:135-136`).

## Consequences

- Allocation: each update builds the view as an owned tree. That is new cost
  against the render contract of requesting no memory for unchanged state
  (`presentation.rs:100-102`). The measurement below counts allocations per update.
- Comparison: each update compares the new tree with the current one, which is
  linear in the node count. The clinical form's node count is not measured yet.
- Layout: a changed document runs the layout pass that the revision already
  triggers. An unchanged document runs none.
- Limits: a programmatic document has the same depth and node limits as a parsed
  one, and the same relevant content-byte limits (`crates/metis-ui-lang/src/layout/geometry.rs:63`). The
  candidate check enforces them before the document is stored.
- `TrackedDocument` gains one operation and its tests. An equal replacement leaves
  the revision unchanged. A different replacement advances it exactly once. A
  refused candidate leaves the previous document in place. A replacement keeps
  focus on a focusable control only when that control keeps its authored id and
  is neither hidden nor removed.
- `FrontendApp`'s edit path stays valid. Moving it to replacement is the component
  slice of ADR 0056.

## Measurement

The adopted path is tested by a measurement, which is part of the acceptance
criteria.

- Extend `frame.rs` with a case that builds the clinical form's view, applies it
  with `replace`, and repaints, at both surfaces the benchmark already uses.
- Record allocations per update and wall time on the controlled host, with
  pinned inputs, `black_box`, and a median with a confidence interval, under the
  repository's benchmark methodology.
- Compare the median and its confidence interval with 16.7 ms, and the repaint
  with the 0.40 ms and 0.49 ms baseline.

## Overturning evidence

- The median and confidence interval of the replacement case exceed 16.7 ms, and
  profiling attributes the excess to building or comparing the view, not to
  layout or rasterization. Incremental edits are then reconsidered in a follow-up
  ADR.
- Allocations per update grow with the node count, and storage reuse cannot bring
  them within the render contract.
- A keystroke update in which building and comparing the view costs more than the
  layout pass that the revision change triggers anyway.
- A focus regression that the hiding rule and the fallback order cannot prevent.

## Open questions

1. The clinical form's node count, its per-update view build cost, and its
   allocation count. None is measured yet.
2. Whether the browser host uses the same operation (ADR 0056, open question 4).
3. Whether `FrontendApp`'s edits migrate to replacement or stay. That is a
   decision for the component slice under ADR 0056.
