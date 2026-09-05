# Metis user manual

Metis builds Rust applications with a declarative presentation layer and a
separate backend process. It uses Atlas providers for execution and rendering,
without a JavaScript runtime or WebView.

This manual describes the working application surface. The current demonstration
runs through pipes and renders into a software framebuffer. It does not yet open
a native window or restrict operating-system permissions.

- [Build and run](getting-started.md): requirements, the demonstration and verification.
- [Create a presentation](presentation.md): supported markup, styles and application state.
- [Connect a backend](backend.md): process ownership, requests and errors.
- [Application gallery](applications.md): snapshots produced by the actual examples.

Public repository: [ryancinsight/metis](https://github.com/ryancinsight/metis).
For individual Rust API contracts, build `cargo doc --workspace --no-deps`.
Architectural alternatives and security boundaries live in the
[decision records](../adr/README.md), outside this user manual.
