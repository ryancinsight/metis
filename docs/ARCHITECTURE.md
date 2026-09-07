# Architecture

## Target and implemented foundation

Metis targets near drop-in Tauri migration, desktop web frontends, and Rust/WASM
applications rendered in the browser. [ADR 0002](adr/0002-web-application-contract.md)
owns that target contract. HTML5/CSS compatibility uses the browser's DOM/layout
engine; the software renderer below remains an implemented bounded presentation
path, not a substitute for web standards. Browser scheduling and request delivery
must be asynchronous; the native blocking `IpcTransport` cannot run on the
browser event thread. `metis-ipc` now supplies `AsyncIpcTransport`,
`AsyncIpcClient` and a bounded WASM WebSocket adapter backed by Moirai's browser
reactor and timer. Shared provider capabilities belong in Moirai and Iris.

The following sections describe the current native foundation, not a completed
web host. No browser renderer or desktop WebView host is implemented yet.

The shared `metis-core` crate owns wire types, error codes and capability claim
encoding. `metis-ipc` owns framing, canonical payload interpretation, transport
correlation and typed failure reporting. `metis-backend` alone owns calculation
policy, session authorization and audit storage. The application entry generates
a fresh backend key and transfers ownership only into the parent service.

`metis-frontend` converts submitted values to a wire request and displays the
correlated response. `metis-ui-lang` parses bounded markup and computes a display
list; `metis-platform` rasterizes it into bounded pixel storage. The renderer
implements Iris's lending `RenderBackend<DisplayList>` contract.

`metis-app` is the application composition boundary. Its default backend role
relaunches the exact `current_exe()` path with `--metis-frontend` and the submitted
inputs. The child dispatches before backend key generation or service creation.
The application links both role libraries while the frontend library dependency
closure remains independent of backend authority and distribution tooling.
One executable image serves separate address spaces; this is not a claim that
backend machine code is absent from the child. [ADR 0006](adr/0006-application-entry.md)
owns role dispatch and command migration.

Moirai owns execution and process lifecycle. Its transport provider is extended
for inherited private pipes, finite teardown and Windows process-tree lifecycle
containment. Metis retains only application dispatch and the form-session budget.
No frontend object, memory address or backend secret crosses the IPC boundary.

## Distribution boundary

`metis-cli` owns application configuration, Cargo artifact selection and packaging.
It consumes Moirai process supervision and the core streaming hash, and never
enters the frontend dependency closure. The same validated payload supplies a
portable directory and the Windows Installer backend. The demonstration declares
one application executable; other application manifests may name optional
sidecars. Rendering, application logic, process authorization and installation
each retain one owner.
[ADR 0005](adr/0005-application-distribution.md) defines the manifest, resource
budgets, native FFI boundary and platform expansion contract.

## Provider selection

| Role | Owner | Decision |
| --- | --- | --- |
| Scheduling/process lifecycle | Moirai | Reuse executor and process transport; fill missing pipe/deadline support upstream. |
| Rendering contract | Iris | Implement its borrowed-frame interface; retain byte-packed pixel storage. |
| General allocation | Mnemosyne | Already reachable through Moirai; no extra global allocator override without an allocation contract/measurement. |
| Recoverable framebuffer allocation | Metis | Mnemosyne aligned storage currently aborts on allocation failure; Metis checks size and uses fallible reservation. |
| In-process brand/borrow proofs | Melinoe | Reached through Moirai; cannot replace authenticated serialized capability claims. |
| Standalone MAC/hash | Metis seed implementation | Moirai crypto currently exposes a TLS provider, not a standalone public primitive surface. Upstream extraction remains required before replacement. |

## Platform coverage

The visible desktop event loop and OS permission sandbox remain incomplete.
Process address-space separation and lifecycle containment do not deny file,
network or device access. Platform support claims require separate host tests
and denial probes, not merely `cfg` branches or successful compilation.

See [ADR 0001](adr/0001-process-contract.md) for the trust boundary and alternatives.
