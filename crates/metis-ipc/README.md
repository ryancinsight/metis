# metis-ipc

Versioned frame I/O, request/response correlation, per-connection replay
rejection, and bounded native and browser transports for Metis.

```rust
use metis_core::protocol::MessageType;
use metis_ipc::{IpcTransport, MemoryTransport};

let (mut sender, mut receiver) = MemoryTransport::pair();
sender.send_message(MessageType::HeartbeatReq, 1, b"heartbeat")?;
let (header, payload) = receiver.recv_message()?;
assert_eq!(header.sequence_id, 1);
assert_eq!(payload, b"heartbeat");
# Ok::<(), metis_core::error::MetisError>(())
```

The wire decoder limits payloads to 65,536 bytes and distinguishes clean
closure from truncated headers and payloads. Memory endpoints buffer at most
16 frames per direction, return `QueueFull` without blocking on backpressure,
and receive with a five-second default deadline. Stream callers configure
read/write deadlines on their underlying I/O; generic blocking OS pipes do
not provide cancellation through the `Read`/`Write` traits.

Browser callers use `AsyncIpcClient` with the WASM-only
`BrowserWebSocketTransport`. `send_request` permits at most sixteen requests
or completed responses at once; one receive consumer routes out-of-order
responses with `RequestId` and `recv_response_for`. Each receive has a finite
deadline and Moirai owns WebSocket callback and timer teardown. A caller that
cancels a browser task can call `cancel_request` or `cancel_all_requests` to
remove its correlation entries; late responses are rejected by sequence
validation instead of being delivered to a later request.

Fault injection modifies encoded wire bytes before delivery. CRC32 detects
accidental corruption; it does not authenticate transport peers. Handshake
principal/version checks validate response consistency, while the application
must establish peer trust and authorize the claimed identity.

Tests exercise every truncation point, malformed UTF-8 and booleans, payload
bounds, queue backpressure, response correlation, replay, and wire corruption.
These are behavioral tests, not a certification or a formal protocol proof.
