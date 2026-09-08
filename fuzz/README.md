# Metis parser fuzzing

This standalone harness exercises every public Metis wire decoder with
arbitrary bytes. It is outside the application workspace, so `libfuzzer-sys`
and its native runtime never enter normal binaries or the locked release graph.

Build and run it with the pinned verification toolchain:

```text
cd fuzz
cargo fuzz run protocol -- -max_total_time=60
```

The target accepts both successful and rejected decodes. A panic, abort, or
allocation growth beyond the decoder's 65,536-byte payload bound is a failure.
Deterministic bounded property and mutation cases run in the ordinary
`metis-ipc` contract test binary with Rust's standard library; this harness
supplies the unstructured-input search that those committed cases cannot
provide.
