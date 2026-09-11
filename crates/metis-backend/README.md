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
bounds child lifetimes; the default session budget is ten seconds and the
interactive native host uses the explicit finite five-minute budget. It does not
restrict file or network permissions. The
default `BackendService` policy binds grants to the contained native origin and
window; browser and desktop hosts must supply their observed context before
they can expose privileged commands. A live acceptor uses
`HostPolicy::observe_origin` and `BackendService::with_trusted_context`, so the
handshake principal can route a session but cannot select its authority. The
example policy is not clinical guidance. Audit storage does not survive restart. See
[architecture](../../docs/ARCHITECTURE.md) and
[verification](../../docs/VERIFICATION.md). This package is unpublished.
The native-only `BrowserHttpService` composes the first-party Moirai HTTP
transport for a bounded loopback demonstration. It exposes only typed session,
fragment and health routes, checks the exact browser origin before dispatch,
retains at most eight sessions and closes after the application's finite
request budget. It does not parse files or own DICOM; a RITK consumer remains
responsible for format-specific loading and viewer state.
After handshake the service answers `CapabilityReq` with its bounded command
catalog. A known command outside that catalog produces an explicit typed
`UnexpectedMessageType` response. Hosts can register a `Plugin` together with
a `PluginExecutor`; `PluginRouter` validates the static manifest and invokes
declared commands after the service verifies the command's capability scope.
The plugin owns its opaque body codec, responses remain bounded by the wire
frame, and the route grants no operating-system authority. Unknown plugins,
commands, missing scopes and executor failures remain typed responses.
After the same handshake, `TargetCapabilityReq` returns the host platform and
the surfaces installed by the service boundary. `BackendService` starts with
the native-process surface; the application adds private-process IPC and the
browser acceptor adds its authenticated WebSocket surface. Missing native
window and operating-system surfaces are reported as absent rather than
inferred from the target platform.
