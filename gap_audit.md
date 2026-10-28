# Open capability and evidence risks

## METIS-PYPI-FIRST-RELEASE-001 [patch] — pending publisher; upload not attempted

The user confirms the `metis-ui` registration is pending. Required identity is
`ryancinsight/metis`, `python-release.yml`, environment `pypi`; the environment
exists, but the detailed PyPI binding and expiry are not available without an
account login. No duplicate registration or deadline is inferred.

Manual run 35507532874 validates the complete wheel set and skips publishing.
There is no matching `metis-ui-v0.5.0` GitHub release, and public PyPI JSON returns
404. This is an uncompleted first publication, not evidence of an upload failure.
#508 now merges as 406b13cb21cb6183fa29246af959463bd6cb09c4. Its exact final
head 919e37dc00030a9c87b7975c61cdb7c75ca4e5f1 passes lockfile, debt, Windows,
Apple, native QUIC and aggregate checks in run 40627800325. Browser, fuzz and
the release SemVer gate are skipped. The old run 40264713611 lockfile failure
and root-checklist debt are superseded. The continuation starts at current main,
retains its native promotion item byte-for-byte and adds only the PyPI item;
it does not replay the superseded checklist consolidation.

The member workflow now retains archive/source/hash receipts before upload,
compares the full public PyPI file set after publication and tests the exact
installed wheel. Bounded archive/index and caller regressions and Actionlint
pass locally; no current hosted wheel build or live receipt is claimed. Manual
verification still runs the complete build matrix without uploading. #505's
provider/security upload hold, #509/#551 hook/tool ownership, full source and
Atlas acceptance, final exact-head independent review and the unverified account
binding/expiry remain separate gates. Protected Pi work remains untouched.

## Native QUIC integration — 2026-10-13 [minor]

Current integration preserves consumer e8dd4c9 and main 1ef9409, including
PR 502 provider coherence and lossless audit identifiers. All 423 consumer Rust
and TLS-fixture paths retain exact bytes. Nine existing direct provider pins
and both canonical locks resolve the published Moirai provider revision recorded by metis PR #503; unrelated
identities and the intentional 0.8.3 provider lag remain. The full native
workspace passes 703 tests; default/minimal transport verification passes
178/115 tests. Four real mutually authenticated process pairs cover IPv4/IPv6
and default/explicit queues. Dyadic-input analytical results are 0.54375/2.175
and 0.360703125/2.885625; each matches its audited event and all processes finish
with acknowledged/drained teardown. Direct dependency ownership, all 23 verifier
tests and native diagnostics pass.

The fresh compatibility baseline fails resolution because main's unpinned
Moirai ^0.8.3 requirement sees the provider's newer main. Complete private/hidden
Rustdoc metadata generated from unchanged baseline source and its canonical
lock passes all 588 applicable native API checks. This does not prove generic
Send-bound compatibility; the existing pre-1.0 migration remains documented.
Provider Windows optional-module diagnostics are corrected at their source;
installed Windows default/minimal/WebView2 cross-compilation passes. Fresh
hosted consumer/provider runtime and protected default-branch promotion remain
gates, and the measured controlled-loss performance limit remains open.

Historical consumer e8dd4c9 consumes the applied-UDP provider's shared PAL queue contract,
accepts independent positive receive/send requests and logs effective native
sizes. Invalid and overflowing inputs fail startup. All 178 default and 115
minimal transport/example tests and strict diagnostics pass locally; 23 verifier
tests and actual dependency-ownership checks pass. Both locks change only the
explicit provider revision, preserving unrelated source identities. Four real
IPv4/IPv6 process pairs retain mutual TLS, exact clinical/audited values and
acknowledged/drained shutdown. Socket queues do not resolve the recorded CUBIC
recovery limit. The provider's hosted Linux/Windows native gates pass; the
following provider change partitions unchanged tests into bounded leaf modules.

