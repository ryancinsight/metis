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

The same generated assets include a format-neutral HTTP boundary probe. Start
the bounded service in a second terminal:

```text
cargo run --locked -p metis-app -- --metis-http-service http://127.0.0.1:8080 8766 66666666666666666666666666666666
```

Open `http://127.0.0.1:8080/http-health.html`. The page sends a real
cross-origin `GET /health`, performs a binary session handshake, dispatches a
generation-bound fragment action and displays the returned text patch. It then
probes malformed, unauthorized and stale-generation requests; the stale probe
must leave the previous text unchanged. The page is an HTML5/CSS demonstration
of the presentation boundary; it does not load, decode or retain DICOM data.
RITK owns that workflow.

To exercise cancellation at the same boundary, start the service with a
bounded asynchronous response delay:

```text
cargo run --locked -p metis-app -- --metis-http-service http://127.0.0.1:8080 8766 66666666666666666666666666666666 --response-delay-ms 4000
```

Activate **Reset mount** while the initial probe is pending. The new
generation must retain the reset status, empty response, fragment and negative
diagnostics after the four-second response window; no completion from the
aborted request may update the remounted page. The probe is bounded to 30,000
milliseconds and uses Moirai's timer, so it does not block the executor.

The in-app browser capture on 2026-09-11 at the pre-change Metis revision
`cadb684ca7c8bda3f1c873f93286871e620e6196` showed the complete live state:
`Authenticated fragment boundary ready`, `200 metis-http-ready`, handshake
`200`, fragment `200 (1 patch)`, accepted `session` action, malformed `400`,
unauthorized `401`, unchanged stale state and lifecycle generation `1`. This
is one real browser-rendered visual trace of the local presentation boundary;
it is not cross-engine WebDriver evidence. No DICOM data entered the page.

The delayed-reset capture was run against Metis revision
`e6b84432bc8a94515f0a332592ff392127788f00`. It entered the pending probe,
advanced from generation `1` to `2` on **Reset mount**, and remained at the
empty reset state after the four-second response window.

## Run the cross-engine conformance trace

The repository includes a dependency-free W3C WebDriver runner. It uses the
same Rust/WASM page and scenario for Chromium, Firefox and WebKit, then writes
one schema-1 JSON trace at `output/browser/runtime/<engine>-<scenario>.json` and PNG
screenshots under `output/browser/runtime/screenshots/<engine>/`. The browser
and its WebDriver endpoint are host or CI prerequisites; the runner does not
install a driver or add a JavaScript test runtime. Configure one endpoint per
engine, for example:

```powershell
$env:METIS_WEBDRIVER_CHROMIUM_URL = "http://127.0.0.1:9515"
$env:METIS_WEBDRIVER_FIREFOX_URL = "http://127.0.0.1:4444"
$env:METIS_WEBDRIVER_WEBKIT_URL = "http://127.0.0.1:4444"
```

Run the disconnected format-neutral workflow against the generated assets:

```text
python scripts/browser_runtime.py --engine chromium --serve-dir output/browser --bridge disconnected
python scripts/browser_runtime.py --engine firefox --serve-dir output/browser --bridge disconnected
python scripts/browser_runtime.py --engine webkit --serve-dir output/browser --bridge disconnected
```

Each command fails if its endpoint is missing, the page does not mount the
Rust-owned form, either of the two input changes is not reflected in the DOM,
or the stop/remount generation retains application controls or an old result.
Waits run inside the browser with a `MutationObserver` or one bounded timer;
the host does not sleep or poll. The trace includes the negotiated browser
capabilities, exact actions and observed values, semantic snapshots, screenshot
hashes/dimensions, the three explicitly unsupported native operations and
cleanup evidence. Review the screenshots and the semantic states together.

The protocol client also provides the format-neutral physical-input seam used
by application-owned canvas scenarios. A consumer resolves its canvas element
through WebDriver and sends trusted pointer and wheel actions without placing
application state in Métis:

