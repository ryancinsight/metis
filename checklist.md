# Metis native QUIC checklist

Active target: Metis 0.5.0 over Moirai 0.9.0/moirai-quic 0.1.0. Phase: Closure.

- [x] [minor] Specify stream/session trust and ownership in ADR 0058.
- [x] [minor] Consume provider QUIC without importing Quinn; retain one frame decoder and asynchronous server dispatcher.
- [x] [minor] Require native QUIC without a feature flag; 176 default tests, 113 minimal IPC/backend tests, the default-built IPv4/IPv6 mutual TLS CLI and warning-free browser libraries pass. CI verifies minimal native sessions.
- [x] [minor] Preserve principal, fresh-key capability, replay, clinical, audit and event contracts across independent streams.
- [x] [patch] Verify 176 backend/IPC/core tests, including existing WebSocket behavior and actual malformed QUIC frames.
- [x] [patch] Check the full native workspace/all targets against the canonical published Git provider without path patches.
- [x] [major] Document the pre-1.0 generic server/Send migration in CHANGELOG.md; remove the old concrete transport coupling.
- [x] [patch] Execute the two-process mutual TLS CLI over IPv4 and IPv6; verify the exact calculation, audited event and acknowledged/drained shutdown. Warning-denying diagnostics and formatting pass.
- [x] [minor] Pin the published provider and verify the canonical Git lock while retaining unrelated dependency revisions. All 588 applicable default-API compatibility checks pass under the stronger minor override; the generic/Send migration remains documented as a pre-1.0 break.
- [x] [patch] Reconcile current main, preserving Consus/provider coherence, lossless audit identifiers, Windows page work and historical artifacts.
- [x] [patch] Verify the reconciled canonical tree: 703 native tests, strict transport diagnostics, full native workspace compilation and synchronized main/fuzz locks.
- [x] [patch] Preserve direct-provider ownership: real concurrent handshakes use the existing Moirai executor; canonical metadata and all 20 verifier tests pass.
- [x] [patch] Verify native sessions on Linux and Windows at published revision ed72092: both 176-test default suites and 113-test minimal suites pass with strict diagnostics.
- [x] [patch] Complete the full hosted Windows/browser consumer gate at ed72092; the unchanged Firefox retry passes after one startup timeout. Record the provider's 108-pair measurement and sustained-loss limit.
- [ ] [minor] Complete protected provider/consumer promotion before release.

Next increment: complete required review/checks for [provider PR 618](https://github.com/ryancinsight/Moirai/pull/618) and [consumer PR 503](https://github.com/ryancinsight/metis/pull/503) before protected release promotion.
