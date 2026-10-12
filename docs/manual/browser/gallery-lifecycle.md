# Browser gallery lifecycle

[Browser manual](../browser.md)


### Replay a saved MRI study with adaptive canvas input

The file-backed gallery runner was replayed against the public 94-file
MRI-DIR T2 study on 2026-09-13. At Metis revision
`abd6578caffb92ad4ab9011507cdd89fc0c2c7e2`, RITK revision
`60044428572461ff94cc0396a1960e9cdc78304a` and Moirai revision
`fd3ec288dd0f1e98912f1d439d4ff9129ca7e25a`, Microsoft Edge
154.0.4258.12 (msedgedriver 153.0.4234.19) accepted all 94 saved files and
read 49,807,236 bytes. RITK presented non-black axial, coronal and sagittal
planes at 512 × 512, 512 × 94 and 512 × 94 pixels. The browser viewport and
the three element captures are the actual decoded study; the canonical public
[gallery image](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-edge-gallery.png?raw=true)
and [provenance record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-edge.json)
remain owned by RITK.

The adaptive trace dispatched one trusted pointer drag and wheel action to each
canvas. Axial moved from slice 47 to 46 of 94 with offsets `(24, 24)` and
`(64, 48)`; coronal and sagittal moved from slice 256 to 255 of 512 with
offsets `(24, 24)` and `(64, 40)`. The `y = 40` value is derived from the
82.4 CSS-pixel height of those short canvases, avoiding the previous out of
bounds `y = 48` point. The RITK validator accepted all six semantic snapshots,
six actions and eight screenshots. Twelve diagnostic listeners were released,
all input sources were released and the WebDriver session closed.

Reproduce the run from the Metis checkout with a configured Edge driver:

```powershell
$consumerRevision = (git -C D:/atlas/repos/ritk rev-parse HEAD)
python scripts/browser_drop.py --driver-url http://127.0.0.1:9516 `
  --browser-name MicrosoftEdge --input chromium `
  --files D:/atlas/repos/ritk/test_data/2_head_mri_t2/DICOM --pattern '*.dcm' `
  --oracle output/browser/mri-oracle.json `
  --consumer-revision $consumerRevision `
  --canvas-trace output/browser/drop-mri-edge-canvas-final/canvas-trace.json `
  --canvas-attribute data-ritk-load-state `
  --canvas-attribute data-ritk-frame-state `
  --canvas-attribute data-ritk-axis `
  --canvas-attribute data-ritk-slice-index `
  --canvas-attribute data-ritk-slice-count `
  --canvas-attribute data-ritk-frame-width `
  --canvas-attribute data-ritk-frame-height `
  --output output/browser/drop-mri-edge-canvas-final
```

Validate the resulting trace from the RITK checkout:

```powershell
ritk-snap.exe --validate-browser-trace `
  D:/atlas/repos/metis/output/browser/drop-mri-edge-canvas-final/canvas-trace.json `
  --canvas-id ritk-snap-axial --canvas-id ritk-snap-coronal `
  --canvas-id ritk-snap-sagittal
```

This evidence establishes the saved-study handoff, decoded pixels, trusted
canvas input and bounded teardown in one Chromium-family engine. Physical
file-manager input, Firefox/WebKit, WebGPU and provider-private resource
counts remain separate acceptance gates. Private clinical studies use the
local saved-study command in the [application gallery](../applications.md) and
are never copied into Metis evidence.

### Consumer-owned gallery controls

