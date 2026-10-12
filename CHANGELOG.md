# Changelog

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
