# 0009 — Atlas crypto provider boundary

Status: Accepted

Date: 2026-09-07

Driver: [METIS-CRYPTO-001](../../backlog.md#METIS-CRYPTO-001).

## Context

Metis carried a local SHA-256, HMAC-SHA256 and fixed-width comparison
implementation for capability tokens, audit hashes and result signatures. The
implementation duplicated the RustCrypto algorithms already used by Moirai's
crypto provider, which made two authentication authorities possible. The
provider package previously exposed only its TLS construction path, so using it
without a dependency boundary would also compile TLS code for protocol-only
consumers.

## Decision

Metis consumes `moirai-crypto` with `default-features = false`. The provider's
standalone `Sha256`, `sha256`, `hmac_sha256` and `constant_time_eq_32` APIs are
the only SHA/HMAC implementation used by capability, audit, backend result and
CLI packaging code. `metis-core::crypto` retains only CRC-32, whose contract is
framing corruption detection rather than peer authentication.

All Moirai dependencies in this workspace advance together to merged revision
`66627b9`. The TLS provider remains available to consumers that enable its
default `provider` feature; Metis's protocol graph does not compile rustls or
the provider's key-exchange, AEAD and certificate dependencies.

## Migration

This is a breaking pre-release API migration. The former `metis_core::Sha256`,
`metis_core::sha256`, `metis_core::hmac_sha256` and
`metis_core::constant_time_eq_32` exports are removed. Callers import the
canonical Moirai functions or type, and the in-repository CLI, backend and
integration tests are migrated in the same change. No forwarding re-export or
compatibility wrapper remains.

## Alternatives

- Keep the Metis implementation: rejected because it leaves duplicate
  authentication behavior and independent maintenance obligations.
- Add direct RustCrypto dependencies to Metis: rejected because it recreates a
  second provider-owned dependency surface.
- Depend on Moirai's default feature set: rejected because protocol hashing
  would compile TLS-only dependencies and expand the browser/CLI graph.
- Retain a re-export from `metis-core`: rejected because it preserves a second
  public path for the same provider operation and delays caller migration.

## Verification

Moirai tests cover FIPS 180-4 SHA-256 vectors, the RFC 4231 HMAC vector,
streaming padding boundaries and equal/different comparisons. Its no-default
feature graph contains only `hmac` and `sha2` beneath the provider crate and
compiles for `wasm32-unknown-unknown`; release assembly shows all 32 comparison
bytes execute before the final result branch. Metis capability, audit, result
signature, CLI digest and wire tests exercise those APIs at their real contract
boundaries. CRC framing remains independently vector-tested.

## Limits

The comparison evidence is source-level constant work and one release assembly
inspection on the development host, not a universal hardware timing proof.
The migration does not establish origin/session authority, durable audit
storage, or a security comparison against Tauri; those remain their owning
backlog items.
