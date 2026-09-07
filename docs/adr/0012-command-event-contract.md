# ADR 0012: Typed command and event contract

Status: Accepted

Date: 2026-09-07

Driver: [METIS-COMMANDS-001](../../backlog.md#METIS-COMMANDS-001).

## Context

Metis currently has a fixed set of typed request and response payloads, but a
host cannot ask a connected service which operations or target surfaces it
admits. Unknown wire
identifiers are rejected during frame decoding and known operations that a
service does not implement return the generic protocol error. The browser
client also needs bounded event subscriptions without importing a second
runtime or retaining callbacks after unsubscribe.

egui, GPUI and Iced provide different state and action models, while Tauri
exposes commands and events through a host boundary. Metis needs one transport-
independent contract that preserves the existing sequence, capability and
bounded-memory rules. Moirai remains the owner of scheduling, sockets and
browser handles; Metis owns command meaning and delivery policy.

## Decision

The wire catalog is an additive operation in the existing protocol version:

- `MessageType` is explicitly non-exhaustive so future wire identifiers do not
  break downstream matches. `CapabilityReq` and `CapabilityResp` are the first
  additive request/response message types under that extensibility contract.
  A catalog request has an empty payload and is accepted only after the
  session handshake.
- `CapabilityCatalogPayload` carries the protocol version and a bounded list
  of `MessageType` request identifiers. The decoder rejects unknown,
  response-only, duplicate or over-limit entries before exposing the list.
- `MessageType::descriptor` supplies the stable request/response pairing and
  human-readable command name for the closed set of protocol commands.
  `SUPPORTED_COMMANDS` is the service's explicit catalog; adding a command
  requires adding its typed payload and handler in the same change.
- `IpcClient` and `AsyncIpcClient` expose `discover_capabilities`. Its
  `CapabilityError` keeps local protocol failures separate from the peer's
  full `ErrorResponsePayload`; a catalog version mismatch is a typed local
  protocol error. A known but unadvertised operation and an unknown wire
  identifier remain explicit `UnexpectedMessageType` errors; neither is
  silently ignored or forwarded.

- `TargetCapabilityReq` and `TargetCapabilityResp` use a separate empty-request
  route after handshake. `TargetCapabilityPayload` carries the protocol
  version, target platform and a bounded list of surfaces installed by the
  host. Unknown platform or surface identifiers, duplicates, truncation,
  trailing bytes and over-limit lists fail before the descriptor is exposed.
  The backend starts with its native-process surface; the application and
  browser acceptor add private-process IPC and authenticated WebSocket
  surfaces at the composition boundary. A platform identifier never implies a
  native window, operating-system permission, accessibility or IME provider.
- Both client variants expose `discover_target_capabilities` with a typed
  `TargetCapabilityError`, preserving peer rejection payloads separately from
  local protocol failures. The asynchronous frontend stores the validated
  descriptor beside the command catalog, and the browser workbench renders
  both the remote host surfaces and its local WASM/DOM/CSS surfaces.

`metis-ipc` also owns `EventHub<E, CAPACITY>` and `Subscription<E>`. Every
  subscription uses a bounded synchronous channel, `publish` reports
  backpressure as `QueueFull`, and `unsubscribe` removes the sender before a
  future event can be delivered. `Subscription::recv_timeout` requires a
  finite deadline. The hub is a local delivery primitive and does not add a
  callback registry, executor, or transport-specific state machine.

The command seam also exposes a host-local `PluginRegistry`. A plugin type
implements `Plugin` with one static `PluginDescriptor`; the descriptor carries
bounded lower-case identifiers, a version, and operation metadata with explicit
non-empty capability scopes. The registry validates duplicate plugin and
operation names, operation counts and capacity before storing static metadata.
It deliberately does not erase plugin handlers, add a dynamic callback list,
or grant operating-system authority. Extension dispatch remains at the host
boundary after the existing capability checks.

## Alternatives

Adding a JSON or string command router would duplicate the versioned binary
protocol, make schema validation a runtime convention and increase allocation
pressure. Reusing `AuditQueryReq` for discovery would conflate audit data with
host capability negotiation. An unbounded channel or callback list would make
subscriber behavior dependent on producer rate and retain browser state past
its lifecycle. A third-party event runtime would duplicate Moirai's scheduler
and violate the Atlas provider boundary.

## Verification

Protocol tests cover catalog round-trips, empty and over-limit lists, unknown
and response-only identifiers, duplicate entries and version mismatch.
Client and backend tests cover post-handshake discovery and explicit rejection
of the known unsupported audit request. Event tests cover input-sensitive
delivery, independent subscriber queues, full-queue backpressure,
unsubscribe, disconnected receivers and finite receive deadlines. The same
Target descriptor tests cover exact platform/surface round-trips, unknown and
duplicate values, bounds and trailing bytes; sync and async clients and the
backend cover correlated target discovery. The same catalog, target and event
conformance tests run against the existing memory transport;
the browser and WebSocket paths continue to use the shared frame and sequence
implementation.

## Limits

The initial catalog and event increments did not claim remote plugin invocation,
target surface discovery, native OS capability discovery or cross-engine
browser coverage. The remote
invocation revision below closes the first of those protocol gaps; native OS
target surface reporting now exposes only installed host mechanisms. Native OS
capability discovery and cross-engine coverage remain owned by the desktop,
services and browser items on the board.

Because `MessageType` is a public enum, the extensibility marker makes this a
major API change for the next published release. Package versions stay at
`0.1.0` while release authority has not assigned that release.

## Revision 2026-09-07

The first command increment left remote event serialization open. The contract
now includes `RemoteEventPayload`, which carries the protocol version, a nonzero
event identifier, a bounded UTF-8 name and bounded body bytes. `EventCodec`
associates a stable name with a typed body; `decode_as` rejects a name mismatch
before invoking the body decoder. Synchronous and asynchronous clients verify
that the frame sequence echoes the envelope identifier, the envelope version
matches the negotiated wire contract, and identifiers increase strictly. The
synchronous and asynchronous servers expose an explicit
send path whose event sequence is consumed before transmission. The asynchronous
client's response pump retains a bounded event queue and keeps correlated
responses available to their request owners.

The added verification covers exact envelope round-trips, malformed bounds,
typed name matching, synchronous send/receive, event/response interleaving,
identifier/version mismatch and replay. A live browser trace exercises the
capability catalog and the backend-produced clinical event; plugin descriptor
validation is covered by the core tests. This revision predates the remote
invocation revision below. `ErrorCode` is now non-exhaustive so future typed protocol and capability
failures do not force downstream match arms; this follows the major release
classification already required by the public `MessageType` extension.

## Revision 2026-09-07 (event production)

The event envelope was previously only an explicit server send primitive. The
server contract now gives `IpcHandler` one bounded event slot drained after a
successful correlated response on both synchronous and asynchronous transports.
`BackendService` fills that slot only for an accepted clinical calculation,
using the signed `ClinicalCalcResponsePayload` as the `clinical.result` body and
the audit sequence as its strictly increasing identifier. The clients consume
the event explicitly; the synchronous client also retains an event encountered
while waiting for a later response so existing request code remains ordered.

Event delivery failures are audited as `FailureContext::Event(EventId)` and the
public failure context is non-exhaustive. The browser workbench receives the
event through `AsyncFrontendApp`, decodes it without dynamic dispatch and
verifies the event identifier and exact body equality against the correlated
response before displaying it. Synchronous memory-transport, asynchronous
WebSocket loopback and browser workflow evidence cover the event path. At this
revision, the remote invocation revision below had not yet landed.

## Revision 2026-09-07 (remote invocation)

Remote plugin invocation now uses typed `PluginInvokeReq` and
`PluginInvokeResp` messages in the same versioned frame contract. The request
payload carries the authenticated session token, validated plugin and
operation identifiers, and one bounded opaque body. The plugin owns its body
codec; malformed body bytes return a typed protocol error at that plugin's
boundary. The response carries one bounded body and remains correlated by the
wire sequence, so no string command router or JSON schema is introduced.

`metis-backend` owns a bounded `PluginRouter`. Registration validates the
static `PluginDescriptor` through the existing `PluginRegistry` and stores a
plugin executor only at the extension boundary. The object-safe executor
trait is deliberately dynamic there because the set of externally supplied
plugin types is open. The dynamic boundary is limited to plugin dispatch;
clinical calculation and frame codecs remain statically dispatched.
Invocation resolves the exact manifest and operation,
authorizes the operation's declared `CapabilityScope` against the trusted
host-bound session token, then invokes the executor. Unknown plugins,
unknown operations, missing scopes and executor failures remain explicit
typed errors. Registration and invocation grant no operating-system authority
and never bypass the host policy.

The capability catalog advertises the invocation route independently of the
currently installed manifests. A host with no matching plugin returns
`PluginNotFound`; a known plugin with no matching operation returns
`PluginOperationNotFound`. This keeps command discovery deterministic while
leaving installed plugin metadata as a separate bounded host-local concern.

The invocation payload and response codecs cover exact round-trips, every
truncation point, invalid identifiers, trailing bytes and the frame-size bound.
Backend service tests exercise an input-sensitive executor, successful scope
authorization, insufficient scope, unknown plugin and unknown command errors.
Both synchronous and asynchronous clients use typed invocation helpers and
preserve peer rejections separately from local transport/decode failures.

## Revision 2026-09-07 (target discovery and lifecycle guard)

The host target is now a first-class, versioned discovery result. A client
requests `TargetCapabilityReq` after handshake and receives the platform plus
the bounded surfaces installed by that service. `BackendService` advertises
the native process boundary by default; `metis-app` adds private-process IPC,
and `serve_browser_websocket` adds its authenticated browser bridge. The WASM
application records its own DOM/CSS/WASM surfaces for the browser workbench.
The descriptor is deliberately separate from the command catalog so a caller
can distinguish an available operation from the host mechanism that can carry
it. Missing native-window, OS-permission, accessibility and IME providers stay
absent and therefore cannot be mistaken for platform support.

The browser host also assigns a monotonic lifecycle generation at each start
or stop boundary. Async connection and submission completions must match the
generation that created them before they store an app, state or DOM render.
This guard complements Moirai's cancellable local task handle: cancellation
releases the pending future, while the generation check prevents a completion
already ready at the boundary from mutating a remounted application. Overflow
of the generation is a typed local error rather than a wraparound.

Target descriptor round-trips, malformed values, service dispatch, sync and
async client correlation, authenticated WebSocket discovery and native
generation rejection are covered by tests. The browser target guard is
covered by the lifecycle unit tests; the delayed browser-server stop/remount
probe is recorded in [VERIFICATION](../VERIFICATION.md#browser-stale-response-evidence--2026-09-07).
Cross-engine capture remains open runtime evidence.
