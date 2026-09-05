# Metis delivery

Registration: [Atlas member item](../../backlog.md#metis-unregistered-member).
Only repository visibility is awaiting user input; local implementation proceeds.

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
- Status: in-progress; integrator: root; last-update: 2026-09-05; dependencies: METIS-PROCESS-001; risk: trust boundary
- Scope: Windows/macOS/Linux native windows, OS-enforced frontend sandbox and real input event loop.
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
- Status: blocked; blocker: repository visibility decision and explicit release authority; re-open: user supplies both.
- Scope: registry metadata, license files, book, advisory scan, CI, standalone dependency lock and platform verification.
- Acceptance: package contents and required verification pass before any registry release.

<a id="METIS-MEMORY-001"></a>
## METIS-MEMORY-001 — Provider allocation count [patch]
- Status: todo; risk: overflow; scope: Moirai's allocation boundary.
- Evidence: `../moirai/moirai-core/src/memory/allocator.rs` multiplies element size by count without a checked operation.
- Acceptance: verify the public count contract and reject overflow with a typed error before allocation; debug/release adversarial tests.
