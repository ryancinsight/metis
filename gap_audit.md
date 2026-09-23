# Open capability and evidence risks

Source baseline and full comparison: [ADR 0003](docs/adr/0003-framework-conformance.md).
Scope: current Metis host, browser bridge, target discovery and the RITK consumer
boundary, inspected 2026-09-20 at Metis `f4b0e35`,
RITK `b91c0f8b866e4799b59cecfccca1c90531d62ab6` and Moirai
`2a54e010532f76c88027fec8a468620c92fe66b3`. The hosted RITK viewport
replay is a separately pinned evidence record: its standalone lock uses Metis
`ab239d70bb561cbe665f852cc55ecbe4275be349` and Moirai
`2a54e010532f76c88027fec8a468620c92fe66b3`. Implementation status
belongs in [backlog.md](backlog.md); this register contains unresolved risks
only.

| Risk | Current evidence | Closure / recheck trigger |
| --- | --- | --- |
| Browser support inferred from compilation | `metis-web` builds and runs a live HTML5/CSS page through Moirai DOM handles. Its explicit asynchronous WebGPU constructors compile against Moirai merge `21b66ba424ad8f50d8574d6e9714be696f807e82` and return typed setup/unsupported errors, but no browser GPU device or visual run is claimed. Hosted RITK run [35512369724](https://github.com/ryancinsight/ritk/actions/runs/35512369724) binds the real 94-file study to four-cycle Chromium and Firefox pixel, file and cleanup oracles, a Chromium application-window capture and a scalar MIP projection; Safari accepts the chooser and fails all bounded reads before presentation. | BROWSER (`METIS-BROWSER-001`), [BROWSER-READ](backlog.md#METIS-BROWSER-READ-001) and [GRAPHICS](backlog.md#METIS-GRAPHICS-001): real GPU capability/output, corrected WebKit file authorization, physical input and provider-private resource evidence. |
| Browser lifecycle treated as complete upstream | Moirai owns DOM callbacks, listeners and cancellable tasks; Metis generation guards reject stale completions before remounted DOM mutation. Edge and Chromium lifecycle traces show clean stop/remount counts, while provider-private resource counts remain outside the Metis surface. | ASYNC (`METIS-ASYNC-001`) and [PERF](backlog.md#METIS-PERF-001): cross-engine service traces and post-drop/repeated-growth measurements. |
| Command capabilities mistaken for OS isolation | `HostPolicy` binds origin, window and session grants; the Windows process lifecycle and native window boundary are implemented. A positive bridge does not prove OS permission denial. | AUTHORITY (`METIS-AUTHORITY-001`) and the per-OS desktop denial suites. |
| Visual baselines preserve missing behavior | Software states and RITK-owned native/browser galleries have value-semantic and pixel oracles. Unsupported browser/OS events, assistive technology and GPU paths remain explicit rather than hidden by a baseline. | VISUAL (`METIS-VISUAL-001`), LAYOUT (`METIS-LAYOUT-001`) and the owning host items. |
| DICOM ownership drifts into Metis | Metis exposes a generic file chooser and bounded byte handoff. RITK configures the DICOM label/filter after mounting and owns scanning, decoding, geometry, clinical display and the real study galleries. | DICOM-003 (`METIS-DICOM-003`), DICOM-005 (`METIS-DICOM-005`) and [RITK-SNAP-DICOM-SUBSTRATE-001](../ritk/backlog.md#RITK-SNAP-DICOM-SUBSTRATE-001): recheck on any format-specific Metis change. |
| Tauri compatibility or toolkit breadth overstated | Metis has a Rust/WASM DOM host, one-executable distribution path, Windows WebView2 adapter and typed capability broker. Tauri API migration, native services, update recovery and non-Windows hosts remain open. | ADR 0003 matrix rows close through their linked items; re-audit after upstream or API movement. |
| Axum server capability inferred from documentation | Axum is a comparator only. Metis has no Axum dependency; the admitted loopback routes use the first-party Moirai transport with bounded authority, deadlines and typed fragments. | AXUM (`METIS-AXUM-001`) and [ADR 0025](docs/adr/0025-axum-server-boundary.md): public deployment and TLS require their own evidence. |
| Comparative claims lack matched evidence | Three-run eframe and native Métis samples bind public CT/MRI captures, and RITK run [35512369724](https://github.com/ryancinsight/ritk/actions/runs/35512369724) adds four-cycle Chromium/Firefox real-study raster evidence plus viewport controls. The browser runners can record aggregate user-agent memory when the secure API exists. Surfaces and process boundaries differ; no matched GPUI/Tauri, WASM allocator, input-to-frame, compositor or security comparison exists. | [PERF](backlog.md#METIS-PERF-001) and QUALITY (`METIS-QUALITY-001`): provide matched fixtures, controlled hosts and remaining denial/allocation probes. |

A closed risk is removed after its regression oracle or owning decision records
the durable lesson. Compile-only, simulated interaction, real host execution,
visual inspection and measurements are distinct evidence categories.
