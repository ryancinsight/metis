# Changelog

## [0.5.0] - Unreleased

### Added

- Required native QUIC sessions in `metis-ipc` and `metis-backend` over Moirai
  0.9.0 and moirai-quic 0.1.0, with `metis-ipc/1` ALPN and full frame-domain
  admission. Independent streams preserve trusted context, fresh MAC key,
  capability, replay, clinical computation, audit and event contracts (ADR 0058).
- `NativeQuic` target capability, synchronous `QuicIpcTransport`, consumed-client
  transport recovery and graceful FIN teardown. A runnable mutual TLS example
  accepts caller-owned certificates/roots and one authorized client identity.

### Changed

- [patch] Reconcile current main and add Linux/Windows native transport checks
  using the canonical main/fuzz provider locks. Preserve Consus/provider coherence
  and lossless native audit-ID encoding.
- [patch] Schedule real QUIC test handshakes through the existing Moirai executor;
  remove the test-only registry dependency rejected by the direct-provider gate.
- [patch] Record successful Linux/Windows native and full Windows/browser gates,
  and pin the published provider's complete controlled-profile evidence and
  passive recovery metrics. Preserve its sustained-loss performance limit as
  an open promotion gate; no workload or deadline change establishes a pass.

- WebSocket and QUIC use one asynchronous IPC server dispatcher through
  `NativeSessionTransport`; the existing frame decoder remains authoritative.
  Validated typed peer-close causes distinguish clean closure from truncation.
- Native session admission helpers share their callers' platform gates, keeping
  browser backend library compilation warning-free.

### Breaking

- Native IPC/backend builds include QUIC and ring's native build requirements
  even with default features disabled. Remove `--features quic` from
  draft-increment commands; native remote-session APIs require no feature flag.
- `AsyncIpcServer<T>` now takes a native message transport as its generic
  parameter. Explicit WebSocket annotations migrate from `AsyncIpcServer<S>` to
  `AsyncIpcServer<WebSocketStream<S>>`; inferred construction remains unchanged.
- Native WebSocket service streams require `Send`, matching the native transport
  future contract. Custom native transports implement asynchronous `send_frame`,
  `recv_frame` and `finish`; no legacy aliases or forwarding API remains.

## [0.4.1] - Unreleased

### Fixed

- [patch] Require Consus compression and raster 0.2, matching their inherited
  workspace version. The consumed codec APIs and feature selections remain
  unchanged; both standalone dependency locks retain Git provider sources.
- Preserve every native audit identifier through the WebView2 JSON object
  boundary by encoding canonical decimal strings in Rust. Actual backend
  calculations and exact integer/rate/error values validate the private wire
  contract; presentation JavaScript remains unchanged.

## [0.4.0] - Unreleased

### Added

- Apple frontend manifests declare optional exact HTTPS `external_links`.
  Moirai 0.2.0 validates their bounds and the signed bundle metadata, then
  hands admitted popup links to the OS browser without replacing the app.
  Both starter logos declare their destinations; no authored JavaScript is
  added. Link activation is a navigation classification, not proof of a
  trusted gesture (ADR 0057).

### Fixed

- Capture/input verification uses the Windows-defined integer widths and real
  `INPUT` union arms on every host. Virtual-desktop movement uses flag 0x4000;
  pointer-sized results and 32-bit `BOOL` returns retain their ABI contracts.
- Capture and input tools have one implementation per domain in acyclic leaf
  modules below 500 lines. All importers target those modules; public CLI
  arguments, wheel-child execution and image/manifest budgets remain intact.
  Compiler binding checks reject shadowed native dependencies; computed
  evidence preserves the original labels and Unicode values. The complete
  dependency-free suite passes 424 tests without skips.

## [0.3.1] - Unreleased

### Fixed

- Correct the Atlas setup documentation: metadata and compilation retain the
  canonical Git locks; local path patches no longer rewrite those sources.
- Require moirai-webview 0.1.2 for Rust-owned native AppKit application, Edit
  and Window menus. Focused Command-Q quits and Command-W closes the last
  window; Minimize, Zoom and full-screen actions use native AppKit behavior.
- Document native geometry/state verification and the capture badge that
  obscures title-bar controls in recordings. No authored JavaScript is added;
  the three WASM initialization modules retain nine lines in total.
- Correct the JavaScript inventory scope: Windows WebView2 still embeds 213
  authored lines for its form and permission probe; their Rust migration is
  tracked separately from the three initialization modules.

## [0.3.0] - Unreleased

### Added

- Rust entry points own workbench configuration/navigation and HTTP host
  start/stop, session, request cancellation and DOM updates (ADR 0056).
