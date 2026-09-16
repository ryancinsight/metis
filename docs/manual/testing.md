# Inspect application output

The current Metis demonstration has five independently useful checks: a real
frontend/backend process exchange, deterministic software captures of a real
backend session through success, edit, rejection, correction, disconnect and
recovery, a browser workbench with Rust/WASM-driven HTML5/CSS controls, a
Windows-native adapter test that creates a real hidden HWND, and visible native
and packaged-WebView2 `metis-app` host workflows. The native checks present the
production framebuffer and drain bounded lifecycle events; the workbench can
connect to the documented one-shot loopback service for a real authenticated
calculation. The Windows platform suite also checks the WebView2 consumer's
packaged-URI configuration without requiring the installed runtime. The
committed host captures cover initial and successful submission states; the OS
permission boundary and remaining host journeys remain open.

## Run the checks

Follow [Build and run](getting-started.md), then run from the repository:

```text
python scripts/verify.py
```

The gate builds the pinned code, exercises native debug/release tests and the
process example, builds the portable WASM libraries including `metis-web`, and
validates the backlog/checklist plan references. It compares the current seven
form captures and their recorded inputs, actions, labels and geometry with the
committed gallery baseline. A passing WASM build
does not run a browser; use the browser workbench command below for that
runtime evidence.

## Inspect hosted verification

The single [Metis verification workflow](../../.github/workflows/ci.yml) runs the
same `python scripts/verify.py` gate on `windows-latest`, where the executable
and MSI workflow is implemented. It installs the pinned `cargo-nextest` and
`wasm-bindgen-cli` tools, runs the native and WASM targets, and uploads the
bounded report and stage logs even when a stage fails. Its scheduled and manual
Ubuntu job runs the standalone LibFuzzer parser campaign with the pinned
nightly toolchain and bounded time, RSS and per-input limits; a crash uploads
the reproducer directory. Atlas's pinned reusable workflow jobs check workflow
syntax, the standalone Cargo lock and the ADR index, while the Atlas SemVer job
reports public-API changes on ready pull requests. They do not claim a native
window, macOS/Linux host, accessibility technology or another unsupported
target.

For a failed run, download the `metis-verification-<run-id>` artifact and open
`output/verification.json` first. Its `status`, `revision`, source/lock hashes,
stage outcome and command budgets identify the exact failed gate; the matching
`output/<stage>.log` contains the bounded command diagnostic and
`output/visual/latest/report.json` describes semantic and pixel differences.
Repair the source or reviewed baseline, rerun `python scripts/verify.py` locally,
and push the fix. CI never refreshes snapshots, so a state, image or parser
regression remains a failing artifact until the implementation is corrected.

## Exercise parser and diagnostic safeguards

The protocol contract suite includes a bounded deterministic generator over
arbitrary byte strings, Unicode and IEEE-754 payload values, deterministic
truncation and bit mutations, and oversized length fields. These cases call
every public wire decoder and treat a panic as a failure. The standalone
LibFuzzer target covers
the same decoder boundary and the packaged SVG admission parser without
entering the application dependency graph;
its locked manifest is checked with:

```text
cargo check --manifest-path fuzz/Cargo.toml --locked
```

On a host with a working LibFuzzer toolchain, run the bounded campaign from the
`fuzz/` directory with the pinned command in [the fuzz harness README](../../fuzz/README.md).
The Windows MSVC environment used for the current local evidence cannot link
the sanitizer runtime; the scheduled Ubuntu job is the runtime campaign, while
the deterministic property and mutation suite remains the reproducible parser
oracle on Windows.

## Run bounded mutation analysis

The decoder mutation slice uses cargo-mutants 27.1.0 with the locked
`metis-ipc` contract tests. Install the pinned tool into the ignored local tool
directory, then run the committed bounded runner from the repository root:

```text
rustup run 1.97.0 cargo install cargo-mutants --version 27.1.0 --locked --root output/cargo-tools
python scripts/mutation.py
```

The runner requires the pinned toolchain and shared Cargo target directory,
uses nextest with two jobs, and enforces 30-second test, 120-second build and
300-second suite budgets. It records the exact revision, source hash, command
and outcome counts at `output/mutation/latest/manifest.json`; the output is
derived and ignored by Git. In managed environments, clear any `RUSTC` or
`RUSTDOC` overrides before invoking the runner so they cannot replace the
workspace toolchain.

