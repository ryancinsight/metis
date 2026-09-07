# 0007 — Browser transport ownership

Status: Accepted

Date: 2026-09-06

Driver: [METIS-ASYNC-001](../../backlog.md#METIS-ASYNC-001).

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

The first client is sequential: one request awaits one correlated response.
Out-of-order multiplexing requires a separate bounded request table and a
conformance test before it is exposed. Authority, origin policy, browser host
creation and HTML/CSS rendering remain separate Metis items.

## Alternatives

Reusing the blocking pipe receiver would freeze or block a browser event-loop
thread. Reimplementing WebSocket callback ownership in Metis would duplicate
Moirai's provider boundary and split its resource limits. An unbounded queue or
an open-ended receive would turn peer input into unbounded application memory.

## Verification

`metis-ipc` native Nextest passes 29/29, including handshake rejection, zero
timeout, sequence correlation and async frame round trips. Warning-denied
Clippy passes for native all-targets and `wasm32-unknown-unknown`; the WASM
check resolves all Moirai packages to `95ff7ae` and the merged Mnemosyne
backend `2eb49c1`. These are static and native evidence only. A real browser
trace must load the WASM, exercise cancellation/disconnect/recovery, verify
listener/task teardown and capture V02/V12 evidence before this item closes.

## Residuals

The browser executor, desktop WebView host, origin/authority policy, concurrent
out-of-order request table, and visual browser snapshots remain open. No
security, memory-efficiency or Tauri-parity claim is derived from compilation.
