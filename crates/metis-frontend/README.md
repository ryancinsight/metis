# metis-frontend

Declarative form state, software rendering and correlated IPC requests. The
dependency closure excludes backend calculation and audit storage. The current
binary exchanges requests over inherited pipes; native window and operating-system
sandbox integration remain unfinished.

`FrontendApp` owns its inputs, document and framebuffer. `set_inputs(...)?`
clears any previous result and paints the edited form immediately. `state()`
returns one outcome: idle, pending, success, peer rejection, preparation failure,
disconnection or handshake failure. Only success carries a backend response.
Read `inputs()`, `document()` and `framebuffer()` without mutating this association.

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
let display = metis_ui_lang::compute_layout(&document, 800, 600)?;
// Submit the resulting display list to an Iris rendering backend.
# Ok::<(), metis_core::MetisError>(())
```

The renderer accepts the documented [presentation subset](../metis-ui-lang/README.md).
Backend responses carry a MAC which this frontend does not verify. See the
[process contract](../../docs/INTERFACE.md) and [verification](../../docs/VERIFICATION.md).
This package is unpublished.
