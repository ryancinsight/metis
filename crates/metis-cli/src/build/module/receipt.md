# Native root dependency evidence

Status: Accepted; internal implementation of ADR 0059, Metis 0.5.0.

## Contract and ownership

Cargo's exported dependency lists omit rustc environment dependencies. A raw
check-phase record also cannot substitute for code generation's record: a real
procedural macro can inspect compiler arguments and introduce a different
`env!` key only while linking. Each phase must supply its actual evidence.

Ordinary browser compilation retains `cargo build` and its fingerprint behavior.
Native preparation transitions from `Checked` to `Compiled<RootInputs>` using
`cargo rustc` with an explicit root dep-info destination. The existing fresh
`Context` owns that native path until generation and input verification finish.
Rust's native `OsString` carries the destination without Unicode conversion.

The input collector matches the selected package ID, target name and noncustom
cdylib role before using the root record. It admits the actual bounded unlinked
regular file; missing or invalid evidence rejects, without selecting another
record. All compiler-reported nonroot records retain their input ownership.
Unsupported source shapes cannot substitute exported lists for raw environment
evidence. Native errors retain their concrete source and attempted path.

`Compiled<BrowserInputs>` does not implement `CompilerOutput`. The compiler
therefore prevents an ordinary browser result from entering native input
collection. Receipt policies use static dispatch; browser receipt state is a
zero-sized type. One compilation algorithm owns both paths.

For each phase, input collection reads the current values of its actual reported
environment keys. The existing equality of source, directory, feature and
environment records between check, compilation and generation remains required.
A phase-dependent input set rejects; previous keys or values are never reused
to manufacture equality.

The semantic compiler configuration records the explicit dep-info emission
kind. Its physical destination remains a typed output, like the existing Cargo
manifest and wasm-bindgen output paths, rather than a portable compiler flag.

## Alternatives and cost

First-output selection loses primary records. Reading all exported files still
loses environment records. Additional rlib generation does not make Cargo report
a raw metadata file. Private layout/hash inference and check-key seeding do not
establish the required record. Compiler wrappers add a separate composition and
cache protocol; they are outside this increment.

Real pinned native and WASM experiments show explicit dep-info emission preserves
the produced library but prevents root fingerprint reuse. Native preparation
accepts this measured compilation cost to obtain actual evidence. Browser builds
retain their existing cache. No cache-freshness or runtime-performance improvement
is claimed; all process deadlines and source checks remain unchanged.

## Verification

Real Cargo fixtures inspect primary/companion roles and computed input records.
The phase-dependent macro case must expose the build-only environment key and
reject differing phase inputs. Fresh release preparation, independent producer
and native admission, all fourteen mutation cases and actual Windows acceptance
remain the integration checks. No root record is included in the app payload.
