Current framework version: Metis 0.4.1. Remaining capability work follows.

<a id="METIS-WEBVIEW-RUST-001"></a>
## METIS-WEBVIEW-RUST-001 — Rust-owned Windows page lifecycle [minor]

- Status: todo; priority: P1; owner: Metis presentation; last-update: 2026-10-12; risk: application behavior lost during script removal.
- Scope: replace the form's 125 and permission probe's 88 embedded JavaScript lines with Rust/WASM presentation and typed WebView2 bridge bindings; retain generated browser interoperability.
- Acceptance: real Windows journeys preserve theme/command focus, shortcuts, input-sensitive calculation replies, malformed/error replies, request admission and all four capability denials; listeners and permission work detach on teardown.
- Verification: build the packaged pages from the canonical Git lock and compare exact model, reply, focus and capability outcomes before/after; count raw-string scripts as well as standalone modules.
- Design: [ADR 0059](docs/adr/0059-windows-rust-pages.md) defines PAL mechanisms, Rust page ownership and generated-asset coherence for Metis 0.5.0 and Moirai 0.9.0. Implementation and real Windows acceptance remain open.

<a id="METIS-BROWSER-READ-001"></a>
## METIS-BROWSER-READ-001 — Diagnose selected-file reads
- Status: blocked; priority: P0; integrator: root; last-update: 2026-09-22.
- Scope: preserve format-neutral provider read failures and reproduce real chooser reads; DICOM semantics remain in RITK.
- Acceptance: exact failing operation identified, owning fix verified on saved MRI-DIR Chromium/Firefox/WebKit gallery and byte oracles.
- Risk: [patch]; dependencies: merged Moirai bounded reader; current baseline is RITK hosted run `35690425860`, RITK PR #588 merge `27ac7d12bc69a04f7fb12e5ba41ef8fd4fe93aa9`, Metis `ad27d2148cc6b8fa9702cbbd4f37d9e106aa6535` and Moirai `0e2e1bbb2d81e16dd9c694ba46a9e9710e034417`.
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-browser-read-001).
- Blocker: SafariDriver/WebKit selected-file authorization; run 35690425860 reproduces the denial after SafariDriver accepts the chooser request, and no Metis read API can grant the denied access. Re-open when the same real selected file is readable under a corrected browser/runner authorization path. DICOM remains in RITK.

