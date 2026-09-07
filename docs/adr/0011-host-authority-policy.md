# ADR 0011: Host authority and origin policy

Status: Accepted

Date: 2026-09-07

Driver: [METIS-AUTHORITY-001](../../backlog.md#METIS-AUTHORITY-001).

## Context

Metis targets the browser and system-WebView application model used by Tauri,
while also allowing DOM state to be authored in Rust/WASM. A page origin,
window identifier or process label supplied in an IPC payload is untrusted
input. CSP limits resource loading but does not authenticate a backend command,
and a capability scope by itself does not identify the host that should use it.
The egui, GPUI and Iced comparison sources describe rendering and application
lifecycles; they do not supply this broker contract.

## Decision

`metis-core` owns a deny-by-default host contract:

- `HostOrigin` accepts only canonical ASCII network origins and rejects
  credentials, paths, fragments, wildcards, opaque schemes and invalid ports.
- `WindowId` and `HostSessionId` are validating nonzero identities.
- `HostPolicy` admits one exact origin and window and creates the context a
  trusted host observed for a session. A command must pass the policy, principal
  check, capability lifetime/scope checks and an HMAC signature bound to the
  context's canonical origin and window.
- The host binding is authenticated as associated data, so the existing 84-byte
  token wire representation remains fixed-width and contains no browser-chosen
  authority fields. `CapabilityToken::issue` remains available for generic
  non-host checks; privileged backend sessions issue and verify through
  `HostContext`/`HostPolicy`.
- `BackendService` receives a `HostPolicy`; the default contained demonstration
  uses `metis://native` and window 1, while deterministic tests can provide an
  explicit policy. The session stores the trusted context and rejects any
  token other than the one issued for that context.
- The browser asset shell uses external CSS and module files under the strict
  same-origin policy in `crates/metis-core/src/content_security_policy.txt`.
  `HostPolicy` includes that source and the browser build checks the HTML
  asset against it. The policy permits the generated WASM loader with
  `'wasm-unsafe-eval'`; its `frame-ancestors` directive is effective only when
  a native or service host sends the policy as a response header. The
  bootstrap blocks cross-origin anchor navigation as defense in depth.
  Downloaded WASM is never an authority source.

This contract is shared by future desktop and authenticated browser bridges.
Those bridges must supply observed host metadata and may not derive authority
from page fields or a caller-selected endpoint.

## Alternatives

Trusting a caller-supplied origin or window would allow substitution at the
trust boundary. Encoding mutable authority fields in the wire payload would
make them attacker-controlled unless another authenticated channel supplied
the same values. CSP alone does not bind a capability to a session. Adding a
third-party broker would duplicate the Atlas-owned Moirai transport and crypto
seams, so the policy stays in Metis and consumes Moirai's HMAC/SHA-256
primitives.

## Verification

Core tests cover canonicalization, strict IPv6, malformed and injection forms, exact policy
matches, origin/window/session substitutions, unbound-token rejection and
retargeted host signatures. Backend process and IPC tests continue to exercise
one-handshake session binding and expiry. The browser asset test and manual
trace verify external assets, strict CSP directives and lifecycle behavior.
Full workspace, release, documentation and visual gates bind the result to the
delivered revision.

## Limits

This decision does not establish a live authenticated WebSocket acceptor,
browser Origin header validation at a service, TLS endpoint policy, OS process
permissions, or cross-engine desktop evidence. Those capabilities remain in
[METIS-ASYNC-001](../../backlog.md#METIS-ASYNC-001),
[METIS-BROWSER-001](../../backlog.md#METIS-BROWSER-001),
and [METIS-DESKTOP-001](../../backlog.md#METIS-DESKTOP-001).
