# Metis user manual

Metis targets Rust applications on desktop and the web, with WASM application
logic and HTML5/CSS presentation. It uses Atlas providers for execution and
rendering. A system WebView is in scope for desktop web compatibility; the
current `metis-app` executable demonstrates backend and software presentation
roles in separate processes, launched from one application executable.

This manual describes the working application surface. The current demonstration
tests separate processes through pipes; the gallery renders real backend sessions
through bounded memory transport into a software framebuffer; and the browser
workbench runs Rust/WASM controls in an HTML5/CSS document. With the documented
loopback service command, the workbench also completes an authenticated
WebSocket handshake and a real backend calculation. The Windows platform crate
exposes a Moirai-backed native pixel and event surface, and the `metis-app`
demonstration connects it to the frontend through the same private process
workflow. Operating-system permissions remain a host gap. The browser shell
enforces its strict CSP, and the service `HostPolicy` binds grants to an exact
origin, window and session. The platform crate also exposes a bounded Windows
WebView2 consumer seam, and `metis-app --metis-webview` demonstrates an
HTML5/CSS form over the same supervised private IPC. The visible installed-runtime
capture is recorded in the native-surface workflow; TLS and OS enforcement remain
separate workflows.

The [target contract](../adr/0002-web-application-contract.md) describes the
Tauri migration goal and required web support. These are implementation targets,
not features available through the build commands below.

- [Build and run](getting-started.md): requirements, the demonstration and verification.
- [Run the Windows native surface](native.md): present a framebuffer and inspect real Win32 events.
- [Build executables and installers](distribution.md): configure an application, create a portable bundle, install and remove it.
- [Use the Python binding](python.md): build and test the typed `metis-rs` PyO3 package.
- [Create a presentation](presentation.md): supported markup, styles and application state.
- [Migrate presentation styles](style-migration.md): handle strict software-style diagnostics and move full CSS to the browser path.
- [Connect a backend](backend.md): process ownership, requests and errors.
- [Application gallery](applications.md): snapshots produced by the actual examples.
- [Inspect application output](testing.md): run visual checks, interpret the current demonstration and review snapshot changes.
- [Run the browser workbench](browser.md): build the WASM host, serve the generated HTML/CSS and inspect real Rust-driven control transitions.
- [Theme and branding](browser.md#theme-and-branding): select a bounded palette, override semantic CSS variables and replace the starter mark.
- [Framework comparison](../adr/0003-framework-conformance.md): source-pinned gaps against Tauri, egui, GPUI and Iced.

Public repository: [ryancinsight/metis](https://github.com/ryancinsight/metis).
For individual Rust API contracts, build `cargo doc --workspace --no-deps`.
Architectural alternatives and security boundaries live in the
[decision records](../adr/README.md), outside this user manual.
