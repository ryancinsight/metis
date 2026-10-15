# Private command observation codec and budget

This hidden cross-crate diagnostic seam reports Rust command-listener entry,
metadata/admission/decision and return. It does not change readiness, command
interpretation, input, backend authority or lifecycle acceptance. Selection
requires the exact document marker and the existing selected-form provider
configuration. Ordinary pages do not construct its producer or collector.

## Closed domain and representation

`Record` owns a typed `PageIdentity`, positive stream `SequenceId`, optional
positive invocation `SequenceId` and closed `Event`. Selected and Quota have no
invocation; every other event has one. Ready metadata admission requires an
actual snapshot and an evaluated decision. Other admissions require no snapshot
and NotEvaluated. The codec does not reproduce accelerator policy or infer that
a reported decision executes successfully. Keyboard key/code, flags, repeat
and trust remain their complete observed values; no extra per-string truncation
or 64-byte decoder restriction is introduced.

Private serialized types require kind `metis_command_observation`,
observation_schema1, exact existing WireIdentity fields, sequence decimal text,
required invocation string/null and an event object. u64 fields use the existing
canonical decimal parser; zero is allowed only in the nonidentity omission
counter. Commands use the existing ApplicationCommand stable ID catalogue.
Every object rejects duplicate and unknown fields in the ORIGINAL JSON.
Internally tagged fieldless variants use empty struct variants because Serde's
unit visitor ignores remaining fields. The private representation serializes
to the same type-only object without adding any public variant.

`is_observation` routes by the semantic top-level kind only; it is not admission.
Decode must receive the original JSON after routing. The bridge parses and
serializes JavaScript objects, so field order, whitespace and equivalent JSON
escapes cannot be treated as identity or validity. The new codec reuses the
canonical page encode/decode/identity owner with visibility widened only to
crate::embedded. Existing page protocol bytes and behavior remain unchanged.

## Finite terminal reserve

Let N=128, B=32768 and T=max(length(maximal Quota JSON),
length(maximal ClosingIntent JSON))=max(413,330)=413. Each maximal template uses
the longest closed page role, 20-digit u64 strings, 10-digit u32 counters,
64 lowercase digest bytes and the longer false spelling. Every variable byte is
therefore bounded by its type/closed spelling. Real codec tests serialize both
terminal types at the maximal widths and require the maximum to equal T.

For one owner with counters (n,b,t), admission computes both checked additions
before committing any field. Ordinary records require n+1≤N−1 and b+L≤B−T.
The one terminal requires n+1≤N, b+L≤B and L>0. Accepted admission commits
both counters and t atomically. Resource rejection returns false with the exact
prior state. Empty records, arithmetic overflow and use after terminal return
typed errors and preserve state. T reserves enough space for a locally encoded
canonical terminal; it does not limit the formatting of a received terminal.
The reserve is unavailable to ordinary records and no record follows an admitted
terminal. Both producer terminal variants explicitly encode within T.

Producer counters count canonical encoded bytes. Native collector counters count
the original received JSON bytes, using a separate Budget. JSON equivalence
does not equate these counters. A representation-equivalent terminal larger than
T remains admissible when it fits the collector's remaining total byte budget.
No re-encoding equality check or comparison of producer/native byte totals is
an admission requirement.

The abstract scalar-length count law is tested independently. Real minimal
Selected encoding is256 bytes, so127×256=32512>B−T=32355: real producer prefix
capacity reaches the byte bound before the record bound. Producer tests own
that actual-encoding maximum-prefix law and failure/terminal state transitions.

## Verification and limits

Portable tests use the real JSON codec, identity hash provider, typed events,
computed UTF-8 lengths and actual production Python configuration serialization.
They cover all stages, commands and outcomes, both roles, Unicode, integer
boundaries, semantic JSON representations, duplicate/unknown objects,
invocation/coherence rejection, inclusive byte budgets, both reserve limits,
atomic errors and terminal closure. Each test captures the hierarchical tracing
span tree with actual arrange/act/assert phases.

These are codec/accounting proofs. Browser callback execution, command posting,
native collector lifecycle and the selected/ordinary full journeys remain
separate real integration gates. No portable record fabricates a browser event,
key delivery, command result, native destruction or artifact acceptance.
