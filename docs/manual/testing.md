# Inspect application output

The current Metis demonstration has four independently useful checks: a real
frontend/backend process exchange, deterministic software captures of a real
backend session through success, edit, rejection, correction, disconnect and
recovery, a browser workbench with Rust/WASM-driven HTML5/CSS controls, and a
Windows-native adapter test that creates a real hidden HWND, presents the
production framebuffer and drains its resize/close lifecycle. The workbench can
connect to the documented one-shot loopback service for a real authenticated
calculation; the adapter does not establish a visible application host or an OS
permission boundary.

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
bounded report and stage logs even when a stage fails. Atlas's pinned reusable
workflow jobs check workflow syntax, the standalone Cargo lock and the ADR
index, while the Atlas SemVer job reports public-API changes on ready pull
requests. They do not claim a native window, macOS/Linux host, accessibility
technology or another unsupported target.

For a failed run, download the `metis-verification-<run-id>` artifact and open
`output/verification.json` first. Its `status`, `revision`, source/lock hashes,
stage outcome and command budgets identify the exact failed gate; the matching
`output/<stage>.log` contains the bounded command diagnostic and
`output/visual/latest/report.json` describes semantic and pixel differences.
Repair the source or reviewed baseline, rerun `python scripts/verify.py` locally,
and push the fix. CI never refreshes snapshots, so a state, image or parser
regression remains a failing artifact until the implementation is corrected.

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
requests, explicit disconnects and task/listener teardown. Browser accessibility
technology, IME and native input capture remain unimplemented.

## What a demonstration proves

A useful application demonstration pairs visible output with expected behavior:
inputs, actions, the resulting values, and the tested target. The browser
service trace identifies the actual host and includes focus, editing, success,
error, disconnect and recovery states. Screen-reader operation, permission
denial and memory use require their own checks in addition to images. Only
runnable examples with captured output appear in this manual;
the [development scenario contract](../VERIFICATION.md#visual-scenarios) records
the remaining coverage.
