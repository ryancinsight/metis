# ADR 0038: Durable audit recovery

- Status: Accepted
- Date: 2026-09-17
- Item: [METIS-AUDIT-001](../../backlog.md#METIS-AUDIT-001)

## Context

`metis-backend` keeps a bounded `METIS-AUDIT-2` hash chain in memory. That
chain correctly detects field changes while the process is alive, but it does
not survive a restart and a process that can rewrite both the records and the
checkpoint can rewrite its history. The Atlas stack map assigns Consus
scientific data formats and exchange, while no Atlas provider owns this
bounded, security-sensitive session audit store. Moving it to Consus would
couple clinical session authority to a scientific format dependency and would
not provide an external trust anchor.

## Decision

The backend owns native audit persistence. `FileAuditStore` stores at most two
authenticated snapshots in an application-selected directory. Each snapshot
has a versioned fixed header, a bounded sequence of fixed-size records, the
retained chain checkpoint, the next sequence and a monotonically increasing
generation. Moirai's standalone HMAC-SHA256 authenticates the complete snapshot
with an `AuditCheckpointKey` supplied by the host; the key is never persisted
and is distinct from the ephemeral IPC session key and registry credentials.

The store writes the inactive slot, flushes and synchronizes it, and advances
its in-memory generation only after the write succeeds. Startup validates every
existing slot's size, version, field bounds, HMAC and `METIS-AUDIT-2` chain. A
missing slot is allowed; any malformed, truncated, duplicated or unauthenticated
existing slot fails closed instead of silently rolling back to an older record
set. The backend records through a verified candidate ledger, so a failed
persistence write does not advance the in-memory chain or acknowledge the
request.

Snapshots contain only sequence, bounded timestamp, session principal bytes,
typed event identity, error code, predecessor hash and record hash. They never
contain request payloads, patient identifiers or the checkpoint key.

## Alternatives

* **Consus scientific persistence** — rejected. Its ownership is scientific
  arrays, formats and transport; introducing that dependency into the backend
  would widen the clinical authority closure without supplying the audit trust
  contract.
* **One replace-in-place file with a backup** — rejected. Windows replacement
  semantics and an attacker-controlled primary would make it ambiguous whether
  a fallback was a crash recovery or a silent rollback.
* **Unauthenticated append-only text** — rejected. Truncation and record edits
  would be indistinguishable from valid history and the format would leak
  unbounded diagnostic text.

## Threat model and limits

The store defends against accidental truncation, partial writes, wrong keys and
offline record edits when the checkpoint key remains secret. It does not defend
against a process or host that can read the key and rewrite both slots, nor does
it establish regulatory evidence or a hardware/remote trust anchor. A deployment
that needs independent anchoring must provide that boundary before claiming
tamper resistance beyond keyed local recovery.

## Verification

Unit tests cover exact restart round trips, two-slot generations, wrong-key and
byte-tamper rejection, bounded event decoding and service integration. V08 adds
the manual recovery demonstration; the full native and release gates exercise
the same persistence path through the backend service.
