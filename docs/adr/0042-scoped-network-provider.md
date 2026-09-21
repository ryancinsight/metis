# ADR 0042: Scoped network provider

Status: Accepted
Date: 2026-09-21
Driver: [METIS-SERVICES-001](../../backlog.md#METIS-SERVICES-001)

## Context

Metis already consumes the first-party Moirai HTTP client for its local
application service, but the platform crate had no consumer-facing network
boundary. A frontend request must not select an arbitrary authority, inject
transport-owned headers, follow a redirect into a new authority, or allocate
an unbounded response. The contract must remain independent of browser and
DICOM concerns.

## Decision

The native platform owns ScopedHttpProvider. The host supplies a non-empty
allowlist of HTTP(S) origins, and the caller supplies a
CapabilityScope::NETWORK witness. ScopedHttpRequest validates the absolute
URL, method, headers and body before the request reaches Moirai. Hop-by-hop
headers are rejected, redirects are disabled, and the response has a finite
body and header budget. Debug output omits URL, header values and payload
bytes.

The provider uses Atlas Moirai HTTP and TLS without adding a second runtime or
client stack. Browser code remains an authenticated service consumer; it does
not receive a native network object. Each request has a finite deadline: the
default is `MAX_SCOPED_HTTP_DEADLINE`, and callers may select a shorter bound.
Dropping the request future cancels the transport; expiry drops it through the
same path.

## Alternatives

Passing a raw HttpClient would make authority caller-controlled and would
permit redirect expansion. A third-party runtime would duplicate Atlas
executor and TLS policy. A browser-only fetch API would leave native sidecars
without the same boundary.

## Verification

Platform tests perform real loopback HTTP exchanges through local TcpListeners
and Moirai's executor, then assert status, header and body values. Rejection
tests cover empty and non-HTTP allowlists, URL fragments, forbidden headers,
oversized bodies, excessive headers and a missing NETWORK witness. A delayed
local server proves the explicit deadline returns `TimedOut` and observes the
client connection closing; invalid deadlines and an unlisted origin fail before
transport. Nextest, offline check and strict Clippy pass for metis-platform.

## Residuals

WebSocket policy, certificate pinning, retries, browser APIs and an operating
system network sandbox remain outside this increment. They stay under
METIS-SERVICES-001. RITK continues to own DICOM parsing and presentation.
