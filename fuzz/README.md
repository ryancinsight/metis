# Metis parser fuzzing

This standalone harness feeds arbitrary bytes to every parser that accepts
untrusted input. It is outside the application workspace, so `libfuzzer-sys`
and its native runtime never enter normal binaries or the locked release
graph.

| Target | Parser |
| --- | --- |
| `protocol` | the wire payload decoders, `FrameHeader::decode` and the packaged SVG admission parser |
| `fragment_patch_set` | `FragmentPatchSet::decode` |
| `fragment_action` | `FragmentAction::decode` |
| `deep_link` | `DeepLink::parse` |
| `host_origin` | `HostOrigin::parse` |
| `window_state` | `WindowState::decode` |
| `accelerator` | `Accelerator::parse` |
| `route_pattern` | `RoutePattern::parse` |
| `ipc_frame` | `read_frame` |
| `ui_markup` | `parse_markup` |
| `computed_style` | `ComputedStyle::parse` |
| `raster_image` | `RasterImage::decode` |
| `typeface` | `Typeface::parse` and the character-map and metrics lookups it defers |
| `archive` | the Linux package archive parser of `metis install` |

Build and run one target with the pinned verification toolchain:

```text
cd fuzz
cargo +nightly-2026-08-01 fuzz run --target x86_64-unknown-linux-gnu deep_link corpus/deep_link seeds/deep_link -- -max_total_time=30 -rss_limit_mb=2048 -timeout=25 -print_final_stats=1
```

`seeds/<target>` holds the tracked seed corpus: valid inputs drawn from each
parser's documented grammar, plus a few inputs it must reject. libFuzzer
replays every seed before it mutates, and writes the inputs it discovers to
the first corpus directory, the ignored `corpus/<target>`, so the tracked seeds
stay unchanged. A new target adds its `[[bin]]` entry, its source file and at
least one seed; `scripts/tests/test_fuzz_harness.py` fails otherwise.

A target accepts both successful and rejected parses. A panic, abort, an input
that exceeds the 25-second timeout, or memory beyond the 2 GiB RSS limit is a
failure, and so is a successful parse that breaks its documented contract:
`FragmentPatchSet`, `FragmentAction`, `WindowState`, `Accelerator` and
`HostOrigin` values re-encode to text or bytes that parse back to the same
value, a decoded image holds one pixel per grid cell, and every admitted archive
path lies under `usr/`.

`metis-cli` is a binary crate, so `src/lib.rs` includes the exact source
files of its archive and SVG parsers (`#[path]`) and exposes one entry point
for each; the included `manifest/payload_path.rs` carries the path rule they
share.

The single Metis verification workflow runs every target on Ubuntu, where the
nightly sanitizer runtime is available, during its scheduled weekly run, an
explicit manual dispatch, or a pull request that changes `fuzz/`. Each target
runs for 30 seconds except `protocol`, which keeps its 300-second budget.
Crash reproducers are uploaded from the ignored `fuzz/artifacts/` directory.
A reproducer found by a campaign is committed to `seeds/<target>` with a
regression unit test in the parser's crate once the parser is fixed.

Deterministic bounded property and mutation cases run in the ordinary
`metis-ipc` contract test binary with Rust's standard library; this harness
supplies the unstructured-input search that those committed cases cannot
provide.
