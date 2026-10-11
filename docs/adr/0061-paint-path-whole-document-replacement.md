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
keeping its siblings. `set_text_content` removes every child of an element and
leaves one text node (`crates/metis-ui-lang/src/dom.rs:113-114`).

`DomElement`, `DomNode` and `DomDocument` derive `PartialEq`
(`crates/metis-ui-lang/src/dom.rs:7,52,74`), so two whole documents compare
directly. `DomDocument::new` validates nothing (`dom.rs:83-85`).

The render path's documented contract is that a render of unchanged state
requests no memory beyond the new display list's command storage
(`presentation.rs:100-102`).

Recorded baseline (ADR 0051, revised 2026-09-24): pinned to one performance core,
with alternating before and after binaries, a keystroke repaint of the authored
form takes 0.40 ms at 800×600 and 0.49 ms at 1600×1200 (200% scale). Those are
the repaint costs the new path must not regress. The 2.8 ms and 8.2 ms figures
are the pre-change history. They do not measure this decision. ADR 0051's
figures predate ADRs 0053 and 0054 (dated 2026-09-25 and 2026-09-27), so the
baseline is re-measured in the same run. The frame budget is 60 Hz, 16.7 ms
(1/60 s; ADR 0043:80 names the budget).

## Decision

A component's view is applied to the native paint document as a whole.

- `TrackedDocument` gains `replace(document: DomDocument) -> Edit`. It stores the
  document and advances the revision once when the document differs from the
  stored one, and returns `Changed`. When the two are equal it returns `Unchanged`
  and keeps the revision. It never returns `Edit::Missing`, since replacement
  addresses no element id. The three existing edits stay. No insert or remove
  edit is added.
- A host that applies a component's view runs these steps in order:
  1. Compare the candidate with the stored document. An equal candidate ends
     here, with no layout and no focus work.
  2. Validate the candidate with the viewport the render uses
     (`presentation.rs:139-142`). Lay it out with `compute_layout`, and build its
     semantic projection and focus order. A candidate with an empty focus order is
     refused, since `reconcile` already errors on that state (`focus.rs:70`), and
     the order is built from the document alone (`cache.rs:39`).
  3. Store the candidate with `replace`, and install the validated layout and
     focus order in `RenderCache` under the new revision, so a changed document
     lays out once.
  A refused candidate leaves the previous document and cache in place.
- The view is built only after a message has changed the component. A render with
  no pending change reuses the stored document, so it allocates no view tree and
  keeps the render contract (`presentation.rs:100-102`).
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
  (`cache.rs:61-63`). The revision advances in both designs, so layout and the
  focus order are recomputed either way. Child-level edits would save only the
  owned candidate tree, the view build and the tree comparison. None of these is
  measured yet, so this rejection is analytical. The edit set would also have to
  agree with the view's tree at every step.
- A keyed diff that derives those edits. It has the same cost structure as an
  edit set, plus a diff algorithm and an id rule that this decision already
  states.
- Unconditional replacement, which advances the revision on every update even
  when the view is unchanged. It invalidates the focusable order and the layout
  (`cache.rs:38,61`). The equality check in step 1 prevents this.
- Painting a `DomElement` directly, without a `DomDocument`. It bypasses
  `RenderCache`, which holds the focusable order and the revision-keyed layout
  (`cache.rs:37-39,56-63`). It would also skip the focus reconcile that runs
  against that order (`presentation.rs:135-136`).

## Consequences

- Allocation: a changed update builds the view as an owned tree, which is new
  cost. A render with no pending change builds none. The measurement counts
  allocations for both.
- Comparison: a changed update compares the new tree with the current one, which
  is linear in the node count. The clinical form's node count is not measured yet.
- Layout: a changed document lays out once, during validation, and `RenderCache`
  installs that layout. An unchanged document runs no layout.
- Limits: a programmatic document has the same depth and node limits as a parsed
  one, and the relevant content-byte limits
  (`crates/metis-ui-lang/src/layout/geometry.rs:63`). Validation enforces them,
  since it runs `compute_layout` before the document is stored.
- `TrackedDocument` gains `replace` and its equality check, and `RenderCache`
  gains an install operation for a validated layout and focus order. Each has
  tests. An equal replacement leaves the revision unchanged. A different
  replacement advances it exactly once. A refused candidate leaves the previous
  document and cache in place. A replacement keeps focus on a focusable control
  only when that control keeps its authored id and is neither hidden nor removed.
- `FrontendApp`'s current edit path stays valid. Whether its edits migrate to
  replacement is open question 3.

## Measurement

The decision is accepted on the analysis above. The measurement tests it, and its
results decide whether the overturning items hold.

- Extend `frame.rs` with separate cases, at both surfaces the benchmark already
  uses: (a) building and comparing the clinical form's view after a message;
  (b) validating the changed candidate, meaning layout and focus order; (c)
  replace and repaint, end to end; (d) a render with no pending change.
- Record wall time and allocations per case on the controlled host, with pinned
  inputs, `black_box`, and a median with a confidence interval, under the
  repository's benchmark methodology.
- Re-measure the ADR 0051 keystroke baseline in the same run, with alternating
  binaries. Compare case (c) and case (d) against that pair.

## Overturning evidence

- The median of case (c) exceeds 16.7 ms. ADR 0056 then rules this path out. The
  case breakdown decides the follow-up: when case (a) dominates, incremental edits
  are reconsidered in a follow-up ADR; when case (b) dominates, the follow-up
  targets layout.
- The keystroke repaint in case (c) is slower than the same-run baseline by more
  than the confidence interval of that difference.
- Case (d) allocates beyond the display list's command storage. The dirty rule
  then does not hold for unchanged renders, and the decision is revised.
- A focus regression that the hiding rule and the fallback order cannot prevent.

## Open questions

1. The clinical form's node count, its per-update view build cost (case (a)), and
   its allocation count. None is measured yet.
2. Whether the browser host uses the same operation (ADR 0056, open question 4).
3. Whether `FrontendApp`'s edits migrate to replacement or stay. That is a
   decision for the component slice under ADR 0056.
