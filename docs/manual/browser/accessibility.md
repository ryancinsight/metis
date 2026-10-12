# Browser accessibility

[Browser manual](../browser.md)


## Exercise the accessibility presentation

The workbench keeps the normal keyboard path in document order. From the
header, press **Tab** through **Session details**, the patient and numeric form
fields, **Submit to authorized backend** when the authorized bridge enables it,
the **View options** controls, the named **Pointer capture surface**, the named
**File drop zone**, and **Clinical note**. A disabled submit control is
skipped by the browser, and a radio group has one tab stop; use its arrow keys
to choose the other unit. The closed session dialog is not in the active tab
order.
Opening **Session details** uses the browser dialog semantics; **Close** is the
dialog action and focus returns to the opener after dismissal. Status and
composition regions use polite, atomic live announcements, while labels and
headings provide names for each control group. The form and result status set
`aria-busy="true"` during an in-flight backend request and return it to
`"false"` when the result settles; the explorer table follows the same state
for a loading result page.

The stylesheet responds to the user's presentation preferences. With
`prefers-reduced-motion: reduce`, scrolling is immediate and transitions or
animations resolve to a single short frame. With `forced-colors: active`, cards
and controls use system `Canvas`, `CanvasText`, `ButtonFace`, `ButtonText` and
`Highlight` colors so borders and focus indicators remain visible. Browser zoom
remains available; the responsive grid stacks below `700px` and keeps content
inside the `960px` bound.

The static browser asset contract checks the semantic names, live regions,
atomic announcements and busy-state wiring,
non-positive focus order and both media-query branches. The W3C runner's
`--accessibility-probe` records the observed media flags, focus sequence,
viewport scale and bounded geometry alongside its PNG captures. For a host
acceptance run, enable a supported screen reader, reduced-motion setting,
forced-colors setting and browser zoom, then capture the accessibility tree,
focus ring and spoken action for each state. Record the browser engine,
operating system, scale factor and assistive-technology version. A semantic
tree or CSS rule by itself does not establish screen-reader support or an
operating-system accessibility bridge.

The same probe records a bounded semantic inventory for the workbench. It
requires the application root, form, session dialog, submit action, file input,
clinical-note textarea and result table to retain their expected DOM roles and
stable IDs across input and stop/remount transitions. It records `aria-busy`,
live-region, expanded, popup, disabled and dialog-open states without copying
form values. This verifies the browser semantic contract; supported
screen-reader speech and native accessibility bridges still require host runs.

On Chromium and Microsoft Edge, the same `--accessibility-probe` also asks the
vendor WebDriver endpoint for `Accessibility.getFullAXTree` at depth eight. The
runner bounds the response by bytes, checks that the visible `main`, `form`, `button`, `textbox`
and `table` roles and the **Submit to authorized backend**, **Choose files**,
**Clinical note** and **Result explorer** names are present, and stores only
role/count/name-presence data in
`metrics.accessibility_native_tree`. Firefox and WebKit record an explicit
`unavailable` result because they do not expose that Chromium protocol through
the configured driver. The tree is evidence of the browser's accessibility
projection; the bounded depth retains the required workbench roles/names while
ignored text descendants remain inside the byte-bounded response. It does not
claim spoken screen-reader output or an operating-system accessibility bridge.

