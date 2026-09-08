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
