# Changelog

## [0.5.0] - Unreleased

### Breaking

- [major] `metis-platform::Color` and its `metis-ui-lang` re-export implement
  Eunomia 0.10's `Pod`/`Zeroable` traits. These are Eunomia-owned nominal traits;
  callers using the former 0.9 bounds must select the same 0.10 provider identity.
  Pixel layout and values are unchanged. ADR0060 records the public contract and
  uniformly versioned workspace/internal requirements at 0.5.0. RITK consumers
  advance their Metis requirements to 0.5.
- [major] `metis-ui-lang::AssetError` converts Consus 0.3's `DecodeError`.
  Callers crossing that public conversion boundary must use the same Consus
  provider identity; ADR0060 records this alongside the Eunomia migration.

### Fixed

- [patch] Admit current published Gaia0.6.9 at the private mesh-example
  dev-dependency boundary to replace vulnerable ZIP2.4.2 with ZIP9.0.5.
  The archive matches current Gaia Git source; Metis's public Git Moirai0.9.3
  identity and runtime feature/default choices remain unchanged (ADR0060).
- [patch] Preserve the authenticated fragment page's observed state and failing
  phase before closing a failed browser session. Driver timeouts retain their
  failure result and deadline; diagnostics expose the application response.
- [patch] Use the canonical blocked/P1 board fields required by the existing
  plan/conformance gates. Initial full Windows verification identifies the
  malformed status; no validator or baseline changes.
- [patch] Move the unchanged numeric-summary implementation to
  `measurement_statistics.py`, shared by the resource CLI and browser drop
  lifecycle. This removes collision with Unix's preloaded stdlib `resource`
  module. The CLI, statistical formula, assertions and time budgets remain
  unchanged; browser provenance fingerprints the actual shared owner.

- [patch] Reconcile the canonical root and fuzz locks with approved provider
  defaults: Eunomia0.10, Hermes0.9, memory0.10, Moirai0.9.3, Leto0.46,
  Gaia0.6 (example dev dependency) and Consus0.3/Apollo core0.17.
  The earlier checkpoint preserved root registry tuples relative to owner
  `a253d9165a124a6240b87a4ef1359c552ebb4f22`, with 17 earlier updates against
  main. The security renewal changes only Gaia's private registry subtree;
  fuzz retains only the earlier Moirai-required
  rustls0.23.45 to0.23.46 security repair. No production source, feature,
  default, test budget, permission or protected owner branch changes.
- [patch] The current full workspace passes698 native tests with zero skips,
  42 doctests, strict all-feature Clippy/docs, formatting and fuzz-target
  checks. Final API/platform/browser/release/package and exact-head hosted
  gates remain required before delivery. Consus raster 0.3.0 was published
  from `beae4ce7` on 2026-10-19 with its original handoff archive checksum;
  normalized Metis packages and exact-head gates are refreshed before release.
  The old source
  3510db09/raster0.2.0 and f69ca65d/raster0.2.1 instructions are obsolete. No credentials or publisher
  registration are configured. Miri remains deferred.

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
