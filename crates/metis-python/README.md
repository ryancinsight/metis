# metis-python

The `metis-python` crate builds the `metis-rs` Python distribution. It exposes
the validated Metis Rust boundary through `import metis`; Python owns no
clinical arithmetic, safety policy or process state.

## Install

```bash
python -m pip install metis-rs
```

The wheels use the stable CPython 3.9 ABI. One wheel per supported operating
system serves CPython 3.9 and newer. The package includes `py.typed` and a
stub module for type checkers.

## Use

```python
from metis import (
    DrugConcentration,
    PatientWeight,
    TargetDose,
    calculate_infusion_rate,
)

result = calculate_infusion_rate(
    PatientWeight(60.0),
    DrugConcentration(2.0),
    TargetDose(0.2),
)

print(result.rate_ml_hr)       # 0.36
print(result.drug_rate_mg_hr)  # 0.72
```

Constructors validate their units and bounds in Rust. The calculation releases
the Python interpreter lock while the Rust backend evaluates the safety
envelope. Invalid values raise `ValueError` with the stable Metis error code
and trace identifier.

The binding also exposes the Rust-owned software presentation contract through
`RasterImage`, `Rect` and `Canvas`. Python supplies composition commands; Rust
validates dimensions, clips placements and performs alpha compositing. The
canvas is a bounded frame surface, not a native window or a DICOM decoder.
Native window and application lifecycle objects will be added only when their
Rust contracts and independent host evidence are available. RITK remains the
owner of DICOM parsing and medical-display semantics.

`Application(width, height)` provides the bounded cross-platform software
lifecycle. Read its `generation`, pass that token to `clear`, `to_rgba`, input
methods and `poll_event`, then call `close`. `reopen` allocates a fresh surface
and returns a new token; stale tokens fail with a typed `ValueError`. Events are
FIFO and bounded by the Rust platform queue. The object synchronizes access
with Rust locking and uses no Python callbacks or second event loop.

The PyPI release caller uses GitHub Actions OIDC Trusted Publishing. It stores
no PyPI token, signing key or developer private key in the repository.

## License

MIT or Apache-2.0, at your option.
