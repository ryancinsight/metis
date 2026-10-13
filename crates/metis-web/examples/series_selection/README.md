# Series selection demonstration

This opt-in Rust/WASM example recovers the sidebar and four-viewport interaction
from Metis preservation PR [#498](https://github.com/ryancinsight/metis/pull/498).
It starts from main `6662f14f54e8646268b15d86d928d9abca59d09e`, not the
preservation branch. The default workbench remains format-neutral (ADR 0035).

## Contract

For `N > 0`, state is `(a, V)` with active viewport `0 <= a < N` and
`V: [Option<SeriesId>; N]`. Each series identifier belongs to the declared
demonstration study. The selected series is derived as `V[a]`; there is no
second mutable selected-series field.

- Selecting `s` yields `(a, V[a := Some(s)])`.
- Activating viewport `b`, where `b < N`, yields `(b, V)`.
- Invalid controls, malformed viewport identifiers and out-of-range viewports
  reject without changing either component.
- Repeating either accepted action is idempotent.
- Selection changes only the active viewport's assignment. Inactive assignments
  persist, including when activating an unassigned viewport.
- Remount resets assignments and retains one click-listener guard. Stop drops
  that guard and removes the presentation.

The reducer owns assignments. The browser adapter owns Moirai DOM handles and
listener lifetime. The presentation derives labels, selected button state and
viewport content from the reducer. Native HTML buttons provide Enter/Space
activation; a delegated listener resolves clicks on descendants.

## Preservation comparison

Commit `9c0933a00da33a7b59698719a3931b60f7733b0f` (October 5, 2026) adds
four files. `controls.rs` contains static study/series labels and four references
to the same `clinical-ct-mip.webp`; `styles.css` adds the sidebar/grid;
`browser-layout-metrics.json` changes its stylesheet digest; the WebP adds
154,270 image bytes. Main contains none of this viewer shell. The preservation
PR is an open, unmerged draft on October 13, 2026.

This example recovers the layout relationship and series-selection behavior.
It does not transplant the nonfunctional imaging buttons, clinical counts,
slice positions, window/level values, image or historical layout metrics.
All four original files and the original commit remain on PR #498's branch;
this increment does not edit or merge that branch.

## Capability boundary

The study and its three series are explicitly named demonstration fixtures.
Viewport presentation consists of assignment labels and fixture descriptions.
It contains no image pixels, loaded clinical metadata or fabricated slice
counts. DICOM loading, decoding, geometry, independent image planes and
multiplanar reconstruction remain consumer-owned RITK capabilities, outside
this example. Selection acceptance cannot establish those capabilities.

## Verification

Native Rust tests cover the algebra across viewport counts 1, 2 and 4, every
fixture series, inactive preservation, restoration, idempotence and rejected
input. The browser acceptance runner checks actual WASM/DOM transitions,
descendant clicks, keyboard activation, focus, stop/remount and narrow layout.
See the repository browser manual for build and execution commands.

Verified October 13, 2026: 4 reducer tests / 69 focused tests pass; Chrome
155.0.8059.40 checks 18 states, actual detached-listener behavior, one render
callback after repeated start, the 2×2 desktop layout and stacked 360px layout.
Native and WASM Clippy deny warnings. `SelectionState::<0>::new()` rejects at
compile time with E0080, while positive layouts instantiate the same reducer.
All new source files stay below 500 lines. State transitions are O(1); the
presentation visits the fixed fixture collection and N viewport slots.

The native public API comparison passes 196 checks (57 inapplicable checks
skip). Both rustdoc JSON inputs compile from their respective canonical locks
with `cargo rustdoc --locked --offline -p metis-web --lib -- -Z unstable-options
--output-format json` and test-only `RUSTC_BOOTSTRAP=1`; cargo-semver-checks
compares those files using `--current-rustdoc`, `--baseline-rustdoc` and
`--release-type minor`. This keeps provider pins intact: the direct checker
performs `cargo update` and encounters upstream consus-raster 0.2.0 while the
manifests and canonical lock require 0.1. The library sources match main.

The broader candidate run passes 690 Rust tests with one nextest leak report
in unchanged invocation code. Main passes 686 tests without that report, so
its association is unresolved. Python passes 410/412 on macOS; both Windows
ABI-size failures reproduce on main. No full multi-OS gate pass is claimed.
