# Open capability and evidence risks

Source baseline and full comparison: [ADR 0003](docs/adr/0003-framework-conformance.md).
Scope: Metis host-authority, browser-bridge and target-discovery increments on
`feat/process-foundation`, inspected 2026-09-07. Earlier rows describe the
pre-host baseline; implementation status belongs in
[backlog.md](backlog.md); this register contains unresolved risks only.

| Risk | Current evidence | Closure / recheck trigger |
| --- | --- | --- |
| Web support inferred from portable compilation | `metis-web` builds, packages and runs a live HTML5/CSS form through Moirai DOM handles; the authenticated bridge validates Origin and session binding, and the workbench reports the installed host/browser surfaces | [BROWSER](backlog.md#METIS-BROWSER-001): cross-engine capture, delayed-response injection, teardown and native host coverage. |
| Browser runtime lifecycle treated as complete upstream | Moirai owns bounded callbacks, DOM listeners, WebSocket receipt, timers and cancellable local tasks; Metis adds lifecycle generations that reject stale completions before remounted DOM mutation | [ASYNC](backlog.md#METIS-ASYNC-001): delayed-response injection, post-drop resource evidence and cross-engine service traces. |
| Command capabilities mistaken for OS isolation | `HostPolicy` now binds a canonical origin/window/session to each backend grant; Windows job lifecycle is implemented; permission denial is not | [AUTHORITY](backlog.md#METIS-AUTHORITY-001) and per-OS desktop denial suites. |
| Golden image preserves missing behavior | Seven software states are captured; browser/OS events and several styles remain unimplemented | [VISUAL](backlog.md#METIS-VISUAL-001), [LAYOUT](backlog.md#METIS-LAYOUT-001): independent value/geometry oracles precede baseline acceptance. |
| Tauri compatibility or toolkit breadth overstated | Browser DOM host and one-executable distribution exist; migration importer, native APIs, live service and cross-platform host evidence remain open | Matrix rows in ADR 0003 close individually through linked items. Re-audit on upstream/API movement. |
| Axum server capability inferred from API documentation | Axum's Router, State, extractor, middleware and IntoResponse APIs were inspected as a comparator; Metis has no Axum dependency or HTTP server | [AXUM](backlog.md#METIS-AXUM-001) and [ADR 0025](docs/adr/0025-axum-server-boundary.md): admit a named server target first, then add the first-party bounded route/authority/fragment tests. |
| Comparative claims lack instrumentation | No matched process-memory/performance/security comparison ran | [PERF](backlog.md#METIS-PERF-001), [QUALITY](backlog.md#METIS-QUALITY-001): measured baselines and denial probes. |
| New source is not yet pinned by Atlas | Public Metis exists; prepared registration needs an approved review path preserving shared work | [MANUAL](backlog.md#METIS-MANUAL-001), linked Atlas member item. |

A closed risk is removed after its regression oracle or owning decision records
the durable lesson. Compile-only, simulated interaction, real host execution,
visual inspection and measurements are distinct evidence categories.
