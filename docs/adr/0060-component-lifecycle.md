# 0060 — General component lifecycle for application state and views

Status: Proposed

Date: 2026-10-11

Driver: METIS-COMPONENT-001. ADR 0003 records the general component lifecycle
as open (its application-state row).

## Context

The native host seam exists. `metis_platform::native::NativeApplication`
supplies the framebuffer, damage, the optional accessibility tree and one event
batch that returns a host action. `NativeForm<T>` implements it
(`crates/metis-app/src/frontend/native.rs:70`). Its state is `FrontendApp<T>`
(`crates/metis-frontend/src/app.rs`), which owns a fixed `FormInputs` clinical
record and a `FormState`, and renders into a `TrackedDocument`
(`crates/metis-frontend/src/document.rs:13`).

The browser host is separate. `AsyncFrontendApp`
(`crates/metis-frontend/src/async_app.rs:18-24`) owns the same `FormInputs` and
`FormState` without a document. `metis-web` uses neither `DomElement` nor
`TrackedDocument`.

No reusable declaration names an application's state, messages and view. The
reactive stores (`Writable`, `derived`, `resource`, `ListModel`) are the state
primitives. `TrackedDocument` changes only through `set_text`, `set_attribute`
and `restyle` (`document.rs:39-52`); it cannot insert or remove children.

Comparators:

- Iced 0.14: Elm-style state, messages, `update` and `view`, with `Task` and
  `Subscription` for asynchronous work (ADR 0003, Iced section).
- Freya 0.5.0-rc.10: hooks (`use_state`, `use_memo`, `use_hook`). `use_hook`
  keeps values in per-scope positional storage
  (`freya-core/src/lifecycle/base.rs:76-101`). A run that calls more hooks than
  the first run, or a hook whose stored type differs, panics with `HOOKS_ERROR`
  (`base.rs:80-89`). Fewer hooks, or reordered hooks of one type, pass unchecked.
- egui: immediate-mode widgets and responses (ADR 0003, application-state row).
- GPUI: entities, views and actions (ADR 0003, application-state row).
- Tauri: the frontend framework supplies widgets and state; Rust exposes
  commands and events (ADR 0003, application-state row; ADR 0012).
- htmx: the event, request, target and swap model; the server returns fragments
  that the browser swaps in (ADR 0003, hypermedia row; ADR 0022 admits the
  typed-patch subset).

Slint is not compared here. Its comparison is in the ADR 0003 text on PR #488,
which is not on main.

## Decision

Adopt a host-neutral component trait in `metis-frontend`. A component's state is
its own fields. Its messages form an enum per component. Its view is a
`DomElement` (`crates/metis-ui-lang/src/dom.rs:8`).

```rust
pub trait Component {
    /// The component's inputs, one enum variant per input.
    type Message: Clone + 'static;
    /// Applies one message and returns the effects the host must perform.
    fn update(&mut self, message: Self::Message) -> Effects<Self::Message>;
    /// Projects the current state.
    fn view(&self) -> DomElement;
}
```

- `Effects` has two variants: an IPC request and a message to self. A timer is
  not a variant (open question 1).
- An effect that completes after its component is dropped, or after its source
  value changed, is discarded. `resource`'s `Completion::is_current` already
  does this (`crates/metis-frontend/src/reactive/resource.rs:43`).
- `update` is infallible. A refusal of domain input is recorded in the
  component's state and rendered by `view`; it is not returned from `update`.
  `set_composition` returns `PayloadTooLarge` today (`app.rs:238-246`); under
  this contract that rejection is state.
- Dispatch re-entry is bounded by the store's round limit, `MAX_CASCADE`
  (`crates/metis-frontend/src/reactive/store.rs:10`). A dispatch past the bound
  is reported as `CascadeLimit` (`store.rs:15`).
- This ADR does not decide how a `DomElement` reaches a `TrackedDocument` or the
  native paint path (open question 2). `NativeForm<T>` keeps its current path
  until that question is answered. The browser host is outside this decision.
- No `dyn Component`. A shell that selects components at run time enumerates a
  closed set, and each component stays monomorphized, as `VirtualListView<T, F>`
  is (ADR 0055, PR #484).

## Rejected alternatives

- Hooks (Freya). Hook state is identified by call order. `base.rs:80-89` panics
  on a longer sequence and on a stored-type mismatch, but a shorter or reordered
  sequence of one type passes unchecked. A component names its state in fields,
  so no call order is involved.
- Immediate mode (egui). ADR 0003 records immediate-mode widgets and responses
  as the egui model. The native host renders a retained `DomDocument` through
  `TrackedDocument`, so a widget layer rebuilt each frame would be a second model
  beside it.
- Entity and view handles (GPUI). They add a second ownership graph beside the
  reactive stores, which already deliver single-thread notification through `Rc`
  (`reactive/model.rs`, `reactive/resource.rs`).
- Status quo. `FrontendApp<T>` and `AsyncFrontendApp<T>` each own the fixed
  `FormInputs` record and `FormState`. Any second application repeats that
  wiring, and the browser host already duplicates it.
- The archived `iced_web` DOM runtime. It is read-only upstream (ADR 0003).

## Consequences

- The contract is a new seam with one present role (the clinical form) and one
  planned role (the starter, `crates/metis-starter`). Its surface stays minimal
  until a second implementor exists.
- Closure of the message set is a convention. `type Message: Clone + 'static`
  admits any type, so each component declares an enum, and `update`'s `match`
  must handle every variant when one is added. The type system does not forbid
  a non-enum message.
- `VirtualListView::dom` refreshes its mirror, so it takes `&mut self` and
  returns `Result`. `view(&self)` cannot call it. A component that holds a list
  refreshes in `update` and projects in `view`; that split is part of the first
  slice.
- One generic test set, `<C: Component>`, is the intended test surface for the
  clinical form and the starter. Each monomorphization that ships is tested.
  The set is not written yet.
- Migration order: the contract and its generic tests, then the clinical form,
  then the starter. Slice 1 does not start until open question 2 is answered
  by the follow-up ADR.

## Overturning evidence

- A component family that needs keyed child identity across updates beyond
  `ListModel` (ADR 0055, PR #484). The `view` return shape then changes by
  revision.
- Measured update and view cost on the clinical form above the frame budget.
  That would move damage tracking into the component.
- A measured full-tree replacement cost into `TrackedDocument` above the frame
  budget. That would require an incremental edit set and reopen question 2.
- A host that needs two UI threads. The `Rc`-based stores would then need a
  `Send` variant, and this ADR would be revised.

## Open questions

1. Whether `Effects` needs a timer variant. The native host's finite wait
   (`run_native_application`, `crates/metis-platform/src/native/application.rs:146`)
   covers animation ticks today. The browser host has no equivalent in this
   design.
2. How a `DomElement` reaches a `TrackedDocument` and the native paint path.
   `TrackedDocument` has `set_text`, `set_attribute` and `restyle` only
   (`document.rs:39-52`), so inserting or removing children needs a new edit or
   a full replacement. Decided by a follow-up ADR before slice 1.
3. Whether multi-window applications share one component tree or own one each.
