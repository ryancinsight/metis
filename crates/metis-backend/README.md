# metis-backend

Backend session authority, validated demonstration arithmetic, bounded in-memory
audit records and supervised frontend processes. Moirai owns worker execution and
process lifecycle; this crate owns the session deadline and protocol policy.

```rust
use metis_backend::clinical::PatientWeightKg;
use metis_core::ErrorCode;
let error = PatientWeightKg::new(f64::NAN).expect_err("NaN is not a weight");
assert_eq!(error.code, ErrorCode::NumericInstability);
```

The `metis-app` application entry obtains a fresh key from the operating system
and launches the presentation role using the same executable. This package
provides a library, not a separate executable. Windows process tree containment
bounds child lifetimes; it does not restrict file or network permissions. The example policy is not clinical guidance. Audit storage does not
survive restart. See [architecture](../../docs/ARCHITECTURE.md) and
[verification](../../docs/VERIFICATION.md). This package is unpublished.
