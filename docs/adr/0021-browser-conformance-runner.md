# ADR 0021: Cross-engine browser conformance runner

Status: Accepted

Date: 2026-09-09, revised through 2026-09-18 with each revision folded into
the Decision.

Driver: `METIS-BROWSER-001`.

## Context

Metis has a Rust/WASM browser workbench and a real authenticated loopback
service, but its existing evidence is tied to one in-app browser surface. The
browser lifecycle contract admits Chromium, Firefox and WebKit, and a
screenshot or successful WASM build cannot establish that the same controls,
bridge result and teardown behavior work in each engine. Automation must also
stay outside the downloaded page: it cannot receive credentials, add native
authority or become part of the application runtime.

The trace needs two input-sensitive observations, an authorized service result,
cancellation of a delayed response, semantic snapshots, visual artifacts and a
teardown assertion. Host polling and unbounded waits would make the evidence
nondeterministic and hide a lifecycle leak.

## Decision

Add `scripts/browser_runtime.py` as a dependency-free Python standard-library
scenario runner, with the W3C client and loopback server in the adjacent
`scripts/browser_protocol.py` module and the CLI orchestration in
`browser_runtime_cli.py`; `browser_runtime.py` stays the entry point. The closed
engine set maps `chromium`, `firefox` and `webkit` to the `chrome`, `firefox`
and `safari` capabilities, and the W3C `MicrosoftEdge` name also selects the
Chromium scenario. The host or CI environment supplies the driver endpoint
through `--driver-url` or an engine-specific environment variable; the
repository does not install or embed a browser driver.

The runner serves the generated `output/browser` directory only when the
disconnected scenario is selected. Authorized scenarios use an already-running
URL whose `endpoint`, decimal `process` and 32-digit hexadecimal `principal`
query values are validated before navigation. The page remains the authority
boundary: the runner waits for the authenticated status and checks the exact
service result `Volume rate: 0.900000 mL/hr`.

Each run changes `weight-kg` to `80` and `target-dose` to `0.75`, dispatches the
delegated Rust change action, and records the displayed values. Authorized
runs submit the real service request; `--cancel` stops and remounts the page
while a bounded delayed response is in flight. Every run then stops and
remounts the host. Waits use one browser-side `MutationObserver` or timer, not
host sleeps or polling. The trace schema records the selected engine, driver
capabilities, actions, semantic snapshots, PNG screenshots, explicit native
operation restrictions and cleanup evidence. A stopped root must contain the
stopped message and zero application controls; a remounted generation must not
contain the old result.

An opt-in accessibility probe observes media preferences, CSS and visual
viewport scale, document extents, DOM focus order and bounded focusable-element
geometry. It focuses visible enabled controls in DOM order, checks the
sequence and blurs the probe focus; Python rejects duplicate identities,
horizontal overflow, empty or off-viewport rectangles and malformed media data.
`--require-reduced-motion` and `--require-forced-colors` make host preferences
acceptance conditions without emulating them; screen-reader speech and native
accessibility bridges remain host-specific evidence.

The same client exposes bounded W3C `actions` and `release actions` commands
for format-neutral canvas consumers. `pointer_drag` emits a trusted pointer
source with element-local coordinates, and `wheel` emits a trusted wheel
source; source count, action count, coordinates and serialized trace size are
validated before the driver request. These methods carry no application or
medical semantics; RITK maps the resulting events to its viewer reducer.

The `canvas` scenario validates a finite list of HTML canvas identifiers,
captures the complete browser window and each element, records intrinsic/CSS
dimensions, applies the bounded pointer and wheel actions to every canvas, and
explicitly releases all WebDriver sources. The trace stores the Metis revision
and an optional 40-hex consumer revision separately, so RITK and other Atlas
consumers reuse the runner without moving their state or format policy into
Metis.

- A finite allowlist of consumer-selected `data-*` attributes is recorded per
  canvas, or `null` when absent, within a 16-name and 1024-byte per-value
  bound. Names and values are opaque, which keeps DICOM and viewer meaning out
  of Metis.
- A bounded capture-phase observer records each action's event type, target
  canvas and browser `isTrusted` value, rejecting missing, mixed, untrusted or
  mis-targeted records. It is removed before the final window capture, so the
  trace proves the runner's own listeners are released while provider-private
  listener counts stay outside the WebDriver contract.
- The file-backed gallery captures the paired canvas trace before its bounded
  overflow rejection probes, and rejection invariance compares each rejected
  batch with that post-input pixel baseline. The generic probes and comparison
  complete before any consumer capture callback, which may stop or remount the
  host as part of its own teardown; running it first would remove the generic
  file picker and tie the host oracle to consumer lifecycle details.

The runner accepts an optional fixed-point device scale between `0.5` and `4`.
Chromium and Edge receive their native force-scale launch arguments, Firefox
receives `layout.css.devPixelsPerPx`, and WebKit rejects non-1 requests because
its WebDriver surface has no scale capability. After navigation every scenario,
the gallery and its paired canvas trace included, probes `devicePixelRatio`,
`innerWidth` and `innerHeight`; a requested scale must match the observed ratio
within one fixed-point unit, and the trace records both.

The runner declares `native-file-dialog`, `native-process-launch` and
`os-permission-grant` unsupported for this browser surface. Native authority,
filesystem handles and DICOM parsing stay outside Metis: RITK owns the DICOM
workflow and Moirai the confined filesystem handles.

## Alternatives

Using only the Codex in-app browser would preserve a useful visual trace but
would not identify the engine or provide a reproducible Chromium/Firefox/WebKit
matrix. Selenium, Playwright or a JavaScript test runner would add a runtime
dependency and a second application stack to the verification path. Separate
scripts per browser would duplicate the scenario and let assertions drift.
Polling or sleeping for delayed responses would make teardown timing part of
the host rather than testing lifecycle events. A browser-side DICOM reader would
duplicate RITK's medical-format authority and is rejected.

## Threat model and limits

Driver responses, script results, diagnostics, screenshots and input values
are bounded. Element identifiers are URL-encoded before path construction;
trace and screenshot paths are confined to `output`, and the optional static
server is confined to `output/browser`. Authorized URL fields are validated but
carry no secret; the service still authenticates the session and origin.

The cleanup assertion observes the public DOM and the absence of a stale
completion after the lifecycle generation changes, recording zero pending
request state only after the remounted form reports `aria-busy=false`. It
cannot inspect a provider's private listener registry or operating-system
handles; that evidence belongs to the Moirai/native host tests and to an
instrumented driver. The scale probe observes the CSS viewport and pixel ratio
only, not a physical monitor transition or native compositor behavior. Engine
evidence exists only when a configured W3C endpoint is run; a missing endpoint
is a failed invocation, never a skipped matrix cell.

## Verification

`python -m unittest discover -s scripts/tests` covers the protocol-shaped
workbench and canvas scenarios across engine names, input-sensitive result
assertions, bounded W3C pointer/wheel payloads, element and full-window
screenshot validation, opaque attribute capture and bounds, disconnected
privilege rejection, teardown state, unsupported operations, device-scale
parsing, engine capabilities, ratio matching and the WebKit rejection;
`python -m py_compile` checks the runner and tests. Each configured engine runs
the browser manual's commands, and its schema-1 trace plus PNGs are reviewed
before the browser item can close. This Windows environment has no configured
WebDriver endpoint, so the increment supplies the runner and deterministic
harness evidence; real three-engine and post-drop resource captures remain open
under `METIS-BROWSER-001`.
