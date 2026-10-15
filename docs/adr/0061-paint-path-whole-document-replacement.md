# 0061 — Whole-document replacement for the native paint path

Status: Proposed

Date: 2026-10-12

Driver: METIS-COMPONENT-001. Answers open question 2 of ADR 0056.

## Context

`FrontendApp` edits its presentation document in place through `restyle`, `set_text` and `set_attribute` (`crates/metis-frontend/src/presentation.rs`). Each edit advances `TrackedDocument`'s revision only on change (`crates/metis-frontend/src/document.rs:65`). Two values key on the revision: the focusable order (`crates/metis-frontend/src/presentation/cache.rs:37-38`, reconciled at `presentation.rs:135-136`, held as an authored id at `focus.rs:42`), and the layout, keyed by revision and viewport (`cache.rs:61-63`). Repaint is decided by display-list damage (`presentation.rs:147-149`; `damage_since` at `crates/metis-ui-lang/src/layout/damage.rs:72-92`; `Damage` at `crates/metis-platform/src/framebuffer/damage.rs:7`).

The three edits cannot insert element children, or remove one child while keeping its siblings: `set_text_content` removes every child (`crates/metis-ui-lang/src/dom.rs:113-114`). `DomElement`, `DomNode` and `DomDocument` derive `PartialEq` (`dom.rs:7,52,74`). A render of unchanged state requests no memory beyond command storage (`presentation.rs:100-102`; `crates/metis-frontend/tests/render_allocations.rs:114-118`). `apply_theme` writes the palette once and relies on the document never being replaced (`crates/metis-frontend/src/presentation/theme.rs:15-19`).

Baseline (ADR 0051, revised 2026-09-24), measured on a pinned core with alternating before and after binaries: the keystroke case of `crates/metis-frontend/benches/frame.rs:36-49` takes 0.40 ms at 800×600 and 0.49 ms at 1600×1200 (200% scale). It times `set_inputs`, which runs the whole `render()` path (`app.rs:212-227`). The 2.8 and 8.2 ms figures are history. ADRs 0052–0054 (dated 2026-09-24 to 2026-09-27) and commit `900064e` (2026-10-01) changed rendering after the baseline, so it is re-measured in the same run. ADR 0043:78-80 uses one percent of the 60 Hz frame budget as its scale for a negligible cost; 16.7 ms is 1/60 s.

## Decision

1. **Ownership.** A document has one owner. A replacement-owned document is the complete view (palette, status badge, command surface and form text), and nothing edits it in place afterward. Migrating the clinical form from the in-place path is a later slice, as ADR 0056 places it. Until then the in-place path is unchanged, and `apply_theme`'s once-only invariant holds on that path.
2. **API.** `TrackedDocument::compare(&self, candidate: DomDocument) -> Comparison` makes one equality check. `Comparison` is `Unchanged` or `Changed(Pending)`. `Pending` holds the candidate and the revision it was compared at, is not `Clone`, and derefs to `DomDocument`. `RenderCache::admit(&mut self, document: &mut TrackedDocument, pending: Pending, viewport) -> Result<()>` runs the steps in order: it refuses a `Pending` whose revision no longer matches the document, with a typed error; it computes the layout with `compute_layout` and the focus order from the candidate alone (`cache.rs:39`), refusing an empty focus order (`focus.rs:70`); it then replaces the document, advancing the revision once (`replace` is crate-private and takes the `Pending`); and it stores the computed layout and focus order under the new revision. The caller never supplies the layout or the focus order, so the cache's rule that a stale part is never served (`cache.rs:15-17`) holds by construction. A refused candidate leaves the document and the cache unchanged. `replace` never produces `Edit::Missing`, because replacement addresses no element id. No insert or remove edit is added.
3. **Host steps.** A host applying a component's view calls `compare`. `Unchanged` ends there, with one comparison and no layout or focus work. `Changed` goes to `admit`.
4. **Applied messages.** The host builds a view only after it applies a message. A render with no applied message builds none (case N).
5. **Focus.** The authored `id` is the identity. `hidden` hides when present. `aria-hidden` hides when `true`, `1` or empty (`semantics.rs:127-128`; `boolean_attribute` at `semantics.rs:352-367`). Either applies to the control's descendants too (`semantics.rs:129-133`). Style alone does not hide (`semantics.rs:162-163`; `focus.rs:157-161`). `disabled` removes a control from the order (`semantics.rs:134-135,162-163`; `focus.rs:161-162`). Reconcile (`focus.rs:61-70`): a control still in the tree but out of the order moves to its nearest popover's `popover-anchor` if that anchor is in the order (`focus.rs:62-66,173-177`). Otherwise, or when the control was removed, focus goes to `label-patient` if it is in the order (`INITIAL_FOCUS`, `focus.rs:17,68`), then the first in the order (`focus.rs:69`), then to a typed error (`focus.rs:70`).
6. **Scope.** The native paint path only. The list projection (ADR 0056 open question 3) and the browser host (ADR 0056 open question 4) are not decided here.

