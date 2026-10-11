# 0060 — General component lifecycle for application state and views

Status: Proposed

Date: 2026-10-11

Driver: METIS-COMPONENT-001. ADR 0003 records the general component lifecycle
as open (application-state row, ADR 0003:525).

## Context

The native host seam exists. `NativeApplication` (`metis_platform::native`)
supplies the framebuffer, damage, the optional accessibility tree and one event
batch that returns a host action. `NativeForm<T>` implements it
(`crates/metis-app/src/frontend/native.rs:70`). Its state is `FrontendApp<T>`
(`crates/metis-frontend/src/app.rs`), which owns a fixed `FormInputs` clinical
record and a `FormState`, and renders into a `TrackedDocument`
(`crates/metis-frontend/src/document.rs:13`).

The browser host is separate. `AsyncFrontendApp`
(`crates/metis-frontend/src/async_app.rs:18-24`) owns the same `FormInputs` and
`FormState` with no document. `metis-web` uses neither `DomElement` nor
`TrackedDocument`. The starter (`crates/metis-starter`) is a wasm32-only cdylib.

No reusable declaration names an application's state, messages and view. The
reactive stores (`Writable`, `derived`, `resource`, `ListModel`) are the state
primitives. `TrackedDocument` changes through `set_text`, `set_attribute` and
`restyle` (`document.rs:39-52`). `set_text` replaces an element's children with
one text node (`crates/metis-ui-lang/src/dom.rs:112-115`), so none of the three
edits inserts element children.

Comparators:

- Iced 0.14: Elm-style state, messages, `update` and `view`, with `Task` and
  `Subscription` for asynchronous work (ADR 0003, Iced section).
- Freya 0.5.0-rc.10: hooks (`use_state`, `use_memo`, `use_hook`). `use_hook`
  keeps values in per-scope positional storage
  (`freya-core/src/lifecycle/base.rs:76-101`). A run that calls more hooks than
  the first run, or a hook whose stored type differs, panics with `HOOKS_ERROR`
  (`base.rs:87-89`). Fewer hooks, or reordered hooks of one type, pass unchecked.
- egui: immediate-mode widgets and responses (ADR 0003:525).
- GPUI: entities, views and actions (ADR 0003:525).
- Tauri: the frontend framework supplies widgets and state (ADR 0003:525);
  commands and events (ADR 0003:534; ADR 0012:19-20).
- htmx: the event, request, target and swap model (ADR 0003:531); the
  typed-patch subset is admitted by ADR 0022.

Slint is not compared here. Its comparison is in the ADR 0003 text on PR #488,
which is not on main.

## Decision

Adopt a host-neutral component trait in `metis-frontend`. A component's state is
its own fields, its messages are an enum per component, and its view is a
`DomElement` value (`crates/metis-ui-lang/src/dom.rs:8`).

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

- `update` is infallible. A refusal of domain input is recorded in state and
  rendered by `view`. `set_composition` returns `PayloadTooLarge` today
  (`app.rs:238-246`); under this contract that rejection is state.
- `Effects` has two variants: an IPC request and a message to self. Whether a
  timer variant exists is open question 1.
- An effect that completes after its component is dropped, or after its source
  value changed, is discarded, as `resource`'s `Completion::is_current` does
  (`crates/metis-frontend/src/reactive/resource.rs:43`).
- A view is a value. Tests assert on the `DomElement` a component returns.
- This decision does not cover the path from a `DomElement` to a
  `TrackedDocument` or the native paint path (open question 2), a component that
  holds a list (open question 3), or the browser host (open question 4).
  `NativeForm<T>` and `FrontendApp<T>` keep their current paths until those
  questions are answered.
- Re-entrant dispatch needs a bound. This ADR does not name one; slice 1 names
  it and its typed refusal.
- No `dyn Component`. A shell that selects components at run time enumerates a
  closed set, and each component stays monomorphized.

## Rejected alternatives

- Hooks (Freya). Hook state is identified by call order. `base.rs:87-89` panics
  on a longer sequence and on a stored-type mismatch, but a shorter or reordered
  sequence of one type passes unchecked. A component names its state in fields,
  so no call order is involved.
- Immediate mode (egui). Its widgets return responses from per-frame calls
  (ADR 0003:525). A response is not a value a test can compare. A `DomElement` is
  such a value, so the contract trades the per-frame model for testability.
- Entity and view handles (GPUI). `update` takes `&mut self`, so one owner
  already serializes changes. Handles would add shared ownership the contract
  does not need. This is a design argument; GPUI's own design is unverified here.
- Status quo. `FrontendApp<T>` and `AsyncFrontendApp<T>` each own the fixed
  `FormInputs` record and `FormState`. The decision leaves both in place until
  open questions 2 and 4 are answered, so the duplicate remains meanwhile.
- The archived `iced_web` DOM runtime. It is read-only upstream (ADR 0003).

## Consequences

- The contract is a new seam with one present role, the clinical form on the
  native host. The starter is a planned role only after the browser host has a
  DOM path (open question 4).
- Closure of the message set is a convention. `type Message: Clone + 'static`
  admits any type. Closure comes from each component declaring an enum and
  `update`'s `match` covering every variant. The type system does not forbid a
  non-enum message.
- Slice 1 is the trait, the two-variant `Effects`, and the generic test set
  `<C: Component>` run on the software host, which paints `DomElement` through
  `metis-ui-lang`, with a test component. Slice 1 does not need open question 2.
- The clinical form migration needs open question 2 and is a later slice.
- Each monomorphization that ships is tested. The test set is not written yet.

## Overturning evidence

- A component family that needs keyed child identity across updates beyond
  `ListModel` (ADR 0055, PR #484, not yet on main). The `view` return shape then
  changes by revision.
- Measured update-plus-view time for the clinical form above 16.7 ms, one 60 Hz
  frame (ADR 0043:80). That would move damage tracking into the component.
- A measured full replacement of the clinical form's `DomElement` tree into the
  native paint path above 16.7 ms. That would require an incremental edit set and
  reopen open question 2.
- A host that needs two UI threads. The `Rc`-based stores would then need a
  `Send` variant, and this ADR would be revised.

## Open questions

1. Whether `Effects` needs a timer variant. The native host's finite wait
   (`run_native_application`, `crates/metis-platform/src/native/application.rs:146`)
   covers animation ticks today. The browser host has none in this design.
2. How a `DomElement` reaches a `TrackedDocument` and the native paint path.
   `TrackedDocument` has no operation that inserts element children
   (`document.rs:39-52`, `dom.rs:112-115`), so a new edit or a full replacement
   is needed. A follow-up ADR decides it.
3. How a component that holds a `ListModel` projects its window.
   `VirtualListView::dom` returns `Result` and takes `&mut self`
   (`crates/metis-frontend/src/list.rs:156-161`, PR #484), but `view(&self)` is
   infallible. The projection can also fail with `LayoutOverflow`
   (`list.rs:244-256`), and a shared `ListModel` can change between `update` and
   `view`.
4. How the browser host gains a DOM path that the contract's view can target,
   and whether the starter then migrates.
5. Whether multi-window applications share one component tree or own one each.
