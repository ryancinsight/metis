# Python binding

Metis exposes a typed Python package over the same Rust validation and backend
calculation used by the application. The native extension is `metis._metis`;
the installable distribution is `metis-rs` because the existing
[PyPI `metis` project](https://pypi.org/project/metis/) is an unrelated
package. Python code owns composition and presentation of values. Rust owns
units, bounds, arithmetic and the safety envelope.

## Build and exercise locally

Install the pinned build tools in an active Python environment:

```powershell
python -m pip install "maturin==1.14.1" "pytest==8.4.2"
python scripts/python_binding.py
```

The script builds a locked release wheel from
`crates/metis-python/pyproject.toml`, extracts it into a temporary directory
and runs the tests in `crates/metis-python/tests`. It does not install into the
active interpreter. The wheel uses the CPython 3.9 stable ABI and includes the
`py.typed` marker and `_metis.pyi` stub.

## Calculate through Rust

```python
import metis

result = metis.calculate_infusion_rate(
    metis.PatientWeight(60.0),
    metis.DrugConcentration(2.0),
    metis.TargetDose(0.2),
)
print(result.rate_ml_hr)      # 0.36
print(result.drug_rate_mg_hr) # 0.72
```

The constructors reject non-finite and out-of-range values before the backend
runs. A custom envelope is explicit:

```python
envelope = metis.SafetyEnvelope(
    adult_rate_ml_hr=300.0,
    pediatric_rate_ml_hr=50.0,
    pediatric_weight_threshold_kg=35.0,
)
result = metis.calculate_infusion_rate(
    metis.PatientWeight(25.0),
    metis.DrugConcentration(1.5),
    metis.TargetDose(0.5),
    envelope,
)
assert result.is_pediatric
```

## Compose a Rust-owned image frame

The wheel exposes the same bounded software image contract used by the Metis
presentation path. Pixels are row-major RGBA bytes; crops and destinations are
validated in Rust, and destinations may be clipped by the canvas.

```python
import metis

image = metis.RasterImage(
    2,
    1,
    bytes((229, 62, 62, 255, 49, 130, 206, 255)),
)
canvas = metis.Canvas(3, 2)
canvas.clear(255, 255, 255, 255)
canvas.draw_image(
    image,
    metis.Rect(0, 0, 2, 1),
    metis.Rect(-1, 0, 4, 2),
)
assert canvas.to_rgba()[:4] == bytes((229, 62, 62, 255))
```

`Canvas.to_rgba()` returns a cold-boundary copy so Python code can hand the
frame to another renderer without sharing Rust storage. The inspected
software-renderer fixture shows the same placement and alpha semantics in the
[image presentation demonstration](applications.md#raster-image-presentation).
This surface does not open a native window, run a second event loop or decode
DICOM bytes; those capabilities stay with the Metis host and RITK contracts.

`ValueError` messages retain the stable Metis error code and trace identifier,
so a caller can distinguish invalid input from a rate interlock without
reimplementing Rust's error taxonomy. The calculation releases the Python
interpreter lock while Rust computes, allowing unrelated Python threads to
progress.

## Release path

`.github/workflows/python-release.yml` accepts a GitHub Release tag of the form
`metis-python-v<version>`. Atlas builds and tests the wheel on the supported
systems, attaches the artifacts and emits a source distribution. The publish
job requests only GitHub's OIDC identity and uses PyPI Trusted Publishing; no
PyPI API token, SSH key or developer private key is stored in the repository.
The `pypi` environment and the `metis-rs` trusted publisher are administrative
release prerequisites. A local build or a workflow dispatch does not publish.

This binding increment does not expose native window classes or DICOM objects.
Those surfaces will follow their public Rust contracts and host or decoder
evidence, so the Python API cannot silently diverge from the desktop and web
paths.
