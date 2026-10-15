# Open capability and evidence risks

## Latest current-provider validation — 2026-10-15

Consus PR172 merges at `f69ca65d` (0.2.1), repairing Apollo core 0.17
admission. A targeted canonical root update now succeeds, preserving every
registry tuple. It selects Eunomia `15fbb94c` (0.10), Hermes `ac60146a` (0.9),
Moirai `44a962f0` (0.9.3), memory `481c7452` (0.10), Gaia `c0db650e` (0.6),
Leto `8b4ab407` (0.46), and Apollo `f266bb0b` (core 0.17). The separate fuzz
lock uses the same providers; its only registry update is the Moirai-required
rustls 0.23.45 to 0.23.46 security repair.

The unchanged full workspace passes 698 Nextest tests with zero skips,
42 doctests, strict all-feature/all-target Clippy, strict documentation and
formatting on Rust 1.97. All native suites retain the 60-second outer bound;
Nextest configuration and existing tests are unchanged. All fuzz harnesses
check on nightly; its existing protected output-reader site emits one
`fetch_update` deprecation warning and is not edited.

Version0.5.0 and ADR0060 retain the real public Eunomia-owned Pod/Zeroable
identity boundary. Existing draft505 remains the review target. Final API,
Windows/browser, release, normalized-package and exact-head gates remain
open; no full delivery is claimed. Consus raster's first registry upload still
requires separate human authentication. No credentials, publisher registration,
permissions, protected503/504 branches or production Rust are changed.

Earlier provider observations below are historical.

## Current resolver result after Gaia 0.6 delivery — 2026-10-15

Gaia175 is merged at c0db650e and Gaia0.6.0 is source/checksum-verified in the
registry. Its PR/default CI, Pages and publication all pass. A targeted update
of this owned Metis draft now stops at a different provider requirement:
Consus raster0.2, source3510db09, requires Apollo DCT/DST core^0.16.0 while
Apollo's shipped730f6262 default provides0.17.0. The update exits101 in5.47s
without changing Cargo.lock. Preserve the existing199 component-test evidence;
full workspace/package acceptance remains pending this actual source admission.
The Consus owner must reconcile and test the DCT dependency before current-
cohort claims; authentication for its first upload remains a separate handoff.
No old revision pin, source substitution, credential change or protected503/504
source edit occurs. Earlier Gaia-owner/publication blockers below are history.

## Latest independent current-provider validation — 2026-10-15

Byte-identical metis-core/metis-platform members pass 199 native tests with
zero skips on canonical current Git providers: Eunomia0.10 `15fbb94c`,
Hermes0.9 `ac60146a`, Moirai0.9.3 `44a962f0`. The temporary component workspace
keeps member source/manifests, provider requirements/features, workspace
package/lints/profiles and test budgets unchanged. The two real loopback tests
initially hit sandbox bind denial; the identical full suite passes with
loopback permission. No test or production setting changes.

This is component evidence, not complete Metis workspace/platform/API/package
acceptance. Gaia175 remains owned by GitHub author `ryancinsight` at `273eb814`,
branch `build/deps-eunomia-0-10`. Its package fails with 149 errors from mixing
Eunomia0.10 with Leto0.45's Eunomia0.9. An external generic consumer proves the
public trait break; Gaia0.6/Leto^0.46 remains required. The isolated Gaia review
commit `497546bb` passes local native/docs/diagnostic/package gates, with an
owner-applicable patch and no duplicate PR. Preserve protected503/504 source.
Gaia default integration and final full-cohort gates remain pending. Consus
first-upload authentication stays with its separate handoff.

Earlier provider-state observations below are historical.

## Current checkpoint — reconciled Leto merged

Leto PR363 reviewed head `5b6eb6a8` merges at `8b4ab407`, version 0.46.0,
with exactly equal trees and successful exact-head CI. Independent refreshed
validation passes all 1,181 normalized registry-package tests (zero skips),
standard publish dry-runs for both packages, and an unseeded Git-default consumer
with exact arithmetic, LU/residual and real storage-error oracles. Previous
conflict/default-0.45.2 observations are historical. Both published 0.46
archives match their registry checksums and source `8b4ab407`, byte-identical
to the verified dry-runs. Final downstream gates remain pending; no downstream
acceptance is inferred from these provider checks.

