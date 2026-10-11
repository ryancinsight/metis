# 0062 — Virtualized list projection over the row model

Status: Accepted

Date: 2026-10-11

Driver: METIS-LIST-001 (virtualized widget named by ADR 0003's Slint-derived
implementation order).

## Context

`metis_ui_lang::layout::VirtualList` windows uniform and variable extents, but
no consumer used it: the result explorer pages rows, and the row model
`metis_frontend::reactive::ListModel` delivers typed changes with no reader that
turns them into a scroll window. Long lists therefore built every row.

Comparators for a windowed list, source-pinned to published crates rather than
documentation alone:

- Freya 0.5.0-rc.10 (`freya-components`, crates.io, 2026-10-11) provides
  `VirtualScrollView` (`src/scrollviews/virtual_scrollview.rs`). Its
  `ItemSize::Fixed(f32)` gives every item one size; `ItemSize::Dynamic` asks a
  callback for each index. Freya's latest stable release is 0.4.3 (2026-08-30);
  the 0.5 line is still a release candidate. Its layout is the `torin` engine,
  its renderer Skia (`freya-engine`), and its tests run headlessly through
  `freya-testing` (`launch_test`, `commit_accessibility`). Freya also has
  experimental Android and Web support. `freya/Cargo.toml:338` gates
  `freya-android` on `target_os = "android"`; `freya-core/src/platform.rs:58-83`
  defines an `Android` platform variant; `freya/src/_docs/platforms.rs:3,14-15,72`
  documents Web as WebAssembly built with Emscripten and rendered through a
  WebGL 2 backend (`platforms.rs:41-44`). The top `freya` crate in these sources
  is 0.5.0-rc.9; `freya-core`, `freya-components` and `freya-testing` are rc.10.
- egui's `ScrollArea::show_rows`, GPUI's `uniform_list`/`list` and Slint's
  `Model`/`ModelNotify` (ADR 0003) supply the same shape: the host builds only
  the rows a scroll position shows.

Metis differs in one place that matters: its DOM and software hosts both render
one `DomDocument`, so a list must project to DOM, not to a toolkit widget.

## Decision

A host-neutral view, `metis_frontend::list::VirtualListView<T, F>`, binds a
`ListModel<T>` to `VirtualList` and projects the window as DOM.

- Row extents come from a pure measure `F: Fn(&T) -> u32`. A row is measured
  only when it is new or changed; shifted rows keep their measurements.
- The subscription records structure only: a change marks the affected
  extents unknown and moves the others by index. Values are read from the
  model at the next query, so no row is measured from a state a later change has
  superseded. A `Reset` rereads every row.
- A query rebuilds the `VirtualList` once after any change. Rebuilding is
  `O(n)` in the row count; per-row Fenwick updates wait for a measured need.
- Projection: a top spacer of `window.start`, one wrapper per built row pinned
  to its measured extent, and a bottom spacer of `total − window.end`. Children
  sum to the total extent by construction.
- The host owns scroll offset, viewport extent and input. The view owns no
  scroll container, scrollbar or wheel policy.
- Bounds: rows ≤ `MAX_MODEL_ROWS` (= `MAX_VIRTUAL_ITEMS`), extents ≤
  `MAX_ITEM_EXTENT`. Pixel values are `i32` layout sizes, converted with
  `i32::try_from`; content beyond that range is `LayoutOverflow`.
- Typed refusals: a model without a free subscriber slot is `QueueFull`; a
  mismatch between the mirror and the model, or a change during measurement,
  is `RenderFailure`. The view never guesses.

Rejected alternatives:

- Fixed item size only (Freya's `ItemSize::Fixed`). Result rows vary in height,
  and the model already tells us which rows changed.
- A per-index size callback re-evaluated for every index (Freya's `Dynamic`).
  It re-measures the whole list after one edit.
- Measuring inside the change listener. Under re-entrant delivery the listener
  can read a state that a later change has already superseded.
- Rebuilding from the model on every change. It contradicts `ListModel`'s
  promise that peers apply the change rather than rereading the list.
- An immediate-mode widget (egui-style). The DOM host renders `DomDocument`,
  so the list must be a document projection.

## Consequences

- `VirtualList` has its first consumer. Its public surface is now exercised
  through the model and the DOM projection.
- Measure calls scale with changed rows. Layout and window queries after a
  structural change scale with the row count.
- Content above `i32::MAX` layout pixels (about 2.1 × 10⁹) is refused, so a list
  of up to 2³¹ px of content is the supported range.
- Verified by `crates/metis-frontend/src/list/tests.rs` (7 tests): the
  incremental mirror equals a list built from scratch after every one of 400
  seeded insert, remove, set and replace operations, compared on totals and
  windows; a change measures only its rows; DOM spacers and rows sum to the
  total; oversized content is refused; a refused subscription and a dropped view
  behave as specified.
- Not yet delivered: a software-host projection of the window, a scroll
  container with wheel and keyboard offset mapping, and a rendered visual
  capture. These are the follow-up items named in the board.

## Overturning evidence

A measured query cost above the frame budget on a 10⁵-row model with a change
per frame would justify per-row Fenwick updates. A required content range above
`i32::MAX` layout pixels would justify a wide layout size type, which is a layout
contract change and needs its own record. A host that cannot consume the DOM
projection would justify a second projection, but not a toolkit dependency.
