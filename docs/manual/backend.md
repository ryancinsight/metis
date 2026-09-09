# Connect a backend

The [application entry](../../crates/metis-app/src/main.rs) runs the backend role
by default. That role owns calculation policy, a fresh ephemeral symmetric
session MAC key and its audit ledger. It launches the same executable in its
presentation role through
Moirai, transfers only the intended standard streams, and services the resulting
`StreamTransport` until completion or failure.

The child role creates its `FrontendApp` from the
[frontend library](../../crates/metis-frontend/src/lib.rs), acquires a session
token, submits form values and reports the
response. Moirai supplies the blocking worker and process lifecycle. Metis owns
the request protocol and application deadline; it does not create another runtime.

## Request lifecycle

1. The frontend handshakes over the private transport and receives a scoped token.
2. Each subsequent request carries an increasing sequence number. Responses must
   match both the expected response family and sequence.
3. The backend validates the exact issued token, configured `HostPolicy`
   origin/window/session binding, scope, wall-clock claims and monotonic session
   lifetime before calculating.
4. Calculation and failure outcomes enter a bounded backend audit ring. The
   frontend displays the response or an explicit error.

`HandshakeError` distinguishes local transport/decoding failures from remote
rejections, including remote codes unknown to the client. Neither case grants an
active token. The [wire contract](../INTERFACE.md) defines frames and payloads.

## Boundaries to preserve

Do not send the backend session MAC key to the frontend. The response MAC is
symmetric; the frontend does not possess a verification mechanism and must not
claim to verify it. This runtime-only key is generated from OS entropy, is never
persisted or exposed to CI, and is unrelated to registry or signing credentials.
Claimed process identifiers are metadata, not operating-system identity proof.
The default contained service policy admits only `metis://native` in window 1;
browser and desktop hosts must pass their own trusted observed context.

The internal `--metis-frontend` argument selects the child role; it is not a
credential. Child dispatch occurs before key generation and backend construction.
Both processes load the same executable image, so library separation does not
mean backend machine code is absent from the child. Private process state and
pipe transfer remain the relevant boundaries; the role argument supplies no
permission restriction.

Windows job containment bounds descendant lifetimes. It does not deny filesystem,
network or device access. The audit ring holds 1,024 records with a retained chain
checkpoint; it is not durable across restart or independently anchored against a
compromised backend. See [risk controls](../RISK_CONTROLS.md) before extending a
trust boundary.
