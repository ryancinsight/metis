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
`BrowserWebSocketTransport` form the client seam for the authorized service
bridge; the native service composition lives in `metis-backend` and
`metis-ipc`.

Without service configuration a submit produces `ERR_CONNECTION_CLOSED`; it
never performs the clinical calculation in WASM and never displays a fabricated
success. When configured, the browser client connects to the bounded Moirai
WebSocket service. The acceptor validates the observed Origin before the 101
response, constructs a trusted host session and then applies Metis handshake,
replay, oversize and capability checks before connecting UI state to a result.

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
closes the local authority-kernel increment; the browser service now consumes
that contract at its pre-response Origin validator.

Revision 2026-09-07: the host reads optional endpoint/process/principal values
from host-provided configuration fields. The bootstrap may populate those
fields from a query string for the local demonstration, but the values do not
grant authority; the service's trusted context and capability signature remain
authoritative.

Revision 2026-09-07: lifecycle generation exhaustion is terminal. A failed
start/stop boundary drops the mounted application before reporting the error,
so completions from the exhausted generation cannot mutate a stale DOM. The
browser session mapper preserves remote handshake error codes in
`FormState::SessionFailed`; local transport failures remain typed disconnects.

The provider revision for this lifecycle increment is Moirai
`be87d009cd0e877beef719b47bdcbadc45659069`; its
native cancellation-state tests and WASM library checks are recorded in ADR
0045. Metis's local request cancellation, live service loopback tests and
stop/remount trace extend the boundary without claiming TLS or OS enforcement.

## Alternatives

Direct `web-sys` calls in Metis would duplicate Moirai's handle and callback
ownership. Keeping only the software renderer would not preserve DOM/CSS
behavior. Connecting to an arbitrary URL from page markup would turn page data
into an authority decision, so endpoint selection waits for the origin/session
contract. The contract is now defined by ADR 0011; a live service must still
enforce it at its acceptor.

## Verification

Moirai's browser PAL at
`be87d009cd0e877beef719b47bdcbadc45659069` passes its
WASM checks, strict Clippy and 39/39 PAL tests. Metis builds `metis-web` for
`wasm32-unknown-unknown`; `scripts/browser.py build` generates the loader and
WASM artifact with `wasm-bindgen` 0.2.128. A local browser trace loaded the
page at `http://127.0.0.1:8080/` with a 1280×720 viewport and device scale
1.25. It connected to the service at `ws://127.0.0.1:8765/socket`, submitted
valid values, rejected zero weight with `0x3001`, observed service disconnect
and recovered after a new service session. The Codex in-app browser did not
expose its engine version; no cross-engine or post-drop allocation measurement
is claimed.

## Residuals

Post-drop allocation measurement, accessibility/IME evidence,
Chromium/Firefox/WebKit matrix and desktop WebView host remain open in the
linked backlog items. Local request cancellation,
pre-response Origin validation and stop/remount listener/task teardown are
covered by the Metis tests and browser trace. This decision establishes the
browser host boundary and runnable local controls; it does not establish TLS,
OS permission isolation, Tauri API parity, security superiority or lower memory
use.
