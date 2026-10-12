# 0061 — Whole-document replacement for the native paint path

Status: Proposed

Date: 2026-10-11

Driver: METIS-COMPONENT-001. Answers open question 2 of ADR 0056: how a
`DomElement` reaches a `TrackedDocument` and the native paint path.

## Context

The native render path edits the presentation document in place. `FrontendApp`
changes it through `restyle`, `set_text` and `set_attribute`
(`crates/metis-frontend/src/presentation.rs`). Each moves `TrackedDocument`'s
revision only when content changes (`crates/metis-frontend/src/document.rs:65`).
Two derived values key on the revision:

- The focusable order (`crates/metis-frontend/src/presentation/cache.rs:37-38`),
  reconciled against on every render (`presentation.rs:135-136`) and held as an
  authored id (`crates/metis-frontend/src/focus.rs:42`).
- The layout, keyed by revision and viewport (`cache.rs:61-63`).

Repaint is decided by display-list damage, not by revision
(`presentation.rs:147-149`; `crates/metis-platform/src/framebuffer/damage.rs:86-92`).

The three edits cannot insert element children, or remove one child while keeping
its siblings. `set_text_content` removes every child and leaves one text node
(`crates/metis-ui-lang/src/dom.rs:113-114`). `DomElement`, `DomNode` and
`DomDocument` derive `PartialEq` (`dom.rs:7,52,74`), and `DomDocument::new`
validates nothing (`dom.rs:83-85`).

A render of unchanged state requests no memory beyond display-list command
storage (`presentation.rs:100-102`; `crates/metis-frontend/tests/render_allocations.rs:114-118`).

Baseline (ADR 0051, revised 2026-09-24). On a pinned core, with alternating before
and after binaries, the keystroke case of `crates/metis-frontend/benches/frame.rs`
takes 0.40 ms at 800×600 and 0.49 ms at 1600×1200 (200% scale). That case times
`set_inputs`, which runs the whole `render()` path (`app.rs:212-227`). The 2.8 and
8.2 ms figures are pre-change history. Later changes to rendering code (ADRs
0052–0054, dated 2026-09-24 to 2026-09-27, and commit `900064e`, 2026-10-01)
postdate the baseline, so it is re-measured in the same run. The frame budget is
60 Hz, 16.7 ms (ADR 0043:80).

## Decision

A component's view is applied to the native paint document as a whole.

- `TrackedDocument` gains `differs(&self, document: &DomDocument) -> bool`, which
  compares with the stored document, and `replace(&mut self, document: DomDocument)`.
  `replace` stores a document the caller has checked with `differs`, advances the
  revision once, and returns nothing. It debug-asserts that the document differs,
  and never produces `Edit::Missing`. The three existing edits stay. No insert or
  remove edit is added.
- `RenderCache` gains `install(&mut self, revision, viewport, layout, focus_order)`.
  It sets the focusable revision (`cache.rs:22`) and the layout key
  (`cache.rs:25`) to the new revision and viewport.
- A host applying a component's view runs these steps in order:
  1. If `differs` is false, stop. An equal candidate costs one comparison.
  2. Validate with the render viewport (`presentation.rs:139-142`). Run
     `compute_layout`, then build the semantic projection and focus order from the
     document alone (`cache.rs:39`). Refuse an empty focus order, since `reconcile`
     already errors on that state (`focus.rs:70`).
  3. `replace` the document, then `install` the validated layout and focus order
     under the new revision.
  A refused candidate leaves the previous document and cache in place.
- The host builds a view only after it applies a message to the component. A render
  with no applied message allocates no view tree.
- Element identity is the authored `id`, never tree position.
- A view hides a focusable control with the `hidden` attribute (any value), or with
  `aria-hidden` set to `true`, `1` or empty (`semantics.rs:127-128`;
  `boolean_attribute`, `semantics.rs:352-367`). The attribute applies to the
  control or an ancestor. Style alone does not hide it: `display: none` leaves the
  control in the focus order (`semantics.rs:162-163`; `focus.rs:157-161`). The
  command menu sets `aria-hidden` on its container (`presentation.rs:226`), and
  descendants inherit it (`semantics.rs:129-130`).
- A focused control that is removed or hidden loses focus to `reconcile`
  (`focus.rs:61-70`), in this order: the `popover-anchor` of its nearest enclosing
  popover, if that anchor is focusable; `label-patient` (`INITIAL_FOCUS`,
  `focus.rs:17`); the first focusable control; otherwise a typed error. The anchor
  step applies only to a hidden control, since a removed one has no ancestors
  (`focus.rs:173-177`).
