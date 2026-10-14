# Tauri native capability review

Reviewed 2026-10-12. The [canonical inventory](../conformance/tauri.json) pins
the official plugin metadata and documentation commits. It covers 30 official
plugins and two built-in surfaces across Windows, Linux, macOS, Android and iOS.

There are 124 admitted native pairs: 46 have a bounded Rust implementation
and 78 remain open. The other 36 pairs are unsupported by the pinned comparator.
Bounded means the named Rust boundary exists; installed behavior, permissions
and the remaining acceptance still require evidence. Browser support does not
establish a corresponding native mobile plugin.

| Capability | Windows | Linux | macOS | Android | iOS | Owner |
| --- | --- | --- | --- | --- | --- | --- |
| autostart | bounded | bounded | bounded | restriction | restriction | [METIS-INTEGRATION-001](../../backlog.md#METIS-INTEGRATION-001) |
| barcode-scanner | restriction | restriction | restriction | open | open | [METIS-MOBILE-001](../../backlog.md#METIS-MOBILE-001) |
| biometric | restriction | restriction | open | open | open | [METIS-MOBILE-001](../../backlog.md#METIS-MOBILE-001) |
| cli | bounded | bounded | bounded | restriction | restriction | [METIS-DISTRIBUTION-003](../../backlog.md#METIS-DISTRIBUTION-003) |
| clipboard-manager | open | open | open | open | open | [METIS-INTEGRATION-001](../../backlog.md#METIS-INTEGRATION-001) |
| deep-link | bounded | bounded | bounded | open | open | [METIS-INTEGRATION-001](../../backlog.md#METIS-INTEGRATION-001) |
| dialog | bounded | open | open | open | open | [METIS-FILES-001](../../backlog.md#METIS-FILES-001) |
| fs | bounded | bounded | bounded | open | open | [METIS-FILES-001](../../backlog.md#METIS-FILES-001) |
| geolocation | restriction | restriction | restriction | open | open | [METIS-MOBILE-001](../../backlog.md#METIS-MOBILE-001) |
| global-shortcut | bounded | open | open | restriction | restriction | [METIS-INTEGRATION-001](../../backlog.md#METIS-INTEGRATION-001) |
| haptics | restriction | restriction | restriction | open | open | [METIS-MOBILE-001](../../backlog.md#METIS-MOBILE-001) |
| http | bounded | bounded | bounded | open | open | [METIS-SERVICES-001](../../backlog.md#METIS-SERVICES-001) |
| localhost | bounded | bounded | bounded | restriction | restriction | [METIS-DISTRIBUTION-003](../../backlog.md#METIS-DISTRIBUTION-003) |
| log | bounded | bounded | bounded | open | open | [METIS-SERVICES-001](../../backlog.md#METIS-SERVICES-001) |
| nfc | restriction | restriction | restriction | open | open | [METIS-MOBILE-001](../../backlog.md#METIS-MOBILE-001) |
| notification | bounded | open | open | open | open | [METIS-INTEGRATION-001](../../backlog.md#METIS-INTEGRATION-001) |
| opener | bounded | bounded | bounded | open | open | [METIS-INTEGRATION-001](../../backlog.md#METIS-INTEGRATION-001) |
| os | bounded | bounded | bounded | open | open | [METIS-SERVICES-001](../../backlog.md#METIS-SERVICES-001) |
| persisted-scope | open | open | open | open | open | [METIS-FILES-001](../../backlog.md#METIS-FILES-001) |
| positioner | bounded | open | open | restriction | restriction | [METIS-INTEGRATION-001](../../backlog.md#METIS-INTEGRATION-001) |
| process | bounded | bounded | bounded | restriction | restriction | [METIS-SERVICES-001](../../backlog.md#METIS-SERVICES-001) |
| shell | bounded | bounded | bounded | open | open | [METIS-SERVICES-001](../../backlog.md#METIS-SERVICES-001) |
| single-instance | bounded | bounded | bounded | restriction | restriction | [METIS-INTEGRATION-001](../../backlog.md#METIS-INTEGRATION-001) |
| sql | open | open | open | open | open | [METIS-FILES-001](../../backlog.md#METIS-FILES-001) |
| store | open | open | open | open | open | [METIS-FILES-001](../../backlog.md#METIS-FILES-001) |
| stronghold | open | open | open | open | open | [METIS-FILES-001](../../backlog.md#METIS-FILES-001) |
| updater | open | open | open | restriction | restriction | [METIS-DISTRIBUTION-004](../../backlog.md#METIS-DISTRIBUTION-004) |
| upload | open | open | open | open | open | [METIS-SERVICES-001](../../backlog.md#METIS-SERVICES-001) |
| websocket | bounded | bounded | bounded | open | open | [METIS-SERVICES-001](../../backlog.md#METIS-SERVICES-001) |
| window-state | bounded | open | open | restriction | restriction | [METIS-INTEGRATION-001](../../backlog.md#METIS-INTEGRATION-001) |
| system-tray | bounded | open | open | restriction | restriction | [METIS-INTEGRATION-001](../../backlog.md#METIS-INTEGRATION-001) |
| window-customization | bounded | open | open | restriction | restriction | [METIS-MACOS-001](../../backlog.md#METIS-MACOS-001) |

The canonical row records full/partial comparator admission, the actual Rust
boundary and remaining acceptance. In particular, biometric authentication
is comparator-supported on macOS as well as mobile and remains open on all
three Metis targets. Persistent scope, SQL, store, encrypted stronghold storage
and signed update recovery retain their existing owners and open gates.

`python scripts/verification/tauri.py` checks coverage, pinned source identity,
live owners, Rust boundaries and the admission/restriction invariant. The
regressions exercise missing sources, owners, files, platforms and forbidden
completion claims against the actual inventory.

Rust browser policy and HTTP migration are specified in [ADR 0056](../adr/0056-rust-browser-shell.md);
[ADR 0059](../adr/0059-windows-rust-pages.md) covers the native Rust pages.
The four production initializers total 359 UTF-8 bytes across twelve physical
lines (nine nonblank). They initialize generated WASM interop and call Rust.
Generated browser bindings and verification tooling remain separate; these
counts do not measure total shipped JavaScript bytes. Actual Windows page and
package acceptance remain open in METIS-WEBVIEW-RUST-001.

Sources: [Tauri plugin inventory](https://v2.tauri.app/plugin/),
[pinned plugin metadata](https://github.com/tauri-apps/plugins-workspace/tree/c91a9e6af5e51d75d03ae289eb69534ef22fb923/plugins),
[pinned documentation](https://github.com/tauri-apps/tauri-docs/tree/26a47c904deceb1d30e7546c8f59fadfc194614b).
