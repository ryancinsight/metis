# 0061 — Reuse across a relayout and a reprojection

Status: Accepted

Date: 2026-10-24

Driver: `METIS-MEM-RENDER-002`.

## Context

A host renders on every event. `RenderCache` lays a document out once per
`(document revision, viewport)` and projects its semantics once per revision, so
an unchanged render requests one allocation — the frame's command storage
(`METIS-MEM-RENDER-001`). An **edit** moves the revision, and both caches then
rebuild their whole value: a one-character `set_inputs` edit requested 358
allocations at `5ea77d7`, of which 82 were the layout and 268 the semantic
projection (`METIS-MEM-RENDER-002` measured 350 at `c9b99c1`; the transition
owner index added eight).

Nothing about that edit is proportional to the form: one label changed, and the
other forty elements were rebuilt because the revision moved. The item's outcome
is that a render which changes one label requests memory for what it changed.

## Decision

1. **A layout writes through a recycle cursor.** `DisplayList` gains `written`,
   `begin_frame` and `end_frame`; layout indices come from `written` rather than
   `commands.len()`, and `push` overwrites the command the previous layout left
   at that position instead of growing a second buffer. `end_frame` truncates to
   what the new layout reached, so a reader never sees the previous frame's tail.
2. **A command whose string repeats shares it.** `held_id` and `held_text` hand
   the layout the `Arc<str>` the previous layout interned when the content at the
   same position is equal, so an unchanged identifier or text run is a refcount
   bump rather than an allocation. The position is a hint, not an invariant: a
   layout whose command sequence shifted simply allocates as before.
3. **Child placements live in one arena.** `PlacementArena` holds the
   `ChildPlacement` records of every container the layout is inside; a container
   appends above its children and truncates once it has aligned them. The arena
   is reserved once from the node count `validate_layout_tree` already walks, so
   the layout requests no memory per container.
4. **`compute_layout_into` writes into a caller's list.** A failed layout leaves
   that list empty rather than half-written, and `RenderCache` drops its key, so
   a stale list is never served under a newer revision.
5. **The projection is refreshed, not rebuilt.** `SemanticTree::refresh(document,
   changed)` reuses every node whose element, subtree and inherited visibility
   the edit left alone, and rebuilds the edited elements, the ancestors whose
   name aggregates their content, and the elements that name them through
   `aria-labelledby` or `aria-describedby`. Reused nodes are moved, not cloned.
6. **The document reports what an edit addressed.** `TrackedDocument` records the
   authored id of each edit that changed content and hands the set to the
   projection once (`take_changed`), which is the only statement of *what* moved;
   the revision says only *that* something moved.
7. **Reuse is guarded structurally.** A node records no structure, so the refresh
   compares the document's element count with the projection's and each reused
   node's child count with its element's; a document that changed shape is
   projected from scratch.

## Rejected alternatives

- **Per-element dirty flags in the DOM.** The tracked document already knows the
  id an edit addressed, and the invalidation set is derived (ancestors,
  referrers), not per element. A flag per element would duplicate the id and
  still not express "this element's *name* changed because a descendant did".
- **Relaying out only the dirty subtree.** Sizes propagate: an automatic height
  or a flex-grown child depends on its siblings, so in the authored form the
  changed label's ancestors are the column that holds the whole form. A whole
  relayout measures 3 requests after the recycling above, so the incrementality
  would buy structure, not memory.
- **Storing `Arc<str>` in the DOM** so the display list can clone identifiers and
  text. The layout already shares them positionally, which costs neither a DOM
  type change nor a second source of truth for an element's id.
- **Serving the cached projection to hosts** through interior mutability.
  `semantic_tree()` returns an owned tree and each host projects on its own
  revision gate; changing that signature is a separate decision with its own
  consumers (`metis-app`'s accessibility projection and capture).

## Consequences

At the delivered revision a one-character `set_inputs` edit requests **44**
allocations against the 321 a whole-document rebuild of the same form requests,
and an unchanged render still requests one. A relayout of a 20-row and of a
200-row document both request 3; one edited label reprojects in 19 requests on 20
rows and 21 on 80, against 449 and 1715 for `from_document`.

Three limits are deliberate. The refresh still walks the document and re-joins
the ancestors' content, so its **time** stays linear in the document even though
its memory is not. Reuse is sound only while the shape holds, which the guards in
decision 7 enforce by falling back to a full projection. And an element whose
aggregated content exceeds `MAX_SEMANTIC_TEXT_BYTES` still fails to project, so a
document whose root names more text than that bound has no projection at all —
unchanged from before, and now reachable through the same error.

## Evidence

- `crates/metis-ui-lang/tests/edit_allocation.rs` — a relayout and a reprojection
  of one edited label request no more on a ten-times larger document, while a
  fresh projection scales with it. Structural, so it cannot pass by shrinking a
  constant.
- `crates/metis-frontend/tests/render_allocations.rs` —
  `an_edit_requests_memory_for_what_it_changed` measures the edit against the
  rebuild it replaced.
- `crates/metis-ui-lang/src/semantics/refresh/tests.rs` — nine differential
  tests: each edits a document, refreshes the projection built before the edit,
  and asserts equality with `from_document` afterwards, including the duplicate
  identity, unresolved reference, role and shape cases.
- `docs/VERIFICATION.md` — the measurements above with their commands.
