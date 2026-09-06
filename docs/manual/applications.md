# Application gallery

These are captures of the production Metis form and its real backend responses,
rendered into an 800×600 software framebuffer through Iris. The runnable
[presentation example](../../examples/presentation.rs) uses bounded IPC and a
Moirai worker, asserts values and audit outcomes, and checks text geometry before
capturing each state. It does not create native controls or execute in a browser.
The separate [process demonstration](getting-started.md) tests actual child processes.

## Initial form

![Initial form](images/form.svg)

The defaults are 72.50 kg, 4.00 mg/mL and 0.500 mcg/kg/min. No result exists yet.

## Successful request

![Backend result](images/form-success.svg)

The example establishes a session and submits patient `demo`, weight 60 kg,
concentration 2 mg/mL and dose 0.2 mcg/kg/min. The backend returns 0.72 mg/hour
and 0.36 mL/hour at audit sequence 2. The MAC is present but not verified by the
frontend. These synthetic values demonstrate the protocol, not treatment guidance.

## Edit invalidates the result

![Edited form awaiting submission](images/form-edited.svg)

Changing weight to 80 kg immediately clears the old rate and MAC. No request has
been sent for these edited inputs; the form displays idle.

## Rejection and correction

![Backend rejects zero weight](images/form-rejected.svg)

The next request uses zero weight. The backend rejects it with code `0x3001` at
audit sequence 3. No prior result remains on screen. Full diagnostics are available
in the typed `FormState::Rejected` value.

![Corrected inputs produce a result](images/form-corrected.svg)

Restoring weight to 60 kg succeeds on the same session at audit sequence 4.

## Disconnection and reconnection

![Closed connection clears the result](images/form-disconnected.svg)

The example joins the finite worker, which closes the real peer endpoint. A
subsequent submission reports `0x4002`, clears the result and shows a closed
session. Synchronization uses worker completion, not a timed sleep.

![New session produces the changed result](images/form-recovered.svg)

A new app and backend session submit weight 80 kg and return 0.96 mg/hour and
0.48 mL/hour. Reconnection never silently retries an uncertain transaction.

## Reproduce and review

```text
python scripts/verify.py
```

The Rust example writes fixed `output/form*.bmp`, `output/form*.svg` and
`output/form*.csv` files. SVG paths encode the actual raster; they do not rebuild
layout as SVG text. The CSV records actual inputs, actions, state, displayed
labels and text geometry alongside independently expected outcomes.

The gate removes previous required captures before execution. It validates all
seven new captures, checks BMP/SVG pixel agreement and compares both images and
semantic records with the [reviewed baseline](images/captures.json). Source,
lockfile and rendering-fixture hashes bind the observations to this run.
For an intentional visual change:

```text
python scripts/verify.py --update-snapshots
python scripts/verify.py
```

Inspect the generated images and semantic records before accepting the baseline.
`output/visual/latest/report.json` records each comparison; adjacent expected,
actual and difference images make failures inspectable. See
[Inspect application output](testing.md) for report interpretation and retention.
`output/verification.json` records the complete gate, including failures before
capture. Browser/OS capture and responsive pending/cancellation remain in the
[visual scenario contract](../VERIFICATION.md#visual-contract).

## DICOM viewer migration baseline

RITK's [synthetic DICOM workflow](../../../ritk/docs/manual/dicom-workflow.md)
now includes a capture of the running egui/eframe viewer alongside exact
software slice images, delivered in
[RITK PR 236](https://github.com/ryancinsight/ritk/pull/236) at `4a9f6eb1`.
Its three-instance study has known decoded values,
anisotropic spacing and physical coordinates; no patient data is required.
The native workflow also requires a missing study to fail without producing a
successful-load screenshot.

The baseline exercises explicit series selection, primary and secondary loads,
authoritative DICOMDIR membership, failed replacement and session restore.
The manual explains the current input limits and how to reproduce both the
pixel checks and native capture. These results establish the existing viewer
baseline for [V09](../VERIFICATION.md#V09); Métis host execution, browser input,
multiframe/color presentation and matched memory measurements remain required
before accepting the migration.