```python
from scripts.browser_protocol import WebDriverClient

client.pointer_drag(canvas_element_id, (24, 24), (64, 48))
client.wheel(canvas_element_id, (64, 48), (0, 120))
client.release_actions()
```

The client bounds element-local coordinates, wheel deltas, action-source count,
per-source action count and serialized request size. RITK owns the DICOM
workflow and interprets these format-neutral events; Métis records transport
and screenshot evidence only. A configured engine endpoint is required before
the action trace can claim Chromium, Firefox or WebKit evidence.

### Run a consumer-owned canvas trace

The same runner has a canvas scenario for an application that owns one or more
HTML5 canvases. It captures the full browser window and each named canvas,
records its intrinsic and CSS dimensions, sends one trusted pointer drag and
wheel action to each element, and releases all WebDriver input sources before
closing the session. The trace keeps the Metis revision and the optional
consumer revision separate; the consumer revision must be a 40-hex Git
revision when supplied.

For the RITK browser viewer, run the page that mounts the RITK-owned canvases
and pass their IDs. The runner has no DICOM knowledge and does not interpret a
slice, voxel, series or medical state:

```text
python scripts/browser_runtime.py --scenario canvas --engine chromium --url http://127.0.0.1:8080/ritk.html --consumer-revision <RITK-40-HEX> --canvas-id ritk-snap-axial --canvas-id ritk-snap-coronal --canvas-id ritk-snap-sagittal
```

Consumers may add a bounded `--canvas-attribute data-*` argument for each
opaque DOM attribute they own. The runner records the requested values under
each canvas snapshot, uses `null` when an attribute is absent, and never
interprets the name or value. At most 16 names are accepted and each returned
value is limited to 1024 UTF-8 bytes. This lets a consumer such as RITK carry
its own semantic evidence without moving that meaning into Metis:

```text
python scripts/browser_runtime.py --scenario canvas --engine chromium --url http://127.0.0.1:8080/ritk.html --consumer-revision <RITK-40-HEX> --canvas-id ritk-snap-axial --canvas-id ritk-snap-coronal --canvas-id ritk-snap-sagittal --canvas-attribute data-ritk-load-state --canvas-attribute data-ritk-frame-state --canvas-attribute data-ritk-axis --canvas-attribute data-ritk-slice-index --canvas-attribute data-ritk-slice-count --canvas-attribute data-ritk-frame-width --canvas-attribute data-ritk-frame-height
```

Repeat the command for `firefox` and `webkit` with their configured driver
endpoints. The resulting schema-1 trace uses `bridge: "canvas"`, records
`consumer_revision`, stores both window and element PNG hashes, and records
the requested opaque attributes under each canvas snapshot. RITK remains
responsible for the DICOM byte drop, viewer reducer, axis/slice assertions and
the clinical visual oracle. A
missing driver endpoint is an unfulfilled evidence requirement, not a passing
or skipped engine result.

For the authorized service path, keep the static server and service running,
then pass the host-provided session tuple in the URL. The runner validates the
`ws`/`wss` endpoint, decimal process identifier and 32-hex-digit principal
before navigation:

```text
python scripts/browser_runtime.py --engine chromium --driver-url http://127.0.0.1:9515 --url "http://127.0.0.1:8080/?endpoint=ws%3A%2F%2F127.0.0.1%3A8765%2Fsocket&process=42&principal=66666666666666666666666666666666" --bridge authorized
```

The query separators in a shell URL must remain `&`; the encoded endpoint is
shown only so the WebSocket value stays one query field. The authorized trace
waits for **Authorized backend session ready**, submits the real service
request, and asserts `Volume rate: 0.900000 mL/hr`. Add `--cancel` while the
service is running with `--response-delay-ms 4000` to stop and remount during a
pending response; the delayed response must not change the new generation:

```text
python scripts/browser_runtime.py --engine chromium --driver-url http://127.0.0.1:9515 --url "http://127.0.0.1:8080/?endpoint=ws%3A%2F%2F127.0.0.1%3A8765%2Fsocket&process=42&principal=66666666666666666666666666666666" --bridge authorized --cancel --cancel-grace-ms 4500
```

