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
scenario across engine names, input-sensitive result assertions, disconnected
privilege rejection, teardown state, screenshot validation and explicit
unsupported operations. `python -m py_compile` checks the runner and tests.
Each configured engine is run with the commands in the browser manual, and its
schema-1 trace plus PNGs are reviewed before the browser item can close. The
current Windows environment has no configured Chromium, Firefox or WebKit
WebDriver endpoint, so this increment supplies the runner and deterministic
harness evidence while the real three-engine and post-drop resource captures
remain open under `METIS-BROWSER-001`.
