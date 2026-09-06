# ADR 0006: Single application entry

Status: Accepted

Date: 2026-09-06

Driver: [METIS-APPLICATION-001](../../backlog.md#METIS-APPLICATION-001).

## Intent and decision

The user requires an application executable suitable for replacing a Tauri
application without a mandatory frontend companion. Executable count and process
count are separate contracts: one deployed image can retain separate process
state and bounded IPC. This decision revises the executable topology in
[ADR 0001](0001-process-contract.md) and the demonstration inventory in
[ADR 0005](0005-application-distribution.md); it does not change the wire format.

A concern-owned `metis-app` crate composes the backend and frontend libraries.
Its default invocation creates the backend session and launches the exact path
returned by `std::env::current_exe()`, adding `--metis-frontend` and the three
submitted inputs. The child role dispatches before OS entropy generation or
backend service construction. Role parsing is closed: `--help` describes the
public command; malformed arguments fail rather than selecting a default role.

Backend service, protocol and supervision stay in their existing libraries.
The application entry owns OS entropy and process-role composition. The
frontend library dependency closure continues to exclude backend authority and
distribution tooling, while the application binary links both libraries.
Moirai already supplies shell-free argument passing, cleared child environment,
private standard-stream pipes, Windows job containment and bounded teardown;
this increment requires no second runtime or new process provider.

The manifest declares only `metis-app` for the demonstration. The general CLI
continues to accept additional explicit binary targets for applications needing
sidecars. The `metis` distribution tool is a developer executable, not a required
file in the installed application. Application resources remain explicit payload
entries; one executable does not imply every application's assets are embedded.

## Alternatives and limits

Keeping a mandatory frontend companion would preserve the user's identified
packaging gap and sibling lookup dependency. Moving both roles into one process
would discard the existing private address-space and transport boundaries.
Duplicating supervision in the application entry would fork Moirai lifecycle
ownership without adding a capability. These alternatives are rejected.

A generic plugin or runtime role registry has no current requirement. A closed
entry dispatcher expresses the two existing roles without adding public library
API. Platform-specific lifecycle extensions belong upstream in Moirai when a
consumer actually requires them. Non-Windows process-tree containment remains an
explicit unsupported capability; this change does not supply a browser host,
native window, privilege sandbox or cross-platform installer.

## Trust model and failure behavior

Assets are the backend session key, authorization policy and audit records.
The key is generated only in the parent and never passed through arguments,
environment or IPC. The child receives the existing private pipe endpoints.
Claimed PIDs and the internal role argument are metadata and routing, not
OS-authenticated credentials. A standalone invocation of the child role cannot
attach itself to an already-running session simply by knowing the argument.

The executable image contains both roles' compiled code. Process separation
protects distinct live state under the existing OS model; it does not prevent a
compromised process from executing code present in its image or deny file,
network or process operations. Job containment bounds child lifetimes, not
privileges. The frontend still does not possess the backend MAC key and cannot
claim independent verification of response authentication. Retain the existing
protocol validation, request sequencing and exact issued-capability checks.

Unknown roles, missing or excess inputs and malformed protocol return failure.
The child never falls through into parent launch, creates a backend key, or
reports a successful calculation after rejection. Under normal parent launch,
Moirai supervises pipe closure and termination under the existing session and
cleanup deadlines. Arbitrary direct child-role invocation can block on an open
input stream; no standalone read deadline is claimed. EOF and malformed input
remain explicit negative test cases.

`current_exe()` selects the running application path, allowing relocation and
renaming without sibling lookup. It is not publisher authentication or an
immutable open-executable handle: filesystem replacement by an attacker with
write access remains outside this trusted-installation assumption. Binary
signing and OS permission restrictions require separate implementations and
host denial probes.

## Migration

Build and run the application with:

```text
cargo run --locked -p metis-app -- 60 2 0.2
cargo run --locked -p metis-app -- --help
```

Update application manifests to select package/bin `metis-app` and set `entry`
to `metis-app`. Remove the former mandatory backend/frontend binary entries.
The `metis-backend` and `metis-frontend` packages now provide libraries only;
their old executable commands are removed without forwarding shims. Library
imports and the IPC representation remain unchanged. Existing unrelated
application sidecars do not need removal.

## Verification and overturning evidence

Acceptance requires running a copied and renamed executable in a temporary
directory containing no companion executable. Compare multiple input scenarios
to the analytical conversion oracle; verify distinct backend/frontend PIDs,
private wire output and audited completion. Exercise invalid numbers, unknown
roles, malformed argument counts, child EOF and malformed protocol without
successful results or recursive launch. Retain existing descendant cleanup and
session deadline coverage under the committed nextest budgets.

The distribution workflow must verify exactly one application executable in
both portable and installed inventories, then perform real MSI installation,
input-sensitive execution and uninstall preserving a user-created file.
Formatting, dependency closure, strict Clippy, debug/release tests, doctests,
WASM library compilation and warning-clean documentation remain gate inputs.
Source-bound visual fixtures must be regenerated and compared after removing
the frontend entry point; no changed image is presumed acceptable from a source
fingerprint change alone. [Verification](../VERIFICATION.md) distinguishes these
requirements from collected outcomes; passing claims require the exact gate
revision and source hashes.

No relative memory or startup improvement is inferred from merging executable
images. Measure full process-tree memory, application payload and startup under
matched workloads before making a comparison. Revisit this design if a concrete
OS sandbox requires separate images or measured resource behavior contradicts
the intended deployment benefit; preserve the user's single-entry workflow and
record the evidence before changing the process boundary.