New overlapping Gaia PR175 `273eb814` still declares Gaia 0.5.3 and Leto
^0.45.0 while admitting the new numeric providers. Preserve that branch and
this task's isolated Gaia 0.6 / Leto ^0.46 proposal pending ownership/version
reconciliation. This task changes no owner branch, limits, tests or settings.
The exact proof and replay fixture are saved in the task's
`evidence/current-provider-release/leto-363-reconciled-independent-validation.json`.

Earlier provider-state paragraphs below are historical checkpoints.

## Current checkpoint — 2026-10-15

Published Aequitas 0.3, Hermes 0.9, memory 0.10 and consumed Moirai 0.9.3
archives are independently verified. Seven Moirai requirements prepare ^0.9.3
with URLs/features/defaults, locks and Rust unchanged. The public Color marker
trait identity requires Metis 0.5; RITK's incoming edges admit that boundary.
Leto PR363 `ed231333` belongs to the active "Recover FFT layout tests" owner
and conflicts with main `7172e573`, still version 0.45.2 after PR364. Its
intended 0.46 delivery/publication and Gaia 0.6 remain upstream gates. Isolated
Leto registry/consumer checks pass, but no Metis cohort acceptance is claimed.
Protected PR503/504 are untouched. Earlier provider states below are history.

## Current-provider reconciliation remains gated [major]

Aequitas 0.3.0 is verified published at `f9c84f8c`. Leto's next update stops
at Hermes ^0.9 versus default 0.8.2, so no further consumer lock is retried.
Moirai `2b189f0a` needs its owner's memory ^0.9 → ^0.10 admission. Protected
Metis504 is now `b7ad7405`; its source stays separate. Older Aequitas-open and
protected-head observations below are historical.

Static public-contract review identifies Color's Eunomia-owned Pod/Zeroable
implementations and both public export routes. Moving from 0.9 to 0.10 changes
trait identity. ADR0060 prepares the workspace and internal requirements at
0.5.0, replacing the patch-only proposal. Pixel and raster source is unchanged;
final API/runtime checks remain pending. Gaia 0.6 is admitted for the dev-only
mesh-viewer example, and RITK's incoming Metis requirements admit 0.5.

The next uncommitted preparation requires the parent's agreed Eunomia 0.10
target. Parent-owned upstream cohort releases and Gaia/Leto version coordination
precede final root/fuzz lock resolution. Draft #505 run `38683447068` fails its
merge-readiness condition while substantive jobs are skipped; it supplies no
hosted native-regression evidence. Protected #503/#504 remain unchanged.

Eunomia/derive 0.10.0 are now verified published from `a231e54d`; default
`846b2d14` adds release records only. Aequitas 0.3 PR136 remains open at
`287a82b5`; current 0.2.2 requires Eunomia ^0.9. Remaining coherent provider
sources still gate fresh resolution. Required upstream Mnemosyne/Hermes Miri
and Aequitas documentation deployment are now authorized for the upstream owner.
Repaired-package publication and prerequisites are now authorized after
exact-revision checks/reviews, superseding the prior hold. Consus's separate
first-upload owner-authentication handoff remains.
The uncommitted Hermes requirement admits the owner-confirmed 0.9 cohort;
all Metis Rust implementation remains unchanged.
The earlier unavailable-Eunomia-default failure below is historical evidence.

The historical isolated 0.4.2 proposal preserves all default-branch implementation and the
protected page/QUIC branches. Its root graph passes 698 native tests, all-target
all-feature strict Clippy, warning-denied docs and 42 doctests using reviewed
published Eunomia/derive 0.9.2 (`06848422`). This does not close API or release
acceptance: fresh API resolution and further root/fuzz lock updates fail because
Eunomia default `0f4898f8` declares 0.9.1 and exposes a distinct public API.
Maintenance `91e28261` is on `release/eunomia-0.9`; the release owner retains that
reconciliation. Apollo default advances to `97236f7d` after the measured root
lock and remains a required adoption. Consus raster 0.2.0 first publication
requires human-owned bootstrap authentication; no credential/access change or
publication is performed here. See [the board](backlog.md#METIS-CURRENT-PROVIDERS-001).

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
32 surfaces, 124 comparator-admitted pairs, 46 bounded implementation pairs
and 78 open pairs. A bounded pair still has unmet acceptance. The validator
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
