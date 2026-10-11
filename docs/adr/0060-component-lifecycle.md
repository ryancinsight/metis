# 0060 — General component lifecycle for application state and views

Status: Proposed

Date: 2026-10-11

Driver: METIS-COMPONENT-001. ADR 0003 records the general component lifecycle
as open.

## Context

The host seam exists. `metis_platform::native::NativeApplication` supplies the
framebuffer, damage, the optional accessibility tree and one event batch that
returns a host action. Two host wrappers implement it: `NativeForm<T>`
(`crates/metis-app/src/frontend/native.rs`) and `BrowserApplication`
(`crates/metis-web/src/browser/application.rs`), each around the frontend app.

The application model is not shared. `FrontendApp<T>`
(`crates/metis-frontend/src/app.rs`) owns a fixed `FormInputs` clinical record,
a `FormState` and a tracked document. No reusable declaration names an
application's state, messages and view. The reactive stores (`Writable`,
`derived`, `resource`, `ListModel`) are the state primitives, and
`TrackedDocument` (`crates/metis-frontend/src/document.rs:13`) already advances
a revision only when the content changes.

Comparators, source-pinned in ADR 0003 and PR #484's ADR 0055:

- Iced 0.14: Elm-style state, messages, `update` and `view`, with `Task` and
  `Subscription` for asynchronous work (ADR 0003, Iced section).
- Freya 0.5.0-rc.10: hooks (`use_state`, `use_memo`, `use_hook`). `use_hook`
  stores values in per-scope positional storage
  (`freya-core/src/lifecycle/base.rs`). A later run whose hook sequence differs
  panics with `HOOKS_ERROR`, a runtime check.
- egui: immediate mode; widgets return responses each frame (ADR 0003).
- GPUI: entities and views; models are handles to shared state (ADR 0003).
- Slint: declared properties and bindings compiled ahead (ADR 0003).
- Tauri: the frontend framework owns component state; Rust exposes commands
  and events (ADR 0002, ADR 0012).
- htmx and HTML5: hypermedia; the document holds state and the server returns
  fragments (ADR 0022 admits the typed-patch subset).

## Decision

Adopt a host-neutral, monomorphized component contract in `metis-frontend`.
State is the component's own fields. Input is a closed, typed message set.
Output is a host-neutral DOM projection (PR #484's PR #484's ADR 0055 `DomElement` model).

```rust
pub trait Component {
    /// Closed set of inputs this component accepts.
    type Message: Clone + 'static;
    /// Applies one message and returns the effects the host must perform.
    fn update(&mut self, message: Self::Message) -> Effects<Self::Message>;
    /// Projects the current state; hosts render the result.
    fn view(&self) -> DomElement;
}
```

- Effects are a closed enum (an IPC request, a timer, or a message to self),
  not a boxed future. The host performs them and feeds results back as
  messages. An effect that completes after its component is dropped is
  discarded by generation, as `resource` already discards superseded results.
- The host wrappers stay thin. `NativeForm<T>` and `BrowserApplication` route
  events into `update` and render `view`. Neither owns UI state.
- Revision: `TrackedDocument` advances when the projected content changes, so
  repaint and layout caches keep their present behavior.
- Bounds: the message queue is bounded at the store cascade limit
  (`MAX_CASCADE`). A message that re-enters `update` past the bound is refused
  with a typed error.
- No `dyn Component`. A shell that selects components at run time enumerates a
  closed set, and each component stays monomorphized (PR #484's PR #484's ADR 0055 generic
  projection).

Rejected alternatives:

- Hooks (Freya). Positional hook storage makes call order the identity of
  state. The type system does not check it, and a changed sequence fails at run
  time with `HOOKS_ERROR`.
- Immediate mode (egui). It rebuilds the tree every frame, but focus, IME and
  the DOM accessibility tree need a retained document (ADR 0014, ADR 0041).
- Entity and view handles (GPUI). They add a second ownership graph beside the
  reactive stores, which already give single-thread notification through `Rc`.
- Status quo. `FrontendApp` cannot host a second application, and the starter
  would duplicate the clinical wiring.
- The archived `iced_web` DOM runtime. It is read-only upstream (ADR 0003).

## Consequences

- Migration proceeds in dependency order: the contract and its generic tests,
  then the clinical form (`FormInputs` and `FormState` become that component's
  model), then the starter application.
- One generic test set, `<C: Component>`, runs on the software host and the
  DOM host. Each monomorphization that ships is tested.
- Messages are typed. Invalid transitions are unrepresentable in the closed
  message enum, and a rejected transition returns a typed error.
- The component contract is a new seam with one present role (the clinical
  form) and one planned role (the starter). Its surface stays minimal until a
  second implementor exists in the tree.

## Overturning evidence

- A component family that needs keyed child identity across updates beyond
  `ListModel` (PR #484's ADR 0055). The `view` return shape then changes by revision.
- Measured update and view cost on the clinical form above the frame budget.
  That would move damage tracking into the component.
- A host that needs two UI threads. The `Rc`-based stores would then need a
  `Send` variant, and this ADR would be revised.
- A measured reduction in starter boilerplate from a hook runtime that exceeds
  its positional-state risk. That evidence would reopen the hooks alternative.

## Open questions

- Whether `Effects` needs a timer variant, or whether the host's finite wait
  (`run_native_application`) covers animation ticks alone.
- Whether multi-window applications share one component tree or own one each.
