# 0008 — Browser host boundary

Status: Accepted

Date: 2026-09-06

Drivers: [METIS-BROWSER-001](../../backlog.md#METIS-BROWSER-001),
[METIS-ASYNC-001](../../backlog.md#METIS-ASYNC-001).

## Context

Metis needs an executable HTML5/CSS target without moving browser resource
ownership into application crates. The async IPC seam and browser WebSocket
transport already exist, but a browser page still needs an owned DOM boundary,
listener teardown and a user-visible failure when no privileged service is
configured. The downloaded WASM must not carry backend keys or infer authority
from possession of a page origin.

## Decision

`metis-web` is the WASM host crate. It mounts an application-authored semantic
form into an existing `#metis-app` element and renders Rust-owned state into
text nodes. The HTML document and CSS remain ordinary page assets; generated
wasm-bindgen glue is packaging output, not application logic.

Moirai owns the browser platform seam. `WebDocument`, `WebElement`,
`WebEvent` and `WebEventListener` wrap DOM access and remove callbacks when
their Rust handles drop. Metis does not import `web-sys` directly or create a
second event-loop/runtime implementation. `AsyncFrontendApp` and
`BrowserWebSocketTransport` remain the consumer seam for the future authorized
service bridge.

The current host has no service endpoint or session grant. A submit therefore
produces `ERR_CONNECTION_CLOSED`; it never performs the clinical calculation in
WASM and never displays a fabricated success. The eventual bridge must bind an
authenticated session to the host origin and reject navigation, replay,
oversize and cross-session requests before connecting this UI state to a result.

The raw `metis_start` and `metis_stop` exports are the WASM ABI boundary. Their
unsafe attributes are isolated and documented; the host state, DOM operations
and callbacks remain safe Rust.

Revision 2026-09-07: the host also exports `metis_stop`. It drops the mounted
listener guards before replacing the root, and `metis_start` clears any prior
application before attempting a remount so failed replacement cannot retain
callbacks for stale markup.

Revision 2026-09-07: [ADR 0011](0011-host-authority-policy.md) adds the shared
`HostOrigin`/`WindowId`/`HostSessionId` contract, host-bound capability HMAC
associated data and the strict external-asset CSP/navigation policy. This
closes the local authority-kernel increment; the browser service still has to
provide trusted Origin/session observations.

The provider revision for this lifecycle increment is Moirai `16a1b88`; its
native cancellation-state tests and WASM library checks are recorded in ADR
0045. Metis's local request cancellation and stop/remount trace extend the
boundary without claiming a live service.

## Alternatives

Direct `web-sys` calls in Metis would duplicate Moirai's handle and callback
ownership. Keeping only the software renderer would not preserve DOM/CSS
behavior. Connecting to an arbitrary URL from page markup would turn page data
into an authority decision, so endpoint selection waits for the origin/session
contract. The contract is now defined by ADR 0011; a live service must still
enforce it at its acceptor.

## Verification

Moirai's browser PAL at `16a1b88` passes its
WASM checks, strict Clippy and 39/39 PAL tests. Metis builds `metis-web` for
`wasm32-unknown-unknown`; `scripts/browser.py build` generates the loader and
WASM artifact with `wasm-bindgen` 0.2.128. A local browser trace loaded the
page at `http://127.0.0.1:8765/index.html` with a 1280×720 viewport and device
scale 1.25. It changed weight to 80 and dose to 0.75, rejected a hostile
numeric edit with `ERR_NUMERIC_INSTABILITY`, and exposed
`ERR_CONNECTION_CLOSED` on submit. The Codex in-app browser did not expose its
engine version; no cross-engine or post-drop allocation measurement is claimed.

## Residuals

The live browser service, service-side Origin/session validation, live task/late-response checks,
accessibility/IME evidence, Chromium/Firefox/WebKit matrix and desktop WebView
host remain open in the linked backlog items. Local request cancellation and
stop/remount listener teardown are covered by the Metis test and browser trace.
This decision establishes
the browser host boundary and runnable local controls; it does not establish
Tauri API parity, security superiority or lower memory use.
