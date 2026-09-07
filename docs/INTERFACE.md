# Interface contract

All integers use big-endian wire order. Protocol version is 0x0100; unsupported
versions fail. CRC32 detects accidental corruption, not adversarial authenticity.

| Header offset | Size | Meaning |
| --- | --- | --- |
| 0 | 4 | ASCII METI |
| 4 | 2 | Protocol version |
| 6 | 2 | Message type |
| 8 | 8 | Strictly increasing nonzero request sequence |
| 16 | 4 | Payload CRC32 |
| 20 | 4 | Payload byte count, maximum 65,536 |

Responses echo the outstanding sequence and use its expected response type or
ErrorResp. A complete-stream EOF is distinct from any truncated header/payload.
Unknown types, malformed UTF-8, noncanonical booleans and trailing payload bytes
are errors. Sender-side encoding applies the same size limits as decoding.

| Message | Type | Payload |
| --- | --- | --- |
| Handshake request | 0x0001 | version u16, claimed process u32, principal [u8;16] |
| Handshake response | 0x0002 | version u16, token |
| Heartbeat request/response | 0x0003/0x0004 | empty |
| Capability catalog request/response | 0x0005/0x0006 | request empty; response version u16, count u16, request identifiers u16[] (maximum 16) |
| Calculation request | 0x0010 | token; weight/concentration/dose IEEE binary64; u16 UTF-8 identifier byte length; identifier |
| Calculation response | 0x0011 | audit sequence u64; rate/drug-rate binary64; pediatric byte 0 or 1; MAC [u8;32] |
| Error response | 0x00ff | error code u16; message byte count u16; UTF-8 message |
| Telemetry event | 0x0030 | version u16; event id u64; name byte count u16; body byte count u32; UTF-8 name; bounded body |

Audit query (0x0020/0x0021) remains reserved and the backend rejects it as a
request with `ERR_UNEXPECTED_MESSAGE_TYPE`. Telemetry (0x0030) is an unsolicited
event sent by an authenticated host. Its envelope rejects zero or replayed
identifiers, empty or oversized names, invalid UTF-8, truncation and trailing
bytes. `EventCodec` associates a stable name with a typed body without dynamic
dispatch. A capability catalog is valid only after handshake and lists the
request identifiers the host currently accepts.

Plugin manifests are host-local metadata rather than a wire message. A
`Plugin` implementation supplies one static `PluginDescriptor` to a bounded
`PluginRegistry` (maximum 16 manifests and 16 combined operations per
manifest). Identifiers are lower-case ASCII names within their byte bounds;
each command or event declares a non-empty `CapabilityScope`, and duplicate
operation names are rejected. Registration is metadata only: a host still
performs capability checks and typed handler dispatch at its extension
boundary.

A token is 84 bytes: id u64, principal [u8;16], scope u32, issuance u64,
expiration u64, issuance discriminator u64, HMAC [u8;32]. Generic token
authentication uses the canonical fixed claims buffer with four zero padding
bytes after the discriminator. Host-issued tokens authenticate that same claims
buffer plus a fixed host-binding record containing a domain tag, SHA-256 digest
of the canonical origin and window u64. The binding is associated data and is
reconstructed by the trusted host; it is not copied into the wire token.
Validity is [issued, expires); the backend also checks its monotonic session
lifetime and locks out detected clock rollback. The principal is an
application session label, not proof of OS identity.

`ErrorCode` is non-exhaustive so future typed failures can be added without
requiring downstream match arms; callers handle unknown future codes at the
boundary.

The backend grants only SUBMIT_CALCULATION. It accepts one handshake per private
session and binds accepted claims to the token issued on that session and the
configured `HostPolicy` context. The result MAC covers the domain tag, request
sequence, canonical request and all response fields except the MAC itself. The
frontend displays its received MAC without asserting that it can verify it.

Each handler result records a typed audit event. Failure contexts distinguish
receive failure with no decoded header, request rejection with identity, handler
failure and response delivery failure. A processed request whose response fails
has two distinct events. Audit records use canonical METIS-AUDIT-2 hashing and
bounded in-memory retention; persistence and trusted checkpoints remain open.
