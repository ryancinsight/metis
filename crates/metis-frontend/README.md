# metis-frontend

Declarative form state, software rendering and correlated IPC requests. The
library dependency closure excludes backend calculation and audit storage. The
`metis-app` executable composes both libraries and runs this presentation in a
child process exchanging requests over inherited pipes. Its shared executable
image contains backend code, but the child role creates no backend key or service.
The Windows native role presents the same framebuffer through the Moirai window
provider; operating-system sandbox integration remains a host concern.

`FrontendApp` owns its inputs, document and framebuffer. `set_inputs(...)?`
clears any previous result and paints the edited form immediately. `state()`
returns one outcome: idle, pending, success, peer rejection, preparation failure,
disconnection or handshake failure. Only success carries a backend response.
Read `inputs()`, `document()` and `framebuffer()` without mutating this association.

Native hosts keep preedit text transient with `set_composition(...)` and commit
the final UTF-8 value through the same bounded `set_inputs(...)` transition used
by ordinary text input. Composition values are capped at 128 UTF-8 bytes and
are cleared when inputs change or focus leaves the native surface.

The authored surface also carries a host-neutral local command menu. `FrontendApp`
owns its `CommandMenuState`, `ApplicationCommand` transitions and bounded
`ApplicationTheme`; native and browser hosts can expose the same focus-patient
and dark/system actions without sending presentation commands through backend
IPC. Menu visibility is projected through the authored semantic tree, so hidden
items cannot receive host actions. Each command also declares one keyboard
accelerator (`ApplicationCommand::accelerator`, Alt+Shift+P/D/S); hosts resolve
their key events through `metis_core::input` and call
`FrontendApp::activate_shortcut`, and the authored menu announces the same
bindings through `aria-keyshortcuts`.

`AsyncFrontendApp` owns the same input/result state for a browser event loop.
It accepts `metis_ipc::AsyncIpcTransport`, sends at most the bounded requests
allowed by the asynchronous client, and changes to a typed disconnected state
when a dispatched response cannot be trusted. Its `init` and
`submit_calculation` methods are futures; they never block the browser thread.
Initialization also obtains the host's versioned command catalog through
`AsyncIpcClient::discover_capabilities`; the validated catalog is available
from `capabilities()` before a command is submitted. It also obtains the
versioned target descriptor through
`AsyncIpcClient::discover_target_capabilities`; callers read it from
`target_capabilities()` and can reject a surface that the host did not install.
`cancel_pending_requests` clears correlation entries left by a cancelled task
and returns the form to idle without presenting an obsolete result.
After a successful calculation, `recv_event().await` (or the synchronous
`FrontendApp::recv_event`) consumes the backend's typed `clinical.result`
event. Event receipt is a separate bounded operation so hosts can choose when
to update an event view.

`ResultExplorer` owns a bounded history of typed backend responses for table
and tree views. `record_response` validates and inserts a response,
`set_filter` and `set_sort` rebuild the visible projection, and
`selected_id` remains stable while a row is retained. `visible_entries()`
yields at most `RESULT_PAGE_SIZE` borrowed group or row entries for a native
or browser renderer without allocating a second result list.

```rust
use metis_frontend::{FormState, FrontendApp};
let (transport, peer) = metis_ipc::MemoryTransport::pair();
let mut app = FrontendApp::new(transport, 800, 600)?;
app.set_inputs("demo", 60.0, 2.0, 0.2)?;
assert_eq!(app.state(), &FormState::Idle);
// A backend session must complete init before submission.
assert_eq!(app.submit_calculation().expect_err("no session").code,
           metis_core::ErrorCode::MissingCapability);
# drop(peer);
# Ok::<(), metis_core::MetisError>(())
```

Input mutation cannot bypass invalidation:

```compile_fail
let (transport, _) = metis_ipc::MemoryTransport::pair();
let mut app = metis_frontend::FrontendApp::new(transport, 800, 600).unwrap();
app.inputs.weight_kg = 80.0;
```

