# metis-core

Shared versioned wire vocabulary, capability claims, typed errors and framing
integrity. Authentication hashing comes from the Atlas-owned
`moirai-crypto` provider with its TLS feature disabled; CRC-32 remains local
because it detects accidental corruption rather than authenticating peers.
This crate contains no backend calculation or audit storage implementation.
The protocol exposes a bounded capability catalog with descriptors for the
closed request/response command set; catalog entries are validated before a
host or client can use them. `RemoteEventPayload` carries a versioned,
bounded unsolicited event envelope; `EventCodec` supplies typed body encoding
and decoding without a dynamic dispatch table. The backend uses the
`clinical.result` codec for the event that follows an accepted calculation.
`PluginRegistry` accepts
statically typed plugin manifests with explicit capability scopes and a bounded
operation set; `PluginInvocationPayload` and
`PluginInvocationResponsePayload` carry a versioned, bounded request and opaque
response body for the host router. This crate validates the wire envelope but
does not execute plugin code or grant operating-system authority.
`TargetCapabilityPayload` reports the host platform and its bounded, explicitly
installed surfaces. It does not infer native windows, operating-system
permissions, accessibility or IME support from a platform identifier.

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
Host-bound grants authenticate the canonical origin and window as associated
data in addition to the session principal. The wire token stays fixed-width;
the trusted host reconstructs this binding before dispatch.
The framing CRC detects corruption and does not authenticate a peer.
The provider's fixed-width comparison and HMAC vectors are tested upstream;
Metis's capability and audit tests exercise the same functions at their
canonical wire boundaries.
See the workspace [wire contract](../../docs/INTERFACE.md) and
[verification limits](../../docs/VERIFICATION.md). This package is unpublished.
