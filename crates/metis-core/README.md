# metis-core

Shared versioned wire vocabulary, capability claims, typed errors and integrity
primitives. This crate depends only on Rust's standard library. It contains no
backend calculation or audit storage implementation.

```rust
use metis_core::{build_frame, FrameHeader, MessageType, HEADER_SIZE};
let bytes = build_frame(MessageType::HandshakeReq, 7, b"request")?;
let header_bytes: &[u8; HEADER_SIZE] = bytes[..HEADER_SIZE].try_into().expect("frame includes header");
let header = FrameHeader::decode(header_bytes)?;
assert_eq!(header.sequence_id, 7);
# Ok::<(), metis_core::MetisError>(())
```

Capability validation controls application commands, not operating-system
privileges. HMAC requires a private backend key; it is not a public signature.
The framing CRC detects corruption and does not authenticate a peer.
See the workspace [wire contract](../../docs/INTERFACE.md) and
[verification limits](../../docs/VERIFICATION.md). This package is unpublished.
