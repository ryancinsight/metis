# ADR 0018: Bounded result explorer

Status: Accepted

Date: 2026-09-08

Driver: [METIS-DATA-001](../../backlog.md#METIS-DATA-001).

## Context

The browser workbench rendered one backend result, but it did not retain a
bounded result history or expose a reusable table, filter, selection, or tree
disclosure contract. Those responsibilities must remain independent of the
HTML host so the same state can serve the native frontend and the browser
surface. The viewer migration also needs an explicit place for live DICOM
calculation results without exposing response signatures or unbounded patient
labels to the view.

## Decision

Add `metis_frontend::ResultExplorer` as the host-independent result view state.
It accepts validated `ClinicalCalcResponsePayload` values, owns at most
`MAX_RESULT_ROWS` rows and `RESULT_PAGE_SIZE` visible entries per page, groups
rows by validated patient labels, and preserves a selected `ResultId` across
updates while the row remains retained. Duplicate audit identities replace the
existing row; new rows report an explicit eviction when the bound is full.

Ordering is represented by `SortOrder` over sequence, patient, volume rate or
drug rate with an ascending or descending direction. `ExplorerStatus` exposes
empty, loading, ready and typed error states. `VisibleEntry` is the only view
projection: a group carries its disclosure state and row count, while a row
provides the validated result values. The response signature remains in owned
state and is never formatted into browser markup.

The browser binds the state through delegated `input`, `change` and `click`
listeners. Rust renders a semantic table with labelled filter and order
controls, keyboard-operable entry buttons, native disclosure semantics and
bounded paging. Empty, loading and error states stay visible through the live
status region. The listener is removed with the existing host lifecycle.

The module is split into `types`, `state`, `state/mutations` and
`state/visibility` so public contracts, state transitions and projections have
one bounded home each. No new dependency or GUI-specific type enters the
frontend core.

## Alternatives

Keeping rows in browser JavaScript would duplicate validation and selection
rules and would not serve the native frontend. Rendering directly from the
latest response would lose the history and update identity needed by the
viewer. An unbounded collection would make a live service response stream an
allocation growth path. A generic GUI table dependency would couple the core
state to one host and add a dependency without an existing Atlas contract.

## Threat model and limits

Patient labels, response values and audit identities cross the backend
boundary. Constructors reject empty, control-containing or oversized labels,
non-finite rates and zero identities; replacement rejects duplicate identities
and more than the row bound. The browser filter is bounded and rejects control
characters. The explorer does not authenticate responses; it consumes the
existing typed frontend/backend contract, whose transport and MAC boundaries
remain owned by their existing modules.

The current browser capture has no configured backend bridge, so it proves the
empty state, filter event and semantic controls only. Native explorer tests
cover real response insertion, ordering, selection retention, replacement,
eviction, disclosure, paging and rejection paths. A live-service capture with
real rows remains part of the viewer integration evidence.

## Verification

`cargo nextest run -p metis-frontend --locked` exercises the explorer's exact
value semantics and boundaries. Native and WebAssembly `cargo check` plus
`cargo clippy -D warnings` cover both host targets. The browser asset test
checks the bounded controls, IDs, labels, listener event types and stylesheet
selectors. The CUA runtime trace at 1280×720 shows the rendered explorer,
empty status, filter, order control, table caption and disabled pager; the
filter was changed to `PT`, then `X`, and cleared through keyboard deletion.