Metis 0.5.0 consumes Moirai's native QUIC provider through one generic
asynchronous IPC server. QUIC is required in native builds, including with
default features disabled; no QUIC feature flag exists. Real backend/IPC/core
verification passes 176 default tests; minimal IPC/backend builds pass 113 tests.
CI verifies minimal native sessions. Browser application graphs exclude native
QUIC, and browser library diagnostics deny warnings. The
full native workspace/all-target compilation and warning-denying diagnostics
pass against the published provider Git revision without path patches. Independent sessions of one principal require separate fresh MAC
keys, retaining foreign-token rejection as well as sequence isolation. TLS
server trust does not authenticate a client; the runnable example requires
mutual TLS plus exact client-certificate authorization.

The canonical lock pins published provider review source and preserves unrelated
locked dependencies. The default-built IPv4/IPv6 two-process mutual TLS CLI verifies the exact
computed result, audited event and FIN/drain lifecycle. All 588 applicable
default-API compatibility checks pass under the stronger minor override. These
checks do not prove generic-bound compatibility: the pre-1.0 explicit server
annotation/Send migration remains documented in CHANGELOG.md. Existing Iris/Gaia
provider lag retains Moirai 0.8.3, following Atlas overlay policy; no stack-wide
version migration is claimed. Default-branch promotion remains an active gate.
Both hosted Linux and Windows pass 176 default and 113 minimal transport tests,
with strict diagnostics at published consumer revision ed72092. Current-main reconciliation retains its provider/Consus
contracts and lossless native audit identifiers; 703 native tests and synchronized
main/fuzz locks pass. Concurrent test handshakes use the existing Moirai executor;
canonical metadata passes the unchanged direct-provider gate. Dedicated native
transport jobs validate both hosted platforms. HTTP/3/WebTransport application mapping is
separate; loopback correctness does not establish wide-area performance.

