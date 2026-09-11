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

The native module declares `gil_used = false` after a compile-time `Send + Sync`
audit of every exposed Rust class. The test suite exercises concurrent mutation
of one `Application` and checks `sys._is_gil_enabled()` when a free-threaded
interpreter is running. The release contains separate CPython 3.9 `abi3` wheels,
version-specific `cp314-cp314t` and `cp315-cp315t` free-threaded wheels, and Python 3.15
`abi3t` wheels. The free-threaded artifacts use the package `abi3t` feature,
which selects PyO3 `abi3t-py315`; the regular artifact keeps the default `abi3`
feature. The same installed-wheel value tests run for both ABI families. The
`abi3t` stable ABI is supported on the platforms provided by Atlas; musllinux
is excluded from the `abi3t` job until a compatible Python 3.15t image exists.

The binding also exposes the Rust-owned software presentation contract through
`RasterImage`, `Rect` and `Canvas`. Python supplies composition commands; Rust
validates dimensions, clips placements and performs alpha compositing. The
canvas is a bounded frame surface, not a native window or a DICOM decoder.
`NativeApplication` now provides a thin Windows host over the existing
Rust-owned native provider. It presents bounded RGBA frames and returns finite
typed event dictionaries without Python callbacks or a second event loop.
Non-Windows construction returns `ERR_UNSUPPORTED_PLATFORM_EVENT` until a
provider is admitted. RITK remains the owner of DICOM parsing and
medical-display semantics.

`Application(width, height)` provides the bounded cross-platform software
lifecycle. Read its `generation`, pass that token to `clear`, `to_rgba`, input
methods and `poll_event`, then call `close`. `reopen` allocates a fresh surface
and returns a new token; stale tokens fail with a typed `ValueError`. Events are
FIFO and bounded by the Rust platform queue. The object synchronizes access
with Rust locking and uses no Python callbacks or second event loop.
RGBA extraction runs in a detached Rust region and creates Python `bytes` only
after reattachment. The Windows native host likewise detaches provider frame
submission, bounded waits, close and reopen operations; event dictionaries are
constructed after the provider call returns.

The PyPI release caller uses GitHub Actions OIDC Trusted Publishing. It stores
no PyPI token, signing key or developer private key in the repository.

## License

MIT or Apache-2.0, at your option.
