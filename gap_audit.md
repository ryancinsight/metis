# Open capability and evidence risks

Source baseline and full comparison: [ADR 0003](docs/adr/0003-framework-conformance.md).
Scope: current Metis host, browser bridge, target discovery and the RITK consumer
boundary, inspected 2026-09-18 at Metis `eca24b9c8667b4ba4fefcfb57a05697ef6c844f2`,
RITK `a210c7402027d0150711334cd2f1542f7e255315` and Moirai
`22ebfab05b5ca0c3d91a1a367ebf44938838f92c`. The current RITK replay's
standalone lock pins Metis `e634fe925d8edb18cea4399d43255fdaa2d444a5` and
Moirai `a2f21496d1d09b2abe6523e3c8cdbf751dcd560a`. Implementation status
belongs in [backlog.md](backlog.md); this register contains unresolved risks
only.

| Risk | Current evidence | Closure / recheck trigger |
| --- | --- | --- |
| Browser support inferred from compilation | `metis-web` builds and runs a live HTML5/CSS page through Moirai DOM handles. Its explicit asynchronous WebGPU constructors compile against Moirai merge `21b66ba424ad8f50d8574d6e9714be696f807e82` and return typed setup/unsupported errors, but no browser GPU device or visual run is claimed. Hosted RITK run [35089121864](https://github.com/ryancinsight/ritk/actions/runs/35089121864) binds the real 94-file study to Chromium and Firefox pixel, file, rejection and cleanup oracles; Safari accepts the chooser and fails its first bounded read. | [BROWSER](backlog.md#METIS-BROWSER-001), [BROWSER-READ](backlog.md#METIS-BROWSER-READ-001) and [GRAPHICS](backlog.md#METIS-GRAPHICS-001): real GPU capability/output, corrected WebKit file authorization, physical input and provider-private resource evidence. |
| Browser lifecycle treated as complete upstream | Moirai owns DOM callbacks, listeners and cancellable tasks; Metis generation guards reject stale completions before remounted DOM mutation. Edge and Chromium lifecycle traces show clean stop/remount counts, while provider-private resource counts remain outside the Metis surface. | [ASYNC](backlog.md#METIS-ASYNC-001) and [PERF](backlog.md#METIS-PERF-001): cross-engine service traces and post-drop/repeated-growth measurements. |
| Command capabilities mistaken for OS isolation | `HostPolicy` binds origin, window and session grants; the Windows process lifecycle and native window boundary are implemented. A positive bridge does not prove OS permission denial. | [AUTHORITY](backlog.md#METIS-AUTHORITY-001) and the per-OS desktop denial suites. |
| Visual baselines preserve missing behavior | Software states and RITK-owned native/browser galleries have value-semantic and pixel oracles. Unsupported browser/OS events, assistive technology and GPU paths remain explicit rather than hidden by a baseline. | [VISUAL](backlog.md#METIS-VISUAL-001), [LAYOUT](backlog.md#METIS-LAYOUT-001) and the owning host items. |
| DICOM ownership drifts into Metis | Metis exposes a generic file chooser and bounded byte handoff. RITK configures the DICOM label/filter after mounting and owns scanning, decoding, geometry, clinical display and the real study galleries. | [DICOM-003](backlog.md#METIS-DICOM-003), [DICOM-005](backlog.md#METIS-DICOM-005) and [RITK-SNAP-DICOM-SUBSTRATE-001](../ritk/backlog.md#RITK-SNAP-DICOM-SUBSTRATE-001): recheck on any format-specific Metis change. |
| Tauri compatibility or toolkit breadth overstated | Metis has a Rust/WASM DOM host, one-executable distribution path, Windows WebView2 adapter and typed capability broker. Tauri API migration, native services, update recovery and non-Windows hosts remain open. | ADR 0003 matrix rows close through their linked items; re-audit after upstream or API movement. |
| Axum server capability inferred from documentation | Axum is a comparator only. Metis has no Axum dependency; the admitted loopback routes use the first-party Moirai transport with bounded authority, deadlines and typed fragments. | [AXUM](backlog.md#METIS-AXUM-001) and [ADR 0025](docs/adr/0025-axum-server-boundary.md): public deployment and TLS require their own evidence. |
| Comparative claims lack matched evidence | Three-run eframe and native Métis samples bind public CT/MRI captures, and the 2026-09-18 standalone-lock replay binds the current Metis executable to the real 94-file MRI frame. The browser runners can record aggregate user-agent memory when the secure API exists. Surfaces and process boundaries differ; no matched GPUI/Tauri, WASM allocator, input-to-frame, compositor or security comparison exists. | [PERF](backlog.md#METIS-PERF-001) and [QUALITY](backlog.md#METIS-QUALITY-001): provide matched fixtures, controlled hosts and remaining denial/allocation probes. |

A closed risk is removed after its regression oracle or owning decision records
the durable lesson. Compile-only, simulated interaction, real host execution,
visual inspection and measurements are distinct evidence categories.
