# 0060 — General component lifecycle for application state and views

Status: Proposed

Date: 2026-10-11

Driver: METIS-COMPONENT-001. ADR 0003 records the general component lifecycle
as open (its application-state row).

## Context

The host seam exists. `metis_platform::native::NativeApplication` supplies the
framebuffer, damage, the optional accessibility tree and one event batch that
returns a host action. `NativeForm<T>` implements it
(`crates/metis-app/src/frontend/native.rs:70`). The browser host,
`BrowserApplication` (`crates/metis-web/src/browser.rs:100`), implements no such
trait. It wraps `AsyncFrontendApp` (`browser.rs:103`) behind its own entry
points.

The application model is not shared. `FrontendApp<T>`
(`crates/metis-frontend/src/app.rs`) owns a fixed `FormInputs` clinical record
and a `FormState`. No reusable declaration names an application's state,
messages and view. The reactive stores (`Writable`, `derived`, `resource`,
`ListModel`) are the state primitives. `TrackedDocument`
(`crates/metis-frontend/src/document.rs:13`) changes through edit methods such
as `restyle` (`document.rs:52`).

Comparators:

- Iced 0.14: Elm-style state, messages, `update` and `view`, with `Task` and
  `Subscription` for asynchronous work (ADR 0003, Iced section).
- Freya 0.5.0-rc.10: hooks (`use_state`, `use_memo`, `use_hook`). `use_hook`
  keeps values in per-scope positional storage
  (`freya-core/src/lifecycle/base.rs:76-101`). A run that calls more hooks than
  the first run, or a hook whose stored type differs, panics with
  `HOOKS_ERROR` (`base.rs:80-89`). Fewer hooks, or reordered hooks of one type,
  pass unchecked.
- egui: immediate mode; widgets return responses each frame (ADR 0003,
  application-state row).
- GPUI: entities, views and actions (ADR 0003, application-state row).
- Tauri: the frontend framework supplies widgets and state; Rust exposes
  commands and events (ADR 0003, application-state row; ADR 0012).
- htmx and HTML5: hypermedia; the document holds state and the server returns
  fragments (ADR 0022 admits the typed-patch subset).

Slint is not compared here. Its comparison row is in the ADR 0003 text on the
peer branch (PR #488), which is not on main.

## Decision

Adopt a host-neutral, monomorphized component contract in `metis-frontend`.
State is the component's own fields. Input is a closed, typed message set.
Output is a host-neutral projection to the existing `DomElement`
(`crates/metis-ui-lang/src/dom.rs:8`).

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

- `Effects` is a closed enum with two variants: an IPC request and a message to
  self. It is not a boxed future. The host performs each effect and feeds its
  result back as a message. A timer is not a variant here (open question 1).
- An effect that completes after its component is dropped is discarded by
  generation, as `resource` discards superseded results
  (`crates/metis-frontend/src/reactive/resource.rs`, `Completion::is_current`).
- Hosts stay thin: `NativeForm<T>` and the browser host route events into
  `update` and render `view`. The browser host implements no trait today, so
  its route is part of its migration slice.
- Re-entry: a message sent while `update` runs waits in the host dispatch
  queue. A dispatch that would exceed `MAX_CASCADE` rounds is reported as the
  store's `CascadeLimit` error (`crates/metis-frontend/src/reactive/model.rs`).
- Revision: the host applies each projected view to `TrackedDocument`. How a
  projection maps onto those edits is open question 2.
- No `dyn Component`. A shell that selects components at run time enumerates a
  closed set, and each component stays monomorphized, as `VirtualListView<T, F>`
  is (ADR 0055).

## Rejected alternatives

- Hooks (Freya). Positional storage makes call order the identity of state. The
  check at `base.rs:80-89` catches a longer sequence and a type change at run
  time, but not a shorter or reordered sequence of one type. The type system
  does not carry the invariant.
- Immediate mode (egui). It rebuilds the tree every frame. Metis hosts render a
  retained document, and a widget renderer duplicated beside the browser's own
  would lose browser-native focus and keyboard handling (ADR 0014:163).
- Entity and view handles (GPUI). They add a second ownership graph beside the
  reactive stores, which already deliver single-thread notification through
  `Rc` (`reactive/model.rs`, `reactive/resource.rs`).
- Status quo. `FrontendApp<T>` owns the fixed `FormInputs` record and
  `FormState`. Any second application with other state repeats that wiring.
- The archived `iced_web` DOM runtime. It is read-only upstream (ADR 0003).

## Consequences

- Migration proceeds in dependency order: the contract and its generic tests;
  then the clinical form, whose `FormInputs` and `FormState` become that
  component's model; then the starter page (`crates/metis-starter`), which binds
  its form in Rust without `FrontendApp`.
- One generic test set, `<C: Component>`, runs on the software host and the DOM
  host. Each monomorphization that ships is tested.
- Messages are a closed enum, so an invalid transition has no variant. `update`
  is infallible. The one refusal, a dispatch past `MAX_CASCADE`, is a typed error
  from host dispatch.
- The contract is a new seam with one present role (the clinical form) and one
  planned role (the starter). Its surface stays minimal until a second
  implementor exists.

## Overturning evidence

- A component family that needs keyed child identity across updates beyond
  `ListModel` (ADR 0055, PR #484). The `view` return shape then changes by
  revision.
- Measured update and view cost on the clinical form above the frame budget.
  That would move damage tracking into the component.
- A host that needs two UI threads. The `Rc`-based stores would then need a
  `Send` variant, and this ADR would be revised.
- A slice in which the projection-to-`TrackedDocument` mapping (open question 2)
  cannot be written with the existing edit methods. That would require a change
  to `TrackedDocument`'s contract.

## Open questions

1. Whether `Effects` needs a timer variant, or whether the host's finite wait
   (`run_native_application`, `crates/metis-platform/src/native/application.rs:146`)
   covers animation ticks alone.
2. How the host maps a projected `DomElement` onto `TrackedDocument` edits, so
   that unchanged content keeps today's repaint and layout caches. The first
   slice needs this answer.
3. Whether multi-window applications share one component tree or own one each.
