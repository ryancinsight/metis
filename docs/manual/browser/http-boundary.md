# Browser http boundary

[Browser manual](../browser.md)


## Current Rust HTTP evidence — 2026-10-12

The current Rust host passes a real local browser session against the Rust
loopback backend: exact Unicode and empty fragments, malformed 400, missing
session 401, changed issued grant 403, target rejection and unchanged stale
patches. Reset advances generation 1 to 2 and admits a new fragment. Stop
drops all four HTTP listener handles, exposes count zero and disables controls.
Reload recovers the original active handshake response. A four-second delayed
request leaves the reset mount idle with all response fields cleared.

The independent streamed HTTP producer verifies every byte for lengths 0, 1
and 16,384; 16,385 decoded bytes and an oversized Content-Length reject. It
checks bodyless 204/205/304 statuses, including 304 representation metadata
above the body budget; echoes exact Unicode POST bytes, observes zero followed redirects, delays
headers and bodies past declared deadlines, and observes connection closure
after the owned Fetch task drops. These are local in-app WebKit observations;
they do not establish physical iOS permissions or cross-engine equivalence.
Run the reproducible probe with `python scripts/http_transport_fixture.py build`,
serve `output/browser` on 8080, run `python scripts/http_transport_fixture.py serve`
and open `/http-transport/`. Port 8767 belongs to this independent fixture;
it must be free before starting. The normal application remains on 8766.

## Hypermedia boundary

