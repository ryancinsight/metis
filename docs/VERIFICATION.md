# Verification

The owning gate is `python scripts/verify.py`. It records bounded logs under the
ignored `output/` directory and checks formatting, strict Clippy, debug and release
nextest suites, doctests, documentation, example execution and dependency closure.
Native tests use `.config/nextest.toml`: slow at 30 seconds, terminate at 60 seconds,
zero retries. The demonstration executable has a 60-second outer budget.

Entry baseline: `cargo check --workspace --offline` passes with documentation and
source warnings. The original native test build fails with E0382 in the threaded
process-isolation test. No original OS sandbox or native-window evidence exists.

## Evidence classes

- Types and compilation: frontend cannot import the backend through its declared dependency closure; validated policy fields cannot be overwritten externally.
- Behavioral tests: exact wire fixtures, canonical decoding, malformed corpus, scope/session/time rejection, audit event outcomes and bounded numerical error.
- Independent numeric evidence: dimensional infusion conversion and exact binary fixtures; arithmetic roundoff uses a stated gamma bound.
- Crypto evidence: published HMAC test vector and SHA-256 known answers plus streaming/padding regressions.
- Process evidence: separately built executables exchanging real pipes; PID and result checks. These do not prove OS least privilege.
- Visual evidence: software framebuffer generated from actual form state and inspected independently of compilation.

Use `python scripts/verify.py`. Resolving commands run with `--locked` outside
the Atlas overlay, following Atlas's standalone-lock workflow. Inside Atlas,
the gate preserves the configured shared build directory and profile budgets;
it never disables the shared overlay or creates another build cache. The
committed lock must describe Git sources, without local-overlay substitutions
or unused-patch records. Earlier `--stack` verification is superseded by this
standalone gate so publishing cannot ship an overlay-only dependency graph.

The user manual replaces a domain book. Its application snapshot is produced by
the Rust presentation example from actual framebuffer pixels and checked against
`docs/manual/images/form.svg`. `--update-snapshots` explicitly refreshes that file;
normal verification rejects drift and missing local manual links.

## Collected Windows evidence — 2026-09-05

The foundation gate passes on Rust 1.97.0,
`x86_64-pc-windows-msvc`: formatting, all-target Clippy with warnings denied,
82/82 debug tests, 82/82 release tests, ten doctests, documentation with warnings
denied, real-process demonstration and presentation rendering. The 800×600 BMP
is inspected: title/status and all form labels fit; viewport background is filled.
The gate writes exact source hashes and lock content to
`output/verification.json`, rejecting source changes during a run.

The resolved host metadata contains 43 packages, including 19 registry packages
through Atlas providers. No Metis package declares a registry dependency.
Separate upstream evidence includes 39/39
Moirai transport tests and 18/18 Atlas overlay-generator tests, with a real Cargo
fixture detecting duplicate local/Git type identities before the generator fix.

The negative executable oracle matches `ERR_NUMERIC_INSTABILITY`, the wire error
code, rather than expecting rejected numeric input in diagnostic text. This
corrects the initial test's message assumption without relaxing rejection.

Windows tests do not prove Linux or macOS behavior. Miri does not execute Windows
native system calls; those require targeted lifecycle tests and further platform
instrumentation. Moirai resolves from pushed commit `0514f11`, not local provider
edits. The public-repository increment collects the standalone gate against the
repaired lock. Advisory scanning, coverage,
mutation analysis and cross-platform sandbox probes remain uncollected.
