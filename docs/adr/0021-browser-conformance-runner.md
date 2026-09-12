# ADR 0021: Cross-engine browser conformance runner

Status: Accepted

Date: 2026-09-09

Driver: [METIS-BROWSER-001](../../backlog.md#METIS-BROWSER-001).

## Context

Metis has a Rust/WASM browser workbench and a real authenticated loopback
service, but its existing evidence is tied to one in-app browser surface. The
browser lifecycle contract admits Chromium, Firefox and WebKit. A screenshot or
a successful WASM build cannot establish that the same controls, bridge result
and teardown behavior work in each engine. Browser automation must also remain
outside the downloaded page: it cannot receive credentials, add native
authority or become part of the application runtime.

The conformance trace needs two input-sensitive observations, an authorized
service result, cancellation of a delayed response, semantic state snapshots,
visual artifacts and a teardown assertion. Host polling and unbounded test
waits would make the evidence nondeterministic and would hide a lifecycle leak.

## Decision

Add `scripts/browser_runtime.py` as a dependency-free Python standard-library
scenario runner, with the W3C client and loopback server in the adjacent
`scripts/browser_protocol.py` module. The closed engine set maps
`chromium`, `firefox` and `webkit` to the `chrome`, `firefox` and `safari`
capabilities. The host or CI environment supplies the driver endpoint through
`--driver-url` or an engine-specific environment variable; the repository does
not install or embed a browser driver.

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

The same client exposes bounded W3C `actions` and `release actions` commands
for format-neutral canvas consumers. `pointer_drag` emits a trusted pointer
source with element-local coordinates, and `wheel` emits a trusted wheel
source; source count, action count, coordinates and serialized trace size are
validated before the driver request. These methods carry no application or
medical semantics. RITK owns the consumer scenario and maps the resulting
events to its viewer reducer.

The runner also has a `canvas` scenario. It validates a finite list of HTML
canvas identifiers, captures the complete browser window and each element,
records intrinsic/CSS dimensions, applies the bounded pointer and wheel actions
to every canvas, and explicitly releases all WebDriver sources. The trace
stores the Metis revision and an optional 40-hex consumer revision separately.
This makes the runner reusable for RITK and other Atlas consumers without
moving their state or format policy into Metis.

Revision 2026-09-11: the canvas scenario accepts a finite allowlist of
consumer-selected `data-*` attributes. It records each requested value, or
`null` when absent, under the canvas snapshot with a 16-name and 1024-byte
per-value bound. The runner treats names and values as opaque; a consumer
owns their interpretation and assertions. This supplies RITK's browser
workflow with semantic evidence while keeping DICOM and viewer meaning out of
Metis.

Revision 2026-09-11: the canvas scenario installs a bounded capture-phase
observer for pointer and wheel events. Each action records the observed event
type, target canvas and browser `isTrusted` value, rejecting missing, mixed,
untrusted or mis-targeted records. The observer is removed before the final
window capture, so the trace proves the runner's own diagnostic listeners are
released while leaving provider-private listener counts outside the WebDriver
contract.

Revision 2026-09-11: the file-backed gallery captures the paired canvas trace
before its bounded overflow rejection probes. When the paired actions move a
slice, rejection invariance compares each rejected batch with that post-input
pixel baseline, keeping the committed window capture on the accepted study.

The runner declares `native-file-dialog`, `native-process-launch` and
`os-permission-grant` unsupported for this browser surface. Native authority,
filesystem handles and DICOM parsing remain outside Metis: RITK owns the DICOM
scanner, decoder, geometry and viewer workflow, while Moirai owns confined
filesystem handles. This runner exercises only the format-neutral Metis
presentation and service boundary.

## Alternatives

Using only the Codex in-app browser would preserve a useful visual trace but
would not identify the engine or provide a reproducible Chromium/Firefox/WebKit
matrix. Selenium, Playwright or a JavaScript test runner would add a runtime
dependency and a second application stack to the verification path. Separate
scripts per browser would duplicate the scenario and allow assertions to drift.
Polling from Python or sleeping for delayed responses would make teardown
timing part of the host rather than testing browser lifecycle events. A
browser-side DICOM reader would duplicate RITK's medical-format authority and
is explicitly rejected.

## Threat model and limits

Driver responses, script results, diagnostics, screenshots and input values
are bounded. Element identifiers are URL-encoded before path construction;
trace and screenshot paths are confined to `output`, and the optional static
server is confined to `output/browser`. Authorized URL fields are validated but
carry no secret; the service still authenticates the session and origin.

The cleanup assertion observes the public DOM and the absence of a stale
completion after the lifecycle generation changes. It records zero pending
public request state only after the remounted form reports `aria-busy=false`;
it cannot inspect a provider's private listener registry or operating-system
handles. Such evidence belongs to the Moirai/native host tests and to a driver
with the relevant instrumentation.
Actual engine evidence exists only when a configured W3C endpoint is run; a
missing endpoint is a failed invocation, never a skipped matrix cell.

## Verification

`python -m unittest discover -s scripts/tests` covers the protocol-shaped
workbench and canvas scenarios across engine names, input-sensitive result
assertions, bounded W3C pointer/wheel payloads, element and full-window
screenshot validation, opaque consumer-attribute capture and bounds,
disconnected privilege rejection, teardown state and explicit unsupported
operations. `python -m py_compile` checks the runner and tests.
Each configured engine is run with the commands in the browser manual, and its
schema-1 trace plus PNGs are reviewed before the browser item can close. The
current Windows environment has no configured Chromium, Firefox or WebKit
WebDriver endpoint, so this increment supplies the runner and deterministic
harness evidence while the real three-engine and post-drop resource captures
remain open under `METIS-BROWSER-001`.
