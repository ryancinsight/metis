# Metis native QUIC checklist

Active target: Metis 0.5.0 over Moirai 0.9.0/moirai-quic 0.1.0. Phase: Closure.

- [x] [minor] Specify stream/session trust and ownership in ADR 0058.
- [x] [minor] Consume provider QUIC without importing Quinn; retain one frame decoder and asynchronous server dispatcher.
- [x] [minor] Preserve principal, fresh-key capability, replay, clinical, audit and event contracts across independent streams.
- [x] [patch] Verify 176 backend/IPC/core tests, including existing WebSocket behavior and actual malformed QUIC frames.
- [x] [patch] Check the full native workspace/all targets with QUIC enabled against the canonical published Git provider without path patches.
- [x] [major] Document the pre-1.0 generic server/Send migration in CHANGELOG.md; remove the old concrete transport coupling.
- [x] [patch] Execute the two-process mutual TLS CLI over IPv4 and IPv6; verify the exact calculation, audited event and acknowledged/drained shutdown. Warning-denying diagnostics and formatting pass.
- [x] [minor] Pin the published provider and verify the canonical Git lock while retaining unrelated dependency revisions. All 588 applicable default-API compatibility checks pass under the stronger minor override; the generic/Send migration remains documented as a pre-1.0 break.
- [ ] [minor] Review/promote provider and consumer changes and collect Windows/Linux native transport evidence before release.

Next increment: review [provider PR 618](https://github.com/ryancinsight/Moirai/pull/618) and [consumer PR 503](https://github.com/ryancinsight/metis/pull/503), then collect Windows/Linux native transport evidence before release.
