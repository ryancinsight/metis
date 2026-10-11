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
(`crates/metis-frontend/src/document.rs:65`). Three derived values key on the
revision:

- The focusable order is recomputed when the revision changes
  (`crates/metis-frontend/src/presentation/cache.rs:37-38`). Focus is reconciled
  against it on every render (`presentation.rs:135-136`). Focus is held by
  authored id (`crates/metis-frontend/src/focus.rs:76`).
- The layout is recomputed when the revision or the viewport changes
  (`cache.rs:61-63`).
- Repaint is limited to the damaged region (ADR 0051, Accepted).

The three edits cannot express child insertion or removal. `set_text_content`
replaces an element's children with one text node
(`crates/metis-ui-lang/src/dom.rs:110-115`).

`DomElement`, `DomNode` and `DomDocument` derive `PartialEq`
(`crates/metis-ui-lang/src/dom.rs:7,52,74`), so two whole documents compare
directly.

The frame benchmark (`crates/metis-frontend/benches/frame.rs`) records a
keystroke repaint of the authored form at about 2.8 ms at 800×600 and about
8 ms at 200% on 1600×1200 (ADR 0051). Those figures date from 2026-09-24 and
have not been re-measured for this decision. The frame budget is 60 Hz, 16.7 ms
(ADR 0043:80).

## Decision

A component's view is applied to the native paint document as a whole.

- `TrackedDocument` gains one operation, `replace(document: DomDocument) -> Edit`.
  It returns `Unchanged` and keeps the revision when the new document equals the
  current one. It returns `Changed` and advances the revision once otherwise.
  The three existing edits stay as they are. No insert or remove edit is added.
- Element identity is the authored `id`. A view keeps each focusable `id` stable
  across updates, so focus survives an update that changes text or style. Ids
  are never derived from tree position.
- Full replacement is the default. Incremental edits are reconsidered only under
  the overturning evidence below.
- This decision covers the native paint path. The list projection (ADR 0056,
  open question 3) and the browser host (open question 4) are not decided here.

## Rejected alternatives

- Child insert and remove edits. Layout is keyed by the whole-document revision
  (`cache.rs:61-63`), so any edit recomputes the full display list. That costs
  the same as a replacement whose content changed. Child-level edits would save
  only building the view and comparing it, and would add an edit set that must
  agree with the view's tree at every step. `set_text_content` already clears
  children (`dom.rs:110-115`), which shows how such a set goes wrong.
- A keyed diff of two trees that derives those edits. It has the same cost
  structure as an edit set, plus a diff algorithm and an id rule that this
  decision already states.
- Unconditional replacement, which advances the revision on every update. It
  invalidates the focusable order and the layout (`cache.rs:38,61`) even when
  the view is unchanged. The equality check is one comparison of the tree.
- Painting a `DomElement` directly, without a `DomDocument`. It skips
  `SemanticTree::from_document`, which validates controls and identity before
  focus is reconciled (`cache.rs:37-39`, `presentation.rs:135-136`).

## Consequences

- One operation is added to `TrackedDocument`. Its tests show that an equal
  replacement leaves the revision unchanged, that a different replacement
  advances it exactly once, and that a replacement with the same authored ids
  keeps focus.
- Each update builds the view, compares it with the current document, and on a
  change runs the layout pass that the revision already triggers. The comparison
  is linear in the node count. The clinical form's node count is not measured
  yet.
- Size limits are the existing ones. A programmatic document has the same depth,
  node and content-byte limits as a parsed one
  (`crates/metis-ui-lang/src/layout/geometry.rs:63`).
- `FrontendApp`'s current edit path stays valid. Moving it to replacement belongs
  to the component slice in ADR 0056.
- The frame benchmark gains a replacement case. Its results go in the pull
  request that adds it, not in this ADR.

## Measurement

The decision depends on a measurement, so the benchmark is an acceptance
criterion.

- Extend `frame.rs` with a case that builds the clinical form's view, applies it
  with `replace`, and repaints.
- Run it on the controlled host with pinned inputs, `black_box`, and a median
  with a confidence interval, under the repository's benchmark methodology.
- Compare the median and its confidence interval with 16.7 ms.

## Overturning evidence

- The median and confidence interval of the replacement case exceed 16.7 ms, and
  profiling attributes the excess to building the view or comparing documents,
  not to layout or rasterization. Incremental edits are then reconsidered in a
  follow-up ADR.
- A keystroke-sized update in which building and comparing the view costs more
  than the layout pass that the revision change triggers anyway.
- A focus or accessibility regression that stable authored ids cannot prevent.

## Open questions

1. The clinical form's node count and its per-update view build cost. Neither is
   measured yet. The benchmark above is the first measurement.
2. Whether the browser host uses the same operation (ADR 0056, open question 4).
3. Whether `FrontendApp`'s edits migrate to replacement or stay as they are. That
   is a decision for the component slice under ADR 0056.