A preparation failure retains the session; a failure after dispatch closes it
because backend execution may already have occurred. Construct a new app with a
new authenticated transport to reconnect. Peer rejection preserves its exact
wire code and diagnostic; it does not automatically mean a clinical rejection.
Pending is rendered before synchronous IPC and does not imply responsive
asynchronous input. See [state ownership and API migration](../../docs/adr/0004-form-state.md).

```rust
use metis_frontend::CLINICAL_SCREEN_XML;
let document = metis_ui_lang::parse_markup(CLINICAL_SCREEN_XML)?;
let display = metis_ui_lang::compute_layout(
    &document,
    metis_ui_lang::LayoutViewport::new(800, 600),
)?;
// Submit the resulting display list to an Iris rendering backend.
# Ok::<(), metis_core::MetisError>(())
```

The renderer accepts the documented [presentation subset](../metis-ui-lang/README.md).
Backend responses carry a MAC which this frontend does not verify. See the
[process contract](../../docs/INTERFACE.md) and [verification](../../docs/VERIFICATION.md).
This package is unpublished.

## Reactive stores

`metis_frontend::reactive` provides Svelte-style stores for Rust-owned state.
A `Writable` notifies subscribers when its value changes, `derived` computes a
`Readable` from another store, and each `Subscription` unsubscribes when it is
dropped. Listeners may set stores during delivery; a change cascade longer
than `MAX_CASCADE` rounds is cut off and reported instead of hanging a frame.

```rust
use metis_frontend::reactive::{Store, Writable, derived};

let dose = Writable::new(0.5_f64);
let label = derived(&dose, |mcg| format!("{mcg:.2} mcg/kg/min"));
let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
let sink = seen.clone();
let subscription = label.subscribe(move |text| sink.borrow_mut().push(text.clone()));
dose.set(0.75).expect("no runaway cascade");
assert_eq!(*seen.borrow(), ["0.50 mcg/kg/min", "0.75 mcg/kg/min"]);
drop(subscription);
```

`derived2` computes from two stores, as a Dioxus memo over two signals does.
`Writable::project` gives a writable view of one field that notifies only when
that field changes, like a Dioxus store field. `resource` holds the outcome of
asynchronous work started for each source value, like `use_resource`: the
caller runs the work on any executor and hands the result to a one-shot
`Completion`, which is ignored once the source has moved on.

```rust
use metis_frontend::reactive::{ResourceState, Writable, derived2, resource};

#[derive(Clone, PartialEq)]
struct Infusion { weight_kg: f64, rate: f64 }

let infusion = Writable::new(Infusion { weight_kg: 70.0, rate: 0.5 });
let weight = infusion.project((|i| &i.weight_kg, |i| &mut i.weight_kg));
let rate = infusion.project((|i| &i.rate, |i| &mut i.rate));
let dose = derived2(&weight, &rate, |kg, rate| kg * rate);
weight.set(80.0).expect("no runaway cascade");
assert_eq!(dose.get(), 40.0);

let lookup = resource(&dose, |value: &f64, done| {
    // Start real work here; this example completes at once.
    let _ = done.complete(Ok::<_, String>(format!("{value} mcg/min")));
});
assert_eq!(lookup.get(), ResourceState::Ready("40 mcg/min".to_owned()));
```

## Navigation

`metis_frontend::navigation::Navigator` pairs a `metis_core::route::Router`
with the current route as a store, so views derive from navigation like any
other state. A host checks a path before committing it to its history
(`check` refuses paths no route matches) and reports every path it arrives
at (`arrive`), including back/forward and typed URLs; an unmatched arrival
sets the current route to `None`. The browser host's `BrowserNavigator`
drives it from the page history.

```rust
use metis_core::route::Router;
use metis_frontend::navigation::Navigator;
use metis_frontend::reactive::derived;

let mut router = Router::new();
router.add("/study/:id", "study")?;
let navigator = Navigator::new(router, "/").expect("decodable path");
let title = derived(&navigator, |current| {
    current.as_ref().and_then(|route| route.parameter("id").map(str::to_owned))
});
navigator.arrive("/study/7").expect("decodable path");
assert_eq!(title.get().as_deref(), Some("7"));
# Ok::<(), metis_core::route::RouteError>(())
```
