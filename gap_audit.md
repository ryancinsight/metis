# Open capability and evidence risks

Source baseline and full comparison: [ADR 0003](docs/adr/0003-framework-conformance.md).
Scope: Metis `65e6af1`, inspected 2026-09-05. Implementation status belongs in
[backlog.md](backlog.md); this register contains unresolved risks only.

| Risk | Current evidence | Closure / recheck trigger |
| --- | --- | --- |
| Web support inferred from portable compilation | Only three libraries build to WASM; no executable browser form | [BROWSER](backlog.md#METIS-BROWSER-001): real runtime/DOM interaction and capture. |
| Browser runtime lifecycle treated as complete upstream | Fetched Moirai default `4db2dc1` and Metis-locked `0514f11` have identical browser PAL/driver sources: messages discarded and callbacks forgotten | [ASYNC](backlog.md#METIS-ASYNC-001): recheck published provider and prove bounded receipt/cancel/teardown. |
| Command capabilities mistaken for OS isolation | Windows job lifecycle is implemented; permission denial is not | [AUTHORITY](backlog.md#METIS-AUTHORITY-001) and per-OS desktop denial suites. |
| Golden image preserves missing behavior | Seven software states are captured; browser/OS events and several styles remain unimplemented | [VISUAL](backlog.md#METIS-VISUAL-001), [LAYOUT](backlog.md#METIS-LAYOUT-001): independent value/geometry oracles precede baseline acceptance. |
| Tauri compatibility or toolkit breadth overstated | No migration importer, native APIs, browser host or distribution path | Matrix rows in ADR 0003 close individually through linked items. Re-audit on upstream/API movement. |
| Provider arithmetic issue treated as confirmed consumer exposure | Local allocator multiplication needs checked-boundary review; exact instantiated path is not established | [MEMORY](backlog.md#METIS-MEMORY-001): verify locked and local paths before classifying/fixing exposure. |
| Comparative claims lack instrumentation | No matched process-memory/performance/security comparison ran | [PERF](backlog.md#METIS-PERF-001), [QUALITY](backlog.md#METIS-QUALITY-001): measured baselines and denial probes. |
| New source is not yet pinned by Atlas | Public Metis exists; prepared registration needs an approved review path preserving shared work | [MANUAL](backlog.md#METIS-MANUAL-001), linked Atlas member item. |

A closed risk is removed after its regression oracle or owning decision records
the durable lesson. Compile-only, simulated interaction, real host execution,
visual inspection and measurements are distinct evidence categories.
