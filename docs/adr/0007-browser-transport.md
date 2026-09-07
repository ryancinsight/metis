# 0007 — Browser transport ownership

Status: Accepted

Date: 2026-09-06

Driver: [METIS-ASYNC-001](../../backlog.md#METIS-ASYNC-001).

Revision 2026-09-06: Metis now consumes Moirai's merged browser PAL at
`66627b9`; the `metis-web` host and
[ADR 0008](0008-browser-host-boundary.md) provide the first runnable DOM
consumer. This updates the provider pin and adds runtime evidence without
claiming a live backend bridge.

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
`66627b9` and its Mnemosyne backend. The
browser workbench now loads the generated WASM and exercises
disconnect/rejection states. A live-service trace must still exercise
cancellation, late responses, listener/task teardown and capture V02/V12
evidence before this item closes.

## Residuals

The browser executor, desktop WebView host, origin/authority policy and visual
browser snapshots remain open. Native correlation tests do not establish a
running browser, callback/task teardown, memory efficiency or Tauri parity.
