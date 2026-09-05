# Application gallery

## Form and isolated calculation backend

![Metis form rendered from its application markup](images/form.svg)

This is the actual 800×600 software framebuffer produced by the
[presentation example](../../examples/presentation.rs), showing the initial form
before a backend request. It is not a native-window screenshot. The fields,
status label and submit graphic come from the application's markup and bitmap
renderer; they are not a separate design mockup.

The [process example](../../examples/clinical_infusion_workflow.rs) runs the same
frontend with a separate backend and checks process completion. For arguments
`60 2 0.2`, the observable result is 0.36 mL/hour and 0.72 mg/hour. The
[executable integration tests](../../crates/metis-backend/tests/process.rs)
also change the weight to verify that results depend on submitted values.

This is the implemented application example today. Further applications belong
in this gallery when they have runnable source and captured output.

## Regenerate a snapshot

```text
python scripts/verify.py --update-snapshots
```

The Rust example writes `output/form.bmp` for pixel inspection and `output/form.svg`
for browser display. SVG paths encode horizontal runs of the actual framebuffer
pixels; no SVG text layout substitutes for the application renderer. The gate
copies the generated image here only with the explicit update option. A normal
gate compares bytes and fails if this tracked snapshot has drifted.
