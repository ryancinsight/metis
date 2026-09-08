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
- Status: done; priority: P1; delivery: `def5f85`; source and artifact audit passed 2026-09-07.
- Outcome: Source-pinned Iced 0.14 comparison and manual evidence satisfy ADR 0003; no Iced runtime dependency or fabricated capture was added.

<a id="METIS-WEB-001"></a>
## METIS-WEB-001 — Web application target contract [arch] [patch]
- Status: done; delivery: [PR 1](https://github.com/ryancinsight/metis/pull/1), `65e6af1`.
- Outcome: [ADR 0002](docs/adr/0002-web-application-contract.md) and WASM library gate define web targets without runtime claims.

<a id="METIS-BROWSER-001"></a>
## METIS-BROWSER-001 — Browser form and command lifecycle [arch] [minor]
- Status: in-progress; priority: P1; owner: Metis frontend/host; integrator: root; last-update: 2026-09-07; branch: `feat/process-foundation`; dependencies: METIS-STATE-001, METIS-ASYNC-001, METIS-AUTHORITY-001; risk: browser/native trust boundary
- Scope: actual HTML5/CSS DOM form, Rust/WASM state, asset loading and bounded asynchronous requests; portable UI never imports native authority.
- Acceptance: Chromium/Firefox/WebKit runtime jobs load WASM and respond to two input changes; authorized service/desktop bridge verifies results; explicit unsupported native-only operations; zero pending requests/listeners after cancel/close.
- Demonstration: [V02](docs/VERIFICATION.md#V02), actual browser captures and copyable build/run commands in the manual. A browser-only local control demo can land before the privileged bridge.
- Constraint: no native secrets or authority in downloaded WASM; private-pipe possession cannot authenticate browser requests. Desktop bridge or service boundary must enforce origin/session authorization.
- Evidence: `metis-web` mounts a real DOM form through Moirai's owned handles and connects `AsyncFrontendApp` through `BrowserWebSocketTransport` when host configuration is present. The live trace completed an authenticated loopback handshake, exact backend results, numeric rejection, service disconnect, stop/remount cancellation and recovery with no browser console diagnostics. Native loopback tests reject an unauthorized Origin before `101 Switching Protocols`; post-drop allocation, TLS, accessibility/IME, cross-engine and desktop evidence remain open.
- Decision: [ADR 0002](docs/adr/0002-web-application-contract.md), [ADR 0008](docs/adr/0008-browser-host-boundary.md).

<a id="METIS-BROWSER-002"></a>
## METIS-BROWSER-002 — Browser stale-response runtime probe [patch]
- Status: done; priority: P1; owner: Metis browser host + verification; integrator: root; last-update: 2026-09-07; branch: `feat/process-foundation`; delivery: `a8cc67c`; dependencies: METIS-BROWSER-001, METIS-COMMANDS-001; risk: stale DOM mutation
- Scope: delay a real service response at the browser transport boundary, stop/remount the WASM host, and observe response disposal and DOM stability; cross-engine, TLS, native desktop and OS permissions remain separate.
- Acceptance: a bounded delayed response cannot change the stopped or remounted DOM; the browser task and WebSocket callbacks are released; the trace records the exact engine, revision, action sequence and observable state.
- Demonstration: [V02](docs/VERIFICATION.md#V02), [browser stale-response evidence](docs/VERIFICATION.md#browser-stale-response-evidence--2026-09-07) and the delayed-response section in the [browser manual](docs/manual/browser.md#verify-stopremount-disposal-at-the-service-boundary).
- Outcome: The single `metis-app` executable exposes a bounded `--response-delay-ms` service probe backed by Moirai's async timer. The live stop/remount trace leaves the remounted DOM at `Backend unavailable [ERR_TRANSPORT_BROKEN]` with no stale result or event after the deadline.

<a id="METIS-SEC-001"></a>
## METIS-SEC-001 — Backend authority [arch] [patch]
- Status: done; priority: P0; delivery: `1517ce5`; authority audit passed 2026-09-07.
- Outcome: Host-bound capability, expiry, cross-session rejection and frontend dependency checks satisfy [ADR 0001](docs/adr/0001-process-contract.md) and [ADR 0011](docs/adr/0011-host-authority-policy.md); OS permissions and durable audit remain separate gaps.

<a id="METIS-IPC-001"></a>
## METIS-IPC-001 — Canonical bounded IPC [patch]
- Status: done; priority: P0; delivery: `7347ecc`; contract audit passed 2026-09-07.
- Outcome: Versioned framing, canonical payloads, typed malformed/replay/oversize errors, bounded correlation and event retention, and sync/async transport tests pass; browser and native host gaps remain separate.

<a id="METIS-UI-001"></a>
## METIS-UI-001 — Bounded presentation [major]
- Status: done; priority: P0; delivery: `ab722a8`; style contract audit passed 2026-09-07.
- Outcome: Strict typed style parsing, parser propagation, positive/negative coverage, software rendering evidence and migration documentation satisfy [ADR 0013](docs/adr/0013-strict-style-contract.md); browser/native CSS and host gaps remain separate.

<a id="METIS-PROCESS-001"></a>
## METIS-PROCESS-001 — Real executable workflow [arch] [minor]
- Status: done; priority: P0; delivery: `e6daa0d`; process workflow audit passed 2026-09-07.
- Outcome: One copied `metis-app` image runs separate backend/frontend processes through Moirai pipes with input-sensitive results, typed failure propagation, bounded Windows tree cleanup and V01/V10 documentation; non-Windows containment, native windows and OS permissions remain separate requirements.

<a id="METIS-DESKTOP-001"></a>
## METIS-DESKTOP-001 — Native restricted desktop [arch] [minor]
- Status: in-progress; priority: P1; owner: Metis Windows host + Moirai; integrator: root; last-update: 2026-09-08; branch: `feat/process-foundation`; dependencies: METIS-AUTHORITY-001, METIS-COMMANDS-001; risk: trust boundary
- Scope: Windows native window/system WebView, real events, multi-window lifecycle and OS-restricted renderer; macOS/Linux have separate items below.
- Acceptance: actual visible form, pointer/keyboard/resize/DPI/close/reopen; file/network/process denial probes; IPC remains functional under restrictions and all child processes drain.
- Demonstration: [V05](docs/VERIFICATION.md#V05), actual Windows window captures, keyboard journey and permission-denied results in the manual.
- Current evidence: Moirai PR #287 (`7ad8eeee`, following PRs #286 and #284) supplies the real Win32 HWND, bounded message translation including IME composition phases, retained ARGB presentation, finite queue waiting and retained-event readiness. `metis-platform::native::NativeSurface` is the safe consumer boundary; `PlatformSurface` and `PlatformEvent` remain portable application-supplied values.
- Completed increments: the adapter test creates a real hidden HWND, presents the production framebuffer, observes resize and closes the window; the `metis-app --metis-native-window` role now composes that surface with the real frontend and supervised private IPC, handling text, transient IME preedit/commit/cancel, Enter/click submit, resize, DPI, focus and close; focused nextest and warning-denied Clippy pass on Windows.
- Decision: [ADR 0015](docs/adr/0015-native-window-provider.md) (accepted); a committed visual capture, installed IME journey, WebView2 composition, OS permission enforcement, accessibility and macOS/Linux providers remain open follow-on slices.

<a id="METIS-AUDIT-001"></a>
## METIS-AUDIT-001 — Durable audit recovery [minor]
- Status: todo; priority: P2; owner: Metis backend + owning Atlas storage provider; dependencies: METIS-CRYPTO-001; risk: persistence
- Scope: versioned durable backend audit, bounded storage, restart recovery and trusted checkpoint; first verify the Atlas storage ownership/contract.
- Acceptance: crash/truncation/tamper/disk-full cases recover exactly or fail closed, with bounded retention and no patient/secret leakage.
- Demonstration: [V08](docs/VERIFICATION.md#V08), audit/recovery inspector showing actual records, denied tampering and restart outcomes; raw secrets never enter captures.

<a id="METIS-VERIFY-001"></a>
## METIS-VERIFY-001 — Verify and deliver foundation [patch]
- Status: done; priority: P0; delivery: `f246c81`; exact standalone gate passed 2026-09-07.
- Outcome: Pinned Rust 1.97.0/nextest verification passed all stages, 155-package locked resolution and provider graph audit; Metis direct runtime edges are Atlas repositories (CLI parser exception documented), visual baselines match, and Windows/browser/WASM limits remain explicit.

<a id="METIS-PROVIDER-001"></a>
## METIS-PROVIDER-001 — Atlas provider adoption [arch] [patch]
- Status: done; priority: P0; delivery: `0a2d8e4`; provider audit passed 2026-09-07.
- Outcome: Clean Moirai `be87d009cd0e877beef719b47bdcbadc45659069` (`main`) and Iris `764ed2b4b1696363abc3950f0d930d244126e4bf` (`main`) match every standalone lock source; 178/178 consumer tests pass with strict diagnostics, no parallel GUI/runtime dependency exists, and the 155-package provider transitive graph is recorded. Runtime crates use Atlas direct dependencies; the distribution CLI's Serde exception is documented in ADR 0005. Browser/native host gaps remain separate.

<a id="METIS-CRYPTO-001"></a>
## METIS-CRYPTO-001 — Shared authentication primitives [arch] [patch]
- Status: done; delivery: Metis PR [#14](https://github.com/ryancinsight/metis/pull/14) merged at `2258266`; [ADR 0009](docs/adr/0009-crypto-provider-boundary.md).
- Outcome: Metis imports Moirai `be87d009cd0e877beef719b47bdcbadc45659069` standalone SHA-256, HMAC-SHA256 and fixed-width comparison APIs with TLS dependencies disabled; CRC-32 remains local and no duplicate authentication implementation remains.

<a id="METIS-RELEASE-001"></a>
## METIS-RELEASE-001 — Publication readiness [patch]
- Status: blocked; priority: P3; owner: Metis delivery; integrator: root; last-update: 2026-09-08; branch: `feat/process-foundation`; dependencies: METIS-DISTRIBUTION-001, METIS-CONFORMANCE-001; blocker: crates.io trusted-publisher registration and explicit release authority are external; re-open: registrations and release authority are available without adding repository secrets
- Scope: release-readiness metadata, package dry runs, final manual and platform evidence; registry authentication uses Atlas OIDC workflows without personal keys; preparation continues without release authority.
- Acceptance: dependency-closed packages and exact-revision evidence; release execution is blocked until explicit authority, registry publisher registration, rollout and rollback details are available.
- Current increment: the thin crates.io caller is pinned to Atlas's reusable OIDC workflow; its contract tests and committed full gate pass. Local crates.io publish validation is blocked by index network access, while registry registration and release authority remain open. The PyO3 package and PyPI caller are tracked by [METIS-PYTHON-001](#METIS-PYTHON-001); PyPI trusted-publisher registration remains an external release action.
- Demonstration: [V10](docs/VERIFICATION.md#V10), locally built package installation/recovery instructions and actual captures before any publication.

<a id="METIS-PYTHON-001"></a>
## METIS-PYTHON-001 — PyO3 application binding [arch] [minor]
- Status: done; priority: P1; delivery: `00f3681`; exact full gate passed 2026-09-08; owner/integrator: Metis Python integration/root.
- Outcome: `metis-python` exposes validated clinical Rust types through an abi3 `import metis` wheel, built-wheel value tests, typed stubs, manual workflow and a tokenless PyPI OIDC caller; visual baseline refreshed for the locked dependency graph.
- Decision: [ADR 0017](docs/adr/0017-python-binding.md); release registration remains external per [METIS-RELEASE-001](#METIS-RELEASE-001).

<a id="METIS-MEMORY-001"></a>
## METIS-MEMORY-001 — Provider allocation count [patch]
- Status: done; priority: P0; delivery: `f246c81`; exact locked-provider and reachable-path audit passed 2026-09-07.
- Scope: verify the public count contract, locked-source exposure and local allocator multiplication before classifying the defect; do not assume all allocations use this path.
- Outcome: Metis does not instantiate Moirai `CacheAlignedAllocator`/`UnifiedRingBuffer` or Mnemosyne allocation statistics; its external allocation paths use finite caps and fallible reservation. The provider arithmetic concern is unreachable from the current graph. Reopen if a Metis API selects provider allocation or V12 integrates provider telemetry.

<a id="METIS-MANUAL-001"></a>
## METIS-MANUAL-001 — Public member and user manual [patch]
- Status: done; priority: P1; owner: Metis documentation/integration; integrator: root; last-update: 2026-09-07; delivery: Atlas `c7db87d0a`
- Scope: public GitHub repository, Atlas gitlink, user-oriented manual and actual rendered application snapshots; no registry release.
- Acceptance: public remote contains tested source; Atlas resolves the pinned commit; manual links resolve and generated snapshot matches the renderer.
- Decision: [ADR 0001](docs/adr/0001-process-contract.md); user manual replaces the domain-book requirement by explicit user direction.
- Outcome: Public source/manual and rendered captures are present; Atlas registers `repos/metis` and pins the verified `feat/process-foundation` revision. Every later item owns its demonstration section, not a deferred documentation phase.

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
- Status: done; delivery: `a1f50ca`; ADR: [0007](docs/adr/0007-browser-transport.md); verification: [VERIFICATION](docs/VERIFICATION.md#live-browser-service-evidence--2026-09-07).
- Outcome: bounded Moirai browser client/server, trusted pre-101 Origin/session validation, and live HTML5/CSS service trace pass; residual platform/resource coverage remains owned by the linked browser, memory, performance, services and desktop items.

<a id="METIS-AUTHORITY-001"></a>
## METIS-AUTHORITY-001 — Host authority and origin policy [arch] [minor]
- Status: done; priority: P0; owner: Metis broker + Moirai host mechanisms; integrator: root; last-update: 2026-09-07.
- Commit: `1517ce5`; ADR: [0011](docs/adr/0011-host-authority-policy.md); verification: [VERIFICATION](docs/VERIFICATION.md#host-authority-and-asset-evidence--2026-09-07).
- Outcome: canonical origin/window/session binding, host-bound HMAC verification, strict CSP/navigation asset policy, and positive/denial coverage pass the full standalone gate. The live browser acceptor validates Origin before the WebSocket response and requires the trusted context; target surface discovery now reports installed mechanisms, while OS enforcement remains a separate item.

<a id="METIS-COMMANDS-001"></a>
## METIS-COMMANDS-001 — Typed commands and event streams [arch] [major]
- Status: done; priority: P1; owner: Metis protocol/client/broker; integrator: root; last-update: 2026-09-07; branch: `feat/process-foundation`; delivery: `0ec681c`; dependencies: METIS-ASYNC-001, METIS-AUTHORITY-001; risk: public wire contract; ADR: 0012
- Evidence: capability catalog, target surface descriptor, explicit unsupported-operation response, bounded event fan-out, versioned remote event envelope, sync/async event receipt, typed plugin manifest validation including operation-count limits, typed plugin invocation and browser display pass the focused run, full verification, and the live WebSocket workbench trace; target route and lifecycle delivery commits `7d2f784` and `4916fc0`; the public enum extension is classified major by semver comparison.
- Outcome: remote event and capability contracts, bounded delivery, typed plugin invocation, target-surface discovery, browser lifecycle generation guards and local/remote session error identity pass focused and workspace gates. The authenticated Moirai WebSocket loopback discovers its browser bridge and invokes an input-sensitive plugin. Browser delayed-response runtime probing is closed by METIS-BROWSER-002; native window and OS permission providers continue under their owning items.
- Scope: general command registration, typed payloads/errors, bounded subscriptions/channels, unsubscribe/cancel, schema/version diagnostics, target capability discovery and explicit unsupported-operation errors; migrate in-repo callers without forwarding shims.
- Acceptance: generic conformance suite across admitted transports; changing inputs changes outputs; capability discovery reports target support, unsupported operations return typed errors, unknown command/version rejects, late responses cannot mutate a new request and unsubscribed handlers receive nothing.
- Demonstration: [V02](docs/VERIFICATION.md#V02) and [V09](docs/VERIFICATION.md#V09), real backend actions/events in the manual.

<a id="METIS-INPUT-001"></a>
## METIS-INPUT-001 — Interactive controls and shared UI state [minor]
- Status: in-progress; priority: P1; owner: Metis UI/host; integrator: root; last-update: 2026-09-07; branch: `feat/process-foundation`; dependencies: METIS-BROWSER-001; risk: input/state mismatch; ADR: 0014 (claimed)
- Scope: buttons, checks, radios, sliders, editable fields, select/menu/dialog controls; focus, pointer capture, drag/drop, wheel/touch/modifiers, shortcuts, reusable state/actions and subscription teardown.
- Acceptance: keyboard and pointer/touch journeys update identical model values; disabled controls reject action; focus survives rerender and subscriptions detach on close; real hit targets agree with rendered geometry.
- Demonstration: [V02](docs/VERIFICATION.md#V02), settings workbench; repeat on each native host as it becomes supported.
- Completed increment: `feat(web): Add semantic browser controls` adds Rust-owned checkbox, radio, bounded range and select state, the Moirai `WebElement::checked` and input/select value seams, and split control/view modules; native `metis-web` tests pass 10/10 and the WASM gate is clean.
- Live evidence: [browser control evidence](docs/VERIFICATION.md#browser-control-evidence--2026-09-07) records pointer radio/visibility changes, two keyboard range steps and a native select change in the authenticated service trace. Presentation changes preserve the correlated backend result.
- Completed increment: `feat(web): Enforce disabled control lifecycle` consumes Moirai `WebElement::disabled`/`set_disabled` from `d879779247c8cfc5870f62f99a5364cbbf2d3c58`; the submit button is disabled until the authenticated bridge is ready and while a request is pending, and programmatic submits are ignored outside the ready state.
- Live evidence: [browser control evidence](docs/VERIFICATION.md#browser-control-evidence--2026-09-07) records the disabled accessibility state before connection, the pending-state disable during a delayed response, re-enablement after the correlated result, and unchanged state after activation was rejected.
- Completed increment: `feat(web): Add native session dialog` consumes Moirai `WebElement::dialog_open`, `show_modal`, `close_dialog` and `focus` from `8f02b8b7de6cf6361b519bd79759d8508568fbdb`; Rust mounts the semantic dialog, opens it modally, closes it through the provider and restores focus to the opener.
- Live evidence: [browser dialog evidence](docs/VERIFICATION.md#browser-dialog-evidence--2026-09-07) records the authenticated modal screenshot, accessibility content, provider open-state read, close action, Escape dismissal and opener focus restoration.
- Completed increment: `feat(web): Add pointer capture surface` consumes Moirai `WebEvent::pointer_id`, `WebElement::set_pointer_capture`, `has_pointer_capture` and `release_pointer_capture` from `5a5e4b1540eff39bc3f082c6907f0c82fa14dcc8`; Rust captures one pointer ID on `pointerdown`, verifies the provider state and releases it on `pointerup` or `pointercancel`.
- Live evidence: [browser pointer-capture evidence](docs/VERIFICATION.md#browser-pointer-capture-evidence--2026-09-07) records pointer ID `1`, release status, semantic surface name and the rendered surface screenshot.
- Completed increment: `feat(web): Render pointer metadata` consumes Moirai `WebEvent::pointer_metadata` and `PointerMetadata` from merged revision `a3c86cd183a18edc35db30f1d35e79fe80092df4`; Rust renders device type, CSS-pixel coordinates, changed/held buttons, modifiers and primary-pointer state on capture and movement.
- Live evidence: [browser pointer metadata evidence](docs/VERIFICATION.md#browser-pointer-metadata-evidence--2026-09-07) records the input-sensitive status and screenshot; the bounded gesture policy is recorded in [browser gesture policy evidence](docs/VERIFICATION.md#browser-gesture-policy-evidence--2026-09-07).
- Completed increment: `feat(web): Render wheel metadata` consumes Moirai `WebEvent::wheel_metadata` and `WheelMetadata` from merged revision `f634b3a802ec0355da22f111ed01067d2435c5cb`; Rust renders bounded deltas, browser unit, viewport coordinates and modifiers on the pointer surface.
- Live evidence: [browser wheel metadata evidence](docs/VERIFICATION.md#browser-wheel-metadata-evidence--2026-09-07) records input-sensitive vertical and horizontal scroll actions and the rendered status; the automation trust limitation is explicit.
- Completed increment: `feat(web): Add bounded gesture policy` moves the Rust-owned `GestureViewport` state machine into a native-tested policy module; pointer drag pans, ordinary wheel input pans and Ctrl+wheel zooms with finite-input checks and bounded CSS transforms.
- Live evidence: [browser gesture policy evidence](docs/VERIFICATION.md#browser-gesture-policy-evidence--2026-09-07) records vertical/horizontal wheel pan and a pointer drag with the transformed content; the CUA hardware-trust limitation is explicit.
- Completed increment: `feat(web): Add bounded browser file drops` consumes Moirai `DropMetadata` and `DroppedFile` from merged revision `630f914bcb34d4d65cc5e3db27a121163d040199`; Rust revalidates metadata, caps the retained batch at 64 entries, classifies DICOM candidates and renders a semantic drop status without trusting paths.
- Completed increment: `feat(web): Read bounded DICOM headers` advances the consumer to Moirai `DropFiles` at merged revision `5c8a9e8be32ad6beac14ed263c2f11c3663b87cb`; the first selected browser file is read through the provider-owned handle into a fixed 132-byte buffer, the Part 10 marker is classified and `reading`/`complete`/`failed` states render in the workbench. RITK remains the owner of full dataset parsing and study decoding.
- Live evidence: [browser file-drop evidence](docs/VERIFICATION.md#browser-file-drop-evidence--2026-09-08) records the native policy suite, strict WASM checks and rendered drop-zone state; CUA cannot provide trusted OS file-drop evidence or a live byte-read trace.
- Residuals: trusted physical file-drop evidence, full RITK DICOM opening/decoding, multi-touch/pinch interpretation, installed IME journeys, accessibility technology, cross-engine parity, post-drop allocation measurement and native-host visual/assistive evidence remain open; re-open this item when those dependencies land.

<a id="METIS-TEXT-001"></a>
## METIS-TEXT-001 — Text, selection and IME [minor]
- Status: in-progress; priority: P1; owner: Metis input/presentation; integrator: root; last-update: 2026-09-08; branch: `feat/process-foundation`; dependencies: METIS-INPUT-001; risk: text corruption
- Scope: DOM text first; grapheme selection, composition/preedit/commit/cancel, clipboard/undo, wrapping, fallback fonts, bidi and text scaling. Custom renderer requires its own admitted text contract.
- Acceptance: Unicode fixture strings/selection ranges and caret/line geometry match the contract; native IME exercised per OS, including CJK, combining marks, emoji and mixed-direction input.
- Demonstration: [V03](docs/VERIFICATION.md#V03), editing specimen with actual composition and committed captures, locale/font details and keyboard instructions.
- Completed increment: browser `TextState` keeps bounded Unicode values, UTF-16 selection coordinates, input metadata and composition start/update/commit/cancel transitions; Moirai provider revision `0862716265d657b8069d5a47fd1e77ae26ddd006` owns the DOM snapshots and listener lifetime.
- Evidence: [browser text and composition evidence](docs/VERIFICATION.md#browser-text-and-composition-evidence--2026-09-08) records 21/21 native policy tests, warning-denied native/WASM Clippy, WASM build and the semantic textarea/value-preview surface.
- Completed increment: native `TextComposition` phases from Moirai `7ad8eeee` are consumed by `metis-app`; preedit text is bounded and transient, commit uses the ordinary bounded patient-field transition, and cancellation/focus loss clears it. The focused Metis suite covers the value transition.
- Residuals: grapheme-safe editing, bidi and line geometry, fallback-font metrics, clipboard/undo, an installed CJK or other native IME journey and assistive-technology acceptance remain open; CUA evidence is limited to HTML/WASM rendering and synthetic browser input.

<a id="METIS-A11Y-001"></a>
## METIS-A11Y-001 — Accessible application interaction [minor]
- Status: in-progress; priority: P1; owner: Metis host/UI; integrator: root; last-update: 2026-09-08; branch: `feat/process-foundation`; dependencies: METIS-INPUT-001; risk: inaccessible controls
- Scope: semantic DOM roles/names/states, focus/action mapping, announcements, reduced motion/high contrast/zoom; OS accessibility bridge for any custom UI path.
- Acceptance: semantic tree identity/actions and keyboard-only completion pass; actual supported screen readers traverse and operate the application; document platform limits instead of claiming certification from tree presence.
- Demonstration: [V03](docs/VERIFICATION.md#V03), readable focus/contrast/zoom captures plus semantic/action and assistive-technology evidence.
- Completed increment: `feat(web): Honor accessibility preferences` adds reduced-motion and forced-colors presentation rules, semantic focus-order assertions and synchronized manual/evidence text.
- Evidence: revision `6bbbd00` passes the full Metis gate and 51 Python tests; the CUA trace observes the document focus path and a visible focus outline on **Clinical note**.
- Residuals: supported screen-reader speech, forced-colors/reduced-motion runtime captures, zoom-scale geometry and native host accessibility bridge evidence remain open.

<a id="METIS-LAYOUT-001"></a>
## METIS-LAYOUT-001 — Responsive layout and style semantics [minor]
- Status: in-progress; priority: P1; owner: Metis presentation; integrator: root; last-update: 2026-09-08; branch: `feat/process-foundation`; dependencies: METIS-STATE-001; risk: silent style mismatch
- Scope: reject or implement currently ineffective custom styles; DOM route uses actual CSS flex/grid, overflow/scrolling, nesting/clipping, min/max sizes, theme and scale. No custom browser-engine rewrite.
- Acceptance: admitted geometry is independently asserted at narrow/wide viewports and display scales; clipping/hit targets match; unsupported custom properties produce diagnostics, not silent success.
- Demonstration: [V04](docs/VERIFICATION.md#V04); browser captures follow BROWSER, custom subset tests can land before it.
- Current increment: Browser viewport captures at `360×640`, `800×600` and `1440×900` CSS pixels now exercise the responsive breakpoint, bounded columns, overflow contract and `44px` option/slider hit targets at device scale `1`.
- Evidence: [ADR 0016](docs/adr/0016-theme-and-branding.md), the browser asset contract, [runtime manifest](docs/manual/images/browser-layout-metrics.json) and [manual captures](docs/manual/browser.md#responsive-runtime-capture) cover the selected modes and geometries.
- Completed increment: browser CSS now applies shared border-box sizing, zero-minimum grid items and long-string wrapping; the page uses a bounded two-column grid above `700px` and a one-column layout with `1rem` padding below it.
- Evidence: [browser responsive-layout evidence](docs/VERIFICATION.md#browser-responsive-layout-evidence--2026-09-08) and the static asset contract test cover the declarations; the full Metis gate passes on the committed revision.
- Residuals: device scale `2`, custom-style diagnostics and platform fractional-scale cases remain open because the available viewport capability does not expose a device-scale override.

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
- Status: in-progress; priority: P0; owner: Metis verification/integration; integrator: root; last-update: 2026-09-08
- Dependencies: METIS-VISUAL-001; risk: unreliable gates
- Scope: one pinned gate/CI definition for implemented targets, plan ID/dependency/link validation, artifact provenance, advisory/dependency/native-link audits, public-API checks, parser fuzz/property/mutation and structured redacted diagnostics; host jobs arrive with working host implementations.
- Acceptance: current [V01](docs/VERIFICATION.md#V01), native and WASM-build suites run under committed budgets; deliberate state/image/parser regressions fail the correct gate; no nonexistent host job reports green.
- Demonstration: manual troubleshooting shows actual gate failure and recovery artifacts. This finite infrastructure item does not close later scenario coverage.
- Completed increment (2026-09-07): pinned workflow and local gate validate plan identifiers, dependencies and local links before artifact-producing stages; cargo-deny audits the locked graph and the gate records reviewed Cargo build-link contracts; 47 Python checks pass.
- Completed increment (2026-09-08, commit `866822016d8ee02b2b38149efee2e26783c77bb2`): bounded arbitrary-byte, Unicode, IEEE-754, truncation, bit-mutation and oversized-length properties cover every public wire decoder; the standalone `fuzz/` LibFuzzer target passes a locked manifest check; `MetisError` and remote error payload `Debug` output redact untrusted message text. Focused nextest passes 96/96 with strict Clippy. The Windows MSVC host cannot link the LibFuzzer sanitizer runtime, so no runtime fuzz result is claimed.
- Completed increment (2026-09-08): `scripts/mutation.py` pins cargo-mutants 27.1.0, nextest, the shared target and finite budgets for the decoder slice; at revision `6af70734965ceb5eb94dd4ce1d663a50e4f532ab` it generated 14 mutants, caught all 6 viable mutants, and reported 0 missed, 0 timed-out and 8 unviable. The derived report is `output/mutation/latest/manifest.json`.
- Residual: a nightly LibFuzzer campaign on a host with a working sanitizer runtime remains open; the full gate is rerun when this increment is integrated.

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