Metis supplies the generic file handoff, canvas contract and input lifecycle.
Consumer controls and their semantic assertions remain with the consumer. The
RITK [DICOM workflow manual](https://github.com/ryancinsight/ritk/blob/main/docs/manual/dicom-workflow.md)
contains the saved-study slice-control command, actual MRI screenshots and
pixel provenance; RITK's `scripts/browser_gallery.py` invokes the generic
Metis runner with its consumer-owned hook. The runner completes its generic
bounded file-rejection probes and post-rejection frame-stability comparison
before invoking that hook. This ordering keeps the host file picker available
while a consumer is still free to stop or remount the host; the hook then owns
consumer controls and teardown evidence.

### Read browser frame timing

The same canvas trace records a bounded `requestAnimationFrame` interval sample
for each named canvas before and after its pointer and wheel actions. The
measurements are under `metrics.frame_intervals` in the schema-1 JSON trace;
each entry contains seven intervals from eight callbacks plus the mean,
population standard deviation, minimum and maximum in milliseconds. This is a
browser frame-boundary measurement for the loaded application. It does not
measure operating-system compositor latency, GPU submission or native-window
latency, so those values stay separate in the V12 comparison protocol.

The runner rejects missing animation-frame support, non-finite or non-positive
intervals, samples outside the 32-callback bound and a frame interval above
the 120-second observation bound. A failed sample fails the trace instead of
being replaced with a default. The native Python suite covers both the
value-semantic record and a non-monotonic timestamp failure. Run the paired
gallery command above and inspect `metrics.frame_intervals` before comparing
fixtures; keep the engine, driver, viewport, application revision and input
trace pinned.

### Read browser JavaScript-heap counters

Pass `--browser-heap-sample` to the canvas or workbench runner when the host
should record the browser's diagnostic JavaScript heap counters. Samples are
stored under `metrics.browser_heap` with the label, used bytes, total bytes and
heap limit. The runner validates finite nonnegative values, the ordering
`used <= total <= limit`, and a one-terabyte observation bound.

The source is Chromium's non-standard `performance.memory` surface. Firefox
and WebKit may report an explicit `available: false` observation, which is
evidence of an unavailable API rather than zero memory. These counters describe
the JavaScript heap only; they do not measure WebAssembly linear memory,
native process memory, allocations, compositor or GPU latency. Keep them
separate in the V12 comparison and do not use them for a universal engine
ranking.

### Read browser-estimated aggregate memory

Pass `--browser-memory-sample` to the workbench, canvas or file-backed gallery
runner when the host can provide the standard
[`performance.measureUserAgentSpecificMemory()`](https://developer.mozilla.org/en-US/docs/Web/API/Performance/measureUserAgentSpecificMemory)
surface. The trace stores samples under `metrics.browser_memory`, using the
label, `estimated_bytes` and source when the call succeeds. The runner requires
a secure, cross-origin-isolated document and applies a five-second observation
deadline; unsupported, rejected and timed-out calls remain explicit records.

The API estimates aggregate memory for the user agent and its value is
implementation-dependent. It is therefore not a WASM allocator count, native
process measurement or allocation profile, and it must not be compared across
engines or browser versions. The option is an additional V12 observation; the
RITK gallery still supplies the DICOM byte and pixel oracles.

```text
python scripts/browser_drop.py --driver-url http://127.0.0.1:9517 --engine chromium --browser-name MicrosoftEdge --files path\to\study --oracle output\browser\consumer\oracle.json --consumer-revision <ritk-revision> --canvas-trace output\browser\drop\canvas.json --input chooser --browser-memory-sample
```

### Repeat the workbench lifecycle

The workbench runner can repeat its stop/remount sequence with a bounded
`--lifecycle-cycles` value from 1 through 8:

```text
python scripts/browser_runtime.py --engine chromium --browser-name MicrosoftEdge --driver-url http://127.0.0.1:9517 --serve-dir output/browser --bridge disconnected --browser-heap-sample --lifecycle-cycles 4
```

Every cycle stops the Rust-owned form, requires zero mounted controls and zero
listener handles, then remounts a new generation with positive controls and
listeners. The trace stores one compact record per cycle under
`metrics.lifecycle_cycles`; each record includes the stopped and remounted
generation, listener count and mounted-control count. Optional heap samples
for later remounts use labels such as `remounted-cycle-2`. Repeated cycles do
not create repeated screenshots, so the trace remains within its 512 KiB
artifact bound. A live Edge 154.0.4258.12 run with msedgedriver
153.0.4234.19 at Metis revision `012ed7175268c283cb4b8cbd617f1a138d955532`
completed all four cycles. Stopped generations 3, 5, 7 and 9 each reported
zero controls and zero listener handles; remounted generations 4, 6, 8 and 10
each reported 27 controls and 31 listener handles. The [revision-bound trace](../images/metis-browser-edge-lifecycle.json)
and the five inspected PNGs above are the visual and semantic evidence for
this run. This remains lifecycle and JavaScript-heap evidence only; it does
not measure native allocations, WebAssembly used memory, compositor or GPU
latency.

### Repeat the saved-study gallery lifecycle

Repeated mode accepts 4–16 same-page cycles with a 300-second suite limit and
reserved cleanup time. It requires `--canvas-trace` and `--keyboard-trace cine-rate`, plus chooser or
Chromium file-backed input. The reproduction command,
source and asset hashes, per-cycle trace hashes, and sample series are in the
[provenance record](../images/metis-browser-mri-growth.json).

On 2026-09-23, Microsoft Edge 154.0.4258.32 loaded the real 94-file MRI-DIR T2
study (49,807,236 bytes) through the standard file chooser at 125% device scale.
The 16 cycles ran in one page; two warmups were excluded, leaving 14 samples per
phase. `WebAssembly.Memory.buffer.byteLength` was 1,835,008 bytes at the first
mount, grew to 404,357,120 bytes at the first transfer, then stayed at
404,357,120 bytes through all later mount, transfer, decode, cine and stop
phases. Every post-warmup phase has 14/14 identical capacity observations.

| Phase | WASM buffer capacity | JavaScript heap mean ± approximate 95% half-width | Observed heap range |
| --- | ---: | ---: | ---: |
| mounted | 385.625 MiB | 56.35 ± 21.31 MiB | 5.87–103.13 MiB |
| transfer | 385.625 MiB | 84.86 ± 10.39 MiB | 57.54–102.76 MiB |
| decoded | 385.625 MiB | 66.66 ± 19.53 MiB | 6.13–104.82 MiB |
| cine | 385.625 MiB | 51.70 ± 22.09 MiB | 5.87–103.04 MiB |
| stopped | 385.625 MiB | 51.74 ± 22.09 MiB | 5.87–103.13 MiB |

The JavaScript values come from Chromium's `performance.memory`. The half-width
uses `1.96 × sample standard deviation / √n`; garbage collection and serial
correlation are not modeled. The large overlapping ranges do not show stable
heap use or prove leak absence, and neither metric reports allocator live bytes.

The Metis lifecycle runner passed all 16 real cycles, the saved-study file and
canvas oracles, and stop cleanup (zero listener and transfer-observer guards).
RITK's stricter trace validator passed 9/16 per-cycle traces. The other seven
passed action, attribute and frame/slice progression before failing the exact
repeated element-screenshot hash check; the validator's later consumer-cleanup
assertion did not run for them. Independently, the Metis runner completed all
16 cycles with zero stopped listener and transfer-observer guards. Across 96 repeated screenshot
pairs, 25 differed by one intensity level in 1–28 pixels; the raw 512×512 and
512×94 RGBA canvas oracles remained exact. No tolerance changed. This leaves an
unresolved screenshot-capture stability limit at 125% scale; the cause is not
established.

The record pins the actual browser bundle and the reported RITK source revision,
but it cannot establish that those served JavaScript and WASM assets were built
from that source revision. DICOM parsing and image correctness remain RITK-owned.
The earlier [RITK four-cycle provenance](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-memory.json)
remains a separate cross-engine baseline.

[Input Events]: https://w3c.github.io/input-events/