- The pinned Tauri inventory reviews 30 official plugins plus two built-in
  surfaces across five native targets: 124 admitted pairs, each with a live
  owner and explicit remaining acceptance. Bounded implementations are not
  complete parity; unsupported comparator pairs alone receive restrictions.
- Real Fetch acceptance checks every streamed byte at the exact body limit,
  excess declared/streamed lengths, Unicode POST, redirect refusal, header/body
  deadlines and peer-observed task cancellation.

### Changed

- Authored application JavaScript falls from 505 to nine lines across the
  workbench, HTTP page and starter. Each module only initializes generated
  wasm-bindgen interop and calls Rust; generated browser bindings remain.
- Both fragment presentations use one Rust preflight/apply implementation
  with a statically selected target policy and authoritative binary codecs.
- Require Moirai PAL 0.8.2 for bounded Fetch and actual Web IDL stream
  dictionaries; Gaia 0.5.2 admits the resulting shared provider graph.
- Browser manuals separate current contracts from historical capture records.

### Fixed

- Reload and remount recover the original live HTTP grant without issuing
  another capability, extending its expiry or changing the command sequence.
- Python extensions use PyO3's build-mode selection. Ordinary Cargo tests link
  the interpreter; maturin 1.14.1 selects extension linking during wheel builds.
- Setup documents use the locked wasm-bindgen CLI version, 0.2.129.
- Resource measurement fixtures resolve their real macOS temporary parent;
  redirected paths and hardlinked output rejection remain unchanged.

### Breaking

- An exact repeated active HTTP handshake returns the original 200 response
  instead of 409. Clients may retry the same admitted principal, process and
  version until the original expiry. A changed active handshake remains 409;
  principal and Origin denials retain their prior rules. No migration changes
  protocol bytes or the underlying one-handshake backend session contract.

## [0.2.1] - Unreleased

### Fixed

- Require moirai-webview 0.1.1, whose Apple resource responses include Fetch
  status 200 and exact MIME/length headers. The previous native host delivers
  WASM bytes with status zero; the loader rejects before binding Greet.
- Native macOS Greet returns exact Rust results for normal, Unicode and empty
  names through the packaged application.

## [0.2.0] - Unreleased

### Added

- `metis-host` packages existing Rust/WASM frontend manifests in Moirai's native
  macOS and iPhone/iPad WebKit lifecycle.
- `metis apple build|run` selects macOS, iOS device or iOS simulator targets,
  stages the same browser asset bytes, and records post-signing file digests.
- macOS `metis dev --watch` owns a supervised host process and reopens the
  window after a successful frontend rebuild.
- Apple application manual and ADR 0055 record setup and verification boundaries.
- Compiler and bundle deployment minima share one contract: macOS 13 and
  iOS/iPadOS 16 supply the WebKit CSP support required by the shared frontend.
- Resource budget and bundle ownership regressions capture hierarchical tracing
  phases and validate the exact limit plus added, missing, modified and linked files.
- The merge gate includes macOS CLI/icon tests and actual macOS, iOS device and
  simulator bundles, with final digest, signature, deployment and asset checks.

### Fixed

- macOS bundles declare and package the application's ICNS shell icon. The
  starter icon derives from its canonical vector at all ten Apple resolutions.
  Development also launches from a signed bundle, retaining application identity
  and icon metadata. Invalid artwork rejects before output replacement.
- macOS tests construct canonical temporary parents instead of following the
  system `/var` symlink through an input path that the contract rejects.
- Starter setup names the actual locked wasm-bindgen CLI version, 0.2.129.
- Windows and Apple CI install the same 0.2.129 loader as the committed lock.
- Browser tooling reads its required loader ABI from the committed lock rather
  than retaining a separate version constant.
- Neutral Cargo execution carries the repository's Rustup pin, preserving
  compiler identity when directory isolation removes Rustup file discovery.
- Bundling rejects assets above the shared provider limit before replacing an
  existing owned output, then validates copied sizes against the same limit.
- Audit persistence tests reserve distinct real directories atomically. macOS
  clock resolution can give concurrent fixtures the same timestamp; a parallel
  authenticated-snapshot test checks independent records and generations.
- The fuzz lock resolves the same provider revisions as the main workspace;
  stale source/version pairs are corrected. The main lock advances only Moirai.
- Verification recognizes the Apple executable role and its tracing boundary,
  while rejecting extra/split targets. Workspace contracts move into their
  own module; all gate/test callers use that authoritative implementation.

### Release gates

Workspace and fuzz locks pin the pushed Moirai provider without path overlays.
Default-branch delivery precedes an Atlas pointer advance. Native
input/lifecycle evidence, physical-device provisioning, and broader mobile
capability closure remain tracked; no release tag is created by this increment.