The [full consumer gate](https://github.com/ryancinsight/metis/actions/runs/38460334046)
passes at ed72092, including Windows and the three browser engines. The first
Firefox fragment-startup attempt times out; its unchanged retry passes. Provider
measurement executes all 108 controlled pairs, with 105 exact-byte successes
and three 60-second deadlines in the maximum-frame/16-stream 1% loss case.
Kernel UDP errors remain zero; recovery-policy evaluation and provider
promotion remain open. The canonical consumer pin adopts the published
provider evidence and passive metrics API without changing session behavior.

## Standalone Consus adoption [patch]

Target: Metis 0.4.1, Consus compression/raster 0.2 and the delivered Moirai
WebView 0.2 contract. The Consus provider's consumed source trees match the
previous locked codec sources; dependency features and transitive requirements
remain standalone verification inputs. Both locks now select the normally
merged Moirai 0.8.3 provider from [PR 617](https://github.com/ryancinsight/Moirai/pull/617).
Historical evidence: [PR 499 CI](https://github.com/ryancinsight/metis/actions/runs/38412617056)
passes against its earlier consumer base. [PR 499](https://github.com/ryancinsight/metis/pull/499)
also records standalone workspace Clippy and 686 tests against Consus
`cf30dc29` and Moirai `81656cc6`. Those checks do not validate the combined
native-logo and capture/input module changes. The combined tree also corrects
native audit-ID precision: Rust emits decimal JSON strings while retaining
backend `u64` values; six private protocol tests reproduce two baseline failures
and verify full-range identifiers with actual adult/pediatric calculations.
The 213 authored Windows controller lines remain unchanged.
Implementation is complete; [PR 502](https://github.com/ryancinsight/metis/pull/502) owns protected delivery.
It retains PR 499 commits; the old remote head has a stale fuzz lock that
prevents baseline compilation. The normal gate verifies current main instead.
The intended graph retains every recorded provider except verified Moirai
17a680ff. All 713 Rust tests pass in 6.314 seconds; native, Windows and WASM
Clippy deny warnings. Three real Apple bundles preserve version, exact URL
authority and both lock bytes. Registry identities remain 272 root/209 fuzz.
Closure: branch script/CI checks and the normal protected consumer merge.

## RITK consumer verification boundary [patch]

Risk: Metis's passing suite does not validate RITK's independent dependency graph.
Evidence: the earlier [PR 499 CI](https://github.com/ryancinsight/metis/actions/runs/38412617056)
passes; combined Metis 0.4.1 checks above do not validate its independent graph.
Owner: RITK #827. Recheck: regenerate its standalone lock and run consumer gates.

Source baseline and full comparison: [ADR 0003](docs/adr/0003-framework-conformance.md).
Scope: current Metis host, browser bridge, target discovery and the RITK consumer
boundary, inspected 2026-09-20 at Metis `f4b0e35`,
RITK `b91c0f8b866e4799b59cecfccca1c90531d62ab6` and Moirai
`2a54e010532f76c88027fec8a468620c92fe66b3`. The hosted RITK viewport
replay is a separately pinned evidence record: its standalone lock uses Metis
`ab239d70bb561cbe665f852cc55ecbe4275be349` and Moirai
`2a54e010532f76c88027fec8a468620c92fe66b3`. Implementation status
belongs in [backlog.md](backlog.md); this register contains unresolved risks
only. The 2026-10-12 Tauri review is [the pinned native inventory](docs/conformance/tauri.json):
32 surfaces, 124 comparator-admitted pairs, 47 bounded implementation pairs
and 77 open pairs. A bounded pair still has unmet acceptance. The validator
rejects missing owners/boundaries and restrictions on admitted targets.

| Risk | Current evidence | Closure / recheck trigger |
| --- | --- | --- |
| Browser support inferred from compilation | `metis-web` builds and runs a live HTML5/CSS page through Moirai DOM handles. Its explicit asynchronous WebGPU constructors compile against Moirai merge `21b66ba424ad8f50d8574d6e9714be696f807e82` and return typed setup/unsupported errors, but no browser GPU device or visual run is claimed. Hosted RITK run [35512369724](https://github.com/ryancinsight/ritk/actions/runs/35512369724) binds the real 94-file study to four-cycle Chromium and Firefox pixel, file and cleanup oracles, a Chromium application-window capture and a scalar MIP projection; Safari accepts the chooser and fails all bounded reads before presentation. | BROWSER (`METIS-BROWSER-001`), [BROWSER-READ](backlog.md#METIS-BROWSER-READ-001) and [GRAPHICS](backlog.md#METIS-GRAPHICS-001): real GPU capability/output, corrected WebKit file authorization, physical input and provider-private resource evidence. |
| Browser lifecycle treated as complete upstream | Moirai owns DOM callbacks, listeners and cancellable tasks; Metis generation guards reject stale completions before remounted DOM mutation. Edge and Chromium lifecycle traces show clean stop/remount counts, while provider-private resource counts remain outside the Metis surface. | ASYNC (`METIS-ASYNC-001`) and [PERF](backlog.md#METIS-PERF-001): cross-engine service traces and post-drop/repeated-growth measurements. |
| Command capabilities mistaken for OS isolation | `HostPolicy` binds origin, window and session grants; the Windows process lifecycle and native window boundary are implemented. A positive bridge does not prove OS permission denial. | AUTHORITY (`METIS-AUTHORITY-001`) and the per-OS desktop denial suites. |
| Visual baselines preserve missing behavior | Software states and RITK-owned native/browser galleries have value-semantic and pixel oracles. Unsupported browser/OS events, assistive technology and GPU paths remain explicit rather than hidden by a baseline. | VISUAL (`METIS-VISUAL-001`), LAYOUT (`METIS-LAYOUT-001`) and the owning host items. |
| WebView2 pointer input hit-tests at the display scale | Measured 2026-10-12 on the development host (single 3072×1728 monitor at 120 dpi, DPI-unaware process): a trusted `SendInput` click at client (404, 490) hit-tests the packaged page's DOM at (505, 613) — the client point multiplied by `dpi/96` (1.25). A click grid over the dose spinner (sent client y 395–425 → DOM y 494–531, matching the dose label/input at client 474–565 at full scroll) and the compensated pointer-journey submission confirm the mapping; keyboard Enter and the compensated click both produce the audited Rate sentence, while the uncompensated button point (505, 613) lands on page background and submits nothing. The published 96-DPI capture manifest is unaffected (`native-captures.json`, captured at 96 dpi where the factor is 1.0). **Fixed 2026-10-12:** `metis-app` now writes `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` carrying `--force-device-scale-factor=<dpi/96>` (the scale probed under a per-monitor-aware thread context) before the WebView2 environment is created, so Chromium's input mapping and rasterization share one scale over the rendered layout. Verified live at 120 dpi: the uncompensated click at (505, 613) submits and produces the audited Rate sentence (`output/rec/webview-uncomp-*.png/.json`; its after frame differs from the compensated-click success frame by 5,018 pixels ≥32 of 849,230 and from the pre-submit form by 114,980). | METIS-DESKTOP-001 physical display-scale increment: measure the same uncompensated submission through a physical mouse and through a live monitor-scale transition (the forced factor is fixed at environment creation and a move to another scale requires a relaunch), then close the row only when the host maps input to the DOM 1:1 without changes that also relaunch the browser. Recheck on any Moirai WebView2 host, WebView2 loader or DPI-awareness change. |
| DICOM ownership drifts into Metis | Metis exposes a generic file chooser and bounded byte handoff. RITK configures the DICOM label/filter after mounting and owns scanning, decoding, geometry, clinical display and the real study galleries. | DICOM-003 (`METIS-DICOM-003`), DICOM-005 (`METIS-DICOM-005`) and [RITK-SNAP-DICOM-SUBSTRATE-001](../ritk/backlog.md#RITK-SNAP-DICOM-SUBSTRATE-001): recheck on any format-specific Metis change. |
| Tauri compatibility or toolkit breadth overstated | Metis has a Rust/WASM DOM host, one-executable distribution path, Windows WebView2 adapter and typed capability broker. The desktop services Tauri ships as plugins now have bounded counterparts: opener, deep links, file associations, window state, global shortcuts, tray with notifications and menus, single instance and autostart ([INTEGRATION](backlog.md#METIS-INTEGRATION-001)). Tauri API migration, update recovery, installed-OS click-through evidence and non-Windows native hosts remain open. | ADR 0003 matrix rows close through their linked items; re-audit after upstream or API movement. |
| Axum server capability inferred from documentation | Axum is a comparator only. Metis has no Axum dependency; the admitted loopback routes use the first-party Moirai transport with bounded authority, deadlines and typed fragments. | AXUM (`METIS-AXUM-001`) and [ADR 0025](docs/adr/0025-axum-server-boundary.md): public deployment and TLS require their own evidence. |
| Comparative claims lack matched evidence | Three-run eframe and native Métis samples bind public CT/MRI captures, and RITK run [35512369724](https://github.com/ryancinsight/ritk/actions/runs/35512369724) adds four-cycle Chromium/Firefox real-study raster evidence plus viewport controls. The browser runners can record aggregate user-agent memory when the secure API exists. Surfaces and process boundaries differ; no matched GPUI/Tauri, WASM allocator, input-to-frame, compositor or security comparison exists. | [PERF](backlog.md#METIS-PERF-001) and QUALITY (`METIS-QUALITY-001`): provide matched fixtures, controlled hosts and remaining denial/allocation probes. |
| Pull-request verification exceeds five minutes | On 2026-09-23, `gh run list --workflow 352699868 --limit 100 --json databaseId,name,event,conclusion,createdAt,updatedAt` returned 18 newest successful rows filtered to `Metis verification`/`pull_request`; the window is run [35887051832](https://github.com/ryancinsight/metis/actions/runs/35887051832) through [35804754265](https://github.com/ryancinsight/metis/actions/runs/35804754265). For each row use `updatedAt - createdAt` seconds: median = average of ordered observations 9 and 10 = 374.5 s (6.24 min); nearest-rank p90 = observation ceil(0.90 × 18) = 17th = 1,418 s (23.63 min). This successful-run turnaround includes queue/setup time and excludes failures/cancellations, so it is not isolated execution time. Windows gate job [107268967606](https://github.com/ryancinsight/metis/actions/runs/35886820730/job/107268967606) for run [35886820730](https://github.com/ryancinsight/metis/actions/runs/35886820730) ran 5m57s (16:09:53–16:15:50Z); that run’s end-to-end turnaround was 6m01s. `.github/workflows/ci.yml` gives the gate a 20-minute timeout. | Recheck with the same filter after workflow changes; close only when the latest 30 successful `pull_request` runs have a median at or below five minutes with current gate coverage. |
| Comparator inspected from pre-release crates | crates.io's latest stable `freya` is 0.4.3 (2026-08-30). The 0.5 line is `0.5.0-rc.9` and `rc.10` (2026-10-10 and 10-11). ADR 0055 (PR #484, not yet on main) pins the rc.10 sources of `VirtualScrollView` and `freya-testing`, so those rows may change before 0.5.0 is stable. | Re-inspect when `freya-components` 0.5.0 is stable or ADR 0055 is revised; closes when the comparator row is re-pinned. |

A closed risk is removed after its regression oracle or owning decision records
the durable lesson. Compile-only, simulated interaction, real host execution,
visual inspection and measurements are distinct evidence categories.

## Apple local increment — 2026-10-11

The shared Rust/WASM bundle compiles for macOS, device iOS and simulator iOS;
ad-hoc macOS/simulator signing, plist validation and iPhone/iPad installation
and launch succeed; each native host delivers the real WASM resource. The browser computes exact normal, Unicode and empty-name
greetings. CPU calculation and process cleanup pass with the Moirai fix.
Native development reload, compiler-error preservation and terminal-child
cleanup pass against real source changes with exact source restoration.
The macOS icon packaging gap is corrected: declared ICNS bytes and metadata
reach both normal and development bundles. Visible Dock branding remains part
of the pending unlocked-desktop verification.

The Mac is unlocked. Native macOS Greet returns exact normal, Unicode and
empty-name results through the packaged Rust application. Safari-specific
browser input, rotation and suspend/reconnect evidence remain open. The
physical iPhone is unavailable; the Mac has no valid
device-signing identity. Apple's iPad simulator reports duplicate accessibility
classes, which require interactive validation. Android native hosting, Apple capability/permission denial
and the full Tauri plugin matrix remain open. This does not close
METIS-MACOS-001 or METIS-MOBILE-001. The new provider sources are pushed;
standalone locks resolve their exact Git revision without an Atlas overlay.
Path-overlay locks remain derived development artifacts
before a release. Recheck through [backlog.md](backlog.md) and the
[Apple manual](docs/manual/apple.md).

## Rust browser delivery — 2026-10-12

[Metis PR 493](https://github.com/ryancinsight/metis/pull/493) merges after all
Windows, Apple, parser fuzz, API, lock and artifact checks pass. The Rust
shell/HTTP increment is complete and removed from the active backlog.
Moirai PRs 611/612/613 and Gaia PR 169 are merged providers. Current local
macOS Greet also passes after closing and reopening the final 0.3.0 bundle.
The native inventory still has 78 open admitted pairs and 46 bounded pairs
with remaining acceptance; this delivery does not close those owning items.

## Native application/window commands — 2026-10-12

The missing native exit path is reproduced: the AppKit host has standard window
style flags but no application main menu, and focused ⌘Q leaves it running.
Moirai's 0.1.2 adapter installs application, Edit and Window menus in Rust.
Local real-source journeys terminate on ⌘Q and ⌘W, preserve the exact Unicode
greeting through minimize/restore, zoom 800×632 to 1728×1034 and back, and
enter/exit full screen. Actual AppKit notifications provide the state/geometry
oracle; capturing the minimized window re-raises it, and captures may retain
their initial dimensions. Standard buttons have nonzero frames and are enabled
and unhidden in normal mode, while recordings place a purple capture badge over
their area. Unobscured glyph evidence remains separate. The native controls add
no JavaScript; the three initialization modules retain nine authored lines.
General programmable native window APIs and the remaining permission/mobile
capability pairs stay open. Moirai PR 615 merges at
`10a828f7e0a8cace10dd30e88e47ce2e64aec6e0`; all 701 consumer Rust tests pass
against that Git source. Both canonical dependency locks remain byte-identical
through the locked test run. The Tauri inventory counts retain their previous
admitted scope.

The Atlas launcher's unlocked metadata step reproduces lock source flattening.
Its corrected source contract removes dependency patches and locks metadata as
well as compilation. Real Atlas macOS bundling preserves both canonical locks,
verifies the ad-hoc signature and packages exact frontend resource bytes. The
shared cache, root build settings and live frontend paths remain intact.

## Authored JavaScript inventory — 2026-10-12

The nine-line count covers the workbench, HTTP page and starter initialization
modules only. `metis-app` also embeds the Windows WebView2 form (125 lines) and
permission probe (88 lines) in `frontend/webview/assets.rs`: 213 authored
JavaScript lines remain outside those loaders. Counts include blank/comment
lines within the raw-string bodies; generated wasm-bindgen output and test-only
script snippets are separate. METIS-WEBVIEW-RUST-001 owns migration of their
theme, command, form/bridge and capability-probe lifecycle into Rust, with real
Windows acceptance required before retiring the scripts.

## Native logo navigation — 2026-10-13

The starter's logo anchors request `_blank`, while the prior native provider
has no popup handler and rejects external destinations. Metis now declares
optional exact `external_links`, validates them through the owning Moirai
contract and serializes the array into signed Apple bundle metadata. Both logo
destinations match the HTML bytes; no JavaScript behavior is added.
Manifest rejection/bound tests and all-target plist serialization/escaping tests
pass against the delivered provider. The starter suite checks real-page
destination identity. Installed macOS opens the exact Metis URL in Safari and
retains the Unicode Rust greeting. Unlisted/existing-frame/subframe requests
and two executed onload popup attempts produce zero native handoffs; the real
logo control produces one. The portable decision matrix rejects download=true;
cross-origin HTML download attributes do not prove that WebKit flag.
`LinkActivated` is a navigation classification, not trusted-gesture proof.
The full admission contract and expected failures live in
[ADR 0057](docs/adr/0057-native-external-links.md). Physical iPhone execution,
capability denial and remaining native services retain their existing gaps.

## Portable native verification ABI — 2026-10-13

Concurrent default-branch reconciliation exposes two input-layout failures on
macOS: host C `long` is 64-bit, while Windows `LONG` is 32-bit. The canonical
ABI module now defines the documented scalar widths, three real `INPUT` union
arms and naturally aligned pointer fields. Virtual-desktop movement uses
`MOUSEEVENTF_VIRTUALDESK` 0x4000; 0x0100 denotes a different mouse event.
Computed record sizes, offsets, negative coordinates, boundary field values
and composed movement flags pass unchanged prior assertions plus new tests.
Capture/input implementations move into 27 acyclic domains; the largest leaf
has 249 lines, and the two public CLI scripts each have six. Importers use
canonical owners, and the wheel child retains the public entrypoint path.
Compiler binding checks reject local shadowing of real module/helper owners.
Evidence formatting checks eight input combinations, exact labels, hashes,
encoded lengths and observed click/scroll values without substituting Win32
delivery. The full suite passes 424 tests without skips in 15.311 seconds.
This verifies portable record construction and retained tool behavior;
physical Windows delivery and monitor transitions retain their existing
acceptance requirements. [Design](scripts/native_input/design.md).
