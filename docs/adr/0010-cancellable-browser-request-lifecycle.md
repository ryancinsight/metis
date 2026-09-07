# ADR 0010: Cancellable browser request lifecycle

Status: Accepted

Date: 2026-09-07

Driver: [METIS-ASYNC-001](../backlog.md#METIS-ASYNC-001),
[Moirai task cancellation](../../../moirai/docs/adr/0045-cancellable-browser-tasks.md).

## Context

The browser client can await a bounded WebSocket response, but a task can be
dropped while its request identifier remains in the client's correlation table.
Keeping that entry makes a later response ambiguous and can retain application
state longer than the task that submitted it. The browser host also needs an
explicit lifecycle boundary that drops DOM listeners when a page replaces or
stops the application.

## Decision

`AsyncIpcClient::cancel_request` removes one pending or retained response, and
`cancel_all_requests` removes the complete bounded table while preserving the
active session capability. Any response for a removed identifier remains
untrusted and is rejected by the existing sequence check; a cancelled request
cannot mutate a later operation. `AsyncFrontendApp::cancel_pending_requests`
uses this contract and returns the form to idle without displaying a stale
result.

The WASM host exports `metis_stop` beside `metis_start`. Stopping drops the
`BrowserApplication`, whose Moirai-owned listener guards detach every DOM
callback, and replaces the root with a stopped message. Starting mounts fresh
state and listeners. A future authorized WebSocket bridge schedules its
request future with Moirai's `spawn_local_with_handle` and calls the frontend
cancellation method after the handle is cancelled; the current page remains
disconnected until origin/session authority exists.

## Alternatives

Leaving cancellation to a browser callback would duplicate correlation-table
ownership and could deliver late data. Clearing the active capability during
request cancellation would force unnecessary re-authentication and conflate
request lifetime with session lifetime. Retaining DOM listeners across a stop
would make a stopped page interactive and keep JavaScript callbacks rooted.

## Verification

Metis IPC tests cover late-response rejection, removal of pending and retained
entries, bounded counts and repeated cancellation. The frontend test covers a
dropped request returning to idle. The WASM host exports and builds
`metis_start`/`metis_stop`; the browser manual exercises stop/remount controls.
Moirai ADR 0045 records native cancellation-state tests, WASM compilation and
warning-denied Clippy for the provider handle.

## Limits

This increment does not provide an authenticated WebSocket service, origin
grant, cross-engine browser run, post-drop JavaScript allocation measurement,
or native desktop window. A task that performs long synchronous work before an
await still occupies the browser thread until it yields.
