# ADR 0017: Python binding boundary

Status: Accepted

Date: 2026-09-08

Driver: [METIS-PYTHON-001](../../backlog.md#METIS-PYTHON-001).

## Context

Metis targets applications that currently use JavaScript, egui or Tauri. A
Python consumer needs a native Rust surface for automation, testing and future
application composition without moving domain logic into Python. The current
workspace already owns validated clinical calculation types in
`metis-backend`; those types provide a real, synchronous boundary that can be
tested independently of a window host or DICOM decoder.

PyO3 is already the Atlas convention for thin Python bindings. The binding must
not make `metis-core` or `metis-backend` depend on Python, hold a Python object
in a Rust domain value, or duplicate validation and arithmetic. The package
also needs the same release evidence as the Rust crates. The PyPI distribution
name `metis` is already occupied by an unrelated package, so the distribution
uses `metis-rs` while its import name remains `metis`.

## Decision

Add the publishable `metis-python` workspace member under `crates/`. It is the
only Metis crate that depends on PyO3. Its `cdylib` module is named
`metis._metis`, and the adjacent Python package re-exports the typed boundary
from `import metis`. The crate uses PyO3 0.29.2 with the stable `abi3-py39`
surface, so one wheel per supported system serves CPython 3.9 and newer.

The first surface wraps the existing clinical contract:

- `PatientWeight`, `DrugConcentration` and `TargetDose` construct the Rust
  validating newtypes and expose read-only values;
- `SafetyEnvelope` owns the validated rate ceilings and supplies the same
  documented defaults as the Rust type;
- `calculate_infusion_rate` accepts those value objects, uses the Rust
  `calculate_infusion_rate` function, maps `MetisError` to `ValueError`, and
  releases the Python interpreter lock with `Python::detach` while calculating;
- `InfusionResult` exposes the Rust result fields without recomputing them in
  Python.

The Python package ships `py.typed` and a `.pyi` module stub. A local gate
builds the wheel with the pinned `maturin` tool, extracts it into an isolated
directory and runs value-semantic pytest cases against the built extension.
The release caller delegates wheel construction to Atlas's pinned
`python-wheels.yml` and publishes through PyPI Trusted Publishing with an
`id-token`; it stores no PyPI token, SSH key, signing key or other repository
secret.

This increment does not invent Python GUI classes or a DICOM model. Those APIs
must first have a public Rust contract and an independent host/decoder oracle;
the follow-on binding work will extend this crate in place when those seams
exist.

## Alternatives

Putting PyO3 in `metis-backend` would make the domain crate depend on the
interpreter and would force every Rust consumer to carry the binding boundary.
A Python reimplementation of the calculation would drift from the safety
envelope and would not prove Rust/Python equivalence. A package named `metis`
would collide with the existing unrelated PyPI project. A long-lived PyPI
token or developer private key would widen credential exposure; the existing
Atlas OIDC workflow already supplies a short-lived publisher token.

## Threat model and limits

Python arguments are untrusted input at the FFI boundary. Constructors reject
non-finite and out-of-range values before the core function runs, and the
result remains bounded by the Rust safety envelope. No Python object crosses
into the detached computation; only copyable validated Rust values do. Error
messages preserve the Rust error code and trace identifier so callers can
classify failures without a second error taxonomy. The clinical example is
engineering arithmetic, not treatment guidance or regulatory evidence.

The wheel does not sandbox Python, restrict OS permissions or provide a native
window. A wheel can only be published after the `metis-rs` PyPI trusted
publisher and the `pypi` environment are registered by the release authority.

## Verification

Rust's existing clinical tests remain the analytical oracle. The Python suite
checks constructor rejection, adult and pediatric results, envelope rejection,
input sensitivity and the exact analytical rate and drug-rate values after
installing the wheel built by `maturin`. The local Metis gate records the wheel
build and pytest command at the verified revision. `cargo clippy`, doctests and
the package metadata gate cover the new crate; the release workflow contract
tests assert OIDC-only publication and the absence of secret or key inputs.
