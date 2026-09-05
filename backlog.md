# Metis delivery

Registration: [Atlas member item](../../backlog.md#metis-unregistered-member).
The user authorizes a public repository; registration follows verified publication.

<a id="METIS-WEB-001"></a>
## METIS-WEB-001 — Web application target contract [arch] [patch]
- Status: review; integrator: root; last-update: 2026-09-05
- Scope: supersede the no-WebView constraint, define Tauri migration/browser boundaries, and gate portable libraries on WASM; no browser-runtime support claim.
- Acceptance: README/manual/architecture agree; ADR 0002 defines compatibility and security/memory oracles; portable target compiles and existing native gate passes.
- Decision: [ADR 0002](docs/adr/0002-web-application-contract.md), number claimed by this item.
- Evidence: WASM library build, 82 debug tests, 82 release tests, 10 doctests and full standalone gate pass; independent contract/gate review finds no actionable issue.

<a id="METIS-BROWSER-001"></a>
## METIS-BROWSER-001 — Browser form and command lifecycle [arch] [minor]
- Status: todo; dependencies: METIS-WEB-001; risk: browser/native trust boundary
- Scope: HTML5/CSS DOM form, Rust/WASM state and asynchronous bounded request correlation; reusable browser scheduling/transport belongs upstream in Moirai.
- Acceptance: real browser inputs change displayed results; malformed/unauthorized commands fail; cancellation, timeout and teardown leave no pending requests/listeners; actual browser snapshot enters the manual.
- Constraint: no native secrets or authority in downloaded WASM; private-pipe possession cannot authenticate browser requests. Desktop bridge or service boundary must enforce origin/session authorization.
- Evidence: existing `IpcTransport` blocks on receipt; Moirai's current `WebReactor::poll_events` returns no events. Neither establishes browser support.
- Decision: [ADR 0002](docs/adr/0002-web-application-contract.md).

<a id="METIS-SEC-001"></a>
## METIS-SEC-001 — Backend authority [arch] [patch]
- Status: review; integrator: root; last-update: 2026-09-05
- Scope: backend-only calculation/audit, validated configuration, session authority, real time and OS entropy.
- Acceptance: malformed/expired/cross-session requests fail; numerical boundary tests pass; frontend dependency closure excludes clinical/audit modules.
- Decision: [ADR 0001](docs/adr/0001-process-contract.md).

<a id="METIS-IPC-001"></a>
## METIS-IPC-001 — Canonical bounded IPC [patch]
- Status: review; integrator: root; last-update: 2026-09-05
- Scope: framing, canonical payloads, correlation/replay, bounded memory transport, wire fault injection.
- Acceptance: exact bytes and typed errors for truncation, corruption, replay, oversize and malformed payloads.

<a id="METIS-UI-001"></a>
## METIS-UI-001 — Bounded presentation [patch]
- Status: review; integrator: root; last-update: 2026-09-05
- Scope: markup/style parsing, layout, software rasterizer, bounded surface/event allocations.
- Acceptance: EOF/depth/Unicode/overflow cases terminate with errors; supported forms still render from input.

<a id="METIS-PROCESS-001"></a>
## METIS-PROCESS-001 — Real executable workflow [arch] [minor]
- Status: review; integrator: root; last-update: 2026-09-05
- Scope: connect backend/frontend binaries through inherited pipes; bounded supervision and process tests.
- Acceptance: separate PIDs, input-sensitive request/result exchange, failure propagation and finite shutdown.
- Decision: [ADR 0001](docs/adr/0001-process-contract.md).

<a id="METIS-DESKTOP-001"></a>
## METIS-DESKTOP-001 — Native restricted desktop [arch] [minor]
- Status: todo; last-update: 2026-09-05; dependencies: METIS-PROCESS-001, METIS-WEB-001; risk: trust boundary
- Scope: Windows/macOS/Linux native windows, system WebView hosting for existing web frontends, OS-enforced frontend sandbox and real input event loop.
- Acceptance: native visible form; file/network/process denial probes; IPC works under restrictions on each OS.
- Current evidence: `PlatformSurface` owns framebuffer/events only; `PlatformEvent` has no OS event producer. The ineffective original privilege assertion is removed. Native lifecycle, input dispatch and permission denial require new provider contracts and platform probes.

<a id="METIS-AUDIT-001"></a>
## METIS-AUDIT-001 — Durable audit recovery [minor]
- Status: todo; dependencies: METIS-SEC-001; risk: persistence
- Scope: versioned durable backend audit, bounded storage, restart recovery and trusted checkpoint.
- Acceptance: crash/truncation/tamper/disk-full tests with replayable fixtures and fail-closed outcomes.

<a id="METIS-VERIFY-001"></a>
## METIS-VERIFY-001 — Verify and deliver foundation [patch]
- Status: todo; dependencies: METIS-SEC-001, METIS-IPC-001, METIS-UI-001, METIS-PROCESS-001
- Scope: source documentation, warning-clean gates, process evidence and Git delivery.
- Acceptance: fmt/clippy/nextest/doc pass; Atlas-only direct dependencies and enumerated provider transitive graph; evidence states host coverage and residual risks.

<a id="METIS-PROVIDER-001"></a>
## METIS-PROVIDER-001 — Atlas provider adoption [arch] [patch]
- Status: review; integrator: root; last-update: 2026-09-05
- Scope: Moirai scheduler/process transport and Iris rendering contract; enumerate provider transitive graph.
- Acceptance: no parallel Metis runtime; local and standalone provider sources coherent; contract tests pass.
- Quarantine: use pushed Moirai `0514f11` with reviewed process support until the 0.6 API lands on main. A later wrong-repository dependency reference is corrected upstream in `69763f7`; remove the snapshot pin after main exposes the API and the consumer gate passes.
- Dependencies: upstream process API and Atlas overlay mixed-version correction.

<a id="METIS-CRYPTO-001"></a>
## METIS-CRYPTO-001 — Shared authentication primitives [arch] [patch]
- Status: todo; dependencies: METIS-PROVIDER-001; risk: authentication
- Scope: extract the required standalone MAC/hash contract into its Atlas provider and remove the Metis seed copy.
- Acceptance: independent vectors and canonical wire/audit tests agree; no TLS dependency needed solely for hashing.

<a id="METIS-RELEASE-001"></a>
## METIS-RELEASE-001 — Publication readiness [patch]
- Status: blocked; blocker: explicit registry release authority; re-open: user authorizes a release.
- Scope: registry metadata, user manual with application snapshots, advisory scan, CI and platform verification.
- Acceptance: package contents and required verification pass before any registry release.

<a id="METIS-MEMORY-001"></a>
## METIS-MEMORY-001 — Provider allocation count [patch]
- Status: todo; risk: overflow; scope: Moirai's allocation boundary.
- Evidence: `../moirai/moirai-core/src/memory/allocator.rs` multiplies element size by count without a checked operation.
- Acceptance: verify the public count contract and reject overflow with a typed error before allocation; debug/release adversarial tests.

<a id="METIS-MANUAL-001"></a>
## METIS-MANUAL-001 — Public member and user manual [patch]
- Status: in-progress; integrator: root; last-update: 2026-09-05
- Scope: public GitHub repository, Atlas gitlink, user-oriented manual and actual rendered application snapshots; no registry release.
- Acceptance: public remote contains tested source; Atlas resolves the pinned commit; manual links resolve and generated snapshot matches the renderer.
- Decision: [ADR 0001](docs/adr/0001-process-contract.md); user manual replaces the domain-book requirement by explicit user direction.
