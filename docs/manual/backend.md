# Connect a backend

The [backend executable](../../crates/metis-backend/src/main.rs) owns calculation
policy, a fresh OS-generated key and its audit ledger. It launches the sibling
frontend through Moirai, transfers only the intended standard streams, and
services the resulting `StreamTransport` until completion or failure.

The [frontend executable](../../crates/metis-frontend/src/main.rs) creates its
`FrontendApp`, acquires a session token, submits form values and reports the
response. Moirai supplies the blocking worker and process lifecycle. Metis owns
the request protocol and application deadline; it does not create another runtime.

## Request lifecycle

1. The frontend handshakes over the private transport and receives a scoped token.
2. Each subsequent request carries an increasing sequence number. Responses must
   match both the expected response family and sequence.
3. The backend validates the exact issued token, scope, wall-clock claims and
   monotonic session lifetime before calculating.
4. Calculation and failure outcomes enter a bounded backend audit ring. The
   frontend displays the response or an explicit error.

`HandshakeError` distinguishes local transport/decoding failures from remote
rejections, including remote codes unknown to the client. Neither case grants an
active token. The [wire contract](../INTERFACE.md) defines frames and payloads.

## Boundaries to preserve

Do not send the backend key to the frontend. The response MAC is symmetric; the
frontend does not possess a verification mechanism and must not claim to verify
it. Claimed process identifiers are metadata, not operating-system identity proof.

Windows job containment bounds descendant lifetimes. It does not deny filesystem,
network or device access. The audit ring holds 1,024 records with a retained chain
checkpoint; it is not durable across restart or independently anchored against a
compromised backend. See [risk controls](../RISK_CONTROLS.md) before extending a
trust boundary.
