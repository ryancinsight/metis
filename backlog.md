# Metis delivery

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

<a id="METIS-GAPS-001"></a>
## METIS-GAPS-001 — Framework conformance plan [arch] [patch]
- Status: done; delivery: [PR 2](https://github.com/ryancinsight/metis/pull/2), content `a6855aa`.
- Outcome: [ADR 0003](docs/adr/0003-framework-conformance.md) maps 22 capability areas to owned work and twelve semantic/visual/manual scenarios; source review and existing gate pass. Runtime gaps remain open in the linked items.

<a id="METIS-GAPS-002"></a>
## METIS-GAPS-002 — Synchronize the framework matrix [patch]
- Status: done; priority: P1; owner: Metis documentation; delivery: [PR 34](https://github.com/ryancinsight/metis/pull/34), merge `056e252`; dependency: METIS-DICOM-002.
- Outcome: Current framework and input records assign DICOM decisions to RITK after the format-neutral handoff; LF-normalized provenance passes the full gate at `a397b18`.

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
- Status: in-progress; priority: P1; owner: Metis frontend/host; integrator: root; last-update: 2026-09-09; branch: `feat/browser-conformance-001`; regions: `scripts/browser_protocol.py`, `scripts/browser_runtime.py`, `scripts/tests/test_browser_runtime.py`, `docs/adr/0021-browser-conformance-runner.md`, `docs/manual/browser.md`, `docs/VERIFICATION.md`; dependencies: METIS-STATE-001, METIS-ASYNC-001, METIS-AUTHORITY-001; risk: browser/native trust boundary
- Scope: actual HTML5/CSS DOM form, Rust/WASM state, asset loading and bounded asynchronous requests; portable UI never imports native authority.
- Acceptance: Chromium/Firefox/WebKit runtime jobs load WASM and respond to two input changes; authorized service/desktop bridge verifies results; explicit unsupported native-only operations; zero pending requests/listeners after cancel/close.
- Demonstration: [V02](docs/VERIFICATION.md#V02), actual browser captures and copyable build/run commands in the manual. A browser-only local control demo can land before the privileged bridge.
- Constraint: no native secrets or authority in downloaded WASM; private-pipe possession cannot authenticate browser requests. Desktop bridge or service boundary must enforce origin/session authorization.
- Evidence: `metis-web` mounts a real DOM form through Moirai's owned handles and connects `AsyncFrontendApp` through `BrowserWebSocketTransport` when host configuration is present. The live trace completed an authenticated loopback handshake, exact backend results, numeric rejection, service disconnect, stop/remount cancellation and recovery with no browser console diagnostics. An in-app browser capture at revision `cadb684ca7c8bda3f1c873f93286871e620e6196` rendered the HTTP health page and observed handshake `200`, fragment `200 (1 patch)`, malformed `400`, unauthorized `401`, unchanged stale state and generation `1`. Native loopback tests reject an unauthorized Origin before `101 Switching Protocols`.
- Completed increment (2026-09-08, commit `c08dcbd`): ordinary `input` and `change` events are delegated at `#metis-app`; closed ID bindings preserve typed field updates while specialized pointer, file, text and dialog listeners remain provider-owned. `metis-web` nextest passes 26/26 and the WASM target check is clean. The committed browser trace covers weight, scale, theme and literal-text transitions at the same viewport ([evidence](docs/VERIFICATION.md#browser-delegated-control-evidence)); the htmx-informed event→action→target boundary is recorded in ADRs 0002/0003 and the browser manual.
- Completed increment (2026-09-09, commit `d5b603d`): `scripts/browser_runtime.py` adds a dependency-free W3C WebDriver runner for Chromium, Firefox and WebKit. It records two input changes, the authenticated service result, cancellation/stop/remount state, bounded semantic snapshots, PNG evidence and explicit unsupported native operations; 74/74 Python tests and bytecode compilation pass.
- Completed increment (2026-09-09, commit `8854ab4`): the runner accepts normal authorized responses without requiring a transient pending observation, keeps cancellation pending-sensitive, and rejects input, teardown, malformed and over-budget screenshot mutants; 78/78 Python tests and bytecode compilation pass.
- Completed increment (2026-09-11, local branch `feat/metis-browser-canvas-001`): `metis-web` exposes a format-neutral borrowed `CanvasFrame` seam and a WASM `CanvasSurface` over Moirai's bounded HTML5 canvas provider. The standalone lock now resolves Moirai to merged PR #321 (`ccdc878d`). RITK's `PresentationFrame` is the consumer adapter; no DICOM parser, metadata, geometry or viewer state enters Métis. Native tests and strict native/WASM clippy pass; the visual baseline provenance was refreshed with unchanged capture pixels. RITK PR #284 records a local synthetic browser canvas smoke through this seam; trusted physical input, cross-engine driver evidence, pointer/three-view/GPU paths and full-window capture remain open.
- Residuals: actual configured browser-driver traces, real cross-engine screenshots, provider-private listener/resource counts, TLS, accessibility/IME, post-drop allocation and native desktop evidence remain open; DICOM parsing and viewer semantics stay with RITK under [RITK-SNAP-DICOM-SUBSTRATE-001](../ritk/backlog.md#RITK-SNAP-DICOM-SUBSTRATE-001).
- Decision: [ADR 0002](docs/adr/0002-web-application-contract.md), [ADR 0008](docs/adr/0008-browser-host-boundary.md).

<a id="METIS-BROWSER-002"></a>
## METIS-BROWSER-002 — Browser stale-response runtime probe [patch]
- Status: done; priority: P1; owner: Metis browser host + verification; integrator: root; last-update: 2026-09-07; branch: `feat/process-foundation`; delivery: `a8cc67c`; dependencies: METIS-BROWSER-001, METIS-COMMANDS-001; risk: stale DOM mutation
- Scope: delay a real service response at the browser transport boundary, stop/remount the WASM host, and observe response disposal and DOM stability; cross-engine, TLS, native desktop and OS permissions remain separate.
- Acceptance: a bounded delayed response cannot change the stopped or remounted DOM; the browser task and WebSocket callbacks are released; the trace records the exact engine, revision, action sequence and observable state.
- Demonstration: [V02](docs/VERIFICATION.md#V02), [browser stale-response evidence](docs/VERIFICATION.md#browser-stale-response-evidence--2026-09-07) and the delayed-response section in the [browser manual](docs/manual/browser.md#verify-stopremount-disposal-at-the-service-boundary).
- Outcome: The single `metis-app` executable exposes a bounded `--response-delay-ms` service probe backed by Moirai's async timer. The live stop/remount trace leaves the remounted DOM at `Backend unavailable [ERR_TRANSPORT_BROKEN]` with no stale result or event after the deadline.

<a id="METIS-FRAGMENT-001"></a>
## METIS-FRAGMENT-001 — Authenticated typed browser actions [arch] [minor]
- Status: review; priority: P1; owner: Metis protocol/browser; integrator: root; last-update: 2026-09-10; dependencies: METIS-BROWSER-001, METIS-COMMANDS-001, METIS-AUTHORITY-001; risk: remote markup and stale lifecycle
- Scope: event→request→target→swap interaction inspired by htmx, with versioned authenticated actions and bounded typed patches; arbitrary markup, scripts, unrestricted selectors and navigation stay outside the contract.
- Acceptance: capability-bound action requests, allowlisted targets, bounded patch count/bytes and atomic application; invalid capability/target/patch/size and stale generation return typed errors without changing prior state; mount/unmount releases listeners and tasks exactly once.
- Demonstration: browser trace and inspected snapshots cover action success, rejected target/patch, stop/remount during an in-flight request and zero retained mounted controls.
- Decision: [ADR 0022](docs/adr/0022-typed-browser-actions.md).
- Completed increment: `FragmentAction`, `FragmentPatchSet`, the scoped `ui` plugin and the WASM target policy use the existing authenticated plugin envelope; all mutations are bounded text/attribute operations with generation checks and atomic preflight. No DICOM knowledge or dependency enters Metis; RITK remains the format and viewer owner.
- Evidence: native focused suites pass 29/29 (`metis-core`), 32/32 (`metis-frontend`/`metis-backend`) and 34/34 (`metis-web`); [typed browser action verification](docs/VERIFICATION.md#typed-browser-action-verification--2026-09-09) and the [browser manual](docs/manual/browser.md#hypermedia-boundary) record the protocol and demonstration.
- Reconciliation: the checksum-verified release-binary gate-tool workflow fix from stale `feat/framework-slices-001` is already in `main` through PR #41 (`e508b4c`); no duplicate patch or DICOM behavior change is required.
- Residuals: configured WebDriver success/rejection/stale-generation captures and provider-private listener/allocation counts remain open. The exact full gate reports matching pixels and semantics with the reviewed capture baseline refreshed, and the locked WASM library build passes.

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
- Status: in-progress; priority: P1; owner: Metis Windows host + Moirai; integrator: root; last-update: 2026-09-09; branch: `feat/process-foundation`; dependencies: METIS-AUTHORITY-001, METIS-COMMANDS-001; risk: trust boundary
- Lease: root — `docs/manual/native.md`, `docs/manual/applications.md`, `docs/VERIFICATION.md`, `docs/ARCHITECTURE.md`, capture manifest and supervisor policy tests — 2026-09-09T12:50:00-04:00
- Scope: Windows native window/system WebView, real events, multi-window lifecycle and OS-restricted renderer; macOS/Linux have separate items below.
- Acceptance: actual visible form, pointer/keyboard/resize/DPI/close/reopen; file/network/process denial probes; IPC remains functional under restrictions and all child processes drain.
- Demonstration: [V05](docs/VERIFICATION.md#V05), actual Windows window captures, keyboard journey and permission-denied results in the manual.
- Current evidence: Moirai PR #287 (`7ad8eeee`, following PRs #286 and #284) supplies the real Win32 HWND, bounded message translation including IME composition phases, retained ARGB presentation, finite queue waiting and retained-event readiness. `metis-platform::native::NativeSurface` is the safe consumer boundary; `PlatformSurface` and `PlatformEvent` remain portable application-supplied values.
- Follow-on provider: Moirai PR #299 merged to `main` at `f4eb4f2b` and its board item is done. It adds the thread-affine WebView2 host, packaged `file:///` navigation policy, bounded WebMessageReceived bridge and callback teardown. Its installed-runtime smoke passes on WebView2 `152.0.4191.66`, and the visible Metis bundle journey is captured in the linked manifest. No registry or signing key is part of that provider.
- Current increment: `metis-platform::native::WebViewSurface` consumes the merged provider through a safe adapter. It sizes the controller, maps validated visibility, forwards combined window/WebView events and preserves bounded navigation and JSON messaging. Metis uses git-plus-version Moirai requirements; Cargo.lock records provider revision `a58344b00ccc4a062c71659a463dd188d67bf1f4`. Configuration and ignored installed-runtime adapter tests pass on WebView2 `152.0.4191.66`, covering packaged navigation, the ready bridge message, external-navigation denial and close. The visible bundle journey is captured in [`docs/manual/images/native-captures.json`](docs/manual/images/native-captures.json).
- Current increment: `metis-app --metis-webview` now runs the same one-executable supervised process workflow through a visible WebView2 form. A bounded temporary HTML/CSS package uses a no-network CSP and typed JSON bridge; submit messages traverse the unprivileged frontend's private pipe and return the capability-authorized, audited result to the page. Revision `0c8bcc3` admits only the operating-system path allowlist required by WebView2 while keeping application variables and credentials isolated. Native and WebView2 initial/submit captures, trusted Enter/pointer actions and exact hashes are documented in the [native manual](docs/manual/native.md#captured-windows-workflows); permission, physical resize/DPI, native accessibility and installed-IME evidence remain open.
- Completed increments: the adapter test creates a real hidden HWND, presents the production framebuffer, observes resize and closes the window; the `metis-app --metis-native-window` role now composes that surface with the real frontend and supervised private IPC, handling text, transient IME preedit/commit/cancel, Enter/click submit, resize, DPI, focus and close; focused nextest and warning-denied Clippy pass on Windows. The adapter now rejects reopening a live surface and recreates a closed surface from its validated configuration; the Windows lifecycle regression test closes and reopens a real hidden HWND. A two-window regression test presents separate frames, verifies independent event batches and confirms closing one leaves the other live.
- Decision: [ADR 0015](docs/adr/0015-native-window-provider.md) (accepted); the committed visual capture is complete for the current native/WebView2 initial and submit journeys. Installed IME, WebView2 composition, OS permission enforcement, accessibility and macOS/Linux providers remain open follow-on slices.

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
- Status: review; priority: P3; owner: Metis delivery; integrator: root; last-update: 2026-09-10; branch: `codex/fix-metis-atlas-pins`; dependencies: METIS-DISTRIBUTION-001, METIS-CONFORMANCE-001; blocker: crates.io trusted-publisher registration and explicit release authority are external; re-open: registrations and release authority are available without adding repository secrets
- Scope: release-readiness metadata, package dry runs, final manual and platform evidence; registry authentication uses Atlas OIDC workflows without personal keys; preparation continues without release authority.
- Acceptance: dependency-closed packages and exact-revision evidence; release execution is blocked until explicit authority, registry publisher registration, rollout and rollback details are available.
- Current increment: all Atlas reusable-workflow callers and contract evidence now use `848e6649c52e8226a9abf7bc336f8cbf0e39ba08`; the committed full gate passes while tokenless OIDC release behavior is unchanged. Local crates.io publish validation is blocked by index network access, while registry registration and release authority remain open. The PyO3 package and PyPI caller are tracked by [METIS-PYTHON-001](#METIS-PYTHON-001); PyPI trusted-publisher registration remains an external release action.
- Completed increment (2026-09-08): the public manual records the exact crates.io and PyPI trusted-publisher fields, the `crates-io`/`pypi` environment boundary and the prohibition on registry secrets, passwords and signing keys. No private-key path is configured.
- Recheck (2026-09-09): the Metis remote is HTTPS, local Git has no signing or SSH-key configuration, and the checked-in callers still contain no registry secret path. GitHub API inspection found zero Actions secrets or variables and two empty environments, `crates-io` and `pypi`, without protection rules. Trusted-publisher registration and release authority remain external; no key or token is requested from the developer. The visible browser is signed out.
- Clarification (2026-09-09, `27853cc`): runtime IPC authentication uses an ephemeral symmetric session MAC key; it is never persisted or exposed to CI and is unrelated to registry or signing credentials.
- Completed increment (2026-09-08): verification now points its public workflow evidence at the current release-only OIDC caller revision `5cd8bde`.
- Demonstration: [V10](docs/VERIFICATION.md#V10), locally built package installation/recovery instructions and actual captures before any publication.

<a id="METIS-PYTHON-001"></a>
## METIS-PYTHON-001 — PyO3 application binding [arch] [minor]
- Status: done; priority: P1; delivery: `00f3681`; exact full gate passed 2026-09-08; owner/integrator: Metis Python integration/root.
- Outcome: `metis-python` exposes validated clinical Rust types through an abi3 `import metis` wheel, built-wheel value tests, typed stubs, manual workflow and a tokenless PyPI OIDC caller; visual baseline refreshed for the locked dependency graph.
- Decision: [ADR 0017](docs/adr/0017-python-binding.md); release registration remains external per [METIS-RELEASE-001](#METIS-RELEASE-001).

<a id="METIS-PYTHON-002"></a>
## METIS-PYTHON-002 — Rust-owned Python presentation surface [minor]
- Status: done; priority: P1; delivery: `1d77de9`, `3bf7697`; owner/integrator: Metis Python/presentation/root.
- Outcome: The abi3 wheel exposes Rust-owned `RasterImage`, `Rect` and bounded `Canvas` composition with exact RGBA, clipping, alpha and invalid-input tests; the manual and inspected fixture demonstrate the workflow.
- Decision: [ADR 0020](docs/adr/0020-python-presentation.md); native window/event lifecycle and DICOM decoding remain provider-owned follow-ons.

<a id="METIS-PYTHON-003"></a>
## METIS-PYTHON-003 — Rust-owned Python application lifecycle [arch] [minor]
- Status: done; priority: P1; owner: Metis Python integration; integrator: root; last-update: 2026-09-11; delivery: this branch; regions: `crates/metis-python`, `scripts/python_binding.py`, `crates/metis-python/tests`, `docs/manual/python.md`, `docs/adr/0023-python-application-lifecycle.md`; dependencies: METIS-PYTHON-002, METIS-STATE-001, METIS-INPUT-001; risk: cross-thread lifecycle and bounded state
- Scope: expose a cross-platform Rust-owned software application surface through PyO3 with bounded events, framebuffer extraction and close/reopen generations; native windows and browser hosts remain provider-owned.
- Acceptance: FIFO input events, bounded queue rejection, deterministic framebuffer output, close invalidation, generation-safe reopen and typed invalid-operation errors; concurrent calls are synchronized without Python callbacks or a second event loop.
- Demonstration: built-wheel pytest and the Python manual exercise input, render, close and reopen, including a concurrent free-threaded probe when the module audit permits it.
- Decision: [ADR 0023](docs/adr/0023-python-application-lifecycle.md).
- Verification: exact full gate on this branch passes compiler, metadata, supply-chain, format, visual, WASM, browser-assets, clippy, build, native-host, nextest, built-wheel, release, doctest, documentation, example, image, presentation and visual stages; the deliberate `capture-failure` negative stage exits 1 as required. The built wheel produced one `cp39-abi3-win_amd64` artifact and its suite passed 19 tests with the free-threaded probe skipped because no free-threaded interpreter is installed. FIFO input, bounded queue rejection, exact framebuffer bytes, close invalidation, generation-safe reopen and concurrent synchronized reads/mutation all pass. The exposed classes pass a `Send + Sync` audit and declare `gil_used = false`; hosted `cp3XXt` artifact evidence remains owned by [METIS-PYTHON-004](#METIS-PYTHON-004).

<a id="METIS-PYTHON-004"></a>
## METIS-PYTHON-004 — Free-threaded Python wheel matrix [arch] [minor]
- Status: review; priority: P1; owner: Metis delivery; integrator: root; dependencies: METIS-PYTHON-003, ATLAS-PUBLISH-001; risk: hosted CPython ABI coverage
- Scope: extend Atlas's reusable Python wheel workflow with version-specific `cp3XXt` and Python 3.15 `abi3t` builds, installation tests and `sys._is_gil_enabled()` assertions; keep Metis's `abi3` caller and tokenless OIDC publication.
- Acceptance: the shared workflow owns the matrix, Metis's release caller opts in without duplicating wheel logic, GIL and free-threaded wheels install and run the same value-semantic suite, and the manual/ADR record exact artifact support.
- Current increment: the release caller uses Atlas merge `848e6649c52e8226a9abf7bc336f8cbf0e39ba08` for the shared `cp314t`/`cp315t` and Python 3.15 `abi3t` matrix, while the default remains CPython 3.9 `abi3`. The caller and package use tokenless OIDC; musllinux is excluded from the `abi3t` job until a compatible 3.15t image exists. Hosted artifact and value-test evidence remain pending.
- Re-open trigger: a hosted free-threaded run or Atlas contract change invalidates the declared matrix.

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
- Outcome: Public source/manual and rendered captures are present; Atlas registers `repos/metis` at the verified public `main` revision. Every later item owns its demonstration section, not a deferred documentation phase.

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
- Completed increment (2026-09-08): commit `16e14afaf3ee8c993e840a06a539dfd3bb0ff5bc` makes the gesture policy and provider capture surface retain at most two distinct pointer identifiers; a second pointer establishes a bounded centroid/distance pinch baseline, a third is rejected, and release clears the baseline. Native tests and strict WASM checks cover the state transitions.
- Evidence: [browser pinch gesture evidence](docs/VERIFICATION.md#browser-pinch-gesture-evidence--2026-09-08) records the two-pointer policy, visual surface update and CUA physical-touch limitation.
- Completed increment: `feat(web): Add bounded browser file drops` consumes Moirai `DropMetadata` and `DroppedFile` from merged revision `630f914bcb34d4d65cc5e3db27a121163d040199`; Rust revalidates generic metadata, caps the retained batch at 64 entries and renders a semantic drop status without trusting paths.
- Superseded increment (before the format-neutral handoff): the browser host read a bounded prefix through a provider-owned handle and rendered `reading`/`complete`/`failed` states. `METIS-DICOM-002` removed the format classification and replaced that path with a generic byte handoff.
- Completed increment (2026-09-08): commit `eee0cd14bc8be102526b045ff46b4445a0314509` makes the browser host read accepted files to their declared ends through bounded asynchronous chunks, enforces a 64 MiB per-file and 256 MiB batch budget, and exposes named bytes through one `metis_web::take_file_drop` handoff slot. RITK receives the bytes before any format scan; stop, remount and a later drop release unconsumed bytes.
- Live evidence: [browser file-drop evidence](docs/VERIFICATION.md#browser-file-drop-evidence--2026-09-08) records the native policy suite, batch ownership tests, strict WASM checks and rendered drop-zone state; CUA cannot provide trusted OS file-drop evidence or a live byte-read trace.
- Completed increment (2026-09-08): `feat(web): Transfer dropped file ownership` adds `FileDropBatch::into_files` and `FileDropPayload::into_parts`; pointer-identity tests prove the RITK handoff moves the collection and byte allocations without a second file-content copy.
- Evidence: [browser file-drop evidence](docs/VERIFICATION.md#browser-file-drop-evidence--2026-09-08) records the ownership-consuming test and its limits; no DICOM decoder claim is made.
- Residuals: trusted physical file-drop evidence, full RITK DICOM opening/decoding, installed IME journeys, accessibility technology, cross-engine parity, post-drop allocation measurement and native-host visual/assistive evidence remain open; re-open this item when those dependencies land.

<a id="METIS-TEXT-001"></a>
## METIS-TEXT-001 — Text, selection and IME [minor]
- Status: in-progress; priority: P1; owner: Metis input/presentation; integrator: root; last-update: 2026-09-08; branch: `feat/process-foundation`; dependencies: METIS-INPUT-001; risk: text corruption
- Scope: DOM text first; grapheme selection, composition/preedit/commit/cancel, clipboard/undo, wrapping, fallback fonts, bidi and text scaling. Custom renderer requires its own admitted text contract.
- Acceptance: Unicode fixture strings/selection ranges and caret/line geometry match the contract; native IME exercised per OS, including CJK, combining marks, emoji and mixed-direction input.
- Demonstration: [V03](docs/VERIFICATION.md#V03), editing specimen with actual composition and committed captures, locale/font details and keyboard instructions.
- Completed increment: browser `TextState` keeps bounded Unicode values, UTF-16 selection coordinates, input metadata and composition start/update/commit/cancel transitions; Moirai provider revision `0862716265d657b8069d5a47fd1e77ae26ddd006` owns the DOM snapshots and listener lifetime.
- Evidence: [browser text and composition evidence](docs/VERIFICATION.md#browser-text-and-composition-evidence--2026-09-08) records 21/21 native policy tests, warning-denied native/WASM Clippy, WASM build and the semantic textarea/value-preview surface.
- Completed increment: native `TextComposition` phases from Moirai `7ad8eeee` are consumed by `metis-app`; preedit text is bounded and transient, commit uses the ordinary bounded patient-field transition, and cancellation/focus loss clears it. The focused Metis suite covers the value transition.
- Completed increment (2026-09-08): the browser text policy rejects UTF-16 offsets inside surrogate pairs before changing state, preserving scalar boundaries while retaining browser-native UTF-16 transport coordinates.
- Evidence: the focused `metis-web` suite covers a rejected split-surrogate selection and unchanged state; the manual and verification record the browser visual trace and its grapheme/IME limits.
- Residuals: grapheme-safe editing, bidi and line geometry, fallback-font metrics, clipboard/undo, an installed CJK or other native IME journey and assistive-technology acceptance remain open; CUA evidence is limited to HTML/WASM rendering and synthetic browser input.

<a id="METIS-A11Y-001"></a>
## METIS-A11Y-001 — Accessible application interaction [minor]
- Status: in-progress; priority: P1; owner: Metis host/UI; integrator: root; last-update: 2026-09-08; branch: `feat/process-foundation`; dependencies: METIS-INPUT-001; risk: inaccessible controls
- Scope: semantic DOM roles/names/states, focus/action mapping, announcements, reduced motion/high contrast/zoom; OS accessibility bridge for any custom UI path.
- Acceptance: semantic tree identity/actions and keyboard-only completion pass; actual supported screen readers traverse and operate the application; document platform limits instead of claiming certification from tree presence.
- Demonstration: [V03](docs/VERIFICATION.md#V03), readable focus/contrast/zoom captures plus semantic/action and assistive-technology evidence.
- Completed increment: `feat(web): Honor accessibility preferences` adds reduced-motion and forced-colors presentation rules, semantic focus-order assertions and synchronized manual/evidence text.
- Evidence: revision `6bbbd00` passes the full Metis gate and 51 Python tests; the CUA trace observes the document focus path and a visible focus outline on **Clinical note**.
- Completed increment (2026-09-08): `feat(web): Announce busy application state` marks the form, primary/result status and explorer table with atomic polite announcements and Rust-owned `aria-busy` transitions during pending or loading work.
- Evidence: [browser accessibility presentation evidence](docs/VERIFICATION.md#browser-accessibility-presentation-evidence--2026-09-08) records the static semantic contract and its one-engine runtime limits.
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
- Completed increment: software-renderer styles without layout or paint semantics (`justify-content`, `align-items`, `min-width`, `min-height`, `border-radius` and `font-weight`) now return `ERR_INVALID_CSS_STYLE`; programmatic DOMs receive the same validation during layout.
- Evidence: [ADR 0013](docs/adr/0013-strict-style-contract.md), style/layout tests, the migrated presentation fixture and the full Metis gate.
- Residuals: device scale `2` and platform fractional-scale cases remain open because the available viewport capability does not expose a device-scale override.

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
## METIS-ASSETS-001 — Images, vectors and media assets [major]
- Status: in-progress; priority: P1; owner: Metis asset/presentation + existing Atlas format providers; integrator: root; last-update: 2026-09-09; branch: `feat/process-foundation`; dependencies: METIS-BROWSER-001, METIS-AUTHORITY-001; risk: hostile content
- ADR: [0019](docs/adr/0019-raster-display-command.md); the public `DisplayCommand` enum addition is a major release change and has no version bump until release authority opens a release increment.
- Scope: bounded local asset loading, image/SVG presentation, font loading and browser audio/video controls; validate paths/origins, dimensions/decoding budgets and target permissions.
- Acceptance: malformed/truncated/oversized/traversal assets fail; declared colors/alpha/aspect ratio/orientation match fixtures; media error and teardown states release resources.
- Demonstration: [V06](docs/VERIFICATION.md#V06), actual asset gallery with source attribution and load/error states.
- Completed increment: local project artwork now includes a scriptless fixed-viewport SVG, a PNG alternate and a seven-resolution PNG-in-ICO asset; `metis.json` declares each format, the CLI validates SVG and ICO bytes before staging, and portable/MSI payloads retain the resources.
- Completed increment: `RasterImage` validates bounded row-major RGBA storage and `ImagePlacement` validates crops, clips off-screen destinations and composites nearest-neighbor pixels; the `image` example emits the inspected software-renderer artifact.
- Evidence: [ADR 0016](docs/adr/0016-theme-and-branding.md), [browser manual](docs/manual/browser.md), [distribution manual](docs/manual/distribution.md), focused `metis-cli` package tests and browser asset tests; runtime asset capture is recorded in [VERIFICATION](docs/VERIFICATION.md#browser-svg-asset-evidence).
- Evidence: `metis-ui-lang` image validation/compositing tests, `cargo run --locked --example image`, and [software image evidence](docs/VERIFICATION.md#software-raster-image-evidence--2026-09-09) cover pixel, alpha and clipping semantics.
- Residuals: browser/native image decode and orientation, font loading, media controls/error teardown, GPU vectors, and runtime shell rendering on a Windows install remain open under V06; the SVG admission and ICO packaging contracts are closed.

<a id="METIS-GRAPHICS-001"></a>
## METIS-GRAPHICS-001 — Custom graphics conformance [arch] [minor]
- Status: todo; priority: P2; owner: Metis custom renderer over Iris; dependencies: METIS-VISUAL-001, METIS-LAYOUT-001; risk: rendering/lifetime correctness
- Scope: admitted vector/image/transform/clip operations and accelerated display path where required by the custom-UI demonstrator; retain one rendering contract and verify Atlas GPU ownership before additions.
- Acceptance: geometry/color/alpha and device-loss/recreate tests; differential software/device output under justified raster bounds; measured profile justifies acceleration and accounts for memory cost.
- Demonstration: [V06](docs/VERIFICATION.md#V06); this path never gates the DOM/browser migration and cannot stand in for HTML5 compatibility.

<a id="METIS-DATA-001"></a>
## METIS-DATA-001 — Tables, lists and live data views [minor]
- Status: done; priority: P1; owner: Metis component/state; integrator: root; last-update: 2026-09-08
- Outcome: bounded typed explorer with exact ordering/filtering, stable selection, disclosure and browser paging; commits `56aae2b`, `f93a556`, `e34830f`; [ADR 0018](docs/adr/0018-result-explorer.md), [V07 evidence](docs/VERIFICATION.md#result-explorer-evidence--2026-09-08).

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

<a id="METIS-AXUM-001"></a>
## METIS-AXUM-001 — First-party bounded HTTP boundary [arch] [minor]
- Status: done; priority: P2; owner: Moirai service + Metis policy; integrator: root; last-update: 2026-09-10; delivery: `bcd3a0c`; dependencies: METIS-SERVICES-001, METIS-AUTHORITY-001, METIS-FRAGMENT-001; risk: server trust boundary
- Named target: the local `metis-app` server demonstration used by the user manual. It is a loopback deployment target for typed browser fragments, not a public network listener.
- Scope: typed routes, bounded extraction, origin/session authorization, deadlines, cancellation, backpressure and text/attribute fragment responses; no Axum dependency, arbitrary markup, scripts or DICOM behavior.
- Acceptance: the admitted target has a real local server integration suite covering route isolation, malformed/oversized bodies, unauthorized origin/session, timeout/cancel, bounded response size, client disconnect and orderly teardown; browser and native captures tie results to one revision.
- Demonstration: [V08](docs/VERIFICATION.md#V08) server journey and [browser manual](docs/manual/browser.md#hypermedia-boundary) capture; RITK remains the DICOM and viewer owner.
- Outcome: the Moirai loopback target asserts missing-session and method denial, session capacity, response-byte limits, malformed/oversized input, origin policy, idle deadlines, disconnect handling and finite teardown; the 47/47 focused native suite, warning-denied Clippy, 84/84 Python suite and updated verification record pass.
- Residual: public deployment, TLS, cross-engine traces and the broader scoped-services item remain outside this finite loopback target; the item reopens only when a named Atlas application requires server rendering or fragment delivery.
- Decision: [ADR 0025](docs/adr/0025-axum-server-boundary.md). Re-open when a named Atlas application requires HTTP server rendering or fragment delivery.

<a id="METIS-MIGRATION-001"></a>
## METIS-MIGRATION-001 — egui and Tauri application migration [arch] [minor]
- Status: in-progress; priority: P1; owner: Metis framework + application owners; integrator: root; last-update: 2026-09-10; risk: lost application behavior.
- Dependencies: METIS-COMMANDS-001, METIS-FILES-001, METIS-INPUT-001, METIS-ASSETS-001, METIS-GRAPHICS-001, METIS-DESKTOP-001, METIS-BROWSER-001, METIS-INTEGRATION-001, METIS-SERVICES-001.
- Named driver: [ritk-snap](../ritk/backlog.md#RITK-SNAP-METIS-001), retaining egui/eframe at RITK while exposing `--metis-native` through merged [PR 269](https://github.com/ryancinsight/ritk/pull/269) (`c1b8130bb`), following [PR 267](https://github.com/ryancinsight/ritk/pull/267) (`2f2058062`). No Tauri dependency is present in its manifest or workspace lock. RITK owns decoder, volume geometry and medical display correctness; Metis supplies the replacement shell.
- Baseline: RITK `8152f483` ([PR 237](https://github.com/ryancinsight/ritk/pull/237)) adds physical display/hit rectangles to selected-study loading, restore/rejection and the original native capture; [manual](docs/manual/applications.md#dicom-viewer-migration-baseline). The merged RITK native MPR increment now proves one bounded Métis framebuffer containing all three spacing-aware planes and panel-specific routing while keeping DICOM semantics in RITK.
- Progress: the Windows `ritk-snap PATH --metis-native` workflow opens the synthetic study, renders axial/coronal/sagittal panels, routes wheel navigation by panel, captures a deterministic content golden and rejects a missing study. Browser input integration, multiframe/color host capture, matched memory measurements, full app-window overlays and packaging remain open acceptance work.
- Scope: inventory the actual viewer and a distinct pinned Tauri fixture; native Metis implementations replace required UI/state/input/render/file/lifecycle surfaces. First viewer journey opens a local DICOM study, selects its series and displays all three orthogonal views; full cutover retains the whole admitted viewer inventory.
- Acceptance: [V09](docs/VERIFICATION.md#V09) plus RITK opening/frames/color/grayscale prerequisites; required symbols/config/plugins and viewer actions are mapped/tested. Existing bugs cannot serve as parity oracles. No retained egui/eframe/Tauri runtime or forwarding shim in the completed migrated viewer.
- Demonstration: actual same-study before/after workflows, verified voxels/physical coordinates and real host captures in the user manual; record JavaScript retained versus Rust/WASM replacement and matched memory evidence.

<a id="METIS-RITK-HOST-001"></a>
## METIS-RITK-HOST-001 — Format-neutral native frame host [arch] [minor]
- Status: done; priority: P1; owner: Metis platform + RITK viewer; integrator: root; delivery: `ecc00a52514ecca4dba439f2626f91a8a17cb07d`; decision: [ADR 0024](docs/adr/0024-native-application-host.md).
- Outcome: `run_native_application` now hosts the existing Metis form and a real hidden Moirai surface; the committed `native_host_capture` example generates and round-trips the trace and BMP/SVG frame, while DICOM parsing, geometry and medical display remain exclusively in [RITK-SNAP-METIS-001](../ritk/backlog.md#RITK-SNAP-METIS-001).
- Evidence: full `python scripts/verify.py` gate passed at `ecc00a5` (173 packages; native-host-capture and visual stages passed), focused 37/37 nextest plus warning-denied Clippy, format, docs and doctests; residual viewer adapter, browser, cross-platform, accessibility and permission work remains in owning items.

<a id="METIS-DICOM-001"></a>
## METIS-DICOM-001 — RITK-backed DICOM open boundary [arch] [minor]
- Status: done; outcome: removed the duplicate GUI-side DICOM crate and retained RITK as the scanner, loader, geometry, and visual-workflow owner; [RITK-SNAP-DICOM-SUBSTRATE-001](../ritk/backlog.md#RITK-SNAP-DICOM-SUBSTRATE-001).
- Evidence: the Metis browser handoff remains bounded and parser-free; the RITK manual and locked workflow cover byte/file opening, rejection, geometry, and captures. Decision: [RITK ADR 0026](../ritk/docs/adr/0026-viewer-presentation-migration.md).

<a id="METIS-DICOM-002"></a>
## METIS-DICOM-002 — Keep the browser handoff format-neutral [patch]
- Status: done; priority: P1; delivery: [PR #32](https://github.com/ryancinsight/metis/pull/32), merge `10e7966`.
- Outcome: `metis-web` retains bounded metadata and named-byte ownership without DICOM candidate or Part 10 classification; the verification gate rejects a direct DICOM package in the Metis workspace, and RITK remains the scanner, decoder, geometry, and visual-workflow owner.
- Evidence: `49d2c98` plus `00ed7b9`; 31/31 native tests, WASM build, the format-neutral positive/negative guard tests and full `python scripts/verify.py` gate passed before merge; [RITK DICOM workflow](../../ritk/docs/manual/dicom-workflow.md) remains authoritative.

<a id="METIS-DICOM-003"></a>
## METIS-DICOM-003 — Remove format-specific browser presentation [patch]
- Status: done; priority: P1; owner: Metis browser presentation; integrator: root; last-update: 2026-09-09; delivery: `c4bcd74`; dependencies: METIS-DICOM-002; risk: ownership drift
- Outcome: browser labels, package README and fixtures describe generic bounded file handoff; format-specific parsing remains in RITK. Static contracts and the full locked gate pass on `c4bcd74`.

<a id="METIS-DICOM-004"></a>
## METIS-DICOM-004 — Remove stale DICOM claims from Metis docs [patch]
- Status: done; commit: `25014a6`; PR: <https://github.com/ryancinsight/metis/pull/40>; last-update: 2026-09-09.
- Outcome: Active Metis artifacts now describe only generic bounded metadata/bytes; superseded captures are labeled, and RITK remains the DICOM owner. Full locked gate passes.

<a id="METIS-DISTRIBUTION-001"></a>
## METIS-DISTRIBUTION-001 — Executables and Windows MSI [arch] [minor]
- Status: done; delivery: `feat(distribution): Build executables and MSI`; decision: [ADR 0005](docs/adr/0005-application-distribution.md).
- Outcome: one manifest, exact Cargo inventory, portable bundle, per-user MSI and manual; 102 debug/release tests, 36 Python tests, visual gate and real install/run/uninstall preserving user files pass.

<a id="METIS-DISTRIBUTION-002"></a>
## METIS-DISTRIBUTION-002 — Developer application lifecycle [minor]
- Status: done; commit: `33e5ddb`; PR: <https://github.com/ryancinsight/metis/pull/39>; last-update: 2026-09-09.
- Outcome: Locked `init`/`dev`/watch/completions lifecycle with bounded native reload and no stale executable; [V10](docs/VERIFICATION.md#V10) records the full gate and smoke evidence. DICOM remains owned by RITK.

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
- Status: done; priority: P0; owner: Metis verification/integration; integrator: root; last-update: 2026-09-09; delivery: hosted run 34293187709
- Dependencies: METIS-VISUAL-001; risk: unreliable gates
- Scope: one pinned gate/CI definition for implemented targets, plan ID/dependency/link validation, artifact provenance, advisory/dependency/native-link audits, public-API checks, parser fuzz/property/mutation and structured redacted diagnostics; host jobs arrive with working host implementations.
- Acceptance: current [V01](docs/VERIFICATION.md#V01), native and WASM-build suites run under committed budgets; deliberate state/image/parser regressions fail the correct gate; no nonexistent host job reports green.
- Demonstration: manual troubleshooting shows actual gate failure and recovery artifacts. This finite infrastructure item does not close later scenario coverage.
- Completed increment (2026-09-07): pinned workflow and local gate validate plan identifiers, dependencies and local links before artifact-producing stages; cargo-deny audits the locked graph and the gate records reviewed Cargo build-link contracts; 47 Python checks pass.
- Completed increment (2026-09-08, commit `866822016d8ee02b2b38149efee2e26783c77bb2`): bounded arbitrary-byte, Unicode, IEEE-754, truncation, bit-mutation and oversized-length properties cover every public wire decoder; the standalone `fuzz/` LibFuzzer target passes a locked manifest check; `MetisError` and remote error payload `Debug` output redact untrusted message text. Focused nextest passes 96/96 with strict Clippy. The Windows MSVC host cannot link the LibFuzzer sanitizer runtime, so no runtime fuzz result is claimed.
- Completed increment (2026-09-08): `scripts/mutation.py` pins cargo-mutants 27.1.0, nextest, the shared target and finite budgets for the decoder slice; at revision `6af70734965ceb5eb94dd4ce1d663a50e4f532ab` it generated 14 mutants, caught all 6 viable mutants, and reported 0 missed, 0 timed-out and 8 unviable. The derived report is `output/mutation/latest/manifest.json`.
- Completed increment (2026-09-08): the single verification workflow adds a scheduled and manually dispatchable Ubuntu LibFuzzer campaign with pinned nightly, locked-manifest verification, bounded time/RSS/input limits and crash-artifact upload; the local full gate passes with the existing Windows sanitizer limitation recorded.
- Outcome: hosted Ubuntu LibFuzzer job passed at source revision `0998e63748faa2c76f963574353374658329e20` in [run 34293187709](https://github.com/ryancinsight/metis/actions/runs/34293187709); the Windows sanitizer limitation remains explicitly covered by that cross-target job.

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
## METIS-REGISTRATION-001 — Public Atlas member registration [patch] — done
- Status: done; priority: P0; delivery: Atlas `8380a789e`; recursive registration and coherence evidence passed 2026-09-09.
- Outcome: Atlas main `e038deb75` pins `repos/metis` to merged public `main` `2e21146c`; the fresh long-path checkout initialized all 28 registered members. No runtime or signing capability depends on this metadata change.

<a id="METIS-APPLICATION-001"></a>
## METIS-APPLICATION-001 — Single executable application [arch] [major]
- Status: done; outcome: one relocated application image runs distinct process roles through Moirai; 108 debug/release tests, 38 Python checks, real single-executable MSI install/run/uninstall and unchanged visual snapshots pass. [Design and migration](docs/adr/0006-application-entry.md).
