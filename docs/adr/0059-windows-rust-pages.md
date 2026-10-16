# 0059 — Rust pages in the Windows WebView2 host

Status: Accepted

Date: 2026-10-13

Change class: [minor]; internal module restructuring is [arch].

Target: Metis 0.5.0; Moirai PAL/workspace 0.9.0 mechanisms and 0.9.1 private
startup observations.

## Intent and scope

Rust replaces the 125-line form controller and 88-line permission controller
embedded in `metis-app/src/frontend/webview/assets.rs` at Metis `731b00d`.
The HTML, CSS, native window, supervised private IPC and backend authority
remain their respective owners. Authored product JavaScript initializes the
generated wasm-bindgen module and calls its Rust entry point. Generated browser
interoperability supplies the browser ABI; it contains no application policy.

The user's request authorizes this internal Rust migration. Accepted records
the selected design, not implementation completion, release approval or native
acceptance. Implementation starts after the independent Metis 0.4.1 dependency
reconciliation and Moirai Windows capture-bound correction finish. The preserved
QUIC design owns ADR 0058; this decision owns ADR 0059.

## Decision and ownership

Moirai PAL already owns Windows WebView2 hosting and WASM WebIDL bindings.
Its `wasm::webview` domain owns the page-side WebView2 bridge; its
`wasm::permissions` domains own real geolocation, media and notification calls.
The independently versioned `moirai-webview` 0.2 package owns AppKit/UIKit and
WebKit hosting. No Windows bridge or duplicate DOM API is added there.

`metis-web::embedded` owns the form/probe controller and shared JSON codec;
serialized DTOs stay private behind its typed domain messages. It reuses
`Theme`, `ApplicationCommand` and `CommandMenuState` while
preserving the native page's focus, physical-key and status-label behavior.
`metis-app` consumes the same contract and forwards admitted submissions to the
existing frontend/backend boundary. It does not compile to WASM. The page does
not acquire a loopback transport or compute a backend result.

The existing Rust CLI compiles and generates the module before native
compilation. A bounded `metis-core::embedded_assets` integrity contract supplies
one typed inventory to the CLI and application build script. Human-readable
build reports derive from that contract. The build script verifies and embeds
the prepared module; it never launches Cargo. Installed applications need no
compiler, Node process or local web server.

The browser build manifest lives under `.config/browser/`. An explicit optional
`application_root` selects one canonical root for every CLI operation; omission
retains the manifest directory. The bounded locator must resolve an unlinked
directory containing the manifest. Cargo, frontend and resource paths retain
their existing portable relative-path admission. No ancestor search or command-
specific root override changes ownership.

The native host uses the existing PAL `WebViewConfig::folder` with
`https://metis.invalid/`. It admits only packaged resources. Rust mount readiness
gates capture; actual browser outcomes and native permission events gate denial
claims. Readiness is a lifecycle observation, not backend authority or an
attestation of hostile page code.

The [domain design](0059-windows-rust-pages/design.md) fixes the dependency graph,
wire schema, asset format, cancellation behavior, failure modes and executable
acceptance plan. Each implementation leaf remains at most 500 lines.

## Alternatives

Direct WebIDL bindings or JavaScript forwarding in Metis duplicate PAL's
platform boundary. Extending the Apple-only package misplaces Windows and WASM
ownership. Compiling `metis-app` to WASM admits backend composition into the page
dependency closure. Replacing the native private bridge with the browser
WebSocket workbench changes authority and behavior. These options are rejected.

Recursive Cargo in `build.rs` risks workspace target-lock contention and hides
the required generation stage. Checked-in generated snapshots, empty missing-
asset fallbacks and runtime compilation cannot establish build coherence.
Moving the scripts into evaluated strings preserves authored JavaScript policy
and does not perform the selected migration. These options are rejected.

## Version and delivery contract

New PAL mechanisms, Metis WASM entry points, the integrity contract and optional
native manifest declaration add public surface. They are not [patch] changes.
Use Metis 0.5.0 and the Moirai 0.9.0 minor increment after the independent 0.8.3
provider patch. The initial source audit uses locked 0.8.2; it does not replace
the delivery baseline. Uncommitted QUIC version edits do not establish a release.
If 0.9.0 lands first, subsequent additive PAL APIs require the next minor
increment. No version change is required in the independent Apple package.

The private startup observation increment adds no supported public Rust API.
Moirai 0.9.1 owns that [patch] under
[provider ADR 0078](https://github.com/ryancinsight/Moirai/blob/main/docs/adr/0078-private-webview-startup-observation.md).
Metis consumes its protected default revision; the
[co-located startup contract](../../crates/metis-app/src/frontend/webview/event_loop/startup.md)
owns fixture selection, output bounds and the separation from page authority.
The backend supervisor transports that one private selection through its
Runtime environment projection without interpreting it. Its Isolated policy
and exclusion of asset inventories and unrelated application variables remain
intact. The provider remains the schema/budget owner; the frontend logging
owner selects only the dedicated trace target. Real two-hop process tests
verify this boundary. No supported API or production feature is added.

Run canonical `cargo-semver-checks` for changed public libraries. Record the
private page schema migration, build preparation requirement and retired scripts
in CHANGELOG's Breaking subsection for the pre-1.0 release. Update every caller
atomically; retain no compatibility aliases, forwarding loaders or old policies.
Deliver provider mechanisms before the consumer adopts their canonical Git
identity. Synchronize manuals, backlog, gap audit, ADR index and all locks with
the implementation. No retired checklist is recreated.

## Verification status

The provider mechanisms pass their real Chrome resource/decimal contracts and
Windows WebView2 capture, repeated-denial and profile-exit acceptance in
[Moirai PR 619](https://github.com/ryancinsight/Moirai/pull/619). The consumer's
Rust controllers, shared codec and preparation pipeline are implemented;
Metis's compiled-page Windows acceptance remains pending. The existing
framebuffer `native_host_capture` gate does not exercise WebView2. No macOS
build, screenshot or four-row completion message substitutes for the Windows
gate specified in the domain design.

[Moirai PR 620](https://github.com/ryancinsight/Moirai/pull/620) passes the actual
31-case Windows policy/frame suite and six installed-runtime fixtures, including
selected Unicode DOM values, a computed value, a missing selector and pending
request shutdown. Metis's own startup and subsequent journeys still require
their independent Windows acceptance.
