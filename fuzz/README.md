# Metis parser fuzzing

This standalone harness exercises every public Metis wire decoder with
arbitrary bytes. It is outside the application workspace, so `libfuzzer-sys`
and its native runtime never enter normal binaries or the locked release graph.

Build and run it with the pinned verification toolchain:

```text
cd fuzz
cargo +nightly-2026-08-01 fuzz run --target x86_64-unknown-linux-gnu protocol -- -max_total_time=300 -rss_limit_mb=2048 -timeout=25 -print_final_stats=1
```

The target accepts both successful and rejected decodes. A panic, abort, or
allocation growth beyond the decoder's 65,536-byte payload bound is a failure.
Deterministic bounded property and mutation cases run in the ordinary
`metis-ipc` contract test binary with Rust's standard library; this harness
supplies the unstructured-input search that those committed cases cannot
provide. The single Metis verification workflow runs this same bounded
campaign on Ubuntu during its scheduled weekly run or an explicit manual
dispatch, where the nightly sanitizer runtime is available. Crash reproducers
are uploaded from the ignored `fuzz/artifacts/` directory.
