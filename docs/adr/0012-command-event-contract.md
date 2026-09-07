# ADR 0012: Typed command and event contract

Status: Accepted

Date: 2026-09-07

Driver: [METIS-COMMANDS-001](../../backlog.md#METIS-COMMANDS-001).

## Context

Metis currently has a fixed set of typed request and response payloads, but a
host cannot ask a connected service which operations it admits. Unknown wire
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

`metis-ipc` also owns `EventHub<E, CAPACITY>` and `Subscription<E>`. Every
  subscription uses a bounded synchronous channel, `publish` reports
  backpressure as `QueueFull`, and `unsubscribe` removes the sender before a
  future event can be delivered. `Subscription::recv_timeout` requires a
  finite deadline. The hub is a local delivery primitive and does not add a
  callback registry, executor, or transport-specific state machine.

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
catalog and event conformance tests run against the existing memory transport;
the browser and WebSocket paths continue to use the shared frame and sequence
implementation.

## Limits

This increment catalogs the closed protocol set and supplies the bounded local
event primitive. It does not claim a complete Tauri plugin registry, remote
event serialization, native OS capability discovery, or cross-engine browser
coverage. Those remain owned by the command, desktop, services and browser
items on the board.

Because `MessageType` is a public enum, the extensibility marker makes this a
major API change for the next published release. Package versions stay at
`0.1.0` while release authority has not assigned that release.
