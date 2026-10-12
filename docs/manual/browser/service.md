# Browser service

[Browser manual](../browser.md)


## Connect the real browser service

The service executable is the same `metis-app` image used by the native
demonstration. It accepts one loopback WebSocket session, checks the request
`Origin` before sending `101 Switching Protocols`, constructs a trusted
`HostContext`, and then runs `AsyncIpcServer` with bounded messages and finite
deadlines. Start it in a second terminal while the static server is running:

```text
cargo run --locked -p metis-app -- --metis-browser-service http://127.0.0.1:8080 8765 66666666666666666666666666666666
```

Open the workbench with host configuration in the URL (the bootstrap copies
these values into the hidden configuration fields before Rust mounts the app):

```text
http://127.0.0.1:8080/?endpoint=ws%3A%2F%2F127.0.0.1%3A8765%2Fsocket&process=42&principal=66666666666666666666666666666666
```

The endpoint, process identifier and 32-hex-digit principal are transport
configuration, not credentials. The service's trusted context and capability
signature remain authoritative. The demonstration is loopback-only and uses
`ws://`; it does not establish TLS server authentication.

## Verify stop/remount disposal at the service boundary

The browser service can delay a successful clinical response for a bounded
interval. This exercises the same WebSocket and `AsyncIpcServer` path as the
normal workflow while leaving handshake, rejection and event frames immediate:

```text
cargo run --locked -p metis-app -- --metis-browser-service http://127.0.0.1:8080 8765 66666666666666666666666666666666 --response-delay-ms 4000
```

With the configured workbench URL open, wait for **Authorized backend session
ready**, activate **Submit to authorized backend**, and immediately activate
**Stop host**. The stopped page must contain only `Metis browser host stopped.`
Start the host again before the four-second delay expires. The remounted form
must show its new generation and either the fresh session state or the explicit
`Backend unavailable [ERR_TRANSPORT_BROKEN]` state when the one-connection
conformance service has closed; it must not show the old result. After at least
four seconds, inspect the page again. The accessibility tree and screenshot
must remain unchanged, with no stale result or event from the stopped request.

The 2026-09-07 trace used the Codex in-app browser at a 1280×720 CSS-pixel
viewport and device scale 1.25. It observed **Request in progress**, stopped
the host, remounted it, then waited beyond the delay. The remounted tree stayed
at `Backend unavailable [ERR_TRANSPORT_BROKEN]` with `Remote events: none`; the
delayed response did not mutate the new DOM. The browser engine version and
post-drop allocation counts were unavailable, so this is lifecycle evidence
for one engine and the real loopback service, not cross-engine or allocation
proof.

## Exercise the real Rust state

Change the weight, concentration or dose fields. The result panel updates from
the Rust-owned `FormInputs` and the status returns to idle for each valid edit.
Enter a non-numeric value to observe the typed `ERR_NUMERIC_INSTABILITY`
display. With no service configuration, submit the form to observe
`ERR_CONNECTION_CLOSED`; this is an explicit disconnected result, not a local
calculation or a fabricated success. The submit control is disabled until an
authenticated bridge is ready and becomes disabled again while a request is in
flight; changing the DOM or dispatching a click cannot bypass that lifecycle
guard.

The **View options** fieldset uses semantic HTML5 controls owned by the Rust
host. Uncheck **Show remote events** to hide the event status line while the
event remains received and validated; select **Volume rate** or **Drug mass
rate** to change the displayed metric; move **Result scale** with the arrow
keys or pointer to select a bounded 50–150% presentation preference; choose
**Clinical summary** or **Audit detail** from **Result detail** to change the
rendered response annotation. The
`options-state` text and `data-result-scale-percent` attribute expose the
Rust-owned state after each change. Tab through the labels, press Space on the
checkbox or radio, use the range arrows, and open the select to reproduce the
keyboard path.

## Explore retained results

The **Result explorer** is a bounded, host-independent view of real backend
responses. Each accepted response is retained under its validated patient
reference and audit sequence. Duplicate audit sequences update their row; the
history evicts its oldest row at the fixed capacity. The explorer never
calculates a value in the browser and never renders the response signature.

Use **Filter patient references** to restrict the visible tree, and choose an
order from **Order results** to sort by audit sequence, patient, volume rate or
drug rate. Patient groups are keyboard-operable disclosure buttons. Their rows
are keyboard-operable selection buttons, and the pager exposes at most eight
visible tree entries at once. The live status reports `empty`, `loading`,
`ready` or a typed producer error; a failed refresh leaves previously accepted
rows available for inspection.

The native `metis-frontend` tests exercise inserted and updated responses,
exact sort and filter values, stable selection, eviction, replacement,
disclosure, paging and malformed input. The browser asset test checks the
labels, table caption, live region, control IDs and delegated listener events.
For a browser capture, build and serve the workbench, scroll to **Result
explorer**, and inspect its empty state before connecting a service. The
2026-09-08 CUA trace used the generated page at
`http://127.0.0.1:8095/?cache=explorer-20260908` in a 1280×720 CSS-pixel
viewport at device scale 1.25. The screenshot showed the result card, filter,
order select, table caption and disabled pager. Changing the filter to `PT`
and then `X` updated the table caption and returned `Entries 0 of 0`; keyboard
deletion cleared it. No backend bridge was configured for this capture, so it
does not claim a live-row rendering; the real row path is covered by the Rust
tests and the connected-service trace remains required for viewer acceptance.
The exact gate and runtime limitations are recorded in the [V07 result explorer
evidence](../../VERIFICATION.md#result-explorer-evidence--2026-09-08).

[Input Events]: https://w3c.github.io/input-events/
