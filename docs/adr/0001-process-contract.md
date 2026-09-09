# 0001 — Process contract and assurance boundary

Status: Accepted

Revision 2026-09-05: the user specifies a public Atlas member and a user manual
instead of a domain book. [METIS-MANUAL-001](../../backlog.md#METIS-MANUAL-001)
owns runnable application guidance and snapshots from actual framebuffer output.
The manual lives under `docs/manual/`; Rustdoc retains API contracts and ADRs
retain decisions. Snapshot generation reuses the presentation example and a
regenerate/compare gate; no independently drawn mockup stands in for an app.

Revision 2026-09-06: [METIS-APPLICATION-001](../../backlog.md#METIS-APPLICATION-001)
replaces sibling executables with one application image serving separate process
roles. [ADR 0006](0006-application-entry.md) owns the entry and migration; private
transport, session authorization and lifecycle containment remain unchanged.

Date: 2026-09-05

Driver: [METIS-SEC-001](../../backlog.md#METIS-SEC-001), [METIS-PROCESS-001](../../backlog.md#METIS-PROCESS-001).

## Decision

Retain the unpublished binary contract and replace unsafe scaffolding in place.
Shared core owns wire vocabulary and error classification. Backend owns domain
calculations, session authorization and audit storage. Frontend owns presentation
and input submission. No frontend dependency may expose backend clinical logic.
Workspace members live under `crates/`, matching current Atlas organization.

The application launcher connects separate instances of its own executable
through inherited anonymous pipes. The frontend library still has no backend
dependency; the composition binary contains both roles.
Pipe possession constrains the session endpoint; claimed PIDs are metadata, not
OS-authenticated identity. An ephemeral symmetric backend MAC key is generated
from OS entropy for each launch and authenticates capability claims. It is runtime
state, never persisted or exposed to CI, and is unrelated to registry or signing
credentials. Ordered requests and exact response correlation reject replay within
a live transport session. HMAC is a symmetric authentication code, not a publicly
verifiable digital signature. The frontend must not claim MAC verification without
possessing and using a verification mechanism.

## Alternatives and evidence

The recovered implementation uses a known key, fixed timestamps, an empty
privilege assertion, a threaded demo and an unbounded parser. These mechanisms
cannot establish isolation or authority. Preserve useful computation and tests;
replace the defective mechanisms rather than preserving compatibility wrappers.
Revision 2026-09-05: the user's clarified goal supersedes the original blanket
WebView/JavaScript prohibition. Desktop web compatibility, WASM application
logic and browser rendering are required by
[ADR 0002](0002-web-application-contract.md). This process contract describes
the implemented native transport; it does not require browser code to spawn
processes or treat WASM as a privileged backend. Atlas provider reuse remains
the dependency policy.
Revision 2026-09-05: the user explicitly directs reuse of Atlas providers and
Moirai threading. Metis now selects Moirai's executor and transport roles instead
of owning another threading/process implementation. Runtime crates keep direct
dependencies on Atlas providers; the distribution CLI directly uses the
maintained Serde and `serde_json` parser for its validated manifest and Cargo
message tooling. Provider transitive dependencies are enumerated by the
verification gate. A zero-transitive interpretation would exclude Moirai and
conflicts with this clarified provider requirement.

Revision 2026-09-05: handshake failures distinguish local protocol/transport
faults from correlated peer rejections through `HandshakeError`. Both
`IpcClient::handshake` and `FrontendApp::init` return this error type; callers
match `Local(MetisError)` or `Remote(ErrorResponsePayload)`. The remote payload
retains its numeric code and diagnostic, including codes unknown to this
client. Mapping every rejection to `UnexpectedMessageType` discards the peer's
decision; forcing unknown codes into the local enum would fabricate a
classification. Malformed error payloads preserve the local decoding error.
Encoded-frame tests cover known and unknown codes, every truncated payload,
trailing bytes, invalid UTF-8, correlation and capability clearing. This is
an unpublished Rust API change; the wire representation is unchanged.

## Trust boundaries and limits

Assets are backend key material, calculation policy and audit records. Hostile
frontend input must not bypass authorization, allocate without bounds, panic a
parser or attribute a response to a different request. Every operation propagates
failure. Clinical-looking sample values illustrate software behavior only: no
clinical threshold source or regulatory submission evidence exists in the seed.

Separate processes do not establish a sandbox. OS permission restriction and
native window implementations require their own executable denial probes.
An in-memory hash chain does not establish durable, independently anchored audit
integrity. These remain tracked requirements, never inferred from passing tests.

## Verification

Canonical wire tests, negative scope/session/time cases, analytical arithmetic
oracles, parser boundary tests and actual subprocess integration tests establish
the foundation's behavior. The committed nextest runner terminates ordinary tests
at 60 seconds with no retries. Unsafe OS calls require local contract comments
and targeted host tests; host tests do not establish other-platform correctness.
