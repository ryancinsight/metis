# 0056 — Rust browser shell and authenticated HTTP presentation

Status: Accepted

Date: 2026-10-12

## Decision and acceptance

Target: Metis 0.3.0. Rust owns both browser shells: query configuration,
start/stop listeners, same-origin anchor policy, HTTP session and request
lifecycle, fragment codecs, allowlists and DOM updates. Each authored module
loads the generated wasm-bindgen module and calls one Rust entry point.
Generated Web API bindings remain the necessary browser ABI.
The HTTP endpoint comes from the host HTML configuration as an exact HTTP(S)
`/health` URL; its typed origin supplies all three closed routes. CSP and the
backend Origin/session policy remain separate enforcement boundaries.

Protocol serialization uses metis-core's existing HandshakeRequestPayload,
HandshakeResponsePayload, PluginInvocationPayload, FragmentAction,
FragmentPatchSet and ErrorResponsePayload. HTTP and WebSocket presentations
share attribute validation and one preflight/apply implementation; each mounted
surface supplies its exact target allowlist. No copied wire offsets remain.

A request has a nonzero mount generation and a strictly advancing request
generation. It may publish only while both still match. Replacing a request,
resetting or stopping drops the owned PAL task and aborts its Fetch. Reset
increments the mount before clearing output. Stale or disallowed patches
leave the complete DOM unchanged. A handshake token becomes usable only after
version, principal and UI-render-scope validation; the server remains the
authentication authority. The browser never verifies or manufactures a grant.
The denial probe preserves the existing transport-session binding: a different
principal has no HTTP session (401/MissingCapability), and a changed signature
fails exact issued-grant equality (403/InvalidPrincipal) before HMAC validation.

HTTP session establishment becomes retry-safe: an exact repeat of the admitted
handshake returns the original response bytes until the original monotonic
expiry. It issues no grant and changes neither expiry nor command sequence.
Origin, principal and non-identical active-handshake denials retain their
existing rules. This closes reload and cancellation after grant issuance.
The pre-1.0 0.3.0 Breaking entry records the previous 409-to-200 behavior change.

The workbench shell outlives application stop so Start remains operable;
reinitialization releases the old shell before installing its successor.
The application listener-count contract continues to count application
listeners; shell and HTTP counts are exposed separately for teardown evidence.
HTTP Stop releases its four handles and disables controls; reloading starts a
new host through the retry-safe handshake.

## Plan and evidence

1. Add bounded cancellable browser mechanisms in Moirai with native bound tests.
2. Remove authored JavaScript behavior and use canonical Rust protocol/policy.
3. Exercise real services, Unicode fragments, denial, remount, cancellation and
   repeated start/stop; compile native and WASM targets with clean diagnostics.
4. Synchronize the Tauri capability review, manuals, backlog, locks and versions;
   deliver provider then consumer through their normal protected checks.

Direct browser bindings in Metis and JavaScript forwarding policy are rejected
because Moirai already owns the browser platform seam. Application-owned
protocol duplicates are removed instead of retained as compatibility aliases.
This increment does not establish physical mobile permissions, Android hosting,
signed update recovery or a matched Tauri performance comparison; their existing
acceptance gates remain open.