Registration: [Atlas member item](../../backlog.md#atlas-member-registration-defects)
is done. Public source and executable packaging are merged, and the Atlas stack
records the verified Metis and RITK revisions through its gitlinks.

The [framework gap matrix](docs/adr/0003-framework-conformance.md) is the scope
inventory. Every implementation item follows the [visual contract](docs/VERIFICATION.md#visual-contract)
and links its completed, runnable demonstration into the user manual. A screenshot
alone is not acceptance. Status and dependencies live here, not in the manual.

Priority order: P0 correctness/security and evidence foundations, P1 usable
browser/desktop interaction, P2 migration/native services, P3 distribution/mobile
and final comparative closure. An item's dependencies take precedence over its
priority. Ready disjoint items can proceed in parallel; hold one integrating item
per agent. This plan authorizes development/verification, not registry releases,
signing identities, deployments or new third-party hosted services.

Each implementation first pins its required capability/target pairs from the
comparison inventory. Target restrictions close only comparator-unsupported or
explicitly out-of-scope pairs; an unimplemented admitted pair remains open with
an owner. “Unsupported” cannot replace delivery of a required mobile/native API.

<a id="METIS-DESKTOP-001"></a>
## METIS-DESKTOP-001 — Native restricted desktop [arch] [major]
- Status: todo; priority: P1; owner: Metis Windows host + Moirai; last-update: 2026-10-12; ADR: [0032](docs/adr/0032-display-scale.md); dependencies: METIS-AUTHORITY-001, METIS-COMMANDS-001; risk: trust boundary
- [x] [patch] Portable verification ABI and module partition (2026-10-13): capture/input Windows `LONG`/`DWORD`/`BOOL` use exact 32-bit widths; `INPUT` has its real three arms and virtual-desktop movement uses 0x4000. Capture/input share 27 acyclic leaf modules, largest 249 lines; all importers and the public wheel-child path are updated without legacy forwarding helpers. Compiler binding checks reject shadowed dependencies, and eight input combinations preserve computed evidence values. All 424 script tests pass without skips; physical Windows input and heterogeneous-DPI acceptance remain open. [Design](scripts/native_input/design.md).
- Trusted-input evidence increment (2026-10-12): `scripts/python_native_input_capture.py` now builds pointer press/release records with `MOUSEEVENTF_ABSOLUTE|MOUSEEVENTF_MOVE` (absolute grid coordinates were previously sent as relative deltas), re-verifies foreground at each delivered input and no longer masks the original failure when a `finally` detach fails; regression tests cover the record construction (focused suite 23 passed, full dependency-free suite 400 passed with 1 intentional skip). The overlay-resilience guards (`_guard_surface_visible` plus coverer naming) were behaviorally verified under a forced visible `CASCADIA_HOSTING_WINDOW_CLASS` occluder, and the wheel notch→shift response was measured live at [125, 125, 125, 65] logical px per notch (clamping at the 440 px scroll boundary); both contracts are documented in the native manual. All three trusted journeys were re-recorded at 120 dpi (`output/rec/*.json`) and each produces the audited Rate sentence: native Enter, WebView2 click→type→Enter and WebView2 pointer click on **Submit calculation**. The pointer journey required the compensated click point (404, 490): measured on this host the WebView2 surface hit-tests pointer input at `dpi/96` × the client point, so the button's client box (505, 613) is reached only through the compensation and an uncompensated physical click misses by 1.25×. The host display-scale defect is registered in [gap_audit.md](gap_audit.md); direct native-window input, uncompensated pointer submission at non-96 dpi, physical-mouse evidence and heterogeneous-DPI transitions remain open.
- Display-scale fix increment (2026-10-12): `metis-app` now writes `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` carrying `--force-device-scale-factor=<dpi/96>` (the scale probed under a per-monitor-aware thread context) before the WebView2 environment is created, so the browser's input mapping and rasterization share one scale over the rendered layout; scoped `unsafe_code` follows the `entropy.rs` pattern with documented `SAFETY` reasons. Verified live on the 120 dpi development host: the uncompensated click at the button client point (505, 613) submits the audited Rate sentence (`output/rec/webview-uncomp-*.png/.json`); its after frame sits 0.77 mean-intensity from the compensated-click success frame (5,018 pixels ≥32 of 849,230) and 20.55 from the pre-submit form (114,980). The forced factor is fixed at environment creation, so a live monitor-scale transition still needs a relaunch; physical-mouse evidence and heterogeneous-DPI transitions stay open in [gap_audit.md](gap_audit.md).
- Permission matrix capture increment (2026-09-22): the WebView2 capture role now waits for the typed `permission_probe_complete` message after all four bounded capability requests settle, then closes the supervised session. Metis commit `543edfb533be0c0422c38445174001b616c96ace` and Moirai `0e2e1bbb2d81e16dd9c694ba46a9e9710e034417` produce a 1025×769 CapturePreview PNG (9,265 bytes, SHA-256 `0f36794bea61fe540e7a4603b2c56d943718344c68c4e46aa3b0d839e7ab3a36`) showing all four completed rows: geolocation and notifications host-denied, camera and microphone browser-rejected. Focused `metis-app` nextest passes 35/35 with strict Clippy; dependency-free script tests pass 331/331 with one intentional skip. The prior geolocation-only image is superseded by this matrix capture; file/network/process denial, installed IME, screen-reader traversal, physical heterogeneous DPI and non-Windows host evidence remain open.
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-desktop-001).
- Scope: Windows native window/system WebView, real events, multi-window lifecycle and OS-restricted renderer; macOS/Linux have separate items below.
- Acceptance: actual visible form, pointer/keyboard/resize/DPI/close/reopen; file/network/process denial probes; IPC remains functional under restrictions and all child processes drain.
- Demonstration: [V05](docs/VERIFICATION.md#V05), actual Windows window captures, keyboard journey and permission-denied results in the manual.
- Current evidence: Moirai PR #287 (`7ad8eeee`, following PRs #286 and #284) supplies the real Win32 HWND, bounded message translation including IME composition phases, retained ARGB presentation, finite queue waiting and retained-event readiness. `metis-platform::native::NativeSurface` is the safe consumer boundary; `PlatformSurface` and `PlatformEvent` remain portable application-supplied values.
- Follow-on provider: Moirai PR #299 merged to `main` at `f4eb4f2b` and its board item is done. It adds the thread-affine WebView2 host, packaged `file:///` navigation policy, bounded WebMessageReceived bridge and callback teardown. Its installed-runtime smoke passes on WebView2 `152.0.4191.66`, and the visible Metis bundle journey is captured in the linked manifest. No registry or signing key is part of that provider.
- Permission matrix increment (2026-09-22): the separate permission-probe page makes bounded sequential requests for geolocation, camera, microphone and notifications. It renders one row per capability, preserves the provider's typed `permission_denied` payload including `user_initiated`, and distinguishes host denial from browser rejection or an unavailable API. The capture role waits for the typed completion message and now records the four-row matrix in a 1025×769 CapturePreview; file/network/process denial, physical display-scale, screen-reader traversal and installed-IME evidence remain open.
- Theme increment (2026-09-20): the packaged WebView2 form now uses the same four bounded presentation modes as the browser workbench (`system`, `light`, `dark` and `high-contrast`). A page-local selector changes only CSS variables and the document theme attribute; it cannot reach the bridge or change backend authority. Static asset tests assert every option, palette selector and fail-closed guard. The bounded `--metis-webview-theme-capture` role produced inspected `CapturePreview` PNGs for all four modes at 1025×769; the hashes and manual links are recorded in [`native.md`](docs/manual/native.md#packaged-webview2-window). Visible palette evidence is delivered; screen-reader, installed-IME, physical display-scale and broader OS-enforcement evidence remain open.
- Permission event increment (2026-09-17): Moirai PR #392 merged at `64d5cdc1465542748d2261663284c9c357850541` and PR #394 merged at `b94f3ed7a0faa436ebe993dbfec49726cef853fa`; the provider denies WebView2 requests synchronously and emits typed `PermissionDenied` values, while Metis maps them to `permission_denied` with `ErrorCode::PermissionDenied`. The ignored geolocation smoke and frontend serialization test pass; the focused consumer gate passes 84/84 with strict Clippy. Broader OS enforcement and a visible permission-probe capture remain open.
- Capture increment (2026-09-17): Metis source `285322892aca245faf3962d57838682dd6689d77` consumes Moirai `d324018efa3b67d2b92350a4e3c781d179019014` `CapturePreview`; the supervised WebView2 permission page was captured on runtime `153.0.4234.32` at 1024×768, 6,561 bytes, SHA-256 `a9df158ff6a167ed3c708e9621a44446cae93b3c0d87e646c818601606a33b8f`. The inspected artifact is linked from the [native manual](docs/manual/native.md#webview2-permission-probe-capture) and manifest; broader OS enforcement, physical display-scale, native accessibility, installed IME and non-Windows hosts remain open.
- Physical-monitor increment (2026-09-19): `scripts/python_native_capture.py --move-monitor` now enumerates attached monitors, moves a real HWND, captures both images and records effective DPI plus fixed-point scale. `--require-dpi-change` fails closed when the host cannot produce a heterogeneous-scale transition; deterministic mapping remains the oracle and equal-scale/single-monitor hosts remain an explicit residual. The development host exercised the real command and reported one 3072×1728 monitor at 96 DPI, identical initial/moved PNG digest `a021194e4ef9406ee6789a9ea2eea00d25223e06d8d6f13e65975e9004acb442`, and `dpi_changed: false`; no transition claim is made.
- Lease discharged: the bounded monitor probe, parser contracts, manual, V05 verification record and ADR 0032 are synchronized in this increment; native accessibility, installed IME, broader OS permissions and non-Windows hosts remain open.
- Completed increments: the adapter test creates a real hidden HWND, presents the production framebuffer, observes resize and closes the window; the `metis-app --metis-native-window` role now composes that surface with the real frontend and supervised private IPC, handling text, transient IME preedit/commit/cancel, Enter/click submit, resize, DPI, focus and close; focused nextest and warning-denied Clippy pass on Windows. The adapter now rejects reopening a live surface and recreates a closed surface from its validated configuration; the Windows lifecycle regression test closes and reopens a real hidden HWND. A two-window regression test presents separate frames, verifies independent event batches and confirms closing one leaves the other live.
- Decision: [ADR 0015](docs/adr/0015-native-window-provider.md) (accepted); committed visual captures cover the current native/WebView2 initial and submit journeys plus the native framebuffer resize. Installed IME, WebView2 composition, broader OS permission enforcement, visible permission-probe, accessibility and macOS/Linux providers remain open follow-on slices.

<a id="METIS-RELEASE-001"></a>
## METIS-RELEASE-001 — Publication readiness [patch]
- Status: blocked; priority: P3; owner: Metis delivery; integrator: root; last-update: 2026-09-17; dependencies: METIS-DISTRIBUTION-001, METIS-CONFORMANCE-001; blocker: crates.io trusted-publisher registration and explicit release authority are external; re-open: registrations and release authority are available without adding repository secrets
- Scope: release-readiness metadata, package dry runs, final manual and platform evidence; registry authentication uses Atlas OIDC workflows without personal keys; preparation continues without release authority.
- Acceptance: dependency-closed packages and exact-revision evidence; release execution is blocked until explicit authority, registry publisher registration, rollout and rollback details are available.
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-release-001).
- Recheck (2026-09-09): the Metis remote is HTTPS, local Git has no signing or SSH-key configuration, and the checked-in callers still contain no registry secret path. GitHub API inspection found zero Actions secrets or variables and two empty environments, `crates-io` and `pypi`, without protection rules. Trusted-publisher registration and release authority remain external; no key or token is requested from the developer. The visible browser is signed out.
- Clarification (2026-09-09, `27853cc`): runtime IPC authentication uses an ephemeral symmetric session MAC key; it is never persisted or exposed to CI and is unrelated to registry or signing credentials.
- Demonstration: [V10](docs/VERIFICATION.md#V10), locally built package installation/recovery instructions and actual captures before any publication.

<a id="METIS-INPUT-001"></a>
## METIS-INPUT-001 — Interactive controls and shared UI state [minor]
- Status: todo; priority: P1; owner: Metis UI/host; last-update: 2026-09-18; dependencies: METIS-BROWSER-001, MOI-WINDOW-KEY-MODIFIERS-2026-09-15; risk: native shortcut modifier state loss; ADR: 0014, 0027
- Scope: buttons, checks, radios, sliders, editable fields, select/menu/dialog controls; focus, pointer capture, drag/drop, wheel/touch/modifiers, shortcuts, reusable state/actions and subscription teardown.
- Acceptance: keyboard and pointer/touch journeys update identical model values; disabled controls reject action; focus survives rerender and subscriptions detach on close; real hit targets agree with rendered geometry.
- Demonstration: [V02](docs/VERIFICATION.md#V02), settings workbench; repeat on each native host as it becomes supported.
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-input-001).
- Live evidence: [browser control evidence](docs/VERIFICATION.md#browser-control-evidence--2026-09-07) records pointer radio/visibility changes, two keyboard range steps and a native select change in the authenticated service trace. Presentation changes preserve the correlated backend result.
- Live evidence: [browser control evidence](docs/VERIFICATION.md#browser-control-evidence--2026-09-07) records the disabled accessibility state before connection, the pending-state disable during a delayed response, re-enablement after the correlated result, and unchanged state after activation was rejected.
- Live evidence: [browser dialog evidence](docs/VERIFICATION.md#browser-dialog-evidence--2026-09-07) records the authenticated modal screenshot, accessibility content, provider open-state read, close action, Escape dismissal and opener focus restoration.
- Live evidence: [browser pointer-capture evidence](docs/VERIFICATION.md#browser-pointer-capture-evidence--2026-09-07) records pointer ID `1`, release status, semantic surface name and the rendered surface screenshot.
- Live evidence: [browser pointer metadata evidence](docs/VERIFICATION.md#browser-pointer-metadata-evidence--2026-09-07) records the input-sensitive status and screenshot; the bounded gesture policy is recorded in [browser gesture policy evidence](docs/VERIFICATION.md#browser-gesture-policy-evidence--2026-09-07).
- Live evidence: [browser wheel metadata evidence](docs/VERIFICATION.md#browser-wheel-metadata-evidence--2026-09-07) records input-sensitive vertical and horizontal scroll actions and the rendered status; the automation trust limitation is explicit.
- Live evidence: [browser gesture policy evidence](docs/VERIFICATION.md#browser-gesture-policy-evidence--2026-09-07) records vertical/horizontal wheel pan and a pointer drag with the transformed content; the CUA hardware-trust limitation is explicit.
- Superseded increment (before the format-neutral handoff): the browser host read a bounded prefix through a provider-owned handle and rendered `reading`/`complete`/`failed` states. `METIS-DICOM-002` removed the format classification and replaced that path with a generic byte handoff.
- Live evidence: [browser file-drop evidence](docs/VERIFICATION.md#browser-file-drop-evidence--2026-09-08) records the native policy suite, batch ownership tests, strict WASM checks and rendered drop-zone state; CUA cannot provide trusted OS file-drop evidence or a live byte-read trace.
- Browser keyboard snapshot increment (2026-09-14): merged Metis `02d4047c5567834667ab9beb796ea27f6257f0ad` records an `after-keyboard` canvas snapshot after each focused `ArrowDown` pair. The RITK consumer can therefore validate wheel progression from the state immediately before the wheel instead of comparing a keyboard-plus-wheel final state with the initial state.
- Residuals: trusted physical file-drop evidence, Safari/WebKit selected-file authorization for saved-study reads, installed IME journeys, screen-reader traversal, provider-private resources, post-drop allocation measurement and native-host visual evidence remain open. RITK real-study opening and decoding now pass on Chromium and Firefox in [run 35089121864](https://github.com/ryancinsight/ritk/actions/runs/35089121864); re-open this item when the remaining dependencies land.

<a id="METIS-TEXT-001"></a>
## METIS-TEXT-001 — Text, selection and IME [minor]
- Status: todo; priority: P1; owner: Metis input/presentation; last-update: 2026-09-22; dependencies: METIS-INPUT-001; risk: text corruption
- Scope: DOM text first; grapheme selection, composition/preedit/commit/cancel, clipboard/undo, wrapping, fallback fonts, bidi and text scaling. Custom renderer requires its own admitted text contract.
- Acceptance: Unicode fixture strings/selection ranges and caret/line geometry match the contract; native IME exercised per OS, including CJK, combining marks, emoji and mixed-direction input.
- Demonstration: [V03](docs/VERIFICATION.md#V03), editing specimen with actual composition and committed captures, locale/font details and keyboard instructions.
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-text-001).
- Lease discharged: `scripts/browser_text_geometry.py`, `scripts/browser_text_stability.py`, `scripts/browser_text_font.py`, `scripts/tests/test_browser_text_geometry.py`, `scripts/tests/test_browser_text_stability.py`, `docs/manual/browser.md`, `docs/VERIFICATION.md` and `backlog.md`.
- Residuals: selected fallback-face identity, cross-engine and native bidi/line/font geometry, trusted clipboard contents/permissions, an installed CJK or other native IME journey and assistive-technology acceptance remain open.

<a id="METIS-A11Y-001"></a>
## METIS-A11Y-001 — Accessible application interaction [minor]
- Status: todo; priority: P1; owner: Metis host/UI; last-update: 2026-09-22; dependencies: METIS-INPUT-001; risk: inaccessible controls
- Scope: semantic DOM roles/names/states, focus/action mapping, announcements, reduced motion/high contrast/zoom; OS accessibility bridge for any custom UI path.
- Acceptance: semantic tree identity/actions and keyboard-only completion pass; actual supported screen readers traverse and operate the application; document platform limits instead of claiming certification from tree presence.
- Demonstration: [V03](docs/VERIFICATION.md#V03), readable focus/contrast/zoom captures plus semantic/action and assistive-technology evidence.
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-a11y-001).
- Upstream delivery (2026-09-22): Moirai PR [#432](https://github.com/ryancinsight/Moirai/pull/432), merge `0e2e1bbb2d81e16dd9c694ba46a9e9710e034417`, adds the matching bounded native roles and AccessKit mappings. `metis-app` now projects the five command-surface roles through that contract; focused Metis tests pass 55/55 in `metis-ui-lang` and 33/33 in `metis-app`.
- Lease discharged: `scripts/python_native_accessibility.py`, `scripts/tests/test_python_native_accessibility.py`, `docs/manual/native.md`, `docs/VERIFICATION.md` and this item are synchronized in this increment.
- Gate correction (2026-09-21): the hosted Windows replay exposed a Node diagnostic subprocess that kept stdout open after bounded cleanup, causing the delayed stream-cancel test to hit its ten-second parent deadline. The test harness now closes stdout and exits only after the serialized result is flushed; the three diagnostic fault cases pass locally, including 25 repeated delayed-cancel runs.
- Visual baseline correction (2026-09-21): hosted replay 35607367715 passed every pixel and semantic comparison and reported only the derived fixture digest as stale; commit 23dd3e4 records fixture SHA-256 16c7f5cae9f02eb0c47bc412499ef54b9a79762b692cfcd5a7f79392a7e93a36, with rerun 35608777745 queued.
- Residuals: supported screen-reader speech, host-specific forced-colors/reduced-motion acceptance and installed-client traversal remain open.

- Native shortcut announcement (2026-09-24): Moirai PR #460 (merge `7f6ca66e`) adds `AccessibilityNode::set_keyboard_shortcut`, projected into AccessKit's `keyboard_shortcut`; the Windows native host now passes each semantic node's `aria-keyshortcuts` through it, so UI Automation clients can announce the Alt+Shift command shortcuts. Screen-reader speech remains installed-host evidence.

<a id="METIS-MACOS-001"></a>
## METIS-MACOS-001 — macOS restricted desktop [arch] [minor]
- Status: todo; priority: P1; owner: Metis macOS host + Moirai; dependencies: METIS-DESKTOP-001; risk: OS boundary
- Scope: native window/WebView events, accessibility/IME integration, process containment, sandbox restrictions and teardown; share broker semantics, not Windows implementation details.
- Acceptance: host-native [V05](docs/VERIFICATION.md#V05) plus unauthorized file/network/process denial, child drain and positive IPC; cross compilation alone cannot close this item.
- [x] [patch] Native application/window menus: moirai-webview 0.1.2 handles Quit, Close, Minimize, Zoom and full screen in Rust. Focused ⌘Q/⌘W exit, exact Unicode output and actual geometry/state pass; capture-obscured glyphs remain separate visual evidence.
- [x] [minor] Native external logo links: Metis 0.4.0/moirai-webview 0.2.0 validate signed exact HTTPS authority and hand popup links to the OS in Rust. Manifest/plist/page tests pass; installed macOS opens the exact Metis URL and retains Unicode Greet. Real unlisted/existing-frame/subframe links and two onload popup attempts produce zero handoffs; the permitted control produces one. [ADR 0057](docs/adr/0057-native-external-links.md).
- Demonstration: actual macOS window/permission captures and manual setup/troubleshooting from the tested revision.
- Local increment (2026-10-11) [minor]: shared frontend packages in a Moirai AppKit/WKWebView host; `apple build|run` and macOS `dev --watch` exist. Ad-hoc bundle validation and process calculation/cleanup pass. Native interaction and OS-denial evidence remain open; [manual](docs/manual/apple.md), [ADR 0055](docs/adr/0055-apple-application-bundles.md).
- macOS identity correction (2026-10-11) [patch]: the starter declares its vector-derived ICNS; packaging copies it and declares `CFBundleIconFile`. Development uses the same signed bundle path. All ten decoded dimensions and exact interior colors pass; malformed, oversized, linked and missing icons preserve every owned output byte. Dock observation requires an unlocked desktop.
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-macos-001).
- Native Greet correction (2026-10-12) [patch]: target Metis 0.2.1; require moirai-webview 0.1.1's status/MIME/length response contract. Actual macOS clicks return exact Rust greetings for normal, Unicode and empty names; eight real provider tests and three Apple ABI checks pass.

<a id="METIS-LINUX-001"></a>
## METIS-LINUX-001 — Linux restricted desktop [arch] [minor]
- Status: todo; priority: P1; owner: Metis Linux host + Moirai; dependencies: METIS-DESKTOP-001; risk: OS/display boundary
- Scope: native WebView hosting under Wayland and X11, input/accessibility/IME, process containment, permission restrictions and bounded teardown.
- Acceptance: [V05](docs/VERIFICATION.md#V05) on both display paths, positive IPC and unauthorized file/network/process denial; unavailable prerequisites produce actionable errors, never sandbox bypass.
- Demonstration: actual captures and distribution/display prerequisites in the manual; no support inferred from a Linux build.
- Portability increment (2026-09-24): the workspace now builds, lints (`clippy -D warnings`) and passes its script suites on Linux, and `metis dev --watch` polls a bounded fingerprint where no native notifier exists. Moirai PR #458 (merge `693350c7`) contains process trees in a POSIX process group on Linux, Android and Apple targets and lets `set_value` select a `<select>` option; with the lock advanced, all 548 workspace tests pass on Linux, including the three `metis-app` process tests, and the browser theme commands use the provider instead of rebuilding options. A parent crash still leaves a Unix group unsignalled, unlike a kill-on-close job.

<a id="METIS-ASSETS-001"></a>
## METIS-ASSETS-001 — Images, vectors and media assets [major]
- Closure: integrator root; Consus [PR 80](https://github.com/ryancinsight/consus/pull/80) supplies Apollo-owned transforms and arithmetic/precision support. Focused nextest passes 53/53, strict Clippy passes, and refreshed V06 matches all 464,000 pixels; full gate and integration remain.
- Native JPEG increment: [PR 312](https://github.com/ryancinsight/metis/pull/312) uses merged [Consus PR 76](https://github.com/ryancinsight/consus/pull/76), with [RITK](../ritk/backlog.md#RITK-JPEG-001) owning medical conversion. The integrated full gate passes 386 debug and 386 release tests; native V06 matches all 464,000 client RGB pixels at 120 DPI. Broader media/GPU acceptance remains open.
- Native PNG increment (2026-09-19): bounded scoped loading, strict complete zlib/PNG decoding, orientation-aware contain and value-semantic malformed/alpha/aspect/orientation fixtures; [ADR 0029](docs/adr/0029-image-orientation.md) and [native V06 gallery](docs/manual/native.md#native-image-assets) own the contract and exact client-pixel capture.
- Media evidence increment (2026-09-21): reviewed browser and native captures now use lossless WebP/PNG assets under the 200 KiB tracked-image budget, the Python capture manifest names the retained files, and the Metis mark is a bounded PNG alternate. The real MRI-DIR CT gallery remains 409 files/216,156,416 bytes with non-black axial, coronal and sagittal views; hosted Windows and lockfile checks passed in [PR #322](https://github.com/ryancinsight/metis/pull/322), merged at `6633064`.
- Status: todo; priority: P1; owner: Metis asset/presentation + existing Atlas format providers; last-update: 2026-09-21; dependencies: METIS-BROWSER-001, METIS-AUTHORITY-001; risk: hostile content
- ADR: [0019](docs/adr/0019-raster-display-command.md); the public `DisplayCommand` enum addition is a major release change and has no version bump until release authority opens a release increment.
- Scope: bounded local asset loading, image/SVG presentation, font loading and browser audio/video controls; validate paths/origins, dimensions/decoding budgets and target permissions.
- Acceptance: malformed/truncated/oversized/traversal assets fail; declared colors/alpha/aspect ratio/orientation match fixtures; media error and teardown states release resources.
- Demonstration: [V06](docs/VERIFICATION.md#V06), actual asset gallery with source attribution and load/error states.
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-assets-001).
- Lease discharged: `metis.json`, `examples/browser/assets/metis-probe.woff2`, `scripts/browser_font.py`, `scripts/browser_runtime.py`, `scripts/browser_runtime_cli.py`, `scripts/browser_runtime_probes.py`, `scripts/tests/test_browser_font.py`, `scripts/tests/test_browser_assets.py`, and `scripts/tests/test_browser_runtime.py`.
- Residuals: native formats beyond the admitted PNG/JPEG subset and clinical orientation metadata, selected fallback-face identity, cross-engine/native font metrics, video playback/native media providers, GPU vectors, and runtime shell rendering on a Windows install remain open under V06; browser local-mark decode, font resource loading, audio playback, media error/teardown, SVG admission, ICO packaging and format-neutral pixel-grid orientation contracts are closed.

<a id="METIS-GRAPHICS-001"></a>
## METIS-GRAPHICS-001 — Custom graphics conformance [arch] [minor]
- Status: todo; priority: P2; owner: Metis custom renderer over Iris; last-update: 2026-09-13; dependencies: METIS-VISUAL-001, METIS-LAYOUT-001; risk: rendering/lifetime correctness; delivery: Metis PR #97, merge `81d7dff`
- Scope: admitted vector/image/transform/clip operations and accelerated display path where required by the custom-UI demonstrator; retain one rendering contract and verify Atlas GPU ownership before additions.
- Acceptance: geometry/color/alpha and device-loss/recreate tests; differential software/device output under justified raster bounds; measured profile justifies acceleration and accounts for memory cost.
- Demonstration: [V06](docs/VERIFICATION.md#V06); this path never gates the DOM/browser migration and cannot stand in for HTML5 compatibility.
- Recovery increment (2026-09-19): the real Edge/NVIDIA device-loss scenario exposes false-success presentation after loss; Moirai `98579514` corrects the owning observer. The repeated gate reports typed loss, creates device 2, checks all 1,024 changed pixels, retains the canvas and seven input guards, then records zero guards and session closure. [ADR 0036](docs/adr/0036-browser-webgpu-canvas.md), [V06](docs/VERIFICATION.md#V06); bounded trace and PNGs under `output/browser/gpu-recovery` bind source/lock/WASM digests.
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-graphics-001).
- Residuals: spontaneous driver-reset and other-browser recovery, alpha/vector parity, a measured GPU profile and GPU resource evidence remain open; deliberate device destruction and opaque recovered pixels pass on Edge/NVIDIA.

<a id="METIS-FILES-001"></a>
## METIS-FILES-001 — Scoped files and persistent state [minor]
- Status: todo; priority: P2; owner: Metis broker + owning Atlas storage provider; last-update: 2026-09-15; dependencies: METIS-DESKTOP-001; risk: user data/TOCTOU
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-files-001).
- Scope: file dialogs, reads/writes/watch, settings/store/database contract and versioned recovery with explicit scope; no raw frontend access to unrestricted paths.
- Acceptance: allowed-handle operations succeed; traversal/symlink/TOCTOU and denied scope fail; atomic writes, crash/disk-full recovery and watcher teardown preserve user data.
- Demonstration: [V08](docs/VERIFICATION.md#V08), document open/save/restart/denial on each supported target; browsers expose selected-file semantics or explicit restrictions.
- Increment (2026-09-14): `metis-platform::native::pick` re-exports Moirai's bounded Windows common-dialog selection seam. Metis returns only a user-selected path; browser file handles and application format policy remain separate. Moirai ADR 0058 owns COM/task-memory lifetime and cancellation semantics.
- Increment (2026-09-15): RITK PR [#392](https://github.com/ryancinsight/ritk/pull/392),
  merge `3f46a08bf`, consumes `pick(DialogSelection::Folder)` from the native
  Métis viewer session for `Ctrl+O` study reopen. The selected folder is
  scanned and decoded by RITK; cancellation and unreadable-study errors leave
  the prior framebuffer intact. The saved MRI picker window and provenance are
  recorded in the [RITK DICOM manual](https://github.com/ryancinsight/ritk/blob/main/docs/manual/dicom-workflow.md).
  This closes the Windows selection-consumer increment; persistent storage,
  native permission policy and non-Windows providers remain in scope.

<a id="METIS-INTEGRATION-001"></a>
## METIS-INTEGRATION-001 — Desktop integration services [minor]
- Status: todo; priority: P2; owner: Metis host/broker; last-update: 2026-09-22; branch: feat/metis-native-command-menu-001; dependencies: METIS-DESKTOP-001; risk: OS interaction
- Scope: menus/tray, clipboard, notifications, global shortcuts, deep links, file associations, opener, single-instance and window state; bind each exposed API to policy.
- Acceptance: typed commands/events agree across supported hosts, denial/error paths remain explicit, clipboard/user-content permissions obey host rules and listeners unregister on shutdown.
- Demonstration: [V08](docs/VERIFICATION.md#V08), actual OS interactions and permission failures; capture menus/dialogs where visible and assert nonvisual events.
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-integration-001).
- Browser command-surface increment (2026-09-22): the workbench now mounts a
  Rust-owned navigation landmark, toolbar and menu with focus-patient and
  validated dark/system theme actions. Menu state and live status render from
  `BrowserState`, and its listener consumes the shared
  `metis_frontend::ApplicationCommand` identities. Escape closes the menu and
  restores trigger focus. The asset contract and WASM/clippy checks cover the
  DOM seam. Native tray,
  notifications, global shortcuts, deep links, file associations, opener,
  single-instance and window-state services remain open.
- WebView2 command-surface increment (2026-09-22): the packaged page now
  mounts the same bounded navigation landmark, toolbar, local theme menu and
  patient-focus command. Asset tests cover roles, state attributes, Escape
  handling and the no-bridge command boundary; native visual menu capture and
  installed accessibility technology remain host-specific evidence.
- Native command-surface increment (2026-09-22): the software-rendered Windows
  role now mounts the same typed navigation, toolbar, menu and menu-item
  surface. Pointer and AccessKit activation mutate `FrontendApp` command state,
  apply dark/system palettes, focus the patient reference and close the menu on
  Escape before exiting. Native component/event tests pass; installed
  screen-reader speech, native tray/notification/shortcut services and physical
  window captures remain host-specific residuals.
- Shortcut increment (2026-09-24): `metis_core::input` parses `CmdOrCtrl` accelerator text, resolves browser `KeyboardEvent` codes/keys and Windows virtual keys onto one key vocabulary and refuses conflicting bindings in a bounded `ShortcutMap`. The native window and browser workbench activate the local commands with Alt+Shift+P/D/S, ignore auto-repeat, and announce the bindings through `aria-keyshortcuts` (carried by the semantic tree). A real Chromium run applied the dark/system themes and focused the patient field by shortcut; that run also exposed and fixed the theme commands failing on the `<select>` provider seam. The packaged WebView2 page dispatches the same shortcuts from its buttons' `aria-keyshortcuts` (exercised in Chromium; its Windows asset test binds the attributes to the Rust declarations). OS-global hotkeys, tray, notifications, deep links, file associations, opener, single-instance and window state remain open.

- Opener increment (2026-09-24): `ScopedOpener` opens allowlisted http(s) URLs through a host-fixed launcher (`xdg-open`, `open` or the Windows URL protocol handler at standard paths) behind the new `OPEN_EXTERNAL` capability. URLs are validated as printable RFC 3986 ASCII without credentials and passed as one argument with a session-only environment. A recording launcher verifies the argument, origin denial, deadline bounds and launcher failure. `HostOrigin::from_url` is now the one URL-origin parser that the network and opener providers share. Menus/tray, notifications, global shortcuts, deep links, file associations, single-instance and window state remain open.

- Deep-link increment (2026-09-24): manifests declare `url_schemes`, validated by the same `metis_core::deep_link::DeepLinkScheme` rule the parser applies. The Linux desktop entry registers `x-scheme-handler` types with `%u`, the macOS bundle adds `CFBundleURLTypes`, and the Windows MSI writes per-user `Software\Classes` rows owned by the entry component. `DeepLink` parses bounded, percent-decoded routes and query pairs and rejects dot segments, control characters and undeclared schemes. Forwarding a link to an already running instance (single instance) and installed-OS click-through evidence remain open.

- Window-state increment (2026-09-24): `metis_core::window_state::WindowState` is the bounded, platform-neutral restored rectangle and maximized flag, with an exact text codec; `WindowStateFile` saves it by write-and-rename and loads it with a byte bound. Moirai `NativeWindow::placement`/`set_placement` and `WindowConfig::with_placement` (Moirai PR #461) let the Windows pixel and WebView2 surfaces restore it before first show and read it on close; a hidden window stays hidden and shows maximized when asked. Menus/tray, notifications, global shortcuts, file associations and single-instance remain open.

- Global-shortcut increment (2026-09-24): Moirai `NativeWindow::register_hotkey` (Moirai PR #462) registers modifier chords through `RegisterHotKey` with repeat suppressed, queues presses apart from `WindowEvent` so exhaustive matchers in Metis and RITK still compile, and releases every registration on close. `native::GlobalShortcuts` binds `Accelerator` values to commands on the pixel and WebView2 surfaces through the `HotkeyHost` seam, reusing `ShortcutMap` for conflict refusal; `Key::windows_virtual_key` inverts the one virtual-key table. Tray, notifications, file associations and single-instance remain open.

- Tray and notification increment (2026-09-24): Moirai `NativeWindow::show_tray_icon`, `show_notification` and `take_tray_events` (Moirai PR #463) show one notification-area icon from a validated 16/32 pixel ARGB image, raise shell-balloon notifications (presented as toasts on Windows 10+ without a packaged identity) and decode version-4 callbacks into `TrayEvent` values queued apart from `WindowEvent`. `native::TrayHost` exposes them on the pixel and WebView2 surfaces, and `tray_image` converts an application-drawn `Framebuffer`. `show_popup_menu` opens a bounded native menu at a context request's anchor and returns the chosen index. File associations and single-instance remain open.

- Single-instance increment (2026-09-24): Moirai `instance::claim` (Moirai PR #464) makes the first launch primary through an advisory lock and owner-only socket on Unix, or the first instance of a session-scoped, local-only named pipe on Windows, and forwards one bounded frame from later launches. `metis_platform::claim_or_forward` encodes the launch arguments, so a second launch hands its deep link to the running instance. File associations and installed-OS click-through evidence remain open.

- File-association increment (2026-09-24): manifests declare bounded, unique `file_associations` (extensions, MIME type, description). The Linux package lists the types in the desktop entry and installs a shared-mime-info package mapping extensions to types, the macOS bundle adds `CFBundleDocumentTypes`, and the Windows MSI writes per-user ProgIDs listed under each extension's `OpenWithProgids`, owned by the entry component. Installed-OS click-through evidence remains open.

- Autostart increment (2026-09-24): `metis_platform::Autostart` enables, checks and disables a per-user login item: an XDG autostart entry, a macOS launch agent, or a Windows `Run` value through Moirai `windows::startup` (Moirai PR #465), which reaches no other registry location. `metis_core::command_line` is now the single Windows CRT and desktop `Exec` quoting implementation shared by the MSI shortcut, the Linux package and login items, and `atomic_file::replace` is the shared write-and-rename save.

- Dioxus comparison (2026-09-24): ADR 0003 adds Dioxus 0.7.10, inspected from its published crates, as the closest Rust comparator. Its signals, memos, store fields and `use_resource` are now matched by `metis_frontend::reactive::{derived2, Writable::project, resource}`, with explicit dependencies and stale-result rejection. `metis_core::route::Router` then closes its typed-routing gap (literal/parameter/rest patterns, order-independent precedence, conflict refusal, `href`, deep-link segments through one shared path decoder). Moirai PR #468 then adds native menu bars and same-origin browser history: `native::MenuBarHost` with `menu_label` shortcuts, and `navigation::Navigator` with `metis_web::BrowserNavigator` for reactive, history-backed routes. Nested layouts, SSR/hydration, code hot patching and mobile bundles remain open.

- Slint comparison (2026-10-09): ADR 0003 adds Slint 1.18.1 (released 2026-09-21), inspected from its crate API, releases and repository docs, as the declarative-markup, row-model and embedded-renderer comparator, and re-pins Tauri core 2.12.0, egui 0.36.2, Axum 0.8.9 and htmx 4.0.0. Its `Model`/`ModelNotify` row vocabulary is now matched by `metis_frontend::reactive::ListModel`: peers hear typed `RowChange` values (`Changed`/`Added`/`Removed`/`Reset`) with drop-to-unsubscribe handles, each subscription begins with `Reset`, equal replacements deliver nothing, rows are bounded by `MAX_MODEL_ROWS` (the `VirtualList` bound) and runaway change cascades are cut off at `MAX_CASCADE` with a reported `ModelError::Cascade`. Focused `metis-frontend` suites pass 58/58 with strict native and WASM Clippy. Slint's virtualized widget, `.slint` tooling loop, gettext translation flow and mobile/embedded targets remain open in their linked items; htmx 4.0.0 reopens the hypermedia attribute/header inventory under `METIS-FRAGMENT-001`.

<a id="METIS-SERVICES-001"></a>
## METIS-SERVICES-001 — Scoped network, shell and sidecars [minor]
- Status: todo; priority: P2; owner: Moirai mechanisms + Metis policy; last-update: 2026-09-21; delivery: [PR #328](https://github.com/ryancinsight/metis/pull/328), merge `e8c37b23e5231234518a29cdc713de0eb21c38d8`; dependencies: METIS-DESKTOP-001, METIS-COMMANDS-001; risk: privilege escalation
- Scope: HTTP/WebSocket and subprocess APIs, sidecar lifecycle, endpoint/argument allowlists, bounded IO and credential redaction; local test services only by default.
- Acceptance: unauthorized endpoints/commands/arguments fail; transient errors, deadlines, cancellation, crash and cleanup are exercised against real processes/local servers; no shell-string injection or secret output.
- Demonstration: [V08](docs/VERIFICATION.md#V08), connection/process status and denial journey; browsers never receive arbitrary native shell access.
- Network-provider increment (2026-09-21): ScopedHttpProvider adds a native-only Moirai HTTP boundary with a host-owned HTTP(S) origin allowlist, NETWORK capability witness, hop-by-hop header denial, redirect suppression, finite request/response budgets and redacted Debug output. Real loopback exchange and invalid-origin, URL, header, body, bound and capability tests pass. WebSocket, certificate pinning, browser APIs, OS sandboxing and broader sidecar lifecycle remain open.
- Process-provider increment (2026-09-19): Metis `e02ad62` adds `RUN_PROCESS`-witnessed direct execution with host-fixed executable and argument values, bounded stdout, private stderr draining, finite cleanup and real-child denial/redaction/deadline tests; Moirai PR #405 merge `8a8daa60` supplies the opt-in stderr pipe.
- Scoped-network increment (2026-09-21): `ScopedHttpProvider` now exposes a bounded per-request deadline. Expiry drops the Moirai transport future, and a real delayed loopback server observes connection cleanup; focused tests also cover invalid deadlines, unlisted-origin denial and the NETWORK witness. Explicit cancellation is the same future-drop lifecycle and remains transport-owned; OS sandboxing, retries, TLS pinning and broader sidecar orchestration remain open.
- Takeover reconciliation (2026-09-21): the stale network branch was resumed, its lock regenerated outside the Atlas overlay, and the series rebased onto Metis main `3830fe7`; the focused native/WASM gates and conformance guard are green.
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-services-001).

<a id="METIS-MIGRATION-001"></a>
## METIS-MIGRATION-001 — egui and Tauri application migration [arch] [minor]
- Status: todo; priority: P1; owner: Metis framework + application owners; last-update: 2026-09-21; delivery: Metis PR #124 (merge `35ba45c`); risk: lost application behavior.
- lease: root — backlog.md current replay citations — 2026-09-22T00:00:00-04:00
- Dependencies: METIS-FILES-001, METIS-INPUT-001, METIS-ASSETS-001, METIS-GRAPHICS-001, METIS-DESKTOP-001, METIS-INTEGRATION-001, METIS-SERVICES-001.
- Named driver: [ritk-snap](../ritk/backlog.md#RITK-SNAP-METIS-001), retaining egui/eframe at RITK while exposing `--metis-native` through merged [PR 269](https://github.com/ryancinsight/ritk/pull/269) (`c1b8130bb`), following [PR 267](https://github.com/ryancinsight/ritk/pull/267) (`2f2058062`). No Tauri dependency is present in its manifest or workspace lock. RITK owns decoder, volume geometry and medical display correctness; Metis supplies the replacement shell.
- Baseline: RITK `8152f483` ([PR 237](https://github.com/ryancinsight/ritk/pull/237)) adds physical display/hit rectangles to selected-study loading, restore/rejection and the original native capture; [manual](docs/manual/applications.md#deterministic-synthetic-dicom-baseline). The merged RITK native MPR increment now proves one bounded Métis framebuffer containing all three spacing-aware planes and panel-specific routing while keeping DICOM semantics in RITK.
- Progress: the Windows `ritk-snap PATH --metis-native` workflow opens the synthetic study, renders axial/coronal/sagittal panels, routes wheel navigation by panel, captures a deterministic content golden and rejects a missing study. Browser chooser, keyboard, cine and diagnostic-tool input are now evidenced by RITK PR #522; multiframe/color host capture, matched memory measurements and packaging remain open acceptance work. The current complete operating-system application-window capture is recorded in the RITK user manual.
- Scope: inventory the actual viewer and a distinct pinned Tauri fixture; native Metis implementations replace required UI/state/input/render/file/lifecycle surfaces. First viewer journey opens a local DICOM study, selects its series and displays all three orthogonal views; full cutover retains the whole admitted viewer inventory.
- Acceptance: [V09](docs/VERIFICATION.md#V09) plus RITK opening/frames/color/grayscale prerequisites; required symbols/config/plugins and viewer actions are mapped/tested. Existing bugs cannot serve as parity oracles. No retained egui/eframe/Tauri runtime or forwarding shim in the completed migrated viewer.
- Demonstration: actual same-study before/after workflows, verified voxels/physical coordinates and real host captures in the user manual; record JavaScript retained versus Rust/WASM replacement and matched memory evidence.
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-migration-001).

- Build increment (2026-09-12): `Cargo.lock` now pins the Metis consumer to merged Iris `a5df753f7a553b8f94b613e2a65a0910a81a7c81`; the exact full gate passes, and the visual baseline refresh changes only its lock-bound fixture digest while all seven SVG captures remain byte-identical with zero semantic or pixel differences.

- Actual saved-study smoke (2026-09-14): RITK opened the saved `test_data/2_skull_ct/DICOM` series through `--metis-native` with its explicit `SeriesInstanceUID` and produced a real 1280 × 800 three-plane CT frame (SHA-256 `f9d715f04eb1a536fa7aa06ebef90d2829e11d13a10df866db5c8550cdf8c190`). Wrapping that frame in the Metis native host produced a 1296 × 839 Windows capture (SHA-256 `1f2fabae6576cba059feff8e1985a1bb2b22ed7e4293376ebef503e0a882ad4`). These are DICOM-derived pixels; Metis remains format-neutral and no patient pixels were committed.
- Historical replay reconciliation (superseded by RITK PR #594): RITK PR #588 merge `27ac7d12bc69a04f7fb12e5ba41ef8fd4fe93aa9` and standalone replay source `b671ae72a20bb6d113de35314e73874f197d2a83` remain preserved with their Metis, Moirai, lock and executable hashes in git history. The 94-file MRI frame and invalid-study result are unchanged; the current comparison basis is the delivery below.
- Demonstration increment (2026-09-18): the application gallery now links RITK's real MRI diagnostic-tool capture and provenance, including 11 trusted tool actions, seven rejected probes and listener teardown from 21 consumer/32 host listeners to zero. DICOM tool semantics remain RITK-owned; Métis supplies the bounded canvas and event host.


<a id="METIS-DISTRIBUTION-003"></a>
## METIS-DISTRIBUTION-003 — macOS and Linux installers [arch] [minor]
- Status: todo; priority: P2; owner: Metis tooling; last-update: 2026-09-21; dependencies: METIS-DISTRIBUTION-001; risk: platform ownership and lifecycle
- Scope: target-specific executable/bundle and installation formats using the same validated inventory; each target requires its native host for installation evidence.
- Acceptance: build/install/run/uninstall on macOS and Linux preserves user files and application behavior; actual host evidence and platform manual instructions. [ADR 0005](docs/adr/0005-application-distribution.md), [V10](docs/VERIFICATION.md#V10).
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-distribution-003).
- Active implementation branch: `feat/metis-linux-install`; scope is the Linux archive installer/uninstaller, relocatable desktop metadata, and the package/manual tests. Native Linux execution evidence remains host-dependent.

<a id="METIS-DISTRIBUTION-004"></a>
## METIS-DISTRIBUTION-004 — Signed update and recovery [arch] [minor]
- Status: todo; priority: P2; owner: Metis tooling; dependencies: METIS-DISTRIBUTION-001; risk: publisher authentication/data loss
- Scope: signed package verification, authenticated update/rollback and format evolution; no silent MSI overwrite or downgrade. Production signing identity/publication needs separate authority.
- Acceptance: corrupt/expired/signature-invalid updates reject, interruption recovers installed state and preserves user files; real local update/recovery workflow and manual. [ADR 0005](docs/adr/0005-application-distribution.md), [V10](docs/VERIFICATION.md#V10).

<a id="METIS-MOBILE-001"></a>
## METIS-MOBILE-001 — Mobile host and capability matrix [arch] [minor]
- Status: todo; priority: P3; owner: Metis mobile host; dependencies: METIS-COMMANDS-001, METIS-INPUT-001, METIS-AUTHORITY-001; risk: mobile lifecycle/permissions
- Scope: Android/iOS WebView host, touch/keyboard/rotation/suspend/resume, platform plugin boundary; enumerate geolocation, biometric, barcode, NFC and haptics requirements from the Tauri inventory.
- Acceptance: real device/emulator journeys and permission prompts, restore/cancel across suspension; pin required Android/iOS capability pairs from Tauri's inventory and implement/test each. Restriction closes only comparator-unsupported/out-of-scope pairs; unmet admitted pairs remain open.
- Demonstration: [V11](docs/VERIFICATION.md#V11), actual phone/tablet screenshots and target-specific manual instructions; app-store release remains separately authorized.
- Local increment (2026-10-11) [minor]: one iOS/iPadOS scene-based WebKit host packages the browser assets. iOS device/simulator compilation and iPhone 17/iPad Air simulator installation/launch pass. Native input/rotation/suspend, physical provisioning, Android hosting and admitted plugin pairs remain open. Browser input is verified separately and does not stand in for native execution.
- Local verification (2026-10-11) [patch]: native macOS development reload and compiler-error preservation pass with real source edits and exact restoration. A terminated child ends supervision without retaining a process. Bundle budget rejection preserves the previous output; all three final inventories match packaged bytes and deployment minima.

<a id="METIS-PERF-001"></a>
## METIS-PERF-001 — Comparative resource and latency evidence [patch]
- Status: todo; priority: verification; needs: none
- outcome: add bounded real-browser resource and growth evidence to V12.
- scope: scripts/browser_drop_lifecycle.py, scripts/resource.py, scripts/browser_drop.py, scripts/tests/test_browser_drop.py, scripts/tests/test_resource.py, docs/manual/browser.md, docs/VERIFICATION.md, docs/manual/images/metis-browser-mri-growth.json
- acceptance: 16 same-page real-study cycles within 300 seconds; exact file and raw-canvas oracles; zero stopped listeners and transfer observers.
- evidence: [manual](docs/manual/browser.md#repeat-the-saved-study-gallery-lifecycle), [provenance](docs/manual/images/metis-browser-mri-growth.json), [V12](docs/VERIFICATION.md#V12).
- WASM: capacity grew from 1,835,008 bytes at first mount to 404,357,120 bytes at first transfer; all 14 post-warmup observations per phase stayed at 404,357,120 bytes.
- heap: Chromium `performance.memory`; 14 samples per phase, mean and approximate 95% half-width plus raw range in the manual.
- uncertainty: `1.96 × sample SD / √n`; garbage collection and serial correlation are not modeled; these samples do not prove leak absence.
- semantic/visual: browser file and raw RGBA oracles passed 16/16; RITK strict screenshot validation passed 9/16, and its later cleanup assertion did not run on seven hash failures; Metis recorded zero cleanup guards; no tolerance changed.
- limits: capacity is not allocator live bytes; served assets are pinned but their build relation to the reported RITK source is unknown; native/process/compositor/GPU usage is unmeasured.
- next: resolve strict screenshot stability; measure allocator-used bytes and longer soak growth; add matched GPUI/Tauri fixtures on controlled hosts.
- basis: 60ac67cbcaa09b1870144920c4024d7a38c08f16
- falsified (2026-09-24, pinned A/B): blending varying-coverage rows (text, shadow edges) in the `u16` opaque lanes gave nothing on shadows and was 3–9% slower on text, because per-pixel f64 coverage-to-alpha conversion dominates and glyph rows skip zero coverage; building the gradient bench with `+fma` was 7% slower, so software `fma` calls are not the diagonal gradient's cost.
- Frame-path increment (2026-09-24): glyph coverage is memoized across repaints with bit-identical frame hashes; the `frame` bench's keystroke repaint fell from 1.01 ms to 0.21 ms (800x600) and from 1.20 ms to 0.28 ms (1600x1200), full theme repaints by 28% and 18%. The result explorer's regrouping and tree rebuild became linear passes (20 × 256 recorded rows across 64 patients: 501 ms to 86 ms, release). Box-shadow masks are then memoized ([ADR 0052](docs/adr/0052-memoized-box-shadow-masks.md)): theme repaints fell a further 43% (800x600) and 41% (1600x1200), keystrokes 21% and 20%, leaving the diagonal gradient as the dominant full-repaint cost. Diagonal gradients then interpolate runs without a stop search and run under Hermes' instruction-set dispatch ([ADR 0053](docs/adr/0053-runtime-isa-dispatch-for-raster-kernels.md)), bit-identical: theme repaints fell a further 15% (800x600) and 16% (1600x1200). Computing each quantity in its own loop over a run's lanes then vectorized the runs: a further 5% and 10%.

<a id="METIS-CONFORMANCE-001"></a>
## METIS-CONFORMANCE-001 — Final capability and target closure [patch]
- Status: todo; priority: P3; owner: Metis integration; last-update: 2026-09-21; risk: structural conformance debt
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-conformance-001).
- Dependencies: METIS-DESKTOP-001, METIS-MACOS-001, METIS-LINUX-001, METIS-TEXT-001, METIS-A11Y-001, METIS-ASSETS-001, METIS-GRAPHICS-001, METIS-FILES-001, METIS-INTEGRATION-001, METIS-SERVICES-001, METIS-MIGRATION-001, METIS-MOBILE-001, METIS-PERF-001
- Scope: reconcile every ADR 0003 matrix row and required target pair against exact-revision evidence, including late package/mobile/service work and current upstream inventory changes.
- Acceptance: [V01](docs/VERIFICATION.md#V01)–[V12](docs/VERIFICATION.md#V12) run on their required real targets, with semantic/visual/security/resource results and manual demos; required unsupported pairs stay open and block closure. No mocked IPC, compile-only host claim or skipped denial/lifecycle suite.
- Demonstration: same-revision gallery, migration guide, measured comparative report and host troubleshooting; final security claims require matched Tauri denial probes under the declared threat model.
- Historical evidence reconciliation (2026-09-16; superseded by the current 2026-09-22 lock replay): Metis main `88c60a0b6410c0e07700e965bcdbea43b7b20789` remains format-neutral and the RITK consumer lock resolves Moirai browser PAL `95275651722583f52e098c0d30b1a53ec82c1fc7` plus runtime/iterator/parallel/sync `c0b1131178177da699f0692079a1413a2fb033e8`. Hosted RITK run [35133971196](https://github.com/ryancinsight/ritk/actions/runs/35133971196) passes the real 94-file MRI study on Chromium/Firefox; Safari's selected-file read remains an external WebKit authorization failure and the hosted Chromium WebGPU job reports no adapter. The DICOM ownership row makes RITK's scanner/decoder/presentation boundary explicit; real GPU output and recovery remain open consumer evidence.
- Historical replay reconciliation (2026-09-19; superseded by the current lock correction below): the standalone-lock replay used RITK source `9ea19856c63dbe5b956c81560bad044a48705ee1`, with Metis `b432446f5dbc39fb328220f45ba0d0341eb03263`, Moirai `5075d4c70ba4f840d4c5a47b67c5d564405badf5` and standalone Cargo.lock SHA-256 `182203ea9a9d13dbd596753055b96777954d68173228cfe22475cf496d58ac32`. It reads all 94 saved MRI-DIR files and reproduced the committed 1280 × 800 MRI frame; the [RITK provenance record](../ritk/docs/manual/images/dicom-metis-real-mri.json) retains this historical visual oracle. RITK owns DICOM decoding and clinical presentation, while Metis remains the format-neutral host.
- Superseded lock correction (RITK PR #588): its exact lock and replay remain historical evidence; the current lock correction is the RITK PR #604 delivery immediately below. The next conformance slices are OS capability enforcement, native accessibility/IME evidence, matched GPUI/Tauri fixtures, and explicit WebGPU recovery; Safari selected-file authorization remains an external host blocker.
- Current replay delivery (2026-09-22, RITK lock PR #604 merge `5e2b74d0ac7550870c147c43225b36757b9b0663`, evidence PR #605 merge `29c51440ad4d33ab4109dd665683b7ef5bf45198`; replay source `e88a94219fac9f07339393da2cd0e7a19164f93b`): the authoritative standalone consumer replay binds Metis `1b10541c2ef7a849e6ff66a3c778874bdf96de7b`, Moirai `b77239dd10bcaf803394c26255c462bc858c1340` and Cargo.lock SHA-256 `d0d6abd6baf3f7d45943e9d1d3f85a2158b7dae605d3ec23ec07981b8b17a9cd`. The locked native replay reads all 94 DICOM files/49,807,236 bytes, exits 0, rejects the invalid-study probe with exit 1 and preserves the revision-bound 1280 × 800 MRI frame (`85071f20ca11cb4a9b2524db0a53b21695b7e93141831e7e0ab293c42fbcd582`, 411,413 non-black pixels); executable SHA-256 `7f81c6fc3d2e76c7e28d603fbaad3eece1fb67c694b7b97bf4ff6c7dc3325484`. Hosted run [35759891764](https://github.com/ryancinsight/ritk/actions/runs/35759891764) passes Chromium, Chromium-window, MIP, responsive Chromium and Firefox lanes; WebKit bounded-read and Chromium WebGPU adapter failures remain explicit host residuals in the [RITK cross-engine provenance](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-cross-engine.json).

<a id="METIS-WORKBENCH-DESIGN-001"></a>
## METIS-WORKBENCH-DESIGN-001 — Refine browser visual hierarchy
- Status: todo; priority: verification; last-update: 2026-09-23.
- Outcome: distinguish primary actions, diagnostics and content through typography, palette and spacing.
- Scope: browser CSS, responsive capture evidence and browser manual; native renderer and IME work excluded.
- Acceptance: inspected desktop/mobile and theme states; no horizontal overflow; 44px option targets; browser asset and capture gates pass.
- Historical evidence: [increment records](docs/backlog/evidence.md#metis-workbench-design-001).
- basis: 7ddca83.
- Increment (2026-09-24): color tokens are declared once through `light-dark()` and themes choose only the color scheme; the status line leads with a state indicator, diagnostics read as a monospace log, cards are elevated on a darker page, section headings carry an accent rule, the primary action is taller and the command menu shows each item's shortcut from `aria-keyshortcuts`. The layout manifest and 360/800/1440 captures were re-recorded in Chromium 141 through Playwright 1.56.1 with classic scrollbars; 44px option targets hold and no viewport overflows. Firefox/WebKit captures and scale-2 evidence remain open.

<a id="METIS-PERF-IDLE-001"></a>
## METIS-PERF-IDLE-001 — Stop rebuilding accessibility and layout when nothing changed [patch]
- Status: todo; priority: tightening; needs: none
- Outcome: an idle event batch allocates nothing and a click reuses the painted layout for hit-testing.
- Scope: `crates/metis-platform/src/native/application.rs`, `native/window.rs`, `crates/metis-app/src/frontend/native.rs`, `native_accessibility.rs`, `crates/metis-frontend/src/app.rs`.
- Acceptance: Every 250 ms batch, including empty ones, rebuilds and deep-clones the accessibility tree (about 430 allocations, application.rs:195-202, window.rs:143-146); a click recomputes the whole layout up to four times through `command_rect` (native.rs:189-217, :437-450, verified at basis) though `FrontendApp` holds the painted display list. Oracle: a counting allocator over the loop with empty batches sees zero allocations and zero provider updates after the first frame; `handle_pointer_up` on a miss allocates nothing.
- basis: 7b50219

<a id="METIS-MEM-RENDER-002"></a>
## METIS-MEM-RENDER-002 — Rebuild only the changed elements' layout and semantics on an edit [patch]
- Status: todo; priority: tightening; needs: none
- Outcome: a render that changes one label requests memory in proportion to the changed elements, not the whole form.
- Scope: `crates/metis-ui-lang/src/layout/geometry.rs`, `grow.rs`, `intrinsic.rs`, `slots.rs`, `crates/metis-ui-lang/src/semantics.rs`, `crates/metis-frontend/src/presentation/cache.rs`.
- Acceptance: An unchanged render now requests one allocation (METIS-MEM-RENDER-001); a one-character `set_inputs` edit still requests 350 (372 before), because `compute_layout` and `SemanticTree::from_document` rebuild every element once the document revision moves. Oracle: a counting allocator over `set_inputs` with a one-character patient edit, with a bound derived from the changed elements; and the pinned keystroke bench: the `frame` bench's `keystroke_*` baselines stored with the PR and compared. The bench smoke (`cargo test --benches -p metis-frontend`) failed to compile `metis-ipc/src/async_server.rs` at c9b99c1; it compiles and runs at origin/main since b6ebd19 (#452), so no blocker item stands.
- basis: c9b99c1

<a id="METIS-MEM-WEB-001"></a>
## METIS-MEM-WEB-001 — Write only changed browser DOM state and reuse transfer buffers [patch]
- Status: todo; priority: tightening; needs: none
- Outcome: a keystroke mutates only the changed nodes and file drop and canvas polling allocate per batch, not per item.
- Scope: `crates/metis-web/src/view.rs`, `browser/listeners.rs`, `browser/file_drop.rs`, `canvas/events/queue.rs`.
- Acceptance: Each input event issues 147 DOM writes and 49 lookups (view.rs:14-321, :464-476); file drop allocates and zero-fills a chunk per file (file_drop.rs:387-411: 94 chunks for the 49.8 MB study); each canvas poll boxes its events (queue.rs:41-42). Oracle: a MutationObserver run through `scripts/browser_input_latency.py` sees at most four records per keystroke; allocations over `read_batch` equal files + 1.
- basis: 7b50219

<a id="METIS-I18N-001"></a>
## METIS-I18N-001 — Localize UI strings through a message catalog [minor]
- Status: todo; priority: P2; needs: METIS-TEXT-001
- Outcome: user-visible strings render in the selected locale instead of the source language they were written in.
- Scope: message catalog + locale selection only; text shaping, bidi, and IME stay in METIS-TEXT-001. The catalog core is delivered (metis#444: `Locale`, `Catalog`, `Resolved`, `catalog!`, `format_message`, 12 tests). What remains is routing display text through it — and measurement shows the first-party Rust surface is almost entirely NOT display text: `metis-app`/`metis-frontend` literals are semantic ids (`label-patient`), ARIA roles (`menuitem`), error/log strings, and test fixtures, none of which may move (localizing an id breaks its references; logs stay source-language for diagnosability). Genuine display text originates from consumers (the RITK viewer strings), window/menu configuration supplied by callers, and HTML/JS assets — a multi-language, multi-repo surface, not a Rust-literal migration.
- Acceptance: a consumer-visible string resolves through the catalog (no literal survives at the migrated call sites); switching locale re-renders without restart; missing key fails closed to the source string with a logged miss (never blank, never panic); locale-specific number/date formatting follows the selected locale where the UI presents quantities.
- basis: 98899f3
- mechanism analysis (2026-09-30, measured): binary size does not discriminate (10 locales compile-timed cost 275 KiB against a multi-MB binary; a 28 KiB runtime parse is microseconds). Both safety and workflow favor a compile-time macro table: keys type-check (a misspelled key is a compile error, eliminating missing-translation runtime bugs as a class), lookup is zero-cost with zero startup, and no locale file crosses the trust boundary. Runtime bundles win only if locales must ship independently of releases, which the release train gates.
- API sketch (delivered as code in metis#444, not just directive): `Locale` as validated BCP-47 newtype; `Catalog<L: LocaleSpec>` mapping `&'static str` keys to `&'static str` values; `resolve(key)` returning the source string plus a logged miss when absent; message patterns as `MessageFormat` with named (never positional) slots. No `dyn`, no heap at lookup, no I/O on the render path.
- next: migrate the first consumer display-text call sites (RITK viewer strings via the shared-text surface, then asset strings) — NOT the first-party Rust literals surveyed, which are ids, roles, errors, and fixtures that must stay put. Coordinate with RITK-SNAP-METIS-SHARED-TEXT-001, which moves the same text to `Arc<str>`: the catalog resolves borrowed text and the call site wraps in `Arc`, so the two changes compose without conflict.

<a id="METIS-LIST-002"></a>
## METIS-LIST-002 — Scroll and project the virtualized window in both hosts [minor]
- Status: todo; priority: feature; needs: none; ADR: 0055 (PR #484, not yet on main)
- Outcome: a user scrolls a long list with the wheel and keyboard, and each host builds only the rows the window shows.
- Scope: a scroll container and offset policy in the DOM projection (`metis-frontend::list`), the software host's list paint, and its visual capture. The overflow declaration needs its admitted-style decision in [ADR 0013](docs/adr/0013-strict-style-contract.md) first.
- Acceptance: at each offset the built rows and spacers equal the fresh-list oracle; one wheel step moves the window by the scrolled extent; a 10,000-row capture is inspected before it enters the manual.
- basis: ce47b2b2ebc11c09b0d6f67d7c2e60487fe71cc6
- next: decide whether `overflow` enters the admitted CSS subset, then add the scroll container with its offset tests.

<a id="METIS-LIST-003"></a>
## METIS-LIST-003 — Measure the changed-row rebuild before adding per-row updates [tightening]
- Status: todo; priority: tightening; needs: none
- Outcome: the `VirtualList` rebuild after a change is replaced by per-row Fenwick updates only if it measurably costs more than a frame.
- Scope: `metis-ui-lang` layout `virtual_list` and `metis-frontend::list`; no production change before the measurement.
- Acceptance: a controlled local benchmark (median with confidence interval, pinned 10⁵-row model, one change per query) records the rebuild cost; per-row updates land only when that cost exceeds the frame budget, and match the rebuild oracle.
- basis: ce47b2b2ebc11c09b0d6f67d7c2e60487fe71cc6
- next: write the benchmark harness and run it on the controlled host.

<a id="METIS-COMPONENT-001"></a>
## METIS-COMPONENT-001 — General component lifecycle for application state and views [arch] [minor]
- Status: todo; priority: architecture; needs: none
- Outcome: an application defines state, messages, update and view once; the clinical form and the starter page use that model instead of `FrontendApp`'s bespoke `FormInputs` path.
- Scope: `metis-frontend` (`app`, `reactive`), `metis-platform::native::NativeApplication` and the browser host in `metis-web`.
- Acceptance: an ADR chooses the seam (Iced 0.14's update/view model and Freya's hook state as comparators) and rejects the alternatives; the clinical form and the starter pass one shared component test set on the software and DOM hosts.
- basis: ce47b2b2ebc11c09b0d6f67d7c2e60487fe71cc6
- next: claim the ADR number in the claim PR, then draft the ADR before the lifecycle slice.

<a id="METIS-DOCS-ADR-001"></a>
## METIS-DOCS-ADR-001 — Compact the ADRs over their 150-line budget [patch]
- Status: todo; priority: verification; needs: none
- Outcome: each ADR records one decision in about 150 lines; the comparator matrices of ADR 0003 move to a gap register, with the Freya row from ADR 0055 added there.
- Scope: `docs/adr/0003` (802 lines), `0014` (235), `0012` (222), `0002` (204), `0005` (194), `0015` (192), `0021` (179), `0029` (168), `0046` (153), measured at basis.
- Acceptance: every ADR is at most 150 lines; each decision, rejected alternative and evidence survives; the index keeps exact statuses; a line census reports zero over-budget ADRs.
- basis: ce47b2b2ebc11c09b0d6f67d7c2e60487fe71cc6
- next: compact ADR 0003 first, since its dated revision paragraphs repeat git and PR history.

<a id="METIS-BOARD-001"></a>
## METIS-BOARD-001 — Closed priority set and item-block budget [patch]
- Status: todo; priority: verification; needs: none
- Outcome: every open item uses the closed set (correctness, architecture, verification, tightening, feature), and no item block exceeds 15 lines.
- Scope: `backlog.md`. At basis, 19 items carry `P0`–`P3` (P0 1, P1 8, P2 7, P3 3). Six blocks exceed 15 lines (measured heading to heading): METIS-MIGRATION-001 68, METIS-INTEGRATION-001 50, METIS-A11Y-001 42, METIS-INPUT-001 41, METIS-TEXT-001 40, METIS-BROWSER-READ-001 32.
- Acceptance: a board lint reports zero off-set priorities and zero blocks over 15 lines; the board stays under 1,000 lines.
- basis: ce47b2b2ebc11c09b0d6f67d7c2e60487fe71cc6
- next: map each `P`-code to the closed set in one commit, then compact the six blocks to their DoR fields.

<a id="METIS-RESIDUAL-001"></a>
## METIS-RESIDUAL-001 — Give each residual that ADR 0003 cites a live owner [patch]
- Status: todo; priority: verification; needs: none
- Outcome: each open residual stated by ADR 0003 has a board item or a gap-register risk with a re-open trigger.
- Scope: `docs/adr/0003-framework-conformance.md` cites 19 item IDs with no board entry. Each has `Item:` commits on `origin/main`: METIS-BROWSER-001 96, METIS-FRAGMENT-001 37, METIS-COMMANDS-001 21, METIS-ASYNC-001 11, METIS-QUALITY-001 10, METIS-AUTHORITY-001 8, METIS-LAYOUT-001 8, and 12 more with 1–9 commits each. Also `gap_audit.md`.
- Acceptance: a scan finds no ADR reference to an ID without a board item, unless the sentence cites a gap-register risk and its re-open trigger.
- basis: ce47b2b2ebc11c09b0d6f67d7c2e60487fe71cc6
- next: list each residual sentence per ID, assign it, and delete the closed ones.

<a id="METIS-INTEG-DEBT-001"></a>
## METIS-INTEG-DEBT-001 — Unlanded peer commits and stale main-tree dirt [patch]
- Status: todo; priority: correctness; needs: none
- Outcome: no metis work exists only on a peer branch, and the main tree returns to `origin/main`.
- Scope: branch `build/metis-deps-20261007`: the commits `feat(presentation): Head every application surface with the mark` (25 files) and `build(deps): Follow hermes-simd` have no PR; its PR #482 is merged; the commit `feat(reactive): Add a Slint comparison and close its row-model gaps` is absorbed by PR #485. Three uncommitted script edits in `repos/metis` dated 2026-10-09. The empty directory `D:/atlas/worktrees/metis-metis-frontend-theme-001`, which is not a git worktree.
- Acceptance: each unlanded commit has a PR or a merge-base proof of landing; the dirty scripts are committed to a PR branch or proven superseded; the empty directory is removed; the main tree sits at `origin/main`.
- basis: ce47b2b2ebc11c09b0d6f67d7c2e60487fe71cc6
- next: open a PR for those two commits from a private-index branch, then return the main tree home.
