# Inspect application output

The current Metis demonstration has two independently useful checks: a real
frontend/backend process exchange, and a deterministic software rendering of the
initial form. It is not yet an interactive browser or desktop application.

## Run the checks

Follow [Build and run](getting-started.md), then run from the repository:

```text
python scripts/verify.py
```

The gate builds the pinned code, exercises native debug/release tests and the
process example, builds the portable WASM libraries, and compares the current
form capture with the committed gallery image. A passing WASM build does not run
a browser; a passing process test does not exercise clicking the painted button.

Open the [application gallery](applications.md) for the committed image, or
inspect `output/form.bmp` and `output/form.svg` produced by this run. The SVG
encodes the same raster pixels; it is not a second layout implementation.

## Read the current form

The initial 800×600 capture shows the title and green ready status, a patient
identifier and input labels, the blue submit graphic and an output panel awaiting
a result. All panels and required labels should fit inside the viewport. The
capture's defaults are 72.50 kg, 4.00 mg/mL and 0.500 mcg/kg/min.

Those are the initial presentation defaults, not the separate process example's
60 kg, 2 mg/mL and 0.2 mcg/kg/min arguments. That process example computes
0.36 mL/hour and 0.72 mg/hour. The existing image does not capture those results.
Neither example is treatment guidance.

For a visual change, inspect the actual output before accepting a new baseline:

```text
python scripts/verify.py --update-snapshots
python scripts/verify.py
```

Review the changed image and source together. A missing label, clipped control,
wrong value or stale result must be fixed in the application; accepting a new
snapshot does not make it correct. On a mismatch, the current gate reports the
snapshot difference; automated difference images and interaction-state captures
are not implemented yet.

## What a demonstration proves

A useful application demonstration pairs visible output with expected behavior:
inputs, actions, the resulting values, and the tested target. When browser/native
demonstrations become available, their gallery entries will identify the actual
host and include focus, editing, success, error and recovery states. Screen-reader
operation, permission denial and memory use require their own checks in addition
to images. Only runnable examples with captured output appear in this manual;
the [development scenario contract](../VERIFICATION.md#visual-scenarios) records
the remaining coverage.
