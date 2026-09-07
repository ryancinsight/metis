# 0002 — Web application and Tauri migration contract

Status: Accepted

Date: 2026-09-05

Driver: [METIS-WEB-001](../../backlog.md#METIS-WEB-001).

Revision 2026-09-06: [ADR 0007](0007-browser-transport.md) records the first
bounded asynchronous browser transport slice. [ADR 0003](0003-framework-conformance.md), driven by
[METIS-GAPS-001](../../backlog.md#METIS-GAPS-001), adds the egui/GPUI/Tauri
capability inventory and per-gap demonstration/verification closure. It retains
this web/native trust boundary and does not claim API parity from toolkit breadth.

## Intent and authority

The user clarifies that Metis must support WASM and web rendering, with the goal
of a near drop-in Tauri replacement offering stronger security and lower memory
use. This supersedes the seed prompt's blanket prohibition on WebViews and
JavaScript. It does not establish that these targets are implemented or measured.
Rust remains the application-logic language; Atlas providers remain the default
owners of shared infrastructure. The user manual and actual application gallery
remain the user-facing documentation.

## Decision

Support browser HTML5/CSS presentation with Rust compiled to WebAssembly for
application state and portable computation. Preserve a desktop system-WebView
hosting path for existing web frontends. Minimize application-authored JavaScript;
host bindings needed to load WASM or access browser APIs belong at a narrow,
auditable boundary. Existing JavaScript applications retain that code until
migrated; no automatic JavaScript removal is promised.

Use the browser DOM and layout engine for ordinary HTML5/CSS. The current
software renderer is a bounded presentation implementation, not an HTML5 engine.
Iris owns the rendering seam; Moirai owns scheduling and transport. Implement
missing reusable browser capabilities upstream rather than creating parallel
runtimes in Metis. A browser uses event-driven receipt and cancellation, never
the existing synchronous pipe receiver on its main event thread.

The initial consumer seam is `metis_ipc::AsyncIpcTransport` and
`AsyncIpcClient`. The WASM-only `BrowserWebSocketTransport` owns a bounded
Moirai WebSocket reactor and races one receive against a finite `WebTimer`.
Native `IpcTransport` remains the blocking stream contract. This slice proves
frame bounds, correlation and cancellation at compile and native-test level;
it does not claim a running browser host, concurrent request multiplexing or
the V02/V12 browser trace. [ADR 0007](0007-browser-transport.md) owns the
consumer contract and its residuals.

Desktop privileged logic remains in a separate Rust backend. Downloaded WASM is
frontend code: it cannot protect server secrets or establish native privilege
separation. A browser-only application can perform unprivileged local operations;
an operation requiring authority must cross an explicitly configured authenticated
service or desktop-host boundary. Unsupported host operations fail explicitly.
No remote service or hosted endpoint is enabled by this decision.

## Compatibility contract

Near drop-in means minimizing application changes against an enumerated Tauri
surface, not declaring Rust crate/API, wire, or plugin binary compatibility.
Each admitted surface requires a representative migrated application and tests.
The existing custom binary protocol is not the Tauri invoke protocol.

| Surface | Target | Current evidence / required acceptance |
| --- | --- | --- |
| HTML5/CSS/assets | Preserve existing web presentation in browser/system WebView | Current XML-like renderer has no web-standards parity; real DOM interaction and visual cases required. |
| Rust/WASM | Shared portable application code with asynchronous host bindings | WASM library compilation is the first gate; browser execution and binding lifetime tests remain required. |
| Commands/events | Typed requests, correlated responses, bounded event delivery and cancellation | Existing private-pipe protocol is tested; generic commands/events and Tauri migration mappings remain required. |
| Windows/lifecycle | Desktop window creation, input, navigation, close and teardown | No native window host yet; host-specific integration and denial probes required. |
| Plugins/native APIs | Explicit permission-scoped supported operations | Inventory file/dialog/clipboard/shell/window capabilities against migrated examples; reject unsupported operations. |
| Configuration/distribution | Reviewed mappings for assets, permissions, build, packaging and updates | No configuration importer, bundler or updater parity claim; migration diagnostics and install/update tests required. |

The migration surface must not forward privileged calls into a retained Tauri
runtime. Implement supported behavior natively at the owning Atlas seam; document
necessary source changes and unsupported APIs. Mobile compatibility needs its own
platform coverage before it can be claimed.

## Threat model

Assets are backend authority, keys, user data, and finite host resources.
The adversary may control document content, scripts, WASM inputs, navigation and
bridge messages. Treat every frontend as untrusted, regardless of its language.

| Boundary | Failure | Required controls and oracle |
| --- | --- | --- |
| Content to host | Injection, navigation or forged origin gains native authority | Deny-by-default commands; bind grants to authenticated origin/session/window; CSP and navigation policy; adversarial injection/origin tests. |
| Message to backend | Replay, oversized allocation, wrong response or denial of service | Versioned canonical parsing, scoped capabilities, finite frame/queue/in-flight limits, deadlines and cancellation; negative and lifecycle tests. |
| WASM to imports | Broad host imports bypass sandbox expectations | Explicit minimal imports, bounded linear-memory growth and buffer ownership; malformed input and out-of-bounds tests. |
| Browser to service | Credentials leak or client identity is mistaken for authority | No backend keys in WASM; explicit authentication and transport protection; replay and cross-session rejection. |
| Close/cancel to resources | Tasks, listeners, handles or buffers survive teardown | Bounded shutdown with ownership-based cleanup; repeated lifecycle tests and allocation/process-memory measurements. |

WASM memory isolation does not authorize host imports, and Rust does not remove
the need to validate untrusted requests. A system WebView remains a dependency
with its own update and security boundary.

## Alternatives

A custom software renderer alone cannot preserve existing DOM/CSS application
behavior, so it cannot be the sole Tauri migration path. Reimplementing a browser
engine inside Metis is not justified by the current renderer or compatibility
requirements. Requiring all frontend JavaScript to disappear before migration
contradicts near drop-in support for existing applications. Bundling an engine is
not selected without a demonstrated system-WebView compatibility gap and measured
artifact/process-memory cost.

## Verification and comparative claims

Compile portable libraries for `wasm32-unknown-unknown` under the pinned toolchain
and lock. This is static portability evidence only. A browser acceptance test
must load the actual WASM, change at least two form inputs, observe distinct
results, exercise errors/cancellation, and capture an actual browser snapshot.
Keep native process tests and backend dependency separation gates.

Before claiming lower memory use, run equivalent Metis and Tauri applications
with the same assets, window count, inputs, browser-engine version, machine and
measurement protocol. Account for the entire process tree, WASM linear memory,
retained allocations, idle/active/peak use and growth across repeated lifecycle
operations. Record bundle size separately from resident memory; smaller artifacts
do not prove lower memory use. Store baselines and report variation under the
committed runtime budget. No comparative measurements exist yet.

Before claiming stronger security, state the threat model and compare tested
denials, permission scope and failure containment against the matched Tauri
configuration. Tauri already separates processes and supports capabilities;
these mechanisms alone are not differentiators.

## Sources

- [Tauri architecture](https://v2.tauri.app/concept/architecture/), introduction and API/tooling sections: web frontend, Rust host, command surface and distribution roles.
- [Tauri process model](https://v2.tauri.app/concept/process-model/): core and WebView process responsibilities.
- [WebAssembly security](https://webassembly.org/docs/security/), security goals and execution semantics: isolated execution and host embedding responsibilities.

Source inspection at Metis `9d96980` finds a bounded software renderer and blocking
native transport, not a browser host. Provider inspection finds Iris's lending
rendering contract and incomplete browser event receipt in Moirai. Neither is
evidence of working browser integration. Reconsider this implementation boundary
when the tracked browser and desktop acceptance suites pass.