- Full replacement is the proposed path. The measurement tests it.
- Scope: the native paint path. The list projection (ADR 0056, open question 3)
  and the browser host (open question 4) are not decided here.

## Rejected alternatives

- Child insert and remove edits. Layout is keyed on the revision
  (`cache.rs:61-63`), which advances in both designs, so layout and focus order
  recompute either way. Child edits would save only the owned candidate tree, the
  view build and the tree comparison. None of these is measured; the saving is an
  analytical claim. The edit set would also have to agree with the view's tree at
  every step.
- A keyed diff that derives those edits. It has the same cost structure, plus a
  diff algorithm and an id rule this decision already states.
- Unconditional replacement, which advances the revision on every update, even
  when the view is unchanged. That invalidates the focus order and the layout
  (`cache.rs:38,61`). Step 1 prevents it.
- Painting a `DomElement` directly. It bypasses `RenderCache`
  (`cache.rs:37-39,56-63`) and the focus reconcile (`presentation.rs:135-136`).

## Consequences

- Allocation: a changed update builds an owned view tree, which is new cost. A
  render with no applied message builds none.
- Comparison: a changed update compares the trees once, in step 1. The comparison is
  linear in node count. The clinical form's node count is not measured.
- Layout: a changed document lays out once, in step 2. An unchanged document runs
  no layout.
- Limits: a programmatic document has the same depth and node limits as a parsed
  one, and the relevant content-byte limits
  (`crates/metis-ui-lang/src/layout/geometry.rs:63`). Step 2 enforces them, because
  `compute_layout` runs before storage.
- Tests: a differing replacement advances the revision once. An equal candidate
  never reaches `replace`. A refused candidate leaves the previous document and
  cache unchanged. Focus stays on a focusable control only when it keeps its
  authored id and is neither hidden nor removed.
- `FrontendApp`'s current edit path stays valid. Whether its edits migrate is this
  ADR's open question 3.

## Measurement

The measurement decides whether the proposal stays or is rejected.

Every end-to-end case spans from the message applied to the rendered framebuffer,
as `frame.rs` times `set_inputs`. Presentation to a window is outside the span,
because the harness presents nothing. Cases run at both surfaces the benchmark
uses (800×600 and 1600×1200 at 200%):

- K-old and K-new: keystroke, existing path and replacement path, same run.
- T-old and T-new: theme change, existing path and replacement path, same run.
- For K-new and T-new, stage timings: build and compare (A), validation (B), and
  paint of the display list (P). Install and store is the remainder.
- N: a render with no applied message, replacement path.

Preconditions are asserted before timing:

- K-new damage is `Damage::Region` (`damage.rs:92`).
- T-new damage is `Damage::Full`. The theme page fill spans the surface
  (`crates/metis-frontend/src/presentation/theme.rs:22`), and `damage.rs:86-88`
  returns `Full` when the grown region covers the surface. The existing repaint
  tests accept any damage for theme edits
  (`crates/metis-frontend/src/presentation/repaint_tests.rs:55-85`), so this
  assertion is new. If it fails, the theme case is revised before timing.

Each case records wall time and allocations, with pinned inputs, `black_box`, and a
median with a confidence interval, under the repository's benchmark methodology.

## Overturning evidence

- The median of K-new or T-new exceeds 16.7 ms. ADR 0056 then rules this path out,
  as it states. The stage breakdown informs a follow-up ADR. It does not reopen
  this path.
- A precondition fails: K-new damage is `Full`, or T-new damage is not `Full`. The
  damage-limited repaint of ADR 0051 is then lost, and the decision is revised.
- N allocates beyond display-list command storage. The applied-message rule then
  fails, and the decision is revised.
- A focus regression that the hiding rule and the fallback order cannot prevent.

The keystroke delta K-new minus K-old is recorded, not gated. A replacement path
that stays inside the frame budget and keeps region damage is accepted even when it
is slower than the existing path. The delta is its measured update cost.

## Open questions

1. The clinical form's node count and its per-update build cost (A). Not measured.
2. Whether the browser host uses this operation (ADR 0056, open question 4).
3. Whether `FrontendApp`'s edits migrate to replacement. This ADR's open question 3.
