# Metis user manual

Metis targets Rust applications on desktop and the web, with WASM application
logic and HTML5/CSS presentation. It uses Atlas providers for execution and
rendering. A system WebView is in scope for desktop web compatibility; the
current `metis-app` executable demonstrates backend and software presentation
roles in separate processes, launched from one application executable.

This manual describes the working application surface. The current demonstration
tests separate processes through pipes; the gallery renders real backend sessions
through bounded memory transport into a software framebuffer; and the browser
workbench runs Rust/WASM controls in an HTML5/CSS document. Metis does not yet
open a native window, connect the browser workbench to an authenticated service,
or restrict operating-system permissions.

The [target contract](../adr/0002-web-application-contract.md) describes the
Tauri migration goal and required web support. These are implementation targets,
not features available through the build commands below.

- [Build and run](getting-started.md): requirements, the demonstration and verification.
- [Build executables and installers](distribution.md): configure an application, create a portable bundle, install and remove it.
- [Create a presentation](presentation.md): supported markup, styles and application state.
- [Connect a backend](backend.md): process ownership, requests and errors.
- [Application gallery](applications.md): snapshots produced by the actual examples.
- [Inspect application output](testing.md): run visual checks, interpret the current demonstration and review snapshot changes.
- [Run the browser workbench](browser.md): build the WASM host, serve the generated HTML/CSS and inspect real Rust-driven control transitions.
- [Framework comparison](../adr/0003-framework-conformance.md): source-pinned gaps against Tauri, egui, GPUI and Iced.

Public repository: [ryancinsight/metis](https://github.com/ryancinsight/metis).
For individual Rust API contracts, build `cargo doc --workspace --no-deps`.
Architectural alternatives and security boundaries live in the
[decision records](../adr/README.md), outside this user manual.
