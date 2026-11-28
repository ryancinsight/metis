# 0059 — Rust pages in the Windows WebView2 host

Status: Accepted

Date: 2026-10-13

Revised 2026-11-27 to record Rust-owned initialization, the current checked-in
asset path and the installed Windows outcomes.

Change class: [minor]; internal module restructuring is [arch].

Target: Metis 0.5.0 and Moirai PAL/workspace 0.9.0.

## Intent and scope

Rust replaces the 125-line form controller and 88-line permission controller
embedded in `metis-app/src/frontend/webview/assets.rs` at Metis `731b00d`.
The HTML, CSS, native window, supervised private IPC and backend authority
remain their respective owners. The package generator appends the module
initialization to wasm-bindgen's generated `page.js`; its generated finalizer
calls `wasm.__wbindgen_start()`, which enters Rust through
`#[wasm_bindgen(start)]`. Generated browser interoperability supplies the
browser ABI and initialization; no authored product JavaScript owns page
behavior.

The user's request authorizes this internal Rust migration. Accepted records
the selected design, not release approval or complete native acceptance.
[PR 604](https://github.com/ryancinsight/metis/pull/604) implements the selected
page ownership and records the installed outcomes listed below. The preserved
QUIC design owns ADR 0058; this decision owns ADR 0059.

## Decision and ownership

Moirai PAL already owns Windows WebView2 hosting and WASM WebIDL bindings.
Its `wasm::webview` domain owns the page-side WebView2 bridge; its
`wasm::permissions` domains own real geolocation, media and notification calls.
The independently versioned `moirai-webview` 0.2 package owns AppKit/UIKit and
WebKit hosting. No Windows bridge or duplicate DOM API is added there.

`metis-webview-page` owns the packaged form, permission probe and private
host-reply decoder. The native host encodes audit identifiers as decimal strings
so they retain their full unsigned 64-bit range across the JavaScript boundary.
`metis-app` consumes the same contract and forwards admitted submissions to the
existing frontend/backend boundary. It does not compile to WASM. The page does
not acquire a loopback transport or compute a backend result.

The shipped application embeds the generated `page.js` and `page_bg.wasm`
checked into `metis-app/src/frontend/webview/generated`. `assets.rs` includes
those files at compile time, and `package.rs` writes the same bytes into its
temporary WebView2 package. `scripts/webview_page.py` owns regeneration and its
drift check. Separately, the Rust CLI's `prepare` command generates a bounded
`metis-core::embedded_assets` inventory and a human-readable report. The
current application has no build script and does not consume that inventory.
Connecting the prepared inventory to a verifying application build script and
owned `OUT_DIR` remains the accepted future coherence path. Installed
applications need no compiler, Node process or local web server.

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
the required generation stage. The accepted future coherence path replaces the
current checked-in generated snapshots with verified prepared assets; until
then, the generator drift check and application asset tests guard those
snapshots. Empty missing-asset fallbacks and runtime compilation cannot
establish build coherence.
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

Run canonical `cargo-semver-checks` for changed public libraries. Record the
private page schema migration, build preparation requirement and retired scripts
in CHANGELOG's Breaking subsection for the pre-1.0 release. Update every caller
atomically; retain no compatibility aliases, forwarding loaders or old policies.
Deliver provider mechanisms before the consumer adopts their canonical Git
identity. Synchronize manuals, backlog, gap audit, ADR index and all locks with
the implementation. No retired checklist is recreated.

## Verification status

On an installed Windows WebView2 runtime, changing the form input produced a
different result and the next audit identifier. An invalid input displayed the
host error and released the submit button. The command menu applied the dark
theme. The permission probe observed native denials for geolocation and
notifications and browser rejections for camera and microphone. The remaining
Windows acceptance cases are tracked in the
[page lifecycle item](../../backlog.md#METIS-WEBVIEW-RUST-001).
