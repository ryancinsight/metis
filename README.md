# Metis

Metis is a Rust application framework in development in the public
[Atlas stack](https://github.com/ryancinsight/atlas). Its goal is a near drop-in
replacement for Tauri, supporting desktop applications, WebAssembly (WASM), and
rendering on the web with HTML5/CSS. Application logic defaults to Rust, compiled
to WASM where it runs in the browser. A desktop system WebView remains in scope
to preserve existing web frontends; minimizing JavaScript dependencies does not
mean banning web technology. Its name refers to Metis, associated with counsel
and practical wisdom.

Stronger security and lower memory use are design goals, not demonstrated
advantages over Tauri. The [target contract](docs/adr/0002-web-application-contract.md)
defines compatibility, trust boundaries, and the measurements required for those
claims. Migrating an existing JavaScript frontend does not automatically remove
its JavaScript, and Tauri API/plugin compatibility remains a mapped work item.

The current implementation provides binary IPC, session capabilities, backend
calculation and audit ownership, a software rasterizer, a headless form workflow,
a runnable HTML5/CSS browser workbench, and a [starter application](crates/metis-starter/README.md)
with create-tauri-app's page and a Rust/WASM greeting behind a two-line loader. The browser workbench keeps state and
events in Rust/WASM while Moirai owns browser handles; its external assets use a
strict same-origin CSP and its service bridge validates Origin before the
upgrade, then binds grants to origin, window and session. The loopback bridge
is implemented for the demonstrator. Authenticated sessions also expose a
bounded command catalog and a local backpressure-aware event hub through the
existing IPC seam. The same seam carries versioned unsolicited events over
synchronous pipes and asynchronous WebSocket sessions, with strict identifiers
and bounded retention; an accepted clinical calculation emits a typed
`clinical.result` event after its correlated response. They also expose a
versioned target descriptor naming the host platform and installed transport
surfaces; the native window role advertises its installed HWND surface. On
Windows, the WebView2 provider denies every page permission request before
profile or operating-system prompting and reports the typed denial to the
application; broader OS sandbox enforcement remains a separate host contract.
The [browser manual](docs/manual/browser.md) also carries a dependency-free W3C
WebDriver runner for the Chromium, Firefox and WebKit conformance matrix; a
configured driver is required for runtime evidence.
Hosts can register bounded, typed plugin manifests with explicit capability
scopes; the versioned
IPC seam also invokes declared plugin commands through a bounded host router and
typed sync/async client methods. Registration and invocation do not grant
operating-system authority; handler erasure is confined to the backend's open
extension boundary. This renderer's bounded
markup subset is
not the intended limit of web support. Windows now exposes a bounded WebView2
consumer through `metis_platform::native::WebViewSurface`; the `metis-app`
demonstration composes that host through `--metis-webview`. The provider and
adapter installed-runtime navigation/bridge smoke passes, and the committed
visible native/WebView2 application captures are recorded in the [native capture
manifest](docs/manual/images/native-captures.json). Metis does not yet provide
Tauri feature parity, a system WebView host on all targets, an OS privilege
sandbox or regulatory certification. The backend has a bounded authenticated
`FileAuditStore`; its recovery contract is documented in [ADR 0038](docs/adr/0038-durable-audit-recovery.md).
Metis does not parse DICOM or define medical volume semantics. Presentation
hosts hand bounded file-drop bytes to RITK's public scanner and receive its
validated image and metadata result; the owning workflow and visual evidence
live in the [RITK manual](https://github.com/ryancinsight/ritk/blob/main/docs/manual/dicom-workflow.md).
The `METIS-DICOM-*` backlog entries record removal and boundary checks for that
ownership decision; they do not add DICOM implementation to Metis.
The RITK manual now leads with the reviewed real MRI-DIR CT window capture,
including axial, coronal, sagittal and axial-MIP panels through the Métis host;
it also documents the copyable local command for opening a saved clinical study.
Private studies remain local and are never copied into Metis. The same capture
is the first image in the [application gallery](docs/manual/applications.md#real-dicom-application-evidence):

![Actual saved CT study rendered through the Métis host](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-installer-ct.png?raw=true)

The gallery links the revision-bound provenance and browser component-state
records so the image is reviewed together with file handoff, canvas, listener,
theme, pointer and teardown observations.
The infusion arithmetic is a synthetic engineering example; its configurable
limits are not treatment guidance.

## Build and run

Use the pinned Rust toolchain, cargo-nextest 0.9.143 and wasm-bindgen-cli 0.2.128.
From this directory:

```text
cargo build --workspace --bins
cargo run -p metis-app -- 60 2 0.2
cargo run -p metis-app -- --help
cargo install cargo-nextest --version 0.9.143 --locked
cargo install wasm-bindgen-cli --version 0.2.128 --locked --root output/wasm-bindgen-cli
python scripts/verify.py
```

The arguments are weight in kg, concentration in mg/mL and dose in mcg/kg/min.
The gate verifies the committed Git dependency lock, including when invoked inside Atlas.
The `metis-app` executable launches another instance of itself for presentation,
receives its submitted values and returns the calculated result. One application
executable therefore serves two processes with separate session state. For these
demonstration inputs the rate is 0.36 mL/hour and drug rate is 0.72 mg/hour.
During a session, standard output carries IPC bytes on the private child pipes;
human-readable results go to standard error. `--help` writes usage to standard output.

## Build executables and installers

The `metis` CLI builds declared Cargo binaries and bundles explicitly named
resources from one [application manifest](metis.json). The demonstration ships
one `metis-app` executable; optional sidecars remain explicit manifest targets.
The `metis` build tool is not part of the application payload. `metis build`
stages a host-native portable application on Windows, macOS or Linux. On
Windows x64 it also authors a per-user MSI with a Start Menu shortcut and
registered uninstall.
See the [distribution manual](docs/manual/distribution.md) for the complete
workflow, host prerequisites and current limits. The bundled example is a
console application; the native and WebView2 desktop hosts are explicit
`metis-app` modes and are documented separately from the console package.
Crates.io release validation and publication use the Atlas OIDC workflow in
`.github/workflows/rust-release.yml`. The `metis-python` crate builds the
`metis-rs` distribution for `import metis`; `.github/workflows/python-release.yml`
uses the same tokenless OIDC model for PyPI. Both registries require their
trusted publishers to be registered by the release authority; the empty
`crates-io` and `pypi` GitHub environments are present and contain no secrets.
The Python binding workflow and the runnable clinical example are documented in
the [Python binding manual](docs/manual/python.md).

## Atlas ownership

Moirai owns worker scheduling, process lifecycle and browser DOM/event handles.
Metis consumes its executor, transport and WASM PAL APIs; missing shared
capabilities are implemented upstream in Moirai. Iris supplies the
`RenderBackend` contract; Metis implements its bounded software renderer against
that contract. The `metis-frontend` library
dependency closure excludes `metis-backend`; the application entry composes both
libraries. A shared executable image does not remove backend code from the child
or establish broader OS permission restrictions. See [application entry design](docs/adr/0006-application-entry.md).
Software-rendered hosts validate the bounded `SemanticTree` from the same
markup before painting. Windows native hosts now project that tree into
Moirai's bounded AccessKit bridge before showing the HWND and dispatch typed
focus/activation actions back to the application; the implementation does not
claim spoken output or screen-reader acceptance without an installed-device
journey.

Runtime crates keep direct third-party dependencies at bounded contract
surfaces: `metis-cli` and `metis-app` use Serde and `serde_json` for manifests
and IPC, `metis-ui-lang` uses `png`, `flate2` and `jpeg-encoder` for bounded
image assets, `metis-web` uses `unicode-segmentation` for browser text policy,
and `metis-python` uses PyO3. Atlas providers
have transitive dependencies; the gate records the actual graph instead of
describing it as dependency-free. The Atlas development overlay resolves first-party code to local
trees. Standalone builds use the corresponding pushed provider revisions recorded
in Cargo.lock. Metis consumes Moirai through git-plus-version requirements;
the current standalone lock records merged Moirai revision
`693350c7ae90cd925c76fd3aabe28d149f729b66`, which includes the Windows
AccessKit accessibility provider, the UI Automation value-action mapping,
WebView2 0.39.1 bindings, POSIX process-group tree containment and
`<select>` values through `WebElement::set_value`. The current
revision retains the historical WebView2 capture provider graph and adds
explicit WebGPU provider recreation. The locked
provider history includes the merged process, browser/API, bounded WebSocket service,
cancellable-task surfaces, semantic control seams, pointer metadata, wheel
metadata, bounded browser file access through a direct first-read plus object-URL continuation stream and
the thread-affine Windows WebView2 provider, stable browser canvas extents and
synchronous WebView2 permission-denial events.
The browser host also exposes explicit asynchronous WebGPU canvas constructors
through the same borrowed frame and bounded input contract. A missing adapter
or device is reported as an unsupported/setup error; the host does not silently
switch a requested GPU surface to raster presentation. Real GPU browser output
and measurements remain consumer evidence owned by RITK.
`metis-platform` enables
`moirai-pal`'s `webview2` feature only for its Windows target; the provider's
default graph remains free of the optional COM binding for other targets. The
browser CSP admits only the
`blob:` source required for those local response streams; network endpoints stay
explicit. The provider and Metis adapter pass the installed WebView2
navigation/bridge smoke on runtime `152.0.4191.66`. The committed [native
capture manifest](docs/manual/images/native-captures.json) records the visible
Windows native and WebView2 initial/submit journeys plus a 1024×768
permission-denial capture from runtime `153.0.4234.32` using `CapturePreview`;
the artifact is 6,561 bytes with SHA-256
`a9df158ff6a167ed3c708e9621a44446cae93b3c0d87e646c818601606a33b8f`. The
[native accessibility item](backlog.md#METIS-A11Y-001) tracks remaining
screen-reader and host-preference evidence. The [desktop item](backlog.md#METIS-DESKTOP-001)
tracks physical display-scale transitions, installed IME, broader OS-enforced
restrictions and non-Windows hosts.

## Design and evidence

- [User manual and application snapshots](docs/manual/README.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Interface contract](docs/INTERFACE.md)
- [Risk controls](docs/RISK_CONTROLS.md)
- [Verification](docs/VERIFICATION.md)
- [Current work](backlog.md)
- [egui, GPUI, Iced, Tauri, Svelte/SvelteKit and Axum gap analysis](docs/adr/0003-framework-conformance.md)

Licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
