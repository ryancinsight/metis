# Run the browser workbench

The browser workbench is the first executable HTML5/CSS host. The document,
external stylesheet and module bootstrap come from `examples/browser/`; Rust
owns the controls, captured inputs and event transitions through `metis-web`.
Moirai owns the DOM handles, WebSocket callbacks and listener lifetimes. The
page contains no backend key and does not perform the privileged clinical
calculation in downloaded WASM.

The page carries the same strict content-security policy as
`metis_core::HostPolicy`. The policy source is
`crates/metis-core/src/content_security_policy.txt`; the browser build checks
the HTML asset against that source before copying it. Scripts, styles and form
actions are same-origin. The development policy also admits the explicit
loopback WebSocket endpoint used by the service command; production hosts must
replace that source with their authenticated endpoint. The generated WASM
loader is permitted by `'wasm-unsafe-eval'`, and plugins are disabled. The
bootstrap also cancels cross-origin anchor navigation as a defense-in-depth
check.

The `frame-ancestors 'none'` directive is present for a host that delivers the
policy as an HTTP response header. Browsers do not enforce `frame-ancestors`
from a document meta tag, so the current static workbench has no framing
enforcement until its native or service host supplies that header. A host must
also enforce origin, session and operating-system policy at its boundary;
downloaded page code is not an authority source.

## Build and serve

Install the pinned `wasm-bindgen-cli` version matching the workspace's
`wasm-bindgen` dependency. The build script checks for exactly 0.2.128 and also
discovers this ignored local root, then run:

```text
cargo install wasm-bindgen-cli --version 0.2.128 --locked --root output/wasm-bindgen-cli
python scripts/browser.py build
python -m http.server 8080 --directory output/browser
```

Open `http://127.0.0.1:8080/` in a browser for the disconnected local-control
workflow. A file URL is not accepted because module and WASM loading require an
HTTP origin.

## Connect the real browser service

The service executable is the same `metis-app` image used by the native
demonstration. It accepts one loopback WebSocket session, checks the request
`Origin` before sending `101 Switching Protocols`, constructs a trusted
`HostContext`, and then runs `AsyncIpcServer` with bounded messages and finite
deadlines. Start it in a second terminal while the static server is running:

```text
cargo run --locked -p metis-app -- --metis-browser-service http://127.0.0.1:8080 8765 66666666666666666666666666666666
```

Open the workbench with host configuration in the URL (the bootstrap copies
these values into the hidden configuration fields before Rust mounts the app):

```text
http://127.0.0.1:8080/?endpoint=ws%3A%2F%2F127.0.0.1%3A8765%2Fsocket&process=42&principal=66666666666666666666666666666666
```

The endpoint, process identifier and 32-hex-digit principal are transport
configuration, not credentials. The service's trusted context and capability
signature remain authoritative. The demonstration is loopback-only and uses
`ws://`; it does not establish TLS server authentication.

## Exercise the real Rust state

Change the weight, concentration or dose fields. The result panel updates from
the Rust-owned `FormInputs` and the status returns to idle for each valid edit.
Enter a non-numeric value to observe the typed `ERR_NUMERIC_INSTABILITY`
display. With no service configuration, submit the form to observe
`ERR_CONNECTION_CLOSED`; this is an explicit disconnected result, not a local
calculation or a fabricated success.

The **View options** fieldset uses semantic HTML5 controls owned by the Rust
host. Uncheck **Show remote events** to hide the event status line while the
event remains received and validated; select **Volume rate** or **Drug mass
rate** to change the displayed metric; move **Result scale** with the arrow
keys or pointer to select a bounded 50–150% presentation preference. The
`options-state` text and `data-result-scale-percent` attribute expose the
Rust-owned state after each change. Tab through the labels, press Space on the
checkbox or radio, and use the range arrows to reproduce the keyboard path.