The same Rust semantic contract covers application command surfaces. A host
may project `nav`/`navigation` into a navigation landmark, `aside`/`complementary`
into a sidebar, `toolbar` into a title bar, and `menu` with `menuitem` children
into a dropdown command surface. Menu items expose the typed `Activate` action;
hidden ancestors and disabled items expose no focus or action. This keeps menu
state in the Rust presentation model and leaves browser, UIA, `NSAccessibility`
and AT-SPI translation at their host boundaries. The Windows native path uses
the matching Moirai PAL role contract from PR [#432](https://github.com/ryancinsight/Moirai/pull/432)
at merge `0e2e1bbb2d81e16dd9c694ba46a9e9710e034417`;
the projection is tested in `metis-ui-lang` and `metis-app`. These checks do not
claim spoken screen-reader output or native accessibility coverage on every
host.

The shipped browser workbench now exercises that command surface directly. Its
Rust-owned header contains an application navigation landmark, a command
toolbar, and a menu with **Focus patient reference**, **Use dark theme**, and
**Use system theme** actions. The menu trigger and menu state expose
`aria-expanded`, `aria-hidden` and `data-command-menu-open`; the same state
drives the responsive CSS presentation. Selecting a theme updates the existing
validated `theme-mode` control and the page palette, while the focus command
returns focus to **Patient reference**. Escape closes the menu and returns focus
to its trigger. No command state is held in page JavaScript. The focused
browser asset contract checks the exact landmark and action IDs; the WASM
`metis-web` check and strict Clippy gate compile the listener and presentation
path. These checks establish the Rust/DOM contract, not native tray,
notification, global-shortcut or screen-reader behavior.

### Decode the shipped image marks

Build the browser output, then add `--asset-probe` to a workbench run. The
probe loads the local SVG and PNG marks through `HTMLImageElement.decode()`,
requires same-origin URLs and finite intrinsic dimensions, and removes its
probe container before the lifecycle trace continues:

```powershell
python scripts/browser.py build
python scripts/browser_runtime.py --engine chromium `
  --driver-url $env:METIS_WEBDRIVER_CHROMIUM_URL `
  --serve-dir output/browser --bridge disconnected --asset-probe `
  --output output/browser/runtime/chromium-assets.json
```

The resulting `metrics.assets` records the source path, decoder, intrinsic
width and height for both marks at each captured lifecycle remount. A decode
error, cross-origin redirect, unsupported decoder or leaked probe element
fails the run; no DICOM bytes or clinical image metadata enter this probe.

### Probe same-origin font loading and teardown

Add `--font-load-probe` to exercise the local WOFF2 fixture through the browser
`FontFace` and `FontFaceSet` APIs. The probe loads
`examples/browser/assets/metis-probe.woff2` as `Metis Probe`, confirms the
loaded face changes the measured width of `AB01`, checks the family with
`FontFaceSet.check()`, deletes the face, and then tries the shipped SVG as an
invalid font resource. Each lifecycle observation requires the registration
count to return to its baseline and bounds every path, family, sample, reason
and pixel measurement:

```powershell
python scripts/browser.py build
python scripts/browser_runtime.py --engine chromium `
  --browser-name MicrosoftEdge `
  --driver-url http://127.0.0.1:9519 `
  --serve-dir output/browser --bridge disconnected --font-load-probe `
  --lifecycle-cycles 2 `
  --output output/browser/runtime/edge-font-loading-20260918.json
```

At revision `c936585423d2c4a383f858303ccfcd154c3d0eb3`, Edge
154.0.4258.12 at device scale 1.25 passed three observations. The loaded
face measured 308 px versus the 158 px sans-serif fallback, reported
`status=loaded` and `check=true`, and returned the font-set count to zero after
teardown. The invalid resource was rejected with a bounded network error and
was not registered. The trace SHA-256 is
`c97981885eedde369855eeb894c6a3bff9714c44facfad78414c536f4cd406a2`.

This is browser font-resource lifecycle evidence for one engine. Native font
providers, selected fallback-face identity, cross-engine metrics and the RITK
patient-image surface remain separate V06 work.

### Probe media error and teardown

Add `--media-probe` to the same workbench run to exercise the browser's error
path for one bounded audio fixture and one bounded video fixture. The fixtures
are intentionally truncated `data:` URLs, so a successful decode is a failure
of the probe. For each element the trace records the `MediaError` code and
bounded message, removes the `src`, calls `load()` to release the resource,
and requires `readyState=0`, `networkState=0` (`NETWORK_EMPTY`) or `3`
(`NETWORK_NO_SOURCE`), an empty `currentSrc`, no `src` attribute, a detached
element and zero remaining probe nodes. The probe does not claim media playback
or clinical image decoding:

```powershell
python scripts/browser.py build
python scripts/browser_runtime.py --engine chromium `
  --driver-url $env:METIS_WEBDRIVER_CHROMIUM_URL `
  --serve-dir output/browser --bridge disconnected --media-probe `
  --lifecycle-cycles 2 `
  --output output/browser/runtime/chromium-media.json
```

The trace's `metrics.media` entry is repeated after each lifecycle remount.
An absent error event, a successful decode, a retained source, an attached
element or a leaked probe container fails the run. This is the browser-side
error and resource-release contract; native media providers and playback
controls remain target-specific V06 work.

### Probe audio playback controls

Add `--media-playback-probe` to exercise a real same-origin audio resource and
its play/pause lifecycle. The fixture is the 1,644-byte PCM file
`examples/browser/assets/metis-tone.wav`, copied to `/assets/metis-tone.wav`
by the browser build. The probe opens the existing **Session details** control
through WebDriver before calling `play()` so the browser's user-gesture policy
is part of the evidence:

```powershell
python scripts/browser.py build
python scripts/browser_runtime.py --engine chromium `
  --browser-name MicrosoftEdge `
  --driver-url http://127.0.0.1:9517 `
  --serve-dir output/browser --bridge disconnected --media-playback-probe `
  --lifecycle-cycles 2 `
  --output output/browser/runtime/edge-media-playback-20260918.json
```

Each `metrics.media_playback` observation requires the same-origin fixture,
`loadedmetadata`, `canplay`, `playing` and `pause` events, a positive 0.2
second duration, controls enabled, and the expected paused transitions. The
probe then pauses the element, removes its source, calls `load()`, waits for
`emptied`, removes the element and requires `readyState=0`,
`networkState=0` or `3`, a null `src` attribute, a detached element and zero
remaining probe nodes. Chromium retains `currentSrc` as a resolved URL in
this empty state; the validator accepts it only when its path is exactly
`/assets/metis-tone.wav`, so this evidence does not claim that the read-only
URL reflection is cleared. The trace records three observations across the
initial mount and two remounts. At revision
`9aaf7c05adaf09f41f2c96b62271d7d5fc8b17b8`, Edge 154.0.4258.12 at device
scale 1.25 produced trace SHA-256
`d25c1e961a6855e4cdb42780a931752b9cd4658d5aa73ca9739158f3b963eb56`.

This closes browser audio playback controls. Video playback, native media
providers, cross-engine playback traces and the RITK clinical image surface
remain target-specific V06 work.

The captured service journey at revision
`d879779247c8cfc5870f62f99a5364cbbf2d3c58` used the Codex in-app
browser at 1280×720 CSS pixels and device scale 1.25. Pointer activation of
**Drug mass rate** changed the summary to `drug mass rate` and the result to
`Drug mass rate: 2.175000 mg/hr` without clearing the accepted backend result.
Pointer activation of **Show remote events** changed the event line to
`Remote events: hidden by preference` while retaining that metric. Two
keyboard **Right** presses on **Result scale** changed the semantic value to
`120` and the summary to `scale 120%`; selecting **Audit detail** changed the
semantic select value to `Audit detail`, changed the annotation to
`Audit detail: sequence 4`, and retained the result. The focused range received
the visible keyboard focus ring.

The header's **Session details** button exercises the native HTML dialog path.
Activate it after the host reports ready. Rust calls the Moirai `show_modal`
seam; the dialog exposes the current session status and capability summary as
text and keeps browser focus on its **Close** button. Activate **Close** to call
the Rust `close_dialog` seam. The native `close` event restores focus to
**Session details** through Moirai's `focus` seam. Press **Escape** to exercise
the browser's cancel path; it closes the dialog and restores the same opener.
The dialog open-state read is used to avoid duplicate modal transitions.

The 2026-09-07 trace at the same 1280×720 CSS-pixel viewport and device scale
1.25 captured the modal backdrop and focus ring, matched the ready status and
capability text in the accessibility tree, verified the `open` attribute while
modal and its absence after both close paths, and observed focus return to the
opener. It used Moirai revision
`8f02b8b7de6cf6361b519bd79759d8508568fbdb`.

The same service trace verified the submit lifecycle. Before the handshake, the
accessibility tree marked **Submit to authorized backend** disabled. Once the
bridge reported ready, the control became enabled. With the service started
using `--response-delay-ms 4000`, submitting changed the status to **Request in
progress** and marked the control disabled; after the delayed response, the
button became enabled and the result showed `Volume rate: 0.543750 mL/hr`.
On a disconnected workbench, attempting to activate the disabled control timed
out without changing the status or accessibility tree.

The 2026-09-08 service-backed CUA trace also inspected the live attributes
while the delayed request was in flight. `metis-status`, `metis-form`,
`result-state` and `explorer-table` all reported `aria-busy="true"` alongside
the **Request in progress** announcement. After the four-second response,
each returned to `"false"`; the tree exposed **Backend result received**, the
retained result row and the enabled submit control. This demonstrates that the
busy state follows the Rust pending/loading state and settles with the real
backend result in the browser engine.

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

The lifecycle status line is visible in the page while these transitions run.
For example, after stopping it reads `Lifecycle: stopped; Rust-owned listeners
released (0 listener handles; generation N)`. A fresh start replaces it with a
mount message that reports the positive listener-handle count and generation
`N + 1` (or a later generation when another transition occurred). The runtime
trace records both counts and generations in its cleanup object; the count is
Metis-owned evidence and does not include provider-private event registrations.

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

[Input Events]: https://w3c.github.io/input-events/
