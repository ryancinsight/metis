# ADR 0025: Axum server boundary comparison

Status: Accepted

Date: 2026-09-10

Driver: [METIS-AXUM-001](../../backlog.md#METIS-AXUM-001)

Revision 2026-09-11: [METIS-AXUM-001](../../backlog.md#METIS-AXUM-001)
admits a loopback-only `metis-app` server demonstration for the user manual.
The target exercises the first-party Moirai HTTP transport and Metis policy;
it does not authorize a public listener, production deployment, or a DICOM
boundary in Metis.

## Context

Axum's 0.8 documentation describes a typed server boundary built from
`Router`, request extractors, shared `State`, middleware layers and
`IntoResponse`. These surfaces are relevant to a Metis deployment that serves
HTML or typed fragments. Metis hosts browser application logic as Rust/WASM
and uses Moirai for its bounded WebSocket and loopback HTTP transports; it
does not depend on Axum.

The current htmx-informed action path already limits mutations to authenticated
generation-bound text and attribute patches. The admitted loopback endpoint
specifies route authorization, request limits, cancellation, response size and
client teardown through the first-party Moirai transport. DICOM parsing, study
selection, geometry and medical display are RITK responsibilities and are
outside this boundary.

## Decision

Treat Axum as a source-pinned server/router comparator. Do not add Axum as a
Metis dependency or present its API as implemented behavior. The admitted
`metis-app --metis-http-service` target is a first-party service boundary over
Moirai and Metis policy. It provides:

- a closed route table and typed request/response envelopes;
- bounded body, response, queue and connection resources;
- origin, session and capability checks before command dispatch;
- explicit deadlines, cancellation and disconnect cleanup; and
- allowlisted text/attribute patches rather than arbitrary markup, scripts or
  navigation.

The browser WebSocket contract remains the default browser bridge; the HTTP
target is a finite loopback demonstration rather than a public deployment. The
service boundary carries presentation messages only; RITK remains the owner of
DICOM and viewer state.

## Alternatives

Adopting Axum immediately would add a third-party server/router owner beside
Moirai without a current server consumer and would not supply the required
authority policy by itself. Sending arbitrary HTML fragments from an existing
browser path would bypass the typed target and generation checks. Leaving the
server question undocumented would make a future endpoint appear complete from
an API comparison alone. The conditional first-party boundary keeps the
current graph small and gives an admitted deployment a testable contract.

## Threat model and limits

The service must treat routes, request bodies, origins, sessions,
capability identifiers and client disconnects as untrusted. Route confusion,
oversized allocation, stale responses, cross-session replay, slow-client
retention and fragment injection are rejected by the bounds and checks above.
TLS termination, operating-system permissions, durable audit and deployment
topology remain separate controls. This ADR contains no public-deployment or
security-superiority claim.

## Verification

The admitted local target is implemented by `metis-backend::BrowserHttpService`
and `metis-app --metis-http-service`. The native suite proves a real Moirai
loopback handshake and fragment exchange, origin and route denial, malformed
and oversized request rejection, exact-origin CORS preflight, disconnected and
idle-peer deadline handling and finite teardown. The generated browser
`http-health.html` probe performs the real cross-origin health request,
authenticated handshake and generation-bound fragment request, then records
malformed, unauthorized and stale-generation outcomes without changing the
mounted text. The invocation suite proves the closed CLI role. This evidence
does not claim a public deployment, TLS, cross-engine capture or DICOM
behavior; those remain separate controls and RITK-owned workflow evidence.
