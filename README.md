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
its JavaScript, and Tauri API/plugin compatibility remains to be implemented.

The current implementation provides binary IPC, session capabilities, backend
calculation and audit ownership, a software rasterizer and a headless form
workflow. This renderer's bounded markup subset is not the intended limit of web
support. Browser execution and a desktop WebView host are not implemented. It
does not yet provide Tauri feature parity, native desktop windows,
an OS privilege sandbox, durable audit storage or regulatory certification.
The infusion arithmetic is a synthetic engineering example; its configurable
limits are not treatment guidance.

## Build and run

Use the pinned Rust toolchain and cargo-nextest 0.9.143. From this directory:

```text
cargo build --workspace --bins
cargo run -p metis-backend -- 60 2 0.2
python scripts/verify.py
```

The arguments are weight in kg, concentration in mg/mL and dose in mcg/kg/min.
The gate verifies the committed Git dependency lock, including when invoked inside Atlas.
The backend launches a separate frontend executable, receives its submitted
values and returns the calculated result. For these demonstration inputs the
rate is 0.36 mL/hour and drug rate is 0.72 mg/hour. Standard output carries IPC
bytes exclusively; human-readable results go to standard error.

## Build executables and installers

The `metis` CLI builds declared Cargo binaries and bundles explicitly named
resources from one [application manifest](metis.json). On Windows x64 it also
authors a per-user MSI with a Start Menu shortcut and registered uninstall.
See the [distribution manual](docs/manual/distribution.md) for the complete
workflow, host prerequisites and current limits. The bundled example is a
console application; packaging does not supply the missing desktop GUI host.

## Atlas ownership

Moirai owns worker scheduling and process lifecycle. Metis consumes its executor
and transport APIs; missing process-pipe/deadline capabilities are implemented
upstream in Moirai. Iris supplies the `RenderBackend` contract; Metis implements
its bounded software renderer against that contract. Frontend dependencies never
include `metis-backend`.

Runtime crates declare no direct third-party crates. The distribution CLI uses
Serde and serde_json for validated manifests and Cargo artifact messages, as
recorded in [ADR 0005](docs/adr/0005-application-distribution.md). Atlas providers
have transitive dependencies; the gate records the actual graph instead of describing it as
dependency-free. The Atlas development overlay resolves first-party code to local
trees. Standalone builds depend on the corresponding pushed provider revisions.
Moirai is pinned to the pushed process-support revision until its 0.6 API reaches
the default branch; the removal trigger is tracked in
[the board](backlog.md#METIS-PROVIDER-001).

## Design and evidence

- [User manual and application snapshots](docs/manual/README.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Interface contract](docs/INTERFACE.md)
- [Risk controls](docs/RISK_CONTROLS.md)
- [Verification](docs/VERIFICATION.md)
- [Current work](backlog.md)
- [egui, GPUI and Tauri gap analysis](docs/adr/0003-framework-conformance.md)

Licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