The [htmx documentation](https://htmx.org/docs/) models a browser interaction as
an event that issues a request and places the response in a selected target.
`hx-target` names the target and `hx-swap` defines standard DOM replacement
modes ([target reference](https://htmx.org/attributes/hx-target/), [swap
reference](https://htmx.org/attributes/hx-swap/)). Metis uses that separation as
an implementation rule without shipping an htmx runtime or accepting arbitrary
HTML from a backend.

Ordinary `input` and `change` events bubble to `#metis-app`. Rust maps the
target ID to a closed input/control binding, applies the typed state transition,
and updates only the allowlisted text and attributes. Pointer, file, text and
dialog events keep their specialized Moirai provider listeners. Static
application markup is installed once; backend values and diagnostics always use
text or attribute setters, so a message cannot create a script or element.
The authenticated binary WebSocket remains the current service contract. The
first typed fragment action uses it: activating **Session details** sends a
generation-bound `FragmentAction` for `status.describe` through the scoped
`ui` plugin. The backend returns a bounded `FragmentPatchSet`; the browser
preflights the allowlisted target and applies the result with a text setter. The
response cannot add markup, scripts, selectors or navigation, and the existing
`metis-events` status target keeps the static control inventory unchanged.

The HTTP demonstration applies the same contract in its own request lifecycle.
It preflights every target and attribute before any DOM setter, reads response
bodies through a 16 KiB streamed bound, and associates health, handshake,
fragment and negative-probe requests with the current mount lease. Reset aborts
the lease, advances the generation and clears the old fragment result;
stale completions cannot update the new page.

Exercise it with an authorized service session by activating **Session details**
and inspecting `metis-events`. The value must contain
`Fragment action status.describe accepted: session-dialog`. Stop and remount the
host while a request is pending; the old generation must not update the new
mount. A configured WebDriver run is still required for cross-engine captures;
the native protocol and policy suites are the current deterministic evidence.

The admitted loopback HTTP demonstration implements that boundary over
Moirai. Start it beside the static workbench with a fixed browser origin:

```powershell
cargo run --locked -p metis-app -- --metis-http-service http://127.0.0.1:8080 8766 66666666666666666666666666666666
```

`GET /health` returns the bounded readiness text. `POST /v1/session` accepts a
binary `HandshakeRequestPayload`; the response is a typed
`HandshakeResponsePayload`. After that response, send the returned capability
token in a `PluginInvocationPayload` to `POST /v1/fragments`. The response body
is a `FragmentPatchSet` whose text and attribute operations are applied through
the same browser allowlists as the WebSocket path. Every request must carry the
exact configured `Origin`. Unknown routes, wrong methods, malformed envelopes,
missing sessions and a full session table return bounded
`ErrorResponsePayload` values. Moirai owns body, header, response, deadline and
connection limits; the application closes after its finite request budget.

The server's real loopback suite covers the successful handshake/fragment
journey, exact-origin CORS preflight, origin and route denial, malformed and
oversized bodies, a disconnected peer, idle-peer deadline and finite teardown.
The optional response-delay probe is bounded and asynchronous so the browser
reset path can be observed against a pending real request. It does not add an
Axum or htmx runtime and it never accepts arbitrary markup
or scripts. [ADR 0025](../../adr/0025-axum-server-boundary.md) records the
comparison and the first-party boundary. RITK remains responsible for DICOM
scanning, decoding, series selection, geometry and viewer state; this fragment
contract carries presentation messages only.

### Capture the authenticated fragment boundary

The dependency-free WebDriver runner has a dedicated `fragment` scenario for
the HTTP page. It waits for the real health response, authenticated handshake
and typed patch, then checks malformed (`400`), unauthorized (`401`),
unallowlisted-target and stale-generation probes before resetting the page and
running the patch again on the new generation. The runner captures the three
stable page states and closes the driver session cleanly. It requires a fixed
same-origin page because the HTTP service binds its exact `Origin` policy:

```powershell
python scripts/browser_runtime.py `
  --scenario fragment `
  --engine chromium `
  --driver-url http://127.0.0.1:9515 `
  --url http://127.0.0.1:8080/http-health.html `
  --browser-heap-sample `
  --timeout-seconds 30 `
  --output output/browser/runtime/chromium-fragment.json
```

Start the static server on port `8080` and the `--metis-http-service` role on
port `8766` with the same origin before running the command. The trace's
`authenticated-success`, `reset-stale-generation` and `remounted-success`
screenshots are component evidence; they do not claim DICOM decoding. RITK's
consumer-owned gallery remains the source for actual CT/MRI pixels. The
demonstration service retains one session per principal until its bounded
connection budget ends. A malformed or disconnected peer consumes a slot and
is reported as `browser_http_connection_error`; it cannot terminate the
listener for other peers. Origin and capability checks remain unchanged.
An exact repeated live handshake returns the original 200 response without
issuing a grant, extending expiry or advancing sequence. Reload and remount
reuse that response. A changed active process/version still returns 409;
principal and Origin denials remain unchanged.

Each captured state must expose `aria-busy="false"` and an enabled fragment
button. Reset must also clear the response, event and negative-probe fields;
the trace derives its idle request state from these observations. These are
page lease assertions, not counts of provider-private resources.

Hosted runs use `scripts/browser_static_server.py` to bind numeric loopback
without reverse DNS and atomically publish its selected port. If startup has
not published after ten seconds, the static-server log receives a stack dump;
the host still rejects startup at its existing twenty-second bound.

<a id="hosted-fragment-matrix-evidence--2026-09-16"></a>
### Hosted fragment matrix evidence — 2026-09-16

Manual dispatch [35173142639](https://github.com/ryancinsight/metis/actions/runs/35173142639)
passes all three browser jobs against revision `c1ee7dbc794adfdd6c00726ee3265c8a0a135eee`.
Each trace asserts health `200`, authenticated fragment `200 (1 patch)`,
malformed `400`, unauthorized `401`, target rejection, unchanged stale state,
reset to generation `2` and remounted fragment `200`. All three snapshots
observe `aria-busy="false"`, an enabled fragment button and idle request state.
Reset clears the response, events and negative-probe fields. Driver session
deletion completes and records `session_closed: true`.

| Engine | Capture artifact | JSON / three PNG bytes | Trace SHA-256 |
| --- | --- | --- | --- |
| Webkit 26.6.2 | [artifact 10477019679](https://github.com/ryancinsight/metis/actions/runs/35173142639/artifacts/10477019679) | 9725 / 279651 | `7899b466ebdf090d810ca2e309b2f00a874043a544a4b75f57d00a5120cc9934` |
| Chromium 152.0.7977.82 | [artifact 10477820928](https://github.com/ryancinsight/metis/actions/runs/35173142639/artifacts/10477820928) | 10315 / 114023 | `a946a23fb6af93e218afa982fdf3c9ec6af81f50787647922c65a861c7f85fac` |
| Firefox 155.0 | [artifact 10477212365](https://github.com/ryancinsight/metis/actions/runs/35173142639/artifacts/10477212365) | 9947 / 163110 | `f9de023c89e81507a1e4c01aae912b7678c357b7d3cf892331f8a8c962b80c4a` |

All nine PNG hashes match their trace records. WebKit's three 800 × 600
captures were inspected: authenticated response and rejection results,
cleared reset state, then the remounted generation's response are visible.
Chromium's 1240 × 474 and Firefox's 1152 × 552 captures match the previously
inspected images byte-for-byte; these viewport-limited images do not show
all response rows, which are asserted by their semantic traces. Each JSON
is below the 512 KiB trace limit and each PNG below the 8 MiB screenshot limit.

The failed preceding WebKit startup emitted no stack, so removing the
verified reverse-DNS dependency is not proof that DNS caused that hosted
stall. This replay establishes successful startup and fragment execution;
future startup stalls receive the bounded diagnostic described above.
The earlier Firefox service termination on a truncated peer is covered by
real TCP regression tests that also assert subsequent Origin rejection and
successful health responses. This replay does not claim to inject that fault.

That historical run uses the then-current JavaScript protocol consumer.
Metis 0.3.0 moves that behavior into Rust and the canonical binary codecs;
current local evidence and the teardown contract are recorded in ADR 0056.
The historical page lease and driver-session assertions do not establish
provider-private listener or allocation counts. Chromium
reports bounded `performance.memory` observations; Firefox and WebKit report
that API unavailable, so there is no cross-engine memory comparison. TLS,
operating-system permissions, physical input, accessibility and IME behavior
remain separate host evidence. The page contains no DICOM bytes; RITK owns
DICOM opening, decoding, geometry and clinical image meaning.

To demonstrate the boundary, serve the workbench, change **Weight (kg)** and
**Result scale**, and capture the form before and after each action at the same
viewport. The semantic tree must retain the same controls while the result text,
`data-result-scale-percent` attribute and `options-state` value change. Enter
`<img src=x onerror=alert(1)>` in **Clinical note**; the preview must show the
literal characters and the DOM must contain no added element. The existing
browser visual and semantic capture records the target engine, viewport and
rendered state for this check; the committed trace is in the [delegated-control
evidence](../../VERIFICATION.md#browser-delegated-control-evidence).

[Input Events]: https://w3c.github.io/input-events/
