# Metis delivery

Registration: [Atlas member item](../../backlog.md#metis-unregistered-member).
Public source and executable packaging are merged. Atlas registration awaits
candidate-aware hook support: its current hook resets an alternate index and its
local auditors read the shared checkout rather than a candidate revision.
Registration must preserve active shared-checkout work; implementation continues.

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

<a id="METIS-GAPS-001"></a>
## METIS-GAPS-001 — Framework conformance plan [arch] [patch]
- Status: done; delivery: [PR 2](https://github.com/ryancinsight/metis/pull/2), content `a6855aa`.
- Outcome: [ADR 0003](docs/adr/0003-framework-conformance.md) maps 22 capability areas to owned work and twelve semantic/visual/manual scenarios; source review and existing gate pass. Runtime gaps remain open in the linked items.

<a id="METIS-ICED-001"></a>
## METIS-ICED-001 — Iced comparator and backend decision [arch] [patch]
- Status: review; priority: P1; owner: Metis integration; integrator: root; last-update: 2026-09-06; branch: `feat/process-foundation`; dependencies: METIS-GAPS-001
- Outcome: source-pinned Iced 0.14 comparison covers state, layout, text, native/web rendering, authority, packaging, testing and performance; the manual and development plan state which capabilities Metis owns and which remain open.
- Acceptance: ADR 0003 links only verified official Iced sources, records that archived `iced_web` does not establish current DOM support, adds no Iced dependency, and links every unresolved capability to an existing Metis item. The manual names the comparator without fabricating a runtime capture.
- Demonstration: [V01](docs/VERIFICATION.md#V01) evidence index and the manual comparison section; a future browser/native Iced fixture is a separate item once a pinned executable target is selected.

<a id="METIS-WEB-001"></a>
## METIS-WEB-001 — Web application target contract [arch] [patch]
- Status: done; delivery: [PR 1](https://github.com/ryancinsight/metis/pull/1), `65e6af1`.
- Outcome: [ADR 0002](docs/adr/0002-web-application-contract.md) and WASM library gate define web targets without runtime claims.

<a id="METIS-BROWSER-001"></a>
## METIS-BROWSER-001 — Browser form and command lifecycle [arch] [minor]
- Status: in-progress; priority: P1; owner: Metis frontend/host; integrator: root; last-update: 2026-09-06; branch: `feat/browser-host`; dependencies: METIS-STATE-001, METIS-ASYNC-001, METIS-AUTHORITY-001; risk: browser/native trust boundary
- Scope: actual HTML5/CSS DOM form, Rust/WASM state, asset loading and bounded asynchronous requests; portable UI never imports native authority.
- Acceptance: Chromium/Firefox/WebKit runtime jobs load WASM and respond to two input changes; authorized service/desktop bridge verifies results; explicit unsupported native-only operations; zero pending requests/listeners after cancel/close.
- Demonstration: [V02](docs/VERIFICATION.md#V02), actual browser captures and copyable build/run commands in the manual. A browser-only local control demo can land before the privileged bridge.
- Constraint: no native secrets or authority in downloaded WASM; private-pipe possession cannot authenticate browser requests. Desktop bridge or service boundary must enforce origin/session authorization.
- Evidence: `metis-web` now mounts a real DOM form through Moirai's owned handles. A local trace changed weight and dose, rejected a non-numeric edit with `ERR_NUMERIC_INSTABILITY`, surfaced `ERR_CONNECTION_CLOSED` without a backend, and verified Stop/Start remount with empty browser diagnostics. Metis request cancellation and listener teardown are covered; live service, origin/session, post-drop allocation and cross-engine evidence remain open.
- Decision: [ADR 0002](docs/adr/0002-web-application-contract.md), [ADR 0008](docs/adr/0008-browser-host-boundary.md).

<a id="METIS-SEC-001"></a>
## METIS-SEC-001 — Backend authority [arch] [patch]
- Status: review; integrator: root; last-update: 2026-09-05
- Scope: backend-only calculation/audit, validated configuration, session authority, real time and OS entropy.
- Acceptance: malformed/expired/cross-session requests fail; numerical boundary tests pass; frontend dependency closure excludes clinical/audit modules.
- Decision: [ADR 0001](docs/adr/0001-process-contract.md).
- Demonstration: [V01](docs/VERIFICATION.md#V01) and [V08](docs/VERIFICATION.md#V08), real scoped request success/rejection with honest authentication status.

<a id="METIS-IPC-001"></a>
## METIS-IPC-001 — Canonical bounded IPC [patch]
- Status: review; integrator: root; last-update: 2026-09-05
- Scope: framing, canonical payloads, correlation/replay, bounded memory transport, wire fault injection.
- Acceptance: exact bytes and typed errors for truncation, corruption, replay, oversize and malformed payloads.
- Demonstration: [V01](docs/VERIFICATION.md#V01) and [V02](docs/VERIFICATION.md#V02), surfaced rejection/disconnection; wire assertions supplement visible outcomes.

<a id="METIS-UI-001"></a>
## METIS-UI-001 — Bounded presentation [patch]
- Status: review; integrator: root; last-update: 2026-09-05
- Scope: markup/style parsing, layout, software rasterizer, bounded surface/event allocations.
- Acceptance: EOF/depth/Unicode/overflow cases terminate with errors; supported forms still render from input.
- Demonstration: [V01](docs/VERIFICATION.md#V01) and [V04](docs/VERIFICATION.md#V04), actual supported-layout captures with typed unsupported-style diagnostics.

<a id="METIS-PROCESS-001"></a>
## METIS-PROCESS-001 — Real executable workflow [arch] [minor]
- Status: review; integrator: root; last-update: 2026-09-05
- Scope: connect backend/frontend binaries through inherited pipes; bounded supervision and process tests.
- Acceptance: separate PIDs, input-sensitive request/result exchange, failure propagation and finite shutdown.
- Demonstration: [V01](docs/VERIFICATION.md#V01), manual process workflow and actual submitted state; [V05](docs/VERIFICATION.md#V05) remains a separate native-host requirement.
- Decision: [ADR 0001](docs/adr/0001-process-contract.md).

<a id="METIS-DESKTOP-001"></a>
## METIS-DESKTOP-001 — Native restricted desktop [arch] [minor]
- Status: todo; priority: P1; owner: Metis Windows host + Moirai; dependencies: METIS-AUTHORITY-001, METIS-COMMANDS-001; risk: trust boundary
- Scope: Windows native window/system WebView, real events, multi-window lifecycle and OS-restricted renderer; macOS/Linux have separate items below.
- Acceptance: actual visible form, pointer/keyboard/resize/DPI/close/reopen; file/network/process denial probes; IPC remains functional under restrictions and all child processes drain.
- Demonstration: [V05](docs/VERIFICATION.md#V05), actual Windows window captures, keyboard journey and permission-denied results in the manual.
- Current evidence: `PlatformSurface` owns framebuffer/events only; `PlatformEvent` has no OS event producer. The ineffective original privilege assertion is removed. Native lifecycle, input dispatch and permission denial require new provider contracts and platform probes.

<a id="METIS-AUDIT-001"></a>
## METIS-AUDIT-001 — Durable audit recovery [minor]
- Status: todo; priority: P2; owner: Metis backend + owning Atlas storage provider; dependencies: METIS-CRYPTO-001; risk: persistence
- Scope: versioned durable backend audit, bounded storage, restart recovery and trusted checkpoint; first verify the Atlas storage ownership/contract.
- Acceptance: crash/truncation/tamper/disk-full cases recover exactly or fail closed, with bounded retention and no patient/secret leakage.
- Demonstration: [V08](docs/VERIFICATION.md#V08), audit/recovery inspector showing actual records, denied tampering and restart outcomes; raw secrets never enter captures.

<a id="METIS-VERIFY-001"></a>
## METIS-VERIFY-001 — Verify and deliver foundation [patch]
- Status: todo; priority: P0; owner: Metis integration; dependencies: METIS-SEC-001, METIS-IPC-001, METIS-UI-001, METIS-PROCESS-001
- Scope: source documentation, warning-clean gates, process evidence and Git delivery.
- Acceptance: fmt/clippy/nextest/doc pass; Atlas-only direct dependencies and enumerated provider transitive graph; evidence states host coverage and residual risks.
- Demonstration: [V01](docs/VERIFICATION.md#V01), current source/capture and evidence limits in the manual; final framework-wide closure belongs to CONFORMANCE.

<a id="METIS-PROVIDER-001"></a>
## METIS-PROVIDER-001 — Atlas provider adoption [arch] [patch]
- Status: review; integrator: root; last-update: 2026-09-06
- Scope: Moirai scheduler/process transport and Iris rendering contract; enumerate provider transitive graph.
- Acceptance: no parallel Metis runtime; local and standalone provider sources coherent; contract tests pass.
- Provider revision: Moirai `16a1b88` is on its default branch and supplies the contained process, browser DOM/event, cancellable local task, transport and standalone crypto APIs consumed here. Metis's standalone lock pins that revision; the previous `66627b9` pin is advanced after PR #268.
- Dependencies: Atlas overlay mixed-version correction and consumer verification on the merged provider.
- Demonstration: [V01](docs/VERIFICATION.md#V01) real provider-backed process workflow and [V02](docs/VERIFICATION.md#V02) browser DOM trace.

<a id="METIS-CRYPTO-001"></a>
## METIS-CRYPTO-001 — Shared authentication primitives [arch] [patch]
- Status: done; delivery: Metis PR [#14](https://github.com/ryancinsight/metis/pull/14) merged at `2258266`; [ADR 0009](docs/adr/0009-crypto-provider-boundary.md).
- Outcome: Metis imports Moirai `66627b9` standalone SHA-256, HMAC-SHA256 and fixed-width comparison APIs with TLS dependencies disabled; CRC-32 remains local and no duplicate authentication implementation remains.

<a id="METIS-RELEASE-001"></a>
## METIS-RELEASE-001 — Publication readiness [patch]
- Status: todo; priority: P3; owner: Metis delivery; dependencies: METIS-DISTRIBUTION-001, METIS-CONFORMANCE-001
- Scope: release-readiness metadata, package dry runs, final manual and platform evidence; preparation continues without release authority.
- Acceptance: dependency-closed packages and exact-revision evidence; release execution alone is blocked until explicit authority, signing/rollout/rollback details are available.
- Demonstration: [V10](docs/VERIFICATION.md#V10), locally built package installation/recovery instructions and actual captures before any publication.

<a id="METIS-MEMORY-001"></a>
## METIS-MEMORY-001 — Provider allocation count [patch]
- Status: todo; priority: P0; owner: Moirai allocation provider; risk: overflow
- Scope: verify the public count contract, locked-source exposure and local allocator multiplication before classifying the defect; do not assume all allocations use this path.
- Acceptance: reachable overflow rejects with a typed error before allocation and debug/release adversarial tests; otherwise close with exact unreachable-path evidence.
- Demonstration: [V12](docs/VERIFICATION.md#V12), bounded-allocation denial shown as an application error when the exposed path is integrated.

<a id="METIS-MANUAL-001"></a>
## METIS-MANUAL-001 — Public member and user manual [patch]
- Status: todo; priority: P1; owner: Metis documentation/integration; last-update: 2026-09-05
- Scope: public GitHub repository, Atlas gitlink, user-oriented manual and actual rendered application snapshots; no registry release.
- Acceptance: public remote contains tested source; Atlas resolves the pinned commit; manual links resolve and generated snapshot matches the renderer.
- Decision: [ADR 0001](docs/adr/0001-process-contract.md); user manual replaces the domain-book requirement by explicit user direction.
- Remaining: Atlas gitlink/review-path resolution; public source/manual/current software capture already exist at `9d96980`. Every later item owns its demonstration section, not a deferred documentation phase.

<a id="METIS-STATE-001"></a>
## METIS-STATE-001 — Correct form state transitions [arch] [major]
- Status: done; merged [PR 3](https://github.com/ryancinsight/metis/pull/3) at `9c38d2f`; [ADR 0004](docs/adr/0004-form-state.md).
- Outcome: owned state clears stale results; 88 debug/release tests, 12 doctests and seven real-session captures pass with independent review.

<a id="METIS-VISUAL-001"></a>
## METIS-VISUAL-001 — Semantic and visual scenario runner [patch]
- Status: done; [PR 4](https://github.com/ryancinsight/metis/pull/4), merge `0465a43`.
- Outcome: seven real-session semantic/raster baselines, three mutation probes, bounded difference artifacts and failure/provenance/alias regressions pass; manual synchronized.

<a id="METIS-ASYNC-001"></a>
## METIS-ASYNC-001 — Bounded browser request lifecycle [arch] [minor]
- Status: in-progress; priority: P0; owner: Moirai async/transport + Metis client; integrator: root; branch: `feat/browser-lifecycle`; last-update: 2026-09-07; stage: live WebSocket lifecycle; risk: hangs/leaks; dependencies: METIS-WEB-001
- Scope: event-driven receive/wakeup, task/request cancellation, deadlines and owned callback teardown; complete the upstream reactor gap and remove blocking browser paths.
- Entry evidence: Moirai `16a1b88` owns contained process lifecycles, browser callbacks, DOM handles, cancellable local tasks, bounded WebSocket state, deadlines and standalone authentication primitives over its merged Mnemosyne backend; Metis uses one pinned Moirai source for native and WASM dependencies. Focused Metis IPC/frontend Nextest passes 39/39, native all-targets Clippy and WASM library Clippy pass, the WASM check passes, and the browser trace proves stop/remount with empty console diagnostics.
- First increments: add the Metis async transport/client seam, then route ordered and out-of-order responses through one bounded receive pump; the browser host now consumes the seam but remains disconnected until authority/session plumbing exists.
- Acceptance: native correlation and queue bounds, request cancellation, late-response rejection and host stop/remount now pass; authenticated browser execution over a live WebSocket, replay/oversize handling through that service, task-handle integration, and post-drop resource evidence remain required.
- Demonstration: [V02](docs/VERIFICATION.md#V02) pending/cancel/disconnected states; [V12](docs/VERIFICATION.md#V12) repeat lifecycle/resource evidence.
- Takeover: the prior `feat/browser-host` claim is stale and its remote branch is gone; this increment owns the live transport fixture and cancellation/teardown evidence.

<a id="METIS-AUTHORITY-001"></a>
## METIS-AUTHORITY-001 — Host authority and origin policy [arch] [minor]
- Status: in-progress; priority: P0; owner: Metis broker + Moirai host mechanisms; integrator: root; last-update: 2026-09-07; branch: `feat/authority-policy`; stage: typed origin/window/session binding; risk: hostile frontend; dependencies: METIS-WEB-001
- Scope: deny-by-default command grants bound to session/origin/window, CSP/navigation/asset policy, target capability discovery and explicit unsupported errors.
- Acceptance: spoofed origin/window, navigation, replay, injection and resource-exhaustion probes cannot elevate authority; backend keys absent from WASM/assets; each grant has positive and denial cases.
- Demonstration: [V08](docs/VERIFICATION.md#V08); screenshots pair visible denial with backend/probe evidence, never substitute for it. OS enforcement lands per desktop item.
- First increment: add a validated host-origin/window/session contract, bind backend capability verification to that contract, and ship a strict browser CSP/navigation policy. Live authenticated service and OS permission probes remain separate acceptance slices.

<a id="METIS-COMMANDS-001"></a>
## METIS-COMMANDS-001 — Typed commands and event streams [arch] [minor]
- Status: todo; priority: P1; owner: Metis protocol/client/broker; dependencies: METIS-ASYNC-001, METIS-AUTHORITY-001; risk: public wire contract
- Scope: general command registration, typed payloads/errors, bounded subscriptions/channels, unsubscribe/cancel and schema/version diagnostics; migrate in-repo callers without forwarding shims.
- Acceptance: generic conformance suite across admitted transports; changing inputs changes outputs; unknown command/version rejects, late responses cannot mutate a new request and unsubscribed handlers receive nothing.
- Demonstration: [V02](docs/VERIFICATION.md#V02) and [V09](docs/VERIFICATION.md#V09), real backend actions/events in the manual.

<a id="METIS-INPUT-001"></a>
## METIS-INPUT-001 — Interactive controls and shared UI state [minor]
- Status: todo; priority: P1; owner: Metis UI/host; dependencies: METIS-BROWSER-001; risk: input/state mismatch
- Scope: buttons, checks, radios, sliders, editable fields, select/menu/dialog controls; focus, pointer capture, drag/drop, wheel/touch/modifiers, shortcuts, reusable state/actions and subscription teardown.
- Acceptance: keyboard and pointer/touch journeys update identical model values; disabled controls reject action; focus survives rerender and subscriptions detach on close; real hit targets agree with rendered geometry.
- Demonstration: [V02](docs/VERIFICATION.md#V02), settings workbench; repeat on each native host as it becomes supported.

<a id="METIS-TEXT-001"></a>
## METIS-TEXT-001 — Text, selection and IME [minor]
- Status: todo; priority: P1; owner: Metis input/presentation; dependencies: METIS-INPUT-001; risk: text corruption
- Scope: DOM text first; grapheme selection, composition/preedit/commit/cancel, clipboard/undo, wrapping, fallback fonts, bidi and text scaling. Custom renderer requires its own admitted text contract.
- Acceptance: Unicode fixture strings/selection ranges and caret/line geometry match the contract; native IME exercised per OS, including CJK, combining marks, emoji and mixed-direction input.
- Demonstration: [V03](docs/VERIFICATION.md#V03), editing specimen with actual composition and committed captures, locale/font details and keyboard instructions.

<a id="METIS-A11Y-001"></a>
## METIS-A11Y-001 — Accessible application interaction [minor]
- Status: todo; priority: P1; owner: Metis host/UI; dependencies: METIS-INPUT-001; risk: inaccessible controls
- Scope: semantic DOM roles/names/states, focus/action mapping, announcements, reduced motion/high contrast/zoom; OS accessibility bridge for any custom UI path.
- Acceptance: semantic tree identity/actions and keyboard-only completion pass; actual supported screen readers traverse and operate the application; document platform limits instead of claiming certification from tree presence.
- Demonstration: [V03](docs/VERIFICATION.md#V03), readable focus/contrast/zoom captures plus semantic/action and assistive-technology evidence.

<a id="METIS-LAYOUT-001"></a>
## METIS-LAYOUT-001 — Responsive layout and style semantics [minor]
- Status: todo; priority: P1; owner: Metis presentation; dependencies: METIS-STATE-001; risk: silent style mismatch
- Scope: reject or implement currently ineffective custom styles; DOM route uses actual CSS flex/grid, overflow/scrolling, nesting/clipping, min/max sizes, theme and scale. No custom browser-engine rewrite.
- Acceptance: admitted geometry is independently asserted at narrow/wide viewports and display scales; clipping/hit targets match; unsupported custom properties produce diagnostics, not silent success.
- Demonstration: [V04](docs/VERIFICATION.md#V04); browser captures follow BROWSER, custom subset tests can land before it.

<a id="METIS-MACOS-001"></a>
## METIS-MACOS-001 — macOS restricted desktop [arch] [minor]
- Status: todo; priority: P1; owner: Metis macOS host + Moirai; dependencies: METIS-DESKTOP-001; risk: OS boundary
- Scope: native window/WebView events, accessibility/IME integration, process containment, sandbox restrictions and teardown; share broker semantics, not Windows implementation details.
- Acceptance: host-native [V05](docs/VERIFICATION.md#V05) plus unauthorized file/network/process denial, child drain and positive IPC; cross compilation alone cannot close this item.
- Demonstration: actual macOS window/permission captures and manual setup/troubleshooting from the tested revision.

<a id="METIS-LINUX-001"></a>
## METIS-LINUX-001 — Linux restricted desktop [arch] [minor]
- Status: todo; priority: P1; owner: Metis Linux host + Moirai; dependencies: METIS-DESKTOP-001; risk: OS/display boundary
- Scope: native WebView hosting under Wayland and X11, input/accessibility/IME, process containment, permission restrictions and bounded teardown.
- Acceptance: [V05](docs/VERIFICATION.md#V05) on both display paths, positive IPC and unauthorized file/network/process denial; unavailable prerequisites produce actionable errors, never sandbox bypass.
- Demonstration: actual captures and distribution/display prerequisites in the manual; no support inferred from a Linux build.

<a id="METIS-ASSETS-001"></a>
## METIS-ASSETS-001 — Images, vectors and media assets [minor]
- Status: todo; priority: P1; owner: Metis asset/presentation + existing Atlas format providers; dependencies: METIS-BROWSER-001, METIS-AUTHORITY-001; risk: hostile content
- Scope: bounded local asset loading, image/SVG presentation, font loading and browser audio/video controls; validate paths/origins, dimensions/decoding budgets and target permissions.
- Acceptance: malformed/truncated/oversized/traversal assets fail; declared colors/alpha/aspect ratio/orientation match fixtures; media error and teardown states release resources.
- Demonstration: [V06](docs/VERIFICATION.md#V06), actual asset gallery with source attribution and load/error states.

<a id="METIS-GRAPHICS-001"></a>
## METIS-GRAPHICS-001 — Custom graphics conformance [arch] [minor]
- Status: todo; priority: P2; owner: Metis custom renderer over Iris; dependencies: METIS-VISUAL-001, METIS-LAYOUT-001; risk: rendering/lifetime correctness
- Scope: admitted vector/image/transform/clip operations and accelerated display path where required by the custom-UI demonstrator; retain one rendering contract and verify Atlas GPU ownership before additions.
- Acceptance: geometry/color/alpha and device-loss/recreate tests; differential software/device output under justified raster bounds; measured profile justifies acceleration and accounts for memory cost.
- Demonstration: [V06](docs/VERIFICATION.md#V06); this path never gates the DOM/browser migration and cannot stand in for HTML5 compatibility.

<a id="METIS-DATA-001"></a>
## METIS-DATA-001 — Tables, lists and live data views [minor]
- Status: todo; priority: P1; owner: Metis component/state; dependencies: METIS-INPUT-001, METIS-COMMANDS-001; risk: growth/selection drift
- Scope: sort/filter/selection, virtualized rows, tree disclosure and asynchronous loading/error/empty states; pure data operations remain independent of the GUI host.
- Acceptance: exact row order/filter values and stable selected identity after updates; bounded visible-window storage and subscriptions; keyboard operation and accessible semantics.
- Demonstration: [V07](docs/VERIFICATION.md#V07), result explorer from deterministic fixtures, including empty/error/live-update captures.

<a id="METIS-FILES-001"></a>
## METIS-FILES-001 — Scoped files and persistent state [minor]
- Status: todo; priority: P2; owner: Metis broker + owning Atlas storage provider; dependencies: METIS-DESKTOP-001; risk: user data/TOCTOU
- Scope: file dialogs, reads/writes/watch, settings/store/database contract and versioned recovery with explicit scope; no raw frontend access to unrestricted paths.
- Acceptance: allowed-handle operations succeed; traversal/symlink/TOCTOU and denied scope fail; atomic writes, crash/disk-full recovery and watcher teardown preserve user data.
- Demonstration: [V08](docs/VERIFICATION.md#V08), document open/save/restart/denial on each supported target; browsers expose selected-file semantics or explicit restrictions.

<a id="METIS-INTEGRATION-001"></a>
## METIS-INTEGRATION-001 — Desktop integration services [minor]
- Status: todo; priority: P2; owner: Metis host/broker; dependencies: METIS-DESKTOP-001; risk: OS interaction
- Scope: menus/tray, clipboard, notifications, global shortcuts, deep links, file associations, opener, single-instance and window state; bind each exposed API to policy.
- Acceptance: typed commands/events agree across supported hosts, denial/error paths remain explicit, clipboard/user-content permissions obey host rules and listeners unregister on shutdown.
- Demonstration: [V08](docs/VERIFICATION.md#V08), actual OS interactions and permission failures; capture menus/dialogs where visible and assert nonvisual events.

<a id="METIS-SERVICES-001"></a>
## METIS-SERVICES-001 — Scoped network, shell and sidecars [minor]
- Status: todo; priority: P2; owner: Moirai mechanisms + Metis policy; dependencies: METIS-DESKTOP-001, METIS-COMMANDS-001; risk: privilege escalation
- Scope: HTTP/WebSocket and subprocess APIs, sidecar lifecycle, endpoint/argument allowlists, bounded IO and credential redaction; local test services only by default.
- Acceptance: unauthorized endpoints/commands/arguments fail; transient errors, deadlines, cancellation, crash and cleanup are exercised against real processes/local servers; no shell-string injection or secret output.
- Demonstration: [V08](docs/VERIFICATION.md#V08), connection/process status and denial journey; browsers never receive arbitrary native shell access.

<a id="METIS-MIGRATION-001"></a>
## METIS-MIGRATION-001 — egui and Tauri application migration [arch] [minor]
- Status: todo; priority: P1; owner: Metis framework + application owners; risk: lost application behavior.
- Dependencies: METIS-COMMANDS-001, METIS-FILES-001, METIS-INPUT-001, METIS-ASSETS-001, METIS-GRAPHICS-001, METIS-DESKTOP-001, METIS-BROWSER-001, METIS-INTEGRATION-001, METIS-SERVICES-001.
- Named driver: [ritk-snap](../ritk/backlog.md#RITK-SNAP-METIS-001), currently egui/eframe at RITK `341228e`; no Tauri dependency found in its manifest/workspace lock. RITK owns decoder, volume geometry and medical display correctness; Metis supplies the replacement shell.
- Baseline: RITK `8152f483` ([PR 237](https://github.com/ryancinsight/ritk/pull/237)) adds physical display/hit rectangles to selected-study loading, restore/rejection and real native capture; [manual](docs/manual/applications.md#dicom-viewer-migration-baseline). This is egui prerequisite evidence, not Métis execution.
- Scope: inventory the actual viewer and a distinct pinned Tauri fixture; native Metis implementations replace required UI/state/input/render/file/lifecycle surfaces. First viewer journey opens a local DICOM study, selects its series and displays all three orthogonal views; full cutover retains the whole admitted viewer inventory.
- Acceptance: [V09](docs/VERIFICATION.md#V09) plus RITK opening/frames/color/grayscale prerequisites; required symbols/config/plugins and viewer actions are mapped/tested. Existing bugs cannot serve as parity oracles. No retained egui/eframe/Tauri runtime or forwarding shim in the completed migrated viewer.
- Demonstration: actual same-study before/after workflows, verified voxels/physical coordinates and real host captures in the user manual; record JavaScript retained versus Rust/WASM replacement and matched memory evidence.

<a id="METIS-DISTRIBUTION-001"></a>
## METIS-DISTRIBUTION-001 — Executables and Windows MSI [arch] [minor]
- Status: done; delivery: `feat(distribution): Build executables and MSI`; decision: [ADR 0005](docs/adr/0005-application-distribution.md).
- Outcome: one manifest, exact Cargo inventory, portable bundle, per-user MSI and manual; 102 debug/release tests, 36 Python tests, visual gate and real install/run/uninstall preserving user files pass.

<a id="METIS-DISTRIBUTION-002"></a>
## METIS-DISTRIBUTION-002 — Developer application lifecycle [minor]
- Status: todo; priority: P2; owner: Metis tooling; dependencies: METIS-DISTRIBUTION-001; risk: stale build/runtime state
- Scope: init/dev commands, generated help/completions and asset invalidation using the existing manifest; no second configuration grammar.
- Acceptance: scaffold builds/runs, actual source/resource changes reload, invalid builds report and never run stale output; repeated reload retains bounded state. [V10](docs/VERIFICATION.md#V10).

<a id="METIS-DISTRIBUTION-003"></a>
## METIS-DISTRIBUTION-003 — macOS and Linux installers [arch] [minor]
- Status: todo; priority: P2; owner: Metis tooling; dependencies: METIS-DISTRIBUTION-001; risk: platform ownership and lifecycle
- Scope: target-specific executable/bundle and installation formats using the same validated inventory; each target requires its native host for installation evidence.
- Acceptance: build/install/run/uninstall on macOS and Linux preserves user files and application behavior; actual host evidence and platform manual instructions. [ADR 0005](docs/adr/0005-application-distribution.md), [V10](docs/VERIFICATION.md#V10).

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

<a id="METIS-PERF-001"></a>
## METIS-PERF-001 — Comparative resource and latency evidence [patch]
- Status: todo; priority: P1; owner: Metis measurement; dependencies: METIS-BROWSER-001, METIS-DESKTOP-001, METIS-INPUT-001; risk: invalid comparative claims
- Scope: instrument the first live app, then compare matched egui/GPUI/Tauri fixtures; total process memory, WASM memory, allocations, idle/active/peak/growth, startup/input/frame latency and bundle/build size separately.
- Acceptance: [V12](docs/VERIFICATION.md#V12) protocol, pinned revisions/assets/traces and controlled host; stored baselines/confidence and resource bounds; resolve production regressions without changing the instrument to move results.
- Demonstration: measured tables/plots in the manual with machine/target/uncertainty and semantic/visual equivalence; no speed/security ranking without its evidence.

<a id="METIS-QUALITY-001"></a>
## METIS-QUALITY-001 — Verification infrastructure [patch]
- Status: todo; priority: P0; owner: Metis verification/integration; dependencies: METIS-VISUAL-001; risk: unreliable gates
- Scope: one pinned gate/CI definition for implemented targets, plan ID/dependency/link validation, artifact provenance, advisory/dependency/native-link audits, public-API checks, parser fuzz/property/mutation and structured redacted diagnostics; host jobs arrive with working host implementations.
- Acceptance: current [V01](docs/VERIFICATION.md#V01), native and WASM-build suites run under committed budgets; deliberate state/image/parser regressions fail the correct gate; no nonexistent host job reports green.
- Demonstration: manual troubleshooting shows actual gate failure and recovery artifacts. This finite infrastructure item does not close later scenario coverage.

<a id="METIS-CONFORMANCE-001"></a>
## METIS-CONFORMANCE-001 — Final capability and target closure [patch]
- Status: todo; priority: P3; owner: Metis integration; risk: incomplete framework claim
- Dependencies: METIS-QUALITY-001, METIS-VERIFY-001, METIS-PROVIDER-001, METIS-STATE-001, METIS-VISUAL-001, METIS-BROWSER-001, METIS-ASYNC-001, METIS-AUTHORITY-001, METIS-COMMANDS-001, METIS-DESKTOP-001, METIS-MACOS-001, METIS-LINUX-001, METIS-TEXT-001, METIS-A11Y-001, METIS-LAYOUT-001, METIS-ASSETS-001, METIS-GRAPHICS-001, METIS-DATA-001, METIS-FILES-001, METIS-INTEGRATION-001, METIS-SERVICES-001, METIS-AUDIT-001, METIS-CRYPTO-001, METIS-MEMORY-001, METIS-MIGRATION-001, METIS-DISTRIBUTION-001, METIS-MOBILE-001, METIS-PERF-001, METIS-MANUAL-001
- Scope: reconcile every ADR 0003 matrix row and required target pair against exact-revision evidence, including late package/mobile/service work and current upstream inventory changes.
- Acceptance: [V01](docs/VERIFICATION.md#V01)–[V12](docs/VERIFICATION.md#V12) run on their required real targets, with semantic/visual/security/resource results and manual demos; required unsupported pairs stay open and block closure. No mocked IPC, compile-only host claim or skipped denial/lifecycle suite.
- Demonstration: same-revision gallery, migration guide, measured comparative report and host troubleshooting; final security claims require matched Tauri denial probes under the declared threat model.

<a id="METIS-VERIFY-002"></a>
## METIS-VERIFY-002 — Verification report storage [patch]
- Status: done; outcome: full gate passes with 102 debug/release native tests and 36 Python tests; JUnit lives under ignored output and no local target reappears. Four crate roots explicitly deny missing documentation.

<a id="METIS-REGISTRATION-001"></a>
## METIS-REGISTRATION-001 — Public Atlas member registration [patch]
- Status: blocked; priority: P0; integrator: root; last-update: 2026-09-06
- Scope: upstream [registration item](../../backlog.md#metis-unregistered-member); preserve unrelated Atlas shared-tree work.
- Acceptance: Atlas records the published default gitlink, member configuration and measured initial conformance baseline; exact candidate passes the committed gate.
- Blocker: Atlas pre-commit unsets GIT_INDEX_FILE before checking staged pins; root auditors inspect live HEAD/configuration. Existing alternate-index workflow cannot establish candidate verification.
- Re-open: candidate-aware hooks/gates or a clean available Atlas checkout. Public Métis source remains independently consumable.
- Completed: public repository and PR 8 verified/merged; registration closure and initial-baseline semantics audited against Atlas source. No runtime or signing capability depends on this metadata change.

<a id="METIS-APPLICATION-001"></a>
## METIS-APPLICATION-001 — Single executable application [arch] [major]
- Status: done; outcome: one relocated application image runs distinct process roles through Moirai; 108 debug/release tests, 38 Python checks, real single-executable MSI install/run/uninstall and unchanged visual snapshots pass. [Design and migration](docs/adr/0006-application-entry.md).
