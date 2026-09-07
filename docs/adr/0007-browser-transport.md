# 0007 — Browser transport ownership

Status: Accepted

Date: 2026-09-06

Driver: [METIS-ASYNC-001](../../backlog.md#METIS-ASYNC-001).

Revision 2026-09-06: Metis now consumes Moirai's merged browser PAL at
`66627b9`; the `metis-web` host and
[ADR 0008](0008-browser-host-boundary.md) provide the first runnable DOM
consumer. This updates the provider pin and adds runtime evidence without
claiming a live backend bridge.

Revision 2026-09-07: Moirai PR #268 merged at `16a1b88`, adding a cancellable
browser-local task handle. Metis now clears request-table entries after task
cancellation and exports `metis_stop` for listener teardown.

Revision 2026-09-07: Moirai PR #269 added the bounded native WebSocket service
and PR #270 merged at `be87d009cd0e877beef719b47bdcbadc45659069` moved
cancellation waiter wakeups outside the provider state mutex. Metis now runs
the browser client against that service through `AsyncIpcServer`; the service
checks Origin before the 101 response and binds the session to `HostContext`.

Revision 2026-09-07: Metis adds a bounded `--response-delay-ms` service probe
using Moirai's async timer. A live stop/remount trace confirms that a delayed
clinical response cannot mutate the new DOM after the browser transport and
listener guards are dropped.

## Context

The native Metis IPC client performs a blocking receive over an owned stream.
That contract cannot run on a browser event-loop thread. Browser WebSocket
callbacks and JavaScript timers are thread-affine and must not retain Rust
waiters, byte buffers or JavaScript roots after cancellation. The transport
boundary also has to preserve Metis frame limits and response correlation.

## Decision

Keep `IpcTransport` for native blocking streams and add
`AsyncIpcTransport` for event-driven hosts. The asynchronous trait sends one
bounded frame immediately and returns a future for one bounded receive with a
finite deadline. `AsyncIpcClient` shares the native framing, sequence allocation,
response validation and handshake decoding; it consumes a sequence before the
send so an uncertain operation is never retried with the same identifier.

On `wasm32`, `BrowserWebSocketTransport` owns a Moirai `WebReactor` descriptor,
registers read/write interest, bounds messages and queued frames, and closes
the descriptor on drop. Each receive owns one Moirai `WebSocketReceive` and
one `WebTimer`; dropping the composed future unregisters the waiter and clears
the timer callback. The adapter decodes exactly one Metis frame and rejects
empty, oversized, malformed or trailing data.

`AsyncIpcClient` exposes a bounded request table of sixteen entries. Callers
send several requests through `send_request`, then drive one receive consumer
through `recv_response_for`; responses that arrive for another outstanding
sequence are retained in the same bounded table. The browser still has one
Moirai receive waiter, so multiplexing is implemented as one pump rather than
parallel WebSocket callbacks. The convenience `send_and_recv` path remains
sequential. Authority, origin policy, browser host creation and HTML/CSS
rendering remain separate Metis items.

## Alternatives

Reusing the blocking pipe receiver would freeze or block a browser event-loop
thread. Reimplementing WebSocket callback ownership in Metis would duplicate
Moirai's provider boundary and split its resource limits. An unbounded queue or
an open-ended receive would turn peer input into unbounded application memory.

## Verification

`metis-ipc` native Nextest passes 33/33, including handshake rejection, zero
timeout, bounded request capacity, ordered and out-of-order sequence
correlation, and async frame round trips. Warning-denied
Clippy passes for native all-targets and `wasm32-unknown-unknown`; the lock
resolves all Moirai packages to merged provider
`be87d009cd0e877beef719b47bdcbadc45659069` and its Mnemosyne backend. The
`metis-backend` loopback tests complete an authenticated handshake and clinical
calculation with exact floating-point values, reject an unauthorized Origin
before `101`, and exercise the same service path used by the browser workbench.
The live browser trace covers service success, numeric rejection,
disconnect/recovery, stop/remount task teardown and a delayed response that is
disposed at the service/session boundary. The local request and listener
cancellation contracts remain covered by Metis tests.

## Residuals

Post-drop allocation measurement, cross-engine visual/runtime runs, TLS server
authentication and the desktop WebView host remain open. The loopback service is
a one-connection conformance host and does
not establish OS permission isolation or Tauri parity.
