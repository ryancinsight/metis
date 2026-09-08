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
`metis-web` is the WASM host crate: it owns application state and rendering for
an HTML5 document, while Moirai owns `WebDocument`, `WebElement` and event
listener lifetimes.

The following sections describe the current native foundation and the first
browser host. The browser workbench mounts a real DOM form and can connect to a
bounded native service when the host supplies explicit endpoint configuration.
`metis-backend` validates the observed `Origin` before the WebSocket upgrade and
requires a trusted host context before it serves requests. Its external assets
carry the strict same-origin CSP from the policy source consumed by
`HostPolicy`, and the bootstrap rejects cross-origin anchor navigation. The
Windows `metis-platform::native::NativeSurface` supplies a Moirai-owned
HWND, bounded event queue, native IME composition phases and ARGB presenter. The
`metis-app --metis-native-window` role composes that surface with the existing
frontend and supervised private IPC; the desktop WebView host and OS permission
boundary remain unimplemented.

The shared `metis-core` crate owns wire types, typed command descriptors,
capability catalog and target-surface encoding, remote event envelopes, bounded
host-local plugin metadata registration and the host-origin/window/session
policy. `metis-ipc`
owns framing, canonical payload interpretation, transport correlation, bounded
local and remote event delivery and typed failure reporting. Plugin registration
stores static metadata with explicit scopes, while its invocation payload keeps
the plugin body opaque to the core protocol. `metis-backend` owns the bounded
`PluginRouter` and its `PluginExecutor` extension boundary: it resolves a
declared command, verifies the command scope against the trusted session token,
then invokes the executor. Dynamic dispatch is confined to that open
extension boundary; the clinical calculation and frame codec paths remain
static, and plugin execution never crosses the OS authority boundary.
`metis-backend` alone owns calculation policy, session authorization and audit
storage. A successful clinical calculation also creates one bounded
`clinical.result` event from the typed response; `metis-ipc` emits it only after
the correlated response. The application entry generates a fresh backend key and
transfers ownership only into the parent service.

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

## Host authority boundary

`HostPolicy` is the shared boundary for every future privileged command. A
trusted native or service host supplies the observed canonical origin, window
identity and nonzero session principal. The policy admits one exact origin and
window, and `HostContext::issue_capability` signs the canonical origin and
window as associated data with the capability claims. Verification reconstructs
the same context before scope, expiry and session checks; a plain capability or
a token retargeted to another origin/window fails signature verification.

The 84-byte token wire format stays fixed-width because authority metadata is
not copied from the browser into the payload. `BackendService` defaults to the
contained native policy (`metis://native`, window 1) and accepts an explicit
policy for deterministic host integration tests. The browser service uses
`accept_websocket_with_validator` to observe and validate the request `Origin`
before it sends `101 Switching Protocols`, then runs `AsyncIpcServer` over
Moirai's bounded message stream. TLS, endpoint allowlists beyond the exact
origin, and OS permission checks remain transport/desktop work.

`RemoteEventPayload` carries a version, nonzero event identifier, bounded UTF-8
name and bounded body. Synchronous and asynchronous clients verify the frame
identifier and strict event ordering; the asynchronous response pump retains a
bounded event queue while it correlates request responses, and the synchronous
client drains the same bounded queue when a later request encounters an event.
`IpcHandler` exposes one event slot after each response, which prevents a
handler from creating an unbounded send loop. `EventCodec` binds a stable name
to a typed body without a dynamic registry. `PluginRegistry` binds static
plugin manifests to bounded command/event metadata. `PluginRouter` adds the
host-side command executor at the extension boundary; the invocation payload
and response are bounded and correlated by the same frame contract. Servers
expose explicit send paths over both admitted transports.
`EventHub<E, CAPACITY>` remains the bounded local fan-out primitive.

`TargetCapabilityPayload` separates target identity from installed host
surfaces. The native service advertises its process boundary, the one-
executable application adds private-process IPC, and the browser acceptor adds
the authenticated WebSocket bridge. The WASM workbench reports its own
WASM/DOM/CSS surfaces. The Windows platform provider supplies a native
window/event boundary and bounded IME composition phases, and the `metis-app`
native role applies the frontend's editing policy; operating-system permissions,
accessibility, installed-IME journeys and other host policy remain open.

The browser shell keeps CSS and module bootstrap files external to satisfy the
same-origin CSP. The build checks the HTML policy against
`crates/metis-core/src/content_security_policy.txt`, which is included by
`HostPolicy`, so the two consumers cannot drift silently. The policy includes
`'wasm-unsafe-eval'` because the generated WASM loader uses WebAssembly
evaluation. `frame-ancestors` is effective only when a native or service host
delivers the policy as a response header; a document meta tag cannot enforce
framing. CSP and the module's navigation guard reduce the page attack surface;
neither turns downloaded WASM into a trusted authority source.

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
| Native window and event lifecycle | Moirai PAL + Metis platform adapter | Moirai owns the thread-affine Win32 HWND, bounded message translation including IME composition phases, finite event waiting and retained ARGB frame; Metis exposes `NativeSurface` and the application composes it with the frontend without importing unsafe OS code into domain crates. WebView, permissions, accessibility and non-Windows hosts remain separate items. |
| Rendering contract | Iris | Implement its borrowed-frame interface; retain byte-packed pixel storage. |
| General allocation | Mnemosyne | Already reachable through Moirai; no extra global allocator override without an allocation contract/measurement. |
| Recoverable framebuffer allocation | Metis | Mnemosyne aligned storage currently aborts on allocation failure; Metis checks size and uses fallible reservation. |
| In-process brand/borrow proofs | Melinoe | Reached through Moirai; cannot replace authenticated serialized capability claims. |
| Standalone MAC/hash | Moirai `moirai-crypto` standalone primitives with its TLS feature disabled | Metis consumes the provider's SHA-256, HMAC-SHA256 and fixed-width comparison; CRC-32 remains in the protocol owner. Independent vectors and canonical wire/audit tests are required. |

## Platform coverage

The visible Windows application loop now creates a real HWND, presents the
frontend framebuffer, consumes bounded native events including IME composition
phases and keeps the backend on a supervised private pipe. The native host still
has no OS permission sandbox;
process address-space separation and lifecycle containment do not deny file,
network or device access. A committed visual host capture and denial probes are
required before the desktop item closes. Platform support claims require
separate host tests, not merely `cfg` branches or successful compilation. The
loopback browser service is a one-connection conformance host; it is not a
production TLS listener or a substitute for the native desktop host.

See [ADR 0001](adr/0001-process-contract.md) for the trust boundary and alternatives.
