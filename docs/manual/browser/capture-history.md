# Browser capture history

[Browser manual](../browser.md)


### Current live fragment capture — 2026-09-14

The current Metis revision `ed3806811f23271310cb04078dff55aba5c90944` was
opened in the Codex in-app Chromium host at
`http://127.0.0.1:8080/http-health.html` while a real loopback
`metis-app.exe --metis-http-service` process served the boundary on port
`8766`. The [machine-readable provenance record](../images/metis-http-fragment-live.json)
binds the source revision, executable digest, service origin and viewport
artifacts. It records three user-visible states from the same live page:

1. [Success](../images/metis-http-fragment-live-success.jpg) reports the `200`
   health response, `200` handshake, one accepted fragment patch, all four
   negative probes and lifecycle generation `1`.
2. [Reset](../images/metis-http-fragment-live-reset.jpg) reports generation `2`
   with empty response, fragment and negative fields after **Reset mount**;
   the prior generation is marked stale.
3. [Recovery](../images/metis-http-fragment-live-recovered.jpg) reports a new
   accepted fragment patch on generation `2` after remount.

These are viewport captures of the actual HTML5/CSS presentation boundary,
not generated artwork. The probe contains no DICOM bytes or patient
identifiers; DICOM loading, decoding and viewer state remain in RITK. This is
one live Chromium observation and does not close cross-engine WebDriver,
provider-private allocation, TLS or operating-system permission evidence.

### Live Edge WebDriver fragment capture — 2026-09-14

The dependency-free runner also completed the same scenario through the
bundled Microsoft Edge WebDriver `153.0.4234.19` and Edge `154.0.4258.12`.
At Metis revision `77b1c278d610562b04d138aea2d37b828ff097a5`, the run used a
1500 × 1074 CSS viewport at device scale `1.25`, captured three 1875 × 1343
PNG states, and closed the WebDriver session. Pending request state was not
measured by that runner. The
[provenance record](../images/metis-http-fragment-webdriver-edge.json) contains
the exact hashes and semantic trace:

```text
python scripts/browser_runtime.py --engine chromium --browser-name MicrosoftEdge \
  --driver-url http://127.0.0.1:9517 --scenario fragment \
  --url http://127.0.0.1:8080/http-health.html \
  --output output/browser/runtime/chromium-fragment.json
```

The [success](../images/metis-http-fragment-webdriver-edge-success.png),
[reset](../images/metis-http-fragment-webdriver-edge-reset.png) and
[recovered](../images/metis-http-fragment-webdriver-edge-recovered.png) captures
show health and handshake `200`, one accepted patch, malformed `400`,
unauthorized `401`, target rejection, unchanged stale state, reset generation
`2` and remounted generation `2`. This is one Chromium-family
WebDriver observation on Windows; Firefox, WebKit, TLS, operating-system
permissions and provider-private resource counts remain separate evidence.

The scheduled or manually dispatched Metis verification workflow now runs the
same authenticated fragment trace in its Chromium, Firefox and WebKit matrix
jobs. Each job builds the real `metis-app` HTTP service, serves the generated
`http-health.html` page, waits for `GET /health`, and writes
`output/browser/runtime/<engine>-fragment.json` with the three inspected PNG
states. Uploaded traces bind to the workflow revision; a missing driver,
service start failure or browser capability remains an explicit failed job.
The current runner requires `aria-busy="false"` on the HTTP status and an
enabled fragment button at each captured state. The page updates this
attribute with its request lease; reset aborts that lease before clearing it.
The cleanup record derives its request state from the remounted observation.
This measures the page's request lease, not provider-private resource counts
or server-side cancellation completion.

[Input Events]: https://w3c.github.io/input-events/