The slice mutates `metis-core` decoder code and runs the integration tests from
`metis-ipc`; the cross-package `--test-package metis-ipc` selection is required
to exercise those tests. A score is computed over viable mutants only, while
unviable, missed and timed-out counts remain visible in the manifest. See the
[cargo-mutants nextest](https://mutants.rs/nextest.html) and
[workspace test-package](https://mutants.rs/workspaces.html) documentation for
the underlying selection contract. The Windows host still needs a separate
nightly LibFuzzer run with a working sanitizer runtime.

`MetisError` retains its full message for `Display`, while `Debug` and
`MetisError::redacted()` expose only the stable error code and trace identifier.
The remote `ErrorResponsePayload` follows the same rule for `Debug`, replacing
the message with `[REDACTED]`. This keeps paths, identifiers and other
untrusted values out of structured diagnostics without changing the user-facing
error text.

## Check the browser transport slice

The IPC package now has a browser-thread contract backed by Moirai's bounded
WebSocket reactor and one-shot deadline timer. Check the exact public graph with:

```text
cargo check --locked -p metis-ipc --target wasm32-unknown-unknown
cargo clippy --locked -p metis-ipc --target wasm32-unknown-unknown -- -D warnings
cargo nextest run --locked -p metis-ipc
```

These commands prove that frame bounds, one-pump ordered and out-of-order
request correlation, the native async server and the native async-client tests
compile against one pinned Moirai revision. They do not open a browser or
produce a browser snapshot. The Windows-native adapter test is included in the
`metis-platform` nextest package run; V05 and V12 remain the acceptance checks
for cross-engine execution, post-drop resource measurements and the remaining
desktop lifecycle evidence; their gallery entries must identify the actual
engine, host and revision.

The browser canvas provider keeps its validated bitmap dimensions when a
subsequent RGBA frame has the same extent and assigns new dimensions only when
the extent changes. Metis pins the direct Moirai packages to merge
`21b66ba424ad8f50d8574d6e9714be696f807e82` for this contract. The upload still
borrows the frame for the call and remains bounded by the same pixel and byte
limits. `CanvasSurface` also exposes explicit asynchronous WebGPU constructors;
missing adapter or device setup is returned as an unsupported/setup error and
never selects the raster path implicitly. These contracts are lifecycle and
capability evidence, not WASM-used-memory, browser-heap, process-memory or
real-GPU visual measurements. Those values require the V12 instrument and a
controlled host.

## Run the browser workbench

Build and serve the actual generated WASM loader and HTML/CSS shell:

```text
python scripts/browser.py build
python -m http.server 8080 --directory output/browser
```

Open `http://127.0.0.1:8080/`. Change weight and dose to see the Rust-owned
values update, enter a non-numeric value to observe `ERR_NUMERIC_INSTABILITY`,
then submit to observe the typed `ERR_CONNECTION_CLOSED` result. Start the
loopback command in [the browser manual](browser.md), reopen the page with its
endpoint, process and principal query values, and submit to exercise the real
handshake and calculation. The recorded trace covers authorized success,
numeric rejection, service disconnect, stop/remount and recovery. Use
**Stop host** to cancel the browser task, remove the form and listener guards,
then **Start host** to remount fresh controls. This is runtime evidence for the
HTML5/CSS host, Moirai transport and pre-response Origin check; it does not
prove TLS, cross-engine behavior, post-drop allocation bounds, accessibility
technology support or OS isolation.

## Validate saved-study slice controls

Exercise the range inputs that an RITK consumer places beneath the three
anatomical canvases with the file-backed runner:

```text
python scripts/browser_drop.py --driver-url http://127.0.0.1:9517 \
  --browser-name MicrosoftEdge --headless --input chooser \
  --files D:/atlas/repos/ritk/test_data/2_head_mri_t2/DICOM --pattern '*.dcm' \
  --oracle output/browser/mri-oracle.json \
  --consumer-revision <ritk-revision> \
  --canvas-trace output/browser/cine/canvas-trace.json \
  --keyboard-trace cine-rate --slice-controls \
  --output output/browser/cine
```

The bounded trace drives click, Home, End, arrow and pointer actions for the
axial, coronal and sagittal ranges. It rejects non-finite, fractional and
out-of-range indices, requires a generation-backed repaint, preserves the
other planes, restores the initial RGBA frame and records released listeners.
The RITK [DICOM workflow manual](https://github.com/ryancinsight/ritk/blob/main/docs/manual/dicom-workflow.md)
contains the real MRI screenshots and pixel provenance; Metis supplies the
host controls and input lifecycle only.

## Check authentication provider ownership

Metis uses Moirai's standalone RustCrypto primitives for capability MACs, audit
hashes, result signatures and CLI package digests. Verify the protocol-only
dependency graph with:

```text
cargo tree --locked -p metis-core --edges normal
cargo check --locked -p metis-core --target wasm32-unknown-unknown
```

The graph contains `moirai-crypto` with only its `hmac` and `sha2` normal
dependencies; `rustls`, key exchange, AEAD and certificate crates stay behind
the disabled provider feature. Metis retains CRC-32 for accidental frame
corruption. Capability and audit tests exercise the same provider functions at
their real wire boundaries. These checks establish ownership and build
closure, not universal side-channel timing or Tauri security superiority.

Open the [application gallery](applications.md) for the committed image, or
inspect `output/form*.bmp` and `output/form*.svg` produced by this run. The SVG
encodes the same raster pixels; it is not a second layout implementation.

## Read the current form

The initial 800×600 capture shows the title and green ready status, a patient
identifier and input labels, the blue submit graphic and an output panel awaiting
a result. All panels and required labels should fit inside the viewport. The
capture's defaults are 72.50 kg, 4.00 mg/mL and 0.500 mcg/kg/min.

Those are the initial presentation defaults, not the separate process example's
60 kg, 2 mg/mL and 0.2 mcg/kg/min arguments. That process example computes
0.36 mL/hour and 0.72 mg/hour. The success and recovery images now capture actual backend results on a bounded
memory transport. They are distinct from the separate-process check.
Neither example is treatment guidance.

For a visual change, inspect the actual output before accepting a new baseline:

```text
python scripts/verify.py --update-snapshots
python scripts/verify.py
```

Review the changed image and source together. A missing label, clipped control,
wrong value or stale result must be fixed in the application; accepting a new
snapshot does not make it correct. Read `output/visual/latest/report.json` for
each scenario's semantic differences, changed pixel count and difference bounds.
Its neighboring `*-expected.svg`, `*-actual.svg` and `*-difference.svg` files
show the compared images and a red-on-black mask of changed pixels.
`manifest.json` exists only for an accepted visual run. The next gate run rotates
these known artifacts to `output/visual/previous/`, retaining one previous run.

Three deliberately changed renders test detection of text, position and color
regressions. Their difference images are test evidence, not application states.
The software gallery exercises production state transitions through API calls;
the browser workbench exercises editable controls, focus, real service
requests, explicit disconnects and task/listener teardown. Browser
accessibility technology, installed IME, native accessibility and OS permission
journeys remain open; native input and visible form capture are covered by the
Windows evidence in [native.md](native.md#captured-windows-workflows).

## Measure a real application lifecycle

`scripts/resource.py` samples the process launched by the caller and all visible
descendants. On Windows it reads the working set, committed private bytes and
handle count through the operating-system process APIs; on systems without
those counters the report keeps the affected fields explicitly unavailable.
The runner records startup observation, duration, initial/final/peak values and
growth at a fixed interval. It sends child output to the system sink and hashes
the exact command, so a local study path or identifier is not copied into the
report. Pass `--capture <path>` when the application writes an image or other
inspectable artifact. The runner hashes that regular file after every repeated
run and records only its byte count and SHA-256; it never copies the path or
artifact bytes into the report. A requested capture that is missing, redirected,
or multiply linked makes the run fail, so an exit code alone cannot present a
stale or absent image as evidence. Repeated runs report whether all observed
capture digests match.

Use it with the saved-study workflow in [applications.md](applications.md):

```powershell
$study = 'C:\path\to\saved-study'
$series = Read-Host 'SeriesInstanceUID shown by the RITK series browser'
$ritkRevision = (git -C D:\atlas\repos\ritk rev-parse HEAD).Trim()
$capture = Join-Path $env:TEMP 'ritk-metis-resource-capture.png'
$report = Join-Path $env:TEMP 'ritk-metis-resource.json'
python D:\atlas\repos\metis\scripts\resource.py `
  --output $report `
  --label 'RITK saved study through Metis' `
  --revision $ritkRevision `
  --phase lifecycle `
  --sample-ms 100 `
  --timeout-seconds 120 `
  --repeat 3 `
  --capture $capture `
  -- `
  D:\atlas\target\debug\ritk-snap.exe $study `
  --series-instance-uid $series `
  --metis-native `
  --capture-application `
  --capture $capture
Get-Content $report
```

The command measures the real RITK decoder and Métis presentation process until
the capture completes; it does not substitute a synthetic image or a mock
process. `--capture` binds each resource observation to the file emitted by the
application, while keeping private study paths and pixels out of the report.
`--repeat` runs the same fixture sequentially and adds the measured
mean, sample standard deviation and explicitly approximate 95% half-width to
the `aggregate` object; the per-run records remain available for inspection.
The product of repeat count and per-run timeout is bounded at 300 seconds.
Keep the report and capture local when the study is private. For a comparison,
run the same command shape, source asset, window and host protocol for each
fixture and compare reports only after recording the machine, target, engine
and revision. A current public CT eframe baseline is recorded in
[the eframe resource provenance](images/dicom-eframe-real-ct-resource.json);
the native Métis MIP run is recorded in
[its resource provenance](images/dicom-metis-real-ct-mip-resource.json).
Both use the same 409-file input, but eframe's fourth viewport is `3d_mip`
while the Métis run uses RITK's `axial_mip` policy; their surface dimensions
and process boundaries also differ. These reports are lifecycle evidence; they
do not establish a memory or latency ranking against Tauri, GPUI or egui. The
three-run saved public MRI baseline was refreshed on
2026-09-14 against the current RITK/Métis/Moirai revisions; its repeated
capture digest and process-tree uncertainty are recorded in [the MRI resource
provenance](images/dicom-metis-real-mri-resource.json). The report records
the real 94-file study and the 1280 × 800 three-plane capture; it does not
include private study paths or pixels.

The [application gallery's V12 table](applications.md#v12-fixture-comparison)
keeps those three measured fixtures together with their uncertainty and the
unmatched GPUI/Tauri residual.

### Enforce matched lifecycle comparisons

Use `scripts/resource_compare.py` when two real provenance records are ready
for a comparison. The command requires explicit dotted JSON fields whose
values must be identical in both records; it also requires at least two
repeated samples for every selected resource metric. It hashes both input
records into the result without copying their paths, and reports right-minus-
left deltas with the combined approximate 95% half-width:

```powershell
python scripts/resource_compare.py `
  --left docs/manual/images/dicom-metis-real-ct-mip-resource.json `
  --right docs/manual/images/dicom-eframe-real-ct-resource.json `
  --left-name 'Métis' `
  --right-name 'eframe baseline' `
  --match runtime.phase `
  --match dataset.series_instance_uid `
  --match dataset.files_submitted `
  --match dataset.bytes_read `
  --metric peak_private_bytes `
  --metric peak_working_set_bytes `
  --metric duration_ms `
  --output output/resource-comparison.json
```

Add a producer-owned `output` semantic key (for example,
`output.semantic_surfaces`) to both records and pass it with `--match` before
interpreting presentation or clinical equivalence. A missing or differing key
fails before any metric is calculated. The output is a measurement record,
not a framework ranking; it does not normalize panel names, infer semantic
equivalence, or replace RITK's image and DICOM oracles.

For the browser side of the same comparison, the paired canvas trace records
`metrics.frame_intervals` around the real RITK canvases. Each bounded sample is
the interval between eight `requestAnimationFrame` callbacks before and after
the trusted pointer/wheel actions. The trace reports the mean, population
spread, minimum and maximum in milliseconds and fails on malformed or missing
samples. These values describe the browser frame boundary only; compositor,
GPU and native-window latency require their own host instrument. Keep the
browser engine, driver, viewport, study, revisions and action trace fixed
before comparing measurements.

Add `--browser-heap-sample` to the same runner command to append
`metrics.browser_heap`. The optional observation uses the browser's
`performance.memory` counters when exposed and records an explicit unavailable
observation otherwise. It validates `used_js_heap_bytes <=
total_js_heap_bytes <= js_heap_limit_bytes` and does not stand in for WASM
linear memory, native process memory or an allocation profiler.

Add `--browser-memory-sample` to append `metrics.browser_memory`. The runner
uses [`performance.measureUserAgentSpecificMemory()`](https://developer.mozilla.org/en-US/docs/Web/API/Performance/measureUserAgentSpecificMemory)
only when the document is secure and cross-origin isolated. It records one
bounded `estimated_bytes` value or an explicit unavailable/rejected/timeout
reason. The browser reports an implementation-dependent aggregate estimate;
values are not comparable across engines or browser versions and do not count
WASM allocations, native process memory, compositor work or GPU memory. Use
the option with the file-backed gallery command as well as the generic canvas
runner when the host exposes the API.

For lifecycle-growth observations, add `--lifecycle-cycles N` to the workbench
runner, where `N` is bounded to 1 through 8. The trace records the semantic
state for every stop/remount cycle under `metrics.lifecycle_cycles` and reports
the total in `cleanup.lifecycle_cycles`. Each stopped state must have no
mounted controls or Rust-owned listener handles; each remounted state must have
a newer generation and positive listener count. The instrument records the
component contract and keeps later remount heap samples separate from native,
WASM, compositor and GPU measurements. A configured WebDriver endpoint is
required for live evidence; the dependency-free suite covers the bounded
records and failure paths.

## What a demonstration proves

A useful application demonstration pairs visible output with expected behavior:
inputs, actions, the resulting values, and the tested target. The browser
service trace identifies the actual host and includes focus, editing, success,
error, disconnect and recovery states. Screen-reader operation, permission
denial and memory use require their own checks in addition to images. Only
runnable examples with captured output appear in this manual;
the [development scenario contract](../VERIFICATION.md#visual-scenarios) records
the remaining coverage.
