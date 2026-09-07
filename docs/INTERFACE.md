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
| Calculation request | 0x0010 | token; weight/concentration/dose IEEE binary64; u16 UTF-8 identifier byte length; identifier |
| Calculation response | 0x0011 | audit sequence u64; rate/drug-rate binary64; pediatric byte 0 or 1; MAC [u8;32] |
| Error response | 0x00ff | error code u16; message byte count u16; UTF-8 message |

Audit query (0x0020/0x0021) and telemetry (0x0030) are reserved identifiers; the
backend does not implement these operations and rejects unsupported requests.

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