## Rejected alternatives

- Child insert and remove edits. Layout keys on the revision (`cache.rs:61-63`), which advances in both designs, so layout and focus order recompute either way. Child edits would save only the candidate tree, the view build and the comparison. Those savings are analytical; none is measured.
- A keyed diff that derives those edits. It has the same structure, plus a diff algorithm and an id rule.
- Unconditional replacement. It advances the revision on unchanged views and invalidates layout and focus order (`cache.rs:38,61`). `compare` prevents it.
- Painting a `DomElement` directly. It bypasses `RenderCache` (`cache.rs:37-39,56-63`) and focus reconcile (`presentation.rs:135-136`).

## Consequences

- Allocation: a changed update builds an owned view tree, which is new cost. Case N builds none.
- Comparison: one per update, linear in node count (analytical).
- Layout: a changed document lays out once, inside `admit`. An unchanged document lays out nothing.
- Limits: a programmatic document has the same depth, node and relevant content-byte limits (`crates/metis-ui-lang/src/layout/geometry.rs:63`). `compute_layout` runs before storage, so `admit` enforces them.
- Tests: a changed candidate advances the revision once; `Unchanged` never reaches `admit`; a stale `Pending` is refused; a refused candidate leaves the document and cache unchanged; the focus tests below pass.
- The in-place path stays valid until the migration slice.

## Measurement

The decision is proposed on this analysis, and the rejection rules below decide it. Methodology, stated here in full: wall time and allocations per case; pinned inputs; `black_box` on inputs and outputs; a median with a 95% confidence interval; the sample size of `frame.rs` (`sample_size(20)`, `frame.rs:89-92`); the baseline re-measured in the same run with alternating binaries. Cases run at 800×600 and 1600×1200 (200%).

- K-old, K-new: end-to-end keystroke on the existing and replacement paths. Span: message applied to rendered framebuffer, as `frame.rs` times `set_inputs`. Presentation is outside the span, because the harness presents nothing.
- T-old, T-new: end-to-end theme change, same span. T-old is reported as the reference for T-new.
- A: from message applied to admission complete (update, view build, comparison, validation and replacement). P: paint of the display list. A and P are sub-spans of the end-to-end cases. The remainder (focus reconcile, focus ring, damage, painted store) is reported, not gated.
- N: a render with no applied message, replacement path.
- Focus tests, not timed (`crates/metis-frontend/src/focus_tests.rs`): after replacement, focus stays on a focusable control that keeps its id; a control hidden by attribute moves to its anchor; a removed or disabled control moves per the fallback order.

Preconditions, asserted before timing: K-new damage is `Damage::Region`, and T-new damage is `Damage::Full`. No existing test asserts the theme damage. If T-new is not `Full`, the theme case is revised and the measurement repeated.

## Rejection rules

The proposal is rejected when any rule holds.

- R1 (ADR 0056:128-129). The median of P for T-new exceeds 16.7 ms. ADR 0056's rule is worded "through any path", so it applies to this path because P is the form's paint.
- R2 (ADR 0056:126-127). The median of A for K-new or T-new exceeds 16.7 ms. This is ADR 0056's update-plus-view rule; its consequence, moving damage tracking into the component, applies in the follow-up.
- R3 (keystroke). The difference K-new minus K-old is a regression when its 95% confidence interval lies entirely above zero and the difference is at least 0.167 ms, one percent of the 60 Hz budget (ADR 0043:78-80). The analysis predicts added cost in A, so R3 is expected to be the decisive test. If it holds, the follow-up is incremental keyed edits. The difference is reported with its interval whether or not R3 holds.
- R4. K-new damage is not `Region`, so ADR 0051's damage-limited repaint is lost.
- R5. A focus test fails.
- R6. Case N allocates beyond display-list command storage.

## Open questions

1. The clinical form's node count, and the value of A. Neither is measured.
2. Whether the browser host uses this operation (ADR 0056 open question 4).
