# Build and run

Install Rust through rustup, Python 3.11 or later for repository automation, and
the pinned `cargo-nextest` runner:

```text
cargo install cargo-nextest --version 0.9.143 --locked
git clone https://github.com/ryancinsight/metis
cd metis
cargo fetch --locked
python scripts/verify.py
```

`rust-toolchain.toml` selects Rust 1.97.0. The verification script builds both
executables and examples, runs debug/release tests and documentation, and checks
that the gallery snapshot matches the renderer. Subsequent Cargo commands in the
gate use the committed lockfile offline. When run inside Atlas, the gate resolves
published Git sources outside the local dependency overlay while retaining the
same build cache and profile settings.

The executable process demonstration currently requires Windows. Its supervisor
requires Windows job containment; Linux/macOS builds do not establish a working
contained demonstration. A Windows development environment needs the MSVC build
tools and Windows SDK. Native desktop event loops are not implemented yet.

## Run the form workflow

```text
cargo build --locked --workspace --bins
cargo run --locked -p metis-backend -- 60 2 0.2
```

The arguments are weight in kilograms, concentration in milligrams per milliliter,
and target dose in micrograms per kilogram per minute. These inputs produce
`rate_ml_hr=0.36` and `drug_rate_mg_hr=0.72`, with two verified in-memory audit
records. Backend and frontend report different process identifiers. Human output
uses standard error; the frontend's standard input/output carry framed IPC.

This is synthetic arithmetic for testing application boundaries, not a treatment
calculator. The configured limits do not constitute clinical validation.

## Diagnose a failure

- An unavailable Git revision is a dependency-resolution failure. Check the
  pinned revision and access to the Atlas providers; do not replace failed
  calculations with default results.
- A numeric rejection reports a stable error such as `ERR_NUMERIC_INSTABILITY`.
  `NaN` is rejected and no result is displayed.
- A session timeout terminates the contained frontend tree. The demonstration's
  ten-second session budget applies to the child; application handlers must also
  finish their own synchronous computation.
- A snapshot mismatch means a visible artifact changed. Inspect the rendered
  `output/form.bmp`, then use `python scripts/verify.py --update-snapshots` to
  regenerate the tracked gallery image deliberately.

Verification logs are overwritten under ignored `output/` paths. See
[the verification record](../VERIFICATION.md) for evidence and remaining coverage.
