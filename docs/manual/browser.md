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

## Series selection demonstration

Preservation PR [#498](https://github.com/ryancinsight/metis/pull/498) retains
the original interface commit. The opt-in example recovers one working
interaction: selecting a demonstration series assigns it to the active viewport;
activating another viewport restores that viewport's assigned series. Inactive
assignments persist. No clinical study, decoded image plane or reconstruction
is presented. The remaining preserved shell stays on its original draft branch.

Use the pinned Rust toolchain and wasm-bindgen CLI from the build instructions:

```text
python scripts/browser_series_selection.py build
python -m http.server 8080 --directory output/browser
```

Open `http://127.0.0.1:8080/gallery.html`. The default workbench remains at `/`.
The build uses the existing explicit consumer-package/gallery validation and
canonical content-security policy. Its two-line JavaScript module initializes
WASM and calls Rust; no JavaScript selection state is added.

Native acceptance runs as `cargo nextest run --locked -p metis-web --profile ci`.
For real-browser acceptance, install Playwright as a local test prerequisite,
then run `node --test scripts/browser_series_selection.mjs`. If Playwright lives
outside the checkout, set `METIS_PLAYWRIGHT_MODULE` to its absolute package path.
`METIS_TEST_BROWSER` selects `chromium` (default), `firefox` or `webkit`;
`METIS_TEST_BROWSER_EXECUTABLE` optionally names the installed test browser.
The runner serves generated assets on a temporary loopback port and closes the
browser/server afterward. Screenshots, source/WASM digests and checked states
are written under `output/series-selection-evidence/<engine>/`. Browser setup
failure is a failed check; the runner does not substitute a simulation.
Local Chrome 155.0.8059.40 acceptance passes on October 13, 2026. Other engine
names are runner options, not claims of executed acceptance.

See [ADR 0058](../adr/0058-series-selection-demonstration.md) and the
[selection algebra](../../crates/metis-web/examples/series_selection/README.md).

## Build and serve

Install the pinned `wasm-bindgen-cli` version matching the workspace's
`wasm-bindgen` dependency. The build script checks for exactly 0.2.129 and also
discovers this ignored local root, then run:

```text
cargo install wasm-bindgen-cli --version 0.2.129 --locked --root output/wasm-bindgen-cli
python scripts/browser.py build
python -m http.server 8080 --directory output/browser
```

A checkout inside the Atlas stack inherits the development overlay, whose local
`[patch]` tables cannot agree with the committed revisions, so the build script
runs Cargo outside that configuration while still writing into the stack's
shared target directory. The committed `Cargo.lock` therefore stays
authoritative whether the command runs standalone or inside Atlas.

Open `http://127.0.0.1:8080/` in a browser for the disconnected local-control
workflow. A file URL is not accepted because module and WASM loading require an
HTTP origin.

The same generated assets include a format-neutral HTTP boundary probe. Start
the bounded service in a second terminal:

```text
cargo run --locked -p metis-app -- --metis-http-service http://127.0.0.1:8080 8766 66666666666666666666666666666666
```

The last argument is the only session principal the service accepts; a
handshake naming any other principal receives HTTP 403. The service keeps one
session; a second handshake for a live session receives HTTP 409, and the first
handshake after the session lifetime elapses replaces it.

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

## Manual chapters

Current Rust shell/HTTP ownership: [ADR 0056](../adr/0056-rust-browser-shell.md).
Authored application JavaScript contains nine initialization lines; generated
interop remains. Native admission and remaining work: [Tauri review](tauri.md).

### Current live fragment capture — 2026-09-14

[Read this section](browser/capture-history.md#current-live-fragment-capture--2026-09-14).

### Live Edge WebDriver fragment capture — 2026-09-14

[Read this section](browser/capture-history.md#live-edge-webdriver-fragment-capture--2026-09-14).

## Run the cross-engine conformance trace

[Read this section](browser/cross-engine.md#run-the-cross-engine-conformance-trace).

### Probe runtime accessibility semantics

[Read this section](browser/cross-engine.md#probe-runtime-accessibility-semantics).

### Run the hosted cross-engine matrix

[Read this section](browser/cross-engine.md#run-the-hosted-cross-engine-matrix).

### Run a consumer-owned canvas trace

[Read this section](browser/cross-engine.md#run-a-consumer-owned-canvas-trace).

### Opt into an explicit WebGPU canvas

[Read this section](browser/cross-engine.md#opt-into-an-explicit-webgpu-canvas).

## Connect the real browser service

[Read this section](browser/service.md#connect-the-real-browser-service).

## Verify stop/remount disposal at the service boundary

[Read this section](browser/service.md#verify-stopremount-disposal-at-the-service-boundary).

## Exercise the real Rust state

[Read this section](browser/service.md#exercise-the-real-rust-state).

## Explore retained results

[Read this section](browser/service.md#explore-retained-results).

## Hypermedia boundary

[Read this section](browser/http-boundary.md#hypermedia-boundary).

### Capture the authenticated fragment boundary

[Read this section](browser/http-boundary.md#capture-the-authenticated-fragment-boundary).

<a id="hosted-fragment-matrix-evidence--2026-09-16"></a>
### Hosted fragment matrix evidence — 2026-09-16

[Read this section](browser/http-boundary.md#hosted-fragment-matrix-evidence--2026-09-16).

## Theme and branding

[Read this section](browser/theme.md#theme-and-branding).

## Measure browser text geometry

[Read this section](browser/text-geometry.md#measure-browser-text-geometry).

### Compare text semantics across engines

[Read this section](browser/text-geometry.md#compare-text-semantics-across-engines).

## Responsive runtime capture

[Read this section](browser/responsive.md#responsive-runtime-capture).

### Hosted device-scale capture

[Read this section](browser/responsive.md#hosted-device-scale-capture).

## Exercise the accessibility presentation

[Read this section](browser/accessibility.md#exercise-the-accessibility-presentation).

### Decode the shipped image marks

[Read this section](browser/accessibility.md#decode-the-shipped-image-marks).

### Probe same-origin font loading and teardown

[Read this section](browser/accessibility.md#probe-same-origin-font-loading-and-teardown).

### Probe media error and teardown

[Read this section](browser/accessibility.md#probe-media-error-and-teardown).

### Probe audio playback controls

[Read this section](browser/accessibility.md#probe-audio-playback-controls).

## Drop a study into the gallery

[Read this section](browser/saved-study.md#drop-a-study-into-the-gallery).

### Use the saved-study chooser

[Read this section](browser/saved-study.md#use-the-saved-study-chooser).

### Replay a saved MRI study with adaptive canvas input

[Read this section](browser/gallery-lifecycle.md#replay-a-saved-mri-study-with-adaptive-canvas-input).

### Consumer-owned gallery controls

[Read this section](browser/gallery-lifecycle.md#consumer-owned-gallery-controls).

### Read browser frame timing

[Read this section](browser/gallery-lifecycle.md#read-browser-frame-timing).

### Read browser JavaScript-heap counters

[Read this section](browser/gallery-lifecycle.md#read-browser-javascript-heap-counters).

### Read browser-estimated aggregate memory

[Read this section](browser/gallery-lifecycle.md#read-browser-estimated-aggregate-memory).

### Repeat the workbench lifecycle

[Read this section](browser/gallery-lifecycle.md#repeat-the-workbench-lifecycle).

### Repeat the saved-study gallery lifecycle

[Read this section](browser/gallery-lifecycle.md#repeat-the-saved-study-gallery-lifecycle).