The captured service journey at revision
`5b7708606d14bcbf62e10925604eb45ebc0ddb86` used the Codex in-app
browser at 1280×720 CSS pixels and device scale 1.25. Pointer activation of
**Drug mass rate** changed the summary to `drug mass rate` and the result to
`Drug mass rate: 2.175000 mg/hr` without clearing the accepted backend result.
Pointer activation of **Show remote events** changed the event line to
`Remote events: hidden by preference` while retaining that metric. Two
keyboard **Right** presses on **Result scale** changed the semantic value to
`120` and the summary to `scale 120%`; the focused range received the visible
keyboard focus ring.

With the service configuration above, the status becomes `Authorized backend
session ready`, and the header lists the commands advertised by that service
(`host.capabilities`, `host.target_capabilities`, `session.heartbeat`,
`clinical.calculate` and `plugin.invoke`). It also displays the host target
platform and installed surfaces (`native-process` and
`browser-websocket` for this service), followed by the local WASM/DOM/CSS
surfaces. These values come from the versioned Metis capability and target
descriptors after the authenticated handshake; they are not page-provided
permission claims. Submit the defaults to observe the real backend response, then
change the fields to `80`, `4` and `0.75` and submit again. The result panel
must show the echoed patient reference and those exact formatted values. Set
weight to `0` to observe `Backend rejected request [0x3001]`; the previous
result is cleared before the rejection is displayed. After an accepted
calculation the header also shows `Remote event: clinical.result` with the
nonzero event identifier, audit sequence and typed rates. A rejected request
does not produce a clinical event and the status line says so.

Hosts can use the same catalog from Rust with the synchronous or asynchronous
IPC client:

```rust
let catalog = client.discover_capabilities()?;
assert!(catalog.supports(metis_core::MessageType::ClinicalCalcReq));
let target = client.discover_target_capabilities()?;
assert!(target.supports(metis_core::TargetCapability::BrowserWebSocket));
```

The target descriptor is evidence of the service boundary that accepted the
session. A Windows or Linux platform value does not claim a native window,
operating-system permissions, accessibility or IME support; those surfaces
appear only after their host providers are implemented and explicitly added.
Starting or stopping the WASM application advances a lifecycle generation.
Completions from a cancelled connection or an earlier mount are discarded
before they can restore state or render into the new DOM.
If a host rejects the handshake, the browser preserves the peer's exact
16-bit error code in the session-failed state; local transport failures remain
typed connection failures.

Local host events use `metis_ipc::EventHub<E, CAPACITY>`. Each subscription has
its own bounded queue; `publish` returns `ERR_QUEUE_FULL` instead of blocking,
and `unsubscribe` removes delivery before a later publication. Callers use
`Subscription::recv_timeout` with a finite deadline. Remote event wire types
are versioned, bounded, and rejected when the envelope version differs from the
negotiated contract. Native callers can send a typed unsolicited event
through the same frame boundary:

```rust
#[derive(Debug, PartialEq, Eq)]
struct Status(u16);

impl metis_core::EventCodec for Status {
    const NAME: &'static str = "host.status";

    fn encode(&self) -> metis_core::Result<Vec<u8>> {
        Ok(self.0.to_be_bytes().to_vec())
    }

    fn decode(bytes: &[u8]) -> metis_core::Result<Self> {
        let value = <[u8; 2]>::try_from(bytes)
            .map(u16::from_be_bytes)
            .map_err(|_| metis_core::MetisError::protocol(
                metis_core::ErrorCode::MalformedPayload,
                "Status event body must contain one big-endian u16",
            ))?;
        Ok(Self(value))
    }
}

let event = metis_core::RemoteEventPayload::from_event(1, &Status(3))?;
server.send_event(&event)?;
let event = client.recv_event()?;
assert_eq!(event.decode_as::<Status>()?, Status(3));
```

Hosts register extension metadata through a typed, bounded registry. The
manifest is static, so registration cannot add an unbounded callback list or
grant an operating-system privilege. Each operation names the capability that
the host must verify before a handler runs:

```rust
use metis_core::{CapabilityScope, MAX_PLUGINS, Plugin, PluginDescriptor,
    PluginOperation, PluginRegistry};

static EVENTS: [PluginOperation; 1] = [PluginOperation::new(
    "result.received",
    CapabilityScope::STREAM_TELEMETRY,
)];
struct ViewerExtension;
impl Plugin for ViewerExtension {
    const DESCRIPTOR: PluginDescriptor =
        PluginDescriptor::new("viewer", 1, &[], &EVENTS);
}

let mut plugins = PluginRegistry::<MAX_PLUGINS>::new()?;
plugins.register::<ViewerExtension>()?;
assert_eq!(
    plugins
        .get("viewer")
        .expect("invariant: registered manifest is present")
        .version(),
    1,
);
```

The registry is host-local metadata, and the backend can invoke a declared
plugin command through the same authenticated bridge. A host implements both
the static manifest and `metis_backend::PluginExecutor`, then registers it
before accepting the session. The executor owns the body schema; the service
checks the command's declared scope against the host-bound token before calling
it. The sync and async clients expose the same typed entry point:

```rust
let invocation = metis_core::PluginInvocationPayload::new(
    session_token,
    "viewer",
    "open",
    encoded_viewer_request,
)?;
let response = client.invoke_plugin(&invocation)?;
let body = response.body();
```

The native Moirai WebSocket loopback in the verification suite sends the same
typed invocation after the clinical event. Its registered `websocket.increment`
executor transforms `[2, 5, 10]` into `[3, 6, 11]`, proving that plugin-owned
body bytes cross the authenticated framed transport. The browser workbench
currently advertises the plugin command but does not invoke it from a page
control.

Unknown plugins and commands, malformed body envelopes, insufficient scopes and
executor failures remain typed responses. Invocation only reaches the
registered host executor; it does not grant file, network, process or other
operating-system authority. The generated workbench registers a `workbench`
manifest during each Rust mount and displays its version in **Registered
frontend extensions**, so stop/start teardown also recreates the typed
registration state.

`AsyncIpcClient::recv_response_for` retains unsolicited events in a bounded
queue while it waits for its request. Browser code calls
`AsyncFrontendApp::recv_event().await` after the accepted calculation; it
decodes `clinical.result` and checks the event body against the correlated
response before rendering. The native WebSocket service and the browser
workbench therefore exercise the same event envelope and typed codec.

The page's **Stop host** control calls the generated `metis_stop` export. The
Rust host cancels the active browser task, drops its listener guards and
replaces `#metis-app` with `Metis browser host stopped.`. **Start host** calls
`metis_start` again and mounts fresh controls. With the service still running,
the new host reconnects; if the one-shot service has exited, the status reports
`ERR_TRANSPORT_BROKEN` and the page remains usable. Restart the service and
start the host again to demonstrate recovery. This exercises the same WASM
module's listener and task teardown rather than a browser reload.

Capture the initial, successful, rejected, stopped and recovered states with the
browser's native screenshot tool. Record browser engine, viewport, device
scale, font environment, service command and WASM revision beside the images.
The verified local trace used the Codex in-app browser at 1280×720 CSS pixels
and device scale 1.25; its engine version was unavailable. The accepted default
calculation displayed `Remote event: clinical.result #4` with audit sequence 4,
rate `0.543750 ml/hr` and drug rate `2.175000 mg/hr`. Selecting **Drug mass
rate** then displayed `Drug mass rate: 2.175000 mg/hr`; hiding events and moving
the range to 120% preserved the result. The zero-weight request displayed
`Backend rejected request [0x3001]` and `Remote events: none (request
rejected)`. Stop/start teardown and service restart restored `Authorized backend
session ready`. The tab's console contained only the expected Moirai
initialization log entries and no warnings or errors. These captures establish
HTML5/CSS execution and focusable controls and pair with the native loopback
tests. They do not close post-drop allocation measurement, cross-engine
behavior, TLS, accessibility technology support or OS permission isolation.
