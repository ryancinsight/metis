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
calculation and audit ownership, a software rasterizer, a headless form workflow
and a runnable HTML5/CSS browser workbench. The browser workbench keeps state and
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
surfaces; the native window role advertises its installed HWND surface while
OS permission surfaces remain absent.
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
adapter installed-runtime navigation/bridge smoke passes, while a committed
visible application capture remains open. Metis does not yet provide Tauri
feature parity, a system WebView host on all targets, an OS privilege sandbox,
durable audit storage or regulatory certification.
Metis does not parse DICOM or define medical volume semantics. Presentation
hosts hand bounded file-drop bytes to RITK's public scanner and receive its
validated image and metadata result; the owning workflow and visual evidence
live in the [RITK manual](https://github.com/ryancinsight/ritk/blob/main/docs/manual/dicom-workflow.md).
The `METIS-DICOM-*` backlog entries record removal and boundary checks for that
ownership decision; they do not add DICOM implementation to Metis.
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
The `metis` build tool is not part of the application payload. On Windows x64 it also
authors a per-user MSI with a Start Menu shortcut and registered uninstall.
See the [distribution manual](docs/manual/distribution.md) for the complete
workflow, host prerequisites and current limits. The bundled example is a
console application; packaging does not supply the missing desktop GUI host.
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
or establish OS permission restrictions. See [application entry design](docs/adr/0006-application-entry.md).

Runtime crates declare no direct third-party crates. The distribution CLI uses
Serde and serde_json for validated manifests and Cargo artifact messages, as
recorded in [ADR 0005](docs/adr/0005-application-distribution.md). Atlas providers
have transitive dependencies; the gate records the actual graph instead of describing it as
dependency-free. The Atlas development overlay resolves first-party code to local
trees. Standalone builds use the corresponding pushed provider revisions recorded
in Cargo.lock. Metis now consumes Moirai through git-plus-version requirements;
the lock records audited main revision `a58344b00ccc4a062c71659a463dd188d67bf1f4`
after WebView2 provider PR #299 merged. That revision includes the merged
process, browser/API, bounded WebSocket service, cancellable-task surfaces,
semantic control seams, pointer metadata, wheel metadata, bounded browser file
access and the thread-affine Windows WebView2 provider. The provider and Metis
adapter pass the installed WebView2 navigation/bridge smoke on runtime
`152.0.4191.66`; visible capture, permission and accessibility evidence remain
tracked in [the desktop item](backlog.md#METIS-DESKTOP-001).

## Design and evidence

- [User manual and application snapshots](docs/manual/README.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Interface contract](docs/INTERFACE.md)
- [Risk controls](docs/RISK_CONTROLS.md)
- [Verification](docs/VERIFICATION.md)
- [Current work](backlog.md)
- [egui, GPUI, Iced, Tauri and Axum gap analysis](docs/adr/0003-framework-conformance.md)

Licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