The browser runner does not open native file dialogs, launch native processes,
grant operating-system permissions or parse DICOM. RITK owns DICOM opening,
series selection, decoding, geometry and viewer state; see the
[RITK DICOM workflow manual](https://github.com/ryancinsight/ritk/blob/main/docs/manual/dicom-workflow.md). The
runner's cleanup statement means that the stopped DOM has zero mounted
controls, the remounted form reports `aria-busy=false`, and no stale completion
appears after remount. Provider-private
listener registries, OS handles, installed IMEs and post-drop allocations
require their owning Moirai/RITK/native evidence and are not inferred from a
browser screenshot.

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

## Verify stop/remount disposal at the service boundary

The browser service can delay a successful clinical response for a bounded
interval. This exercises the same WebSocket and `AsyncIpcServer` path as the
normal workflow while leaving handshake, rejection and event frames immediate:

```text
cargo run --locked -p metis-app -- --metis-browser-service http://127.0.0.1:8080 8765 66666666666666666666666666666666 --response-delay-ms 4000
```

With the configured workbench URL open, wait for **Authorized backend session
ready**, activate **Submit to authorized backend**, and immediately activate
**Stop host**. The stopped page must contain only `Metis browser host stopped.`
Start the host again before the four-second delay expires. The remounted form
must show its new generation and either the fresh session state or the explicit
`Backend unavailable [ERR_TRANSPORT_BROKEN]` state when the one-connection
conformance service has closed; it must not show the old result. After at least
four seconds, inspect the page again. The accessibility tree and screenshot
must remain unchanged, with no stale result or event from the stopped request.

The 2026-09-07 trace used the Codex in-app browser at a 1280×720 CSS-pixel
viewport and device scale 1.25. It observed **Request in progress**, stopped
the host, remounted it, then waited beyond the delay. The remounted tree stayed
at `Backend unavailable [ERR_TRANSPORT_BROKEN]` with `Remote events: none`; the
delayed response did not mutate the new DOM. The browser engine version and
post-drop allocation counts were unavailable, so this is lifecycle evidence
for one engine and the real loopback service, not cross-engine or allocation
proof.

## Exercise the real Rust state

Change the weight, concentration or dose fields. The result panel updates from
the Rust-owned `FormInputs` and the status returns to idle for each valid edit.
Enter a non-numeric value to observe the typed `ERR_NUMERIC_INSTABILITY`
display. With no service configuration, submit the form to observe
`ERR_CONNECTION_CLOSED`; this is an explicit disconnected result, not a local
calculation or a fabricated success. The submit control is disabled until an
authenticated bridge is ready and becomes disabled again while a request is in
flight; changing the DOM or dispatching a click cannot bypass that lifecycle
guard.

The **View options** fieldset uses semantic HTML5 controls owned by the Rust
host. Uncheck **Show remote events** to hide the event status line while the
event remains received and validated; select **Volume rate** or **Drug mass
rate** to change the displayed metric; move **Result scale** with the arrow
keys or pointer to select a bounded 50–150% presentation preference; choose
**Clinical summary** or **Audit detail** from **Result detail** to change the
rendered response annotation. The
`options-state` text and `data-result-scale-percent` attribute expose the
Rust-owned state after each change. Tab through the labels, press Space on the
checkbox or radio, use the range arrows, and open the select to reproduce the
keyboard path.

## Explore retained results

The **Result explorer** is a bounded, host-independent view of real backend
responses. Each accepted response is retained under its validated patient
reference and audit sequence. Duplicate audit sequences update their row; the
history evicts its oldest row at the fixed capacity. The explorer never
calculates a value in the browser and never renders the response signature.

Use **Filter patient references** to restrict the visible tree, and choose an
order from **Order results** to sort by audit sequence, patient, volume rate or
drug rate. Patient groups are keyboard-operable disclosure buttons. Their rows
are keyboard-operable selection buttons, and the pager exposes at most eight
visible tree entries at once. The live status reports `empty`, `loading`,
`ready` or a typed producer error; a failed refresh leaves previously accepted
rows available for inspection.

The native `metis-frontend` tests exercise inserted and updated responses,
exact sort and filter values, stable selection, eviction, replacement,
disclosure, paging and malformed input. The browser asset test checks the
labels, table caption, live region, control IDs and delegated listener events.
For a browser capture, build and serve the workbench, scroll to **Result
explorer**, and inspect its empty state before connecting a service. The
2026-09-08 CUA trace used the generated page at
`http://127.0.0.1:8095/?cache=explorer-20260908` in a 1280×720 CSS-pixel
viewport at device scale 1.25. The screenshot showed the result card, filter,
order select, table caption and disabled pager. Changing the filter to `PT`
and then `X` updated the table caption and returned `Entries 0 of 0`; keyboard
deletion cleared it. No backend bridge was configured for this capture, so it
does not claim a live-row rendering; the real row path is covered by the Rust
tests and the connected-service trace remains required for viewer acceptance.
The exact gate and runtime limitations are recorded in the [V07 result explorer
evidence](../VERIFICATION.md#result-explorer-evidence--2026-09-08).

## Hypermedia boundary

The [htmx documentation](https://htmx.org/docs/) models a browser interaction as
an event that issues a request and places the response in a selected target.
`hx-target` names the target and `hx-swap` defines standard DOM replacement
modes ([target reference](https://htmx.org/attributes/hx-target/), [swap
reference](https://htmx.org/attributes/hx-swap/)). Metis uses that separation as
an implementation rule without shipping an htmx runtime or accepting arbitrary
HTML from a backend.

Ordinary `input` and `change` events bubble to `#metis-app`. Rust maps the
target ID to a closed input/control binding, applies the typed state transition,
and updates only the allowlisted text and attributes. Pointer, file, text and
dialog events keep their specialized Moirai provider listeners. Static
application markup is installed once; backend values and diagnostics always use
text or attribute setters, so a message cannot create a script or element.
The authenticated binary WebSocket remains the current service contract. The
first typed fragment action uses it: activating **Session details** sends a
generation-bound `FragmentAction` for `status.describe` through the scoped
`ui` plugin. The backend returns a bounded `FragmentPatchSet`; the browser
preflights the allowlisted target and applies the result with a text setter. The
response cannot add markup, scripts, selectors or navigation, and the existing
`metis-events` status target keeps the static control inventory unchanged.

The HTTP demonstration applies the same contract in its own request lifecycle.
It preflights every target and attribute before any DOM setter, reads response
bodies through a 16 KiB streamed bound, and associates health, handshake,
fragment and negative-probe requests with the current mount lease. Reset aborts
the lease and clears the old fragment result before incrementing the generation;
stale completions cannot update the new page.

Exercise it with an authorized service session by activating **Session details**
and inspecting `metis-events`. The value must contain
`Fragment action status.describe accepted: session-dialog`. Stop and remount the
host while a request is pending; the old generation must not update the new
mount. A configured WebDriver run is still required for cross-engine captures;
the native protocol and policy suites are the current deterministic evidence.

The admitted loopback HTTP demonstration implements that boundary over
Moirai. Start it beside the static workbench with a fixed browser origin:

```powershell
cargo run --locked -p metis-app -- --metis-http-service http://127.0.0.1:8080 8766 66666666666666666666666666666666
```

`GET /health` returns the bounded readiness text. `POST /v1/session` accepts a
binary `HandshakeRequestPayload`; the response is a typed
`HandshakeResponsePayload`. After that response, send the returned capability
token in a `PluginInvocationPayload` to `POST /v1/fragments`. The response body
is a `FragmentPatchSet` whose text and attribute operations are applied through
the same browser allowlists as the WebSocket path. Every request must carry the
exact configured `Origin`. Unknown routes, wrong methods, malformed envelopes,
missing sessions and a full session table return bounded
`ErrorResponsePayload` values. Moirai owns body, header, response, deadline and
connection limits; the application closes after its finite request budget.

The server's real loopback suite covers the successful handshake/fragment
journey, exact-origin CORS preflight, origin and route denial, malformed and
oversized bodies, a disconnected peer, idle-peer deadline and finite teardown.
The optional response-delay probe is bounded and asynchronous so the browser
reset path can be observed against a pending real request. It does not add an
Axum or htmx runtime and it never accepts arbitrary markup
or scripts. [ADR 0025](../adr/0025-axum-server-boundary.md) records the
comparison and the first-party boundary. RITK remains responsible for DICOM
scanning, decoding, series selection, geometry and viewer state; this fragment
contract carries presentation messages only.

To demonstrate the boundary, serve the workbench, change **Weight (kg)** and
**Result scale**, and capture the form before and after each action at the same
viewport. The semantic tree must retain the same controls while the result text,
`data-result-scale-percent` attribute and `options-state` value change. Enter
`<img src=x onerror=alert(1)>` in **Clinical note**; the preview must show the
literal characters and the DOM must contain no added element. The existing
browser visual and semantic capture records the target engine, viewport and
rendered state for this check; the committed trace is in the [delegated-control
evidence](../VERIFICATION.md#browser-delegated-control-evidence).

## Theme and branding

The **Theme** select applies one of four bounded presentation modes:

- **System preference** sets `data-metis-theme="system"` and follows the
  browser's `prefers-color-scheme` value.
- **Light** sets `data-metis-theme="light"`.
- **Dark** sets `data-metis-theme="dark"`.
- **High contrast** sets `data-metis-theme="high-contrast"` and keeps borders
  and focus indicators legible on a black and white palette.

Rust owns the selected mode through `metis_web::Theme`; the browser host writes
the same value to the document body and `#metis-app`. `styles.css` maps those
attributes to semantic variables such as `--metis-page`, `--metis-surface`,
`--metis-text`, `--metis-accent` and `--metis-focus`. An application can keep
the selector and replace those variables in a same-origin stylesheet to apply
its own palette. Theme state changes presentation only; it does not change
backend authority, IPC messages or application data.

The page loads the starter [Métis vector mark](../../examples/browser/assets/metis-mark.svg)
from the same-origin `assets/` directory and uses it as the favicon and header
image. A PNG alternate and the multi-resolution
[native icon](../../examples/browser/assets/metis-mark.ico) remain in the same
directory for browsers and packaged applications that need those formats.
`python scripts/browser.py build` copies all three local assets and fails if a
declared mark is missing. Replace the SVG, PNG, ICO and the `.metis-mark` rule
with project-owned artwork for a branded application. The browser host does
not fetch an icon, font or media resource from a remote origin.

The packaging boundary admits a strict SVG subset so an installer never stores
an unbounded document language: one fixed positive viewport, optional
`xMidYMid meet`, literal hexadecimal paints, finite numeric opacity and
self-closing path elements. XML declarations, entities, scripts, external
references, unknown attributes, malformed geometry and dimensions over 4096
pixels are rejected before the resource enters a portable or MSI payload.

To demonstrate the contract, build and serve the workbench, select each mode,
and capture the header, view-options card and focus ring at the same viewport.
The static asset suite checks all four selector values and variable branches;
runtime captures must record the browser engine and operating-system
presentation settings because CSS media preferences are host behavior.

The 2026-09-08 CUA trace used the generated page at 1280×720 CSS pixels and
device scale 1.25. It selected **Light**, **High contrast** and **Dark** in the
native Theme control; each accessibility snapshot reported the matching option
and `View options: ... theme <mode>`, while the inspected screenshots showed the
light, black-and-white and dark palettes with the same mark and layout. A DOM
read after the dark selection reported `data-metis-theme="dark"` on both the
body and `#metis-app`, dark page/text colors and a loaded local mark. This is
explicit mode evidence for one browser engine; system media preference,
forced-colors, the V04 viewport/scale matrix and native host integration remain
separate host checks. MSI shortcut icon wiring is covered by the native package
database test and the distribution workflow.

The **Pointer capture** card demonstrates the browser pointer lifecycle that a
drag interaction needs. Press or drag **Pointer capture surface**. On
`pointerdown`, Rust reads the browser `PointerMetadata` snapshot through
Moirai, captures the identifier on the surface and verifies that the browser
retained it. The status includes the pointer device, viewport coordinates,
changed and held buttons, modifier keys and primary-pointer state. Captured
`pointermove` events update the same metadata record while dragging. On
`pointerup` or `pointercancel`, Rust releases the same identifier and updates
the status to the released metadata record for the trace's first pointer.
The capture handle is owned by the mounted listener set, so **Stop host**
drops the callbacks with the rest of the browser application. The surface
accepts at most two distinct identifiers at a time, captures each through
Moirai and reports a typed browser-host error if a duplicate, third pointer or
provider capture/release operation is rejected.

The 2026-09-07 pointer trace used the generated browser build at a 1280×720
CSS-pixel viewport and device scale 1.25. Its accessibility tree exposed the
surface as a named group, and the full-page screenshot showed the pointer card,
the semantic status and the unchanged backend-result panel. The provider
revision and observed status are recorded in
[browser pointer-capture evidence](../VERIFICATION.md#browser-pointer-capture-evidence--2026-09-07).
The metadata trace and provider revision are recorded in
[browser pointer-metadata evidence](../VERIFICATION.md#browser-pointer-metadata-evidence--2026-09-07).
The pointer trace used provider revision
`a3c86cd183a18edc35db30f1d35e79fe80092df4`; the current consumer lock uses
the co-evolution revision `9e045f73be49b9ae6272045dd71705dcf14eccf8`.

The **Wheel** status below the pointer status demonstrates the browser scroll
boundary. Scroll the named **Pointer capture surface**. Rust reads Moirai's
`WheelMetadata` record and renders the horizontal, vertical and depth deltas,
their pixel/line/page unit, viewport coordinates and modifier keys. The
listener prevents the browser default action after the event kind is
validated; the gesture section below applies the bounded pan and zoom policy.

The 2026-09-07 wheel trace used the generated browser build at 1280×720 CSS
pixels and device scale 1.25. An in-app browser scroll action produced
`Wheel: delta (0.00, -129.60, 0.00) pixel at (386, 580), modifiers none`; a
horizontal action produced `delta (426.40, 0.00, 0.00)` at the same target.
The trace is automation-generated browser input; the CUA surface does not
expose the hardware `isTrusted` flag, so this evidence does not claim a
physical-wheel or cross-engine result. The provider revision is
`f634b3a802ec0355da22f111ed01067d2435c5cb`; full command output and the
rendered screenshot are recorded in
[browser wheel metadata evidence](../VERIFICATION.md#browser-wheel-metadata-evidence--2026-09-07).

The pointer surface also exercises the Rust-owned gesture policy. Drag from
one point to another to pan the content; ordinary wheel input pans by its
normalized CSS-pixel delta; hold Control while scrolling to zoom. With two
captured pointers, move either contact to pan by the pair's centroid and zoom
by its distance ratio. The policy accepts at most two distinct pointers,
rejects a third, clamps pan to ±1024 CSS pixels and zoom to 50–300%,
normalizes line and page units to 16 and 640 CSS pixels, and rejects non-finite
deltas without changing state. `gesture-status` reports the action, pan and
zoom, while the content's CSS transform provides the visible result. A
zero-distance pair waits for a valid baseline before changing zoom. Touch
pointers use the same bounded policy.

The 2026-09-07 gesture trace used the generated build at the same 1280×720
CSS-pixel viewport and device scale 1.25. An upward scroll rendered
`Gesture: wheel pan; pan (0.0, -129.6) CSS px; zoom 100%`; a rightward scroll
then rendered `pan (426.4, -129.6) CSS px`; dragging from `(300, 590)` to
`(420, 620)` rendered `Gesture: release; pan (546.4, -99.6) CSS px; zoom 100%`.
The live screenshot showed the translated content and the semantic status.
The trace is automation-generated; CUA cannot expose hardware trust, native
touch, IME or another browser engine. Native policy tests cover line/page
normalization, bounded zoom and non-finite rejection; the live trace does not
claim physical-input or cross-engine parity.

The two-pointer policy is covered by the native Rust suite: the second pointer
establishes a centroid and finite-distance baseline, movement updates pan and
zoom from that baseline, a zero-distance pair waits for valid separation, and
a third pointer is rejected. The generated page instruction and semantic
pointer card are visible in the [pinch gesture evidence](../VERIFICATION.md#browser-pinch-gesture-evidence--2026-09-08).
The Codex browser cannot inject trusted physical touch, so a live screenshot
does not claim hardware multi-touch behavior.

The **File drop** card demonstrates the browser file and metadata workflow.
Drag one or more files onto **File drop**. Rust prevents the browser's default
navigation, asks Moirai for a bounded `DropFiles` capture, and renders the file
count and the first three display names with byte sizes. The zone exposes
`dragenter`, `dragover`, `dragleave` and `drop` state through the semantic
`drop-status` region and its `data-drop-state` attribute. A rejected metadata
record leaves the zone in the typed rejected state and reports the provider
error without retaining the batch.

For an accepted drop, the browser host reads every selected file asynchronously
through Moirai's owned browser `File` handles. Each file is limited to 64 MiB
and the batch to 256 MiB; a 64 KiB continuation buffer keeps each `FileReader`
turn bounded while the provider still enforces its 1 MiB maximum read. The
consumer reports `reading` and then `complete` or `failed` through
`drop-byte-status` and `data-byte-state`. The handoff performs no format
classification. A completed batch is available to a trusted WASM consumer
through `metis_web::take_file_drop`, which transfers ownership of the batch.
Call `FileDropBatch::into_files` and
`FileDropPayload::into_parts` when the consumer must retain the entries; moving
the tuple preserves each byte allocation. A later drop replaces an unconsumed
batch, and stop/remount drops it. No browser name is turned into a filesystem
path. A RITK adapter receives the named bytes and performs any format-specific
scan, decode and study opening; this slice closes the bounded zero-copy byte
handoff, not a format-specific workflow.

The provider caps one drop at 64 files, 4096 UTF-8 bytes per name and 256 bytes
per media type. The CUA browser surface cannot synthesize a trusted
operating-system file drop, attach a local file to a synthetic event or expose
`isTrusted`, so a manual trace must record the browser engine and whether the
drop came from a physical file operation. Native policy tests cover bounds,
typed rejection and payload size budgets; the WASM gate compiles the real
provider-backed read path.

The 2026-09-08 CUA trace opened the generated build at a 1280×720 CSS-pixel
viewport with device scale 1.25. The accessibility tree exposed **File drop**,
both status regions and the named **File drop zone** group; the
screenshot showed the drop card between the pointer and backend-result cards
with its ready state and focus outline. The browser engine version was
unavailable, and no trusted local file was attached, so the trace does not
claim a successful live byte read or format-specific decode. The provider-backed
full batch path is established by the native policy suite and the strict WASM
build; the RITK consumer smoke is now recorded in the
[RITK DICOM workflow](https://github.com/ryancinsight/ritk/blob/main/docs/manual/dicom-workflow.md#inspect-the-browser-canvas-visual-smoke).
The ownership-consuming API is covered by a pointer-identity test, so moving
a completed batch does not copy its file contents.

The **Text and composition** card exercises the browser's native editing
surface while keeping application state in Rust. Focus **Clinical note**, type
ordinary text, select a range with the pointer or keyboard, and inspect
`text-status`, `selection-status` and the `data-selection-*` attributes. The
seeded value includes an accented character, an em dash, CJK text and mixed
symbols so the displayed selection offsets make the browser's UTF-16
coordinate convention visible. The `text-preview` line is regenerated from
the bounded Rust value after every input event.

For an IME-capable environment, switch to a CJK input method and type into
**Clinical note**. `compositionstart` and `compositionupdate` show the bounded
preedit and locale, `compositionend` reports the commit transition, and
`compositioncancel` clears the preedit without inventing a committed value.
The browser `InputEvent` snapshot also reports its operation name, optional
data and composing marker. The listener set is dropped with **Stop host**, so
remounting starts with the seeded value and a fresh selection.

The 2026-09-08 CUA trace opened the generated page at
`http://127.0.0.1:8095/?cache=text-clean-20260908` in a 1280×720 CSS-pixel
viewport at device scale 1.25. The accessibility tree and inspected screenshot
showed the seeded value and a `16`-unit caret. Browser `typeText` input appended
` typedX`; the rendered value became `Résumé — 東京 / 影像 typedX`, the text
status reported `Text: input insertText applied; data X`, and the selection
status reported a `23`-unit forward caret. This is ordinary browser input
evidence; CUA does not expose a trusted operating-system IME or `isTrusted`.

The policy caps the value at 1 MiB, event metadata at 128 UTF-8 bytes and the
locale at 64 bytes. UTF-16 offsets are transport coordinates; the Rust policy
rejects an offset inside a surrogate pair before it changes state, while
grapheme boundaries remain a host contract. This workflow therefore does not
claim grapheme-safe caret movement, bidi shaping, fallback-font metrics,
clipboard/undo behavior, assistive-technology behavior or native IME delivery.
CUA can show the real HTML textarea, statuses and focus ring, but it cannot
provide a trusted OS IME event or expose `isTrusted`; record the browser engine
and input method when collecting host evidence.
The browser provider for this workflow is Moirai revision
`0862716265d657b8069d5a47fd1e77ae26ddd006`; the consumer lock is updated to the
same merged revision.

## Responsive runtime capture

The page uses a bounded responsive grid. At widths below `700px`, the form and
options stack in one column with `1rem` page padding; wider viewports use two
`minmax(0, 1fr)` columns inside a `960px` content bound. Grid items accept
long status and clinical strings without widening the page, and the host
buttons use the same narrow-viewport padding. Option rows and the result-scale
slider expose a `44px` CSS hit target so their labels remain usable on touch
and keyboard layouts. The CSS contract test checks these declarations.

The Browser viewport capability captured the generated workbench at
`360×640`, `800×600` and `1440×900` CSS pixels with device scale `1`. The
runtime manifest records the exact card rectangles, grid columns, scroll
extents and interactive target rectangles in
[`browser-layout-metrics.json`](images/browser-layout-metrics.json), bound to
the stylesheet SHA-256 recorded in that file. The captures show the narrow
single-column layout, the two-column fixture and the bounded wide layout:

![Metis at 360 by 640 CSS pixels](images/browser-layout-360x640.jpg)

![Metis at 800 by 600 CSS pixels](images/browser-layout-800x600.jpg)

![Metis at 1440 by 900 CSS pixels](images/browser-layout-1440x900.jpg)

The measured pages have no horizontal overflow; every card and required
interactive target ends inside the viewport, and the option rows and slider
measure `44px` high at all three widths. The viewport capability does not
expose a device-scale override, so scale `2`, custom-style diagnostics and
platform fractional-scale cases remain open under V04.

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
non-positive focus order and both media-query branches. For a host acceptance
run, enable a supported screen reader, reduced-motion setting, forced-colors
setting and browser zoom, then capture the accessibility tree, focus ring and
spoken action for each state. Record the browser engine, operating system,
scale factor and assistive-technology version. A semantic tree or CSS rule by
itself does not establish screen-reader support or an operating-system
accessibility bridge.

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
