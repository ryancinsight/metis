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

This first binding is deliberately bounded to the existing clinical contract.
Native window, application lifecycle and DICOM objects will be added only when
their Rust contracts and independent host or decoder evidence are available.

The PyPI release caller uses GitHub Actions OIDC Trusted Publishing. It stores
no PyPI token, signing key or developer private key in the repository.

## License

MIT or Apache-2.0, at your option.
