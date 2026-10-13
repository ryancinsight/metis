# ADR 0058: Native remote Metis sessions over QUIC

Date: 2026-10-13
Status: Accepted

Decision scope: the user-requested native transport increment.
Change class: [minor]. Target: Metis 0.5.0.

Native-build contract: `metis-ipc` and `metis-backend` require the QUIC provider
without a Cargo feature. Their default and `--no-default-features` native
builds expose the same authenticated remote-session API. The runnable session
requires no feature argument. TLS trust, client authorization, fresh session
keys and finite transport budgets remain admission requirements in either
build. Browser builds retain their platform-owned transports.

## Intent and selection

Native remote hosts use Moirai's required QUIC provider for independent ordered
sessions over one authenticated UDP connection. Each bidirectional stream owns
one session's handler, replay sequence and capability token. Application commands
remain reliable and ordered within that stream. The remote transport does not
replace private process pipes or browser HTTP/WebSocket transports.

Moirai owns TLS, QUIC, sockets, scheduling, deadlines and bounded wire messages.
Metis consumes the existing frame decoder and request dispatcher; the binary
protocol, clinical computations, capability validation and audit remain canonical.
The adapter does not import Quinn or introduce application retransmission.

## Formal session contract

Given a completed TLS 1.3 handshake negotiating `metis-ipc/1`, a caller supplies
an independently established trusted HostContext for each accepted stream.
Client-authentication/authorization is caller policy: server certificate trust
alone does not identify a remote client. Missing trusted context is rejected
before receiving application bytes. The service never derives a principal or
privilege from an unauthenticated network field.

Exactly one complete Metis frame occupies each bounded Moirai message. For
header H and payload P, received length equals `HEADER_SIZE + H.payload_len`,
`H.payload_len <= MAX_PAYLOAD_SIZE`, and CRC(P) equals H.payload_crc32. Existing
strictly increasing request/event identifiers remain unchanged. A different
stream cannot inherit another session's token or sequence state.

Session reads and writes use finite deadlines. Cancellation invalidates the
partial stream framing; no uncertain command is retried. A rejected/closed stream
does not close sibling sessions. Commands never use lossy datagrams or 0-RTT.
Transport selection remains explicit; browser HTTP/3 and automatic TCP downgrade
are separate contracts, not implied by native QUIC availability.

## Alternatives and verification

One stream for every request would reorder a shared service's replay sequence
and require a new cross-stream authorization/correlation protocol. One stream
per session preserves the existing ordered contract and permits independently
authorized sessions to progress concurrently. HTTP/2 retains TCP loss coupling;
HTTP/3 adds unused HTTP semantics to the native binary protocol.

Tests use real UDP/TLS endpoints and BackendService: handshake, input-sensitive
clinical results, target capability, principal/capability rejection, replay,
truncated/oversized frames, deadline cancellation and sibling stream progress.
The runnable native example consumes caller-provided certificates/trust roots;
public fixture keys are used only in tests. No keys or trust policy are installed.

Provider decision: [Moirai ADR 0075](../../../moirai/docs/adr/0075-quic-multiplexed-sessions.md).

## Typed session closure

The shared native session seam identifies a validated WebSocket close through
`WebSocketPeerClosed`, preserving its status/reason and keeping truncated input
as an error. Close validation returns borrowed fields; only a received peer
close allocates its owned diagnostic cause. No diagnostic string establishes
protocol state.
