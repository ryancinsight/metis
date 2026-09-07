# Verification

The owning gate is `python scripts/verify.py`. It records bounded logs under the
ignored `output/` directory and checks formatting, strict Clippy, debug and release
nextest suites, doctests, documentation, example execution and dependency closure.
Native tests use `.config/nextest.toml`: slow at 30 seconds, terminate at 60 seconds,
zero retries. The demonstration executable has a 60-second outer budget.

Entry baseline: `cargo check --workspace --offline` passes with documentation and
source warnings. The original native test build fails with E0382 in the threaded
process-isolation test. No original OS sandbox or native-window evidence exists.

## Evidence classes

- WASM portability: compile `metis-core`, `metis-platform`, `metis-ui-lang` and
  `metis-web` libraries for `wasm32-unknown-unknown`. Compilation is static
  evidence; it does not by itself validate host bindings or establish Tauri
  compatibility. The browser workbench has a separate runtime trace below.

- Types and compilation: frontend cannot import the backend through its declared dependency closure; validated policy fields cannot be overwritten externally.
- Behavioral tests: exact wire fixtures, canonical decoding, malformed corpus, scope/session/time rejection, audit event outcomes and bounded numerical error.
- Independent numeric evidence: dimensional infusion conversion and exact binary fixtures; arithmetic roundoff uses a stated gamma bound.
- Crypto evidence: Moirai `9f0c7fb9ed485797f3e9ab31df75b65063ac6ef3` publishes the shared HMAC/SHA-256 and
  fixed-width comparison primitives; independent vectors and streaming/padding
  regressions run upstream, while Metis capability, audit, result-signature and
  CLI tests exercise those functions at their real boundaries.
- Process acceptance: separate instances of one application executable exchange real pipes; PID, result and standalone relocation checks distinguish executable packaging from process state. These do not prove OS least privilege.
- Visual evidence: software framebuffer generated from actual form state and inspected independently of compilation.

Use `python scripts/verify.py`. Resolving commands run with `--locked` outside
the Atlas overlay, following Atlas's standalone-lock workflow. Inside Atlas,
the gate preserves the configured shared build directory and profile budgets;
it never disables the shared overlay or creates another build cache. The
committed lock must describe Git sources, without local-overlay substitutions
or unused-patch records. Earlier `--stack` verification is superseded by this
standalone gate so publishing cannot ship an overlay-only dependency graph.

The user manual replaces a domain book. Its seven application snapshots are produced by
the Rust presentation example from real backend exchanges and actual framebuffer
pixels, checked against `docs/manual/images/form*.svg`. `--update-snapshots`
explicitly refreshes those files and the semantic/fixture baseline
`docs/manual/images/captures.json`; normal verification rejects drift and missing
local manual links. Comparator unit tests and the actual capture-failure process
probe each run under a 60-second bound. A write failure must terminate the
Moirai session, collect the worker and preserve the primary diagnostic.

## Collected Windows evidence — 2026-09-05

The same pinned-toolchain gate also compiles `metis-core`, `metis-platform`,
`metis-ui-lang` and their Iris rendering dependency for
`wasm32-unknown-unknown`. No browser-runtime execution is claimed by this build.

The form-state gate passes on Rust 1.97.0,
`x86_64-pc-windows-msvc`: formatting, all-target Clippy with warnings denied,
88/88 debug tests, 88/88 release tests, twelve doctests, documentation with warnings
denied, real-process demonstration and presentation rendering. The 800×600 BMP
is inspected: title/status and all form labels fit; viewport background is filled.
The gate writes exact source hashes and the lock hash to
`output/verification.json`, rejecting source changes during a run.

The resolved host metadata contains 43 packages, including 19 registry packages
through Atlas providers. At that revision no Metis package declares a registry dependency; distribution
tooling subsequently admits the Serde JSON parser under ADR 0005.
Separate upstream evidence includes 39/39
Moirai transport tests and 18/18 Atlas overlay-generator tests, with a real Cargo
fixture detecting duplicate local/Git type identities before the generator fix.

The negative executable oracle matches `ERR_NUMERIC_INSTABILITY`, the wire error
code, rather than expecting rejected numeric input in diagnostic text. This
corrects the initial test's message assumption without relaxing rejection.

Windows tests do not prove Linux or macOS behavior. Miri does not execute Windows
native system calls; those require targeted lifecycle tests and further platform
instrumentation. That earlier increment resolved Moirai from pushed commit
`0514f11`, not local provider edits. The current browser-host increment advances
the standalone lock to provider revision
`9f0c7fb9ed485797f3e9ab31df75b65063ac6ef3`; comparative security/memory evidence
against Tauri and live-service browser tests remain required by [ADR 0002](adr/0002-web-application-contract.md).
Advisory scanning, coverage,
mutation analysis and cross-platform sandbox probes remain uncollected.

## Collected Windows distribution evidence — 2026-09-06

The earlier two-executable distribution increment passes 102 debug and 102 release native tests,
36 Python gate/oracle tests, strict Clippy, doctests and documentation. The
actual MSI installs into a custom directory, both installed process scenarios
match independent rational conversion oracles, and uninstall without an
`INSTALLDIR` override removes owned payload/registration/shortcut while preserving
a user-created file. The empty application Start Menu directory is removed.
`output/distribution/latest/workflow.json` records package hashes, commands and
outcomes; this is host workflow evidence, not signing or OS isolation evidence.

All seven gallery images retain identical pixels and semantic records. Updating
the dependency-lock-bound fixture changes only `captures.json`'s fixture hash;
no image or expected outcome changes. The gate records exact source hashes and
rejects a stale fixture rather than silently accepting the dependency change.

## Single-application verification — 2026-09-06

[METIS-APPLICATION-001](../backlog.md#METIS-APPLICATION-001) and
[ADR 0006](adr/0006-application-entry.md) require a copied and renamed
`metis-app` executable to complete input-sensitive sessions from a directory
without companion executables. Verify distinct parent/child PIDs, calculation
values against the arithmetic oracle, private wire output and audit completion.
Unknown/malformed role arguments, missing inputs, invalid numbers and direct
child invocation with EOF or malformed protocol must fail without a result or
recursive parent launch. Existing supervision deadlines and descendant cleanup
remain required. The portable and installed payload must each contain exactly
one application executable; repeat the real MSI install/run/uninstall and
user-file-preservation workflow.

The historical Windows gate passes 126 debug and 126 release native tests,
40 Python checks, WASM library compilation, strict Clippy, doctests, rustdoc,
examples and seven unchanged visual snapshots. The real MSI workflow verifies
one installed application executable, both input-sensitive process sessions,
shortcut ownership, removal and preservation of the user-created file. The
collected report binds outcomes to the exact revision and source hashes.
The distribution binary uses its help/manual documentation; disabling its
colliding rustdoc output leaves the root `metis` library as that path's owner.

The shared image includes both libraries. Dependency checks establish that the
frontend library does not import backend authority; they do not establish OS
permission isolation or exclusion of backend code from the child process.

## Browser workbench verification — 2026-09-06

The `metis-web` package builds for `wasm32-unknown-unknown`, and
`python scripts/browser.py build` generates `metis_web.js`,
`metis_web_bg.wasm` and the HTML page with `wasm-bindgen` 0.2.128. A local
HTTP server at `http://127.0.0.1:8765/index.html` loaded those generated
artifacts in the Codex in-app browser. The captured viewport was 1280×720 CSS
pixels at device scale 1.25; the browser harness did not expose an engine
version. Console error and warning logs were empty.

The accessibility trace showed the semantic form, four labelled controls and
the submit button. Changing weight from 72.5 to 80 and dose from 0.5 to 0.75
updated the Rust-owned result values to `80.00 kg` and `0.750 mcg/kg/min`.
Replacing the concentration with `x` produced `Input rejected
[ERR_NUMERIC_INSTABILITY]`; restoring `4` and submitting produced
`Backend unavailable [ERR_CONNECTION_CLOSED]`. The latter is the required
failure when no authenticated service bridge is configured. Screenshots from
the initial, invalid-input and disconnected states were inspected during the
trace; they are runtime observations, not committed browser golden images.

This evidence establishes HTML5/CSS loading, Rust/WASM state updates, semantic
focusable controls and explicit failure handling for the pre-service local
workbench. The live service trace below establishes the authenticated loopback
path. Late-response injection, post-drop resource counts, cross-engine behavior,
accessibility technology support and OS permission isolation remain open in
[METIS-BROWSER-001](../backlog.md#METIS-BROWSER-001),
[METIS-MEMORY-001](../backlog.md#METIS-MEMORY-001),
[METIS-PERF-001](../backlog.md#METIS-PERF-001),
[METIS-SERVICES-001](../backlog.md#METIS-SERVICES-001) and
[METIS-DESKTOP-001](../backlog.md#METIS-DESKTOP-001).

## Browser lifecycle evidence — 2026-09-07

After rebuilding the generated artifacts from the standalone lock at Moirai
`9f0c7fb9ed485797f3e9ab31df75b65063ac6ef3`, the Codex in-app browser loaded
`http://127.0.0.1:8765/index.html` and exposed `Start host` and `Stop host`
controls in the accessibility tree. Clicking **Stop host** removed the form
and exposed the exact text `Metis browser host stopped.`; the inspected
1280×720 screenshot contained only the lifecycle controls and stopped message.
Clicking **Start host** restored the form with the default values, labelled
controls and idle status. Browser console warnings and errors were empty.

This trace demonstrates that the WASM host drops its Rust-owned listener
guards, clears the mounted DOM, and remounts fresh form state through the same
module. It is the disconnected lifecycle baseline; the authenticated bridge
trace below covers the service path.

## Host authority and asset evidence — 2026-09-07

The `metis-core` host contract now parses canonical ASCII network origins,
rejects credentials/paths/wildcards/opaque schemes and invalid ports, canonicalizes
numeric ports including HTTP(S) defaults, and binds
one exact origin and window to a nonzero session principal. Seven core tests
cover positive authorization, origin/window/session substitutions, unbound
tokens and retargeted host signatures. `metis-backend` stores the trusted
context in each session and issues its fixed-width token with the host binding
as HMAC associated data; the default contained policy is `metis://native` and
window 1. Existing backend/IPC tests continue to cover one-handshake and
cross-session rejection.

The browser shell moved its CSS and module bootstrap to external same-origin
assets. Static asset tests verify that the document has no inline style or
module body, that its policy equals the `HostPolicy` source, that WASM is
permitted through `'wasm-unsafe-eval'`, and that the bootstrap blocks
cross-origin anchor navigation. The browser build rejects policy drift before
copying the page. The `frame-ancestors` directive requires a response header
from the native or service host; it is not an effective document-meta framing
control. The live service trace below supplies that host-side Origin check for
the loopback demonstrator.

This evidence establishes the local authority kernel, asset policy and its
consumer-side integration. It does not establish TLS endpoint policy, OS
permission isolation, post-drop allocation counts or cross-engine host parity.

## Live browser service evidence — 2026-09-07

The standalone lock resolves all Moirai packages to
`9f0c7fb9ed485797f3e9ab31df75b65063ac6ef3`, including the bounded HTTP/WebSocket
service and the cancellation-wakeup fix. The focused command
`cargo nextest run --locked -p metis-backend -p metis-ipc -p metis-core`
passes 55/55; native all-targets Clippy for `metis-backend` and `metis-ipc`,
the WASM `metis-web` check and WASM-target Clippy pass. The browser build
command `python scripts/browser.py build` produces the same WASM loader used by
the trace.

For the runtime check, a Python static server served `output/browser` at
`http://127.0.0.1:8080/` and the one-connection `metis-app
--metis-browser-service` accepted `ws://127.0.0.1:8765/socket` with principal
`66666666666666666666666666666666`. The Codex in-app browser opened the page
with endpoint, process and principal query values at a 1280×720 CSS-pixel
viewport and device scale 1.25; the engine version was unavailable and browser
console warnings/errors were empty.

The accessibility trace observed `Authorized backend session ready`, submitted
the default values and received the backend result. It then changed weight to
`80`, concentration to `4` and dose to `0.75`, received the echoed values,
rejected weight `0` with `Backend rejected request [0x3001]`, stopped the host,
restarted it after the service exited to observe `ERR_TRANSPORT_BROKEN`, and
recovered after starting a new service session. The native loopback tests also
assert the exact response values (`0.54375` and `2.175`), verify that an
unauthorized Origin receives no `101 Switching Protocols` response, and reject
replayed sequences and a 65,561-byte WebSocket message through the same service
adapter.

This is runtime evidence for the Rust/WASM DOM host, Moirai transport,
pre-response Origin validation, target-surface discovery and bounded backend
session. It does not close late-response injection, post-drop JavaScript
allocation, TLS, accessibility/IME, cross-engine or native desktop/OS
permission scenarios.

## Browser control evidence — 2026-09-07

The control model adds semantic HTML5 checkbox, radio and range inputs to the
same Rust/WASM workbench. `cargo nextest run --locked -p metis-web` passes
10/10, including a regression that applies visibility, display-unit and scale
changes to a successful response without clearing that response. Native
warning-denied Clippy, the WASM-target check and WASM-target Clippy pass for
`metis-web`; `python scripts/browser.py build` regenerates the loader and WASM
from the standalone lock at Moirai
`9f0c7fb9ed485797f3e9ab31df75b65063ac6ef3`.

In the authenticated service trace, the Codex in-app browser exposed the
semantic control names and values at a 1280×720 CSS-pixel viewport and device
scale 1.25; the engine version was unavailable. Pointer activation of **Drug
mass rate** selected the radio and changed the rendered metric to
`Drug mass rate: 2.175000 mg/hr` while retaining `Backend result received` and
the correlated event. Pointer activation of **Show remote events** changed the
event text to `Remote events: hidden by preference` while retaining the metric.
Two keyboard **Right** presses on **Result scale** changed its accessibility
value to `120`, updated `View options: ... scale 120%`, and left the visible
focus ring on the range. The browser console contained only expected Moirai
initialization entries and no warnings or errors.

This closes the checkbox/radio/range browser slice and its real pointer and
keyboard demonstration. Select/menu/dialog controls, pointer capture,
drag/drop, wheel/touch/modifier events, IME, accessibility technology,
cross-engine parity, post-drop allocation and native-window input remain open
under the linked backlog items.

## Final gate evidence — 2026-09-07

The delivered revision passes `python scripts/verify.py`. The gate reports zero
exit status for compiler identity, dependency metadata, revision and fixture
freshness, formatting, visual tests, WASM library checks, Clippy, debug and
release builds, distribution, debug and release nextest suites, doctests,
documentation, the runnable example, presentation checks and visual capture
comparison. The deliberate capture-failure probe exits 1 as its negative oracle;
the gate records that result as expected and still passes overall.

The debug and release native suites each run 196 tests with zero failures or
skips. The package workflow tests run 14/14. The gate resolves 155 packages and
records the exact revision, source hash, lock hash and visual report under the
ignored `output/` directory. All seven captures and three mutation probes pass.

## Typed command and event evidence — 2026-09-07

`METIS-COMMANDS-001` adds `CapabilityReq`/`CapabilityResp`,
`TargetCapabilityReq`/`TargetCapabilityResp` and
`PluginInvokeReq`/`PluginInvokeResp` to the existing versioned frame contract.
`CapabilityCatalogPayload` rejects unknown, response-only, duplicate and
over-limit identifiers before a caller can use the catalog. `IpcClient` and
`AsyncIpcClient` decode the same versioned results; the backend requires a
completed handshake and advertises its capability, heartbeat, clinical
calculation and plugin-invocation commands. A known but unadvertised audit
request returns the typed `ERR_UNEXPECTED_MESSAGE_TYPE` response.
Target discovery returns the host platform and only its installed surfaces;
the browser workbench also renders its local WASM/DOM/CSS descriptor. Native
window, operating-system permission, accessibility and IME surfaces remain
absent until a provider is implemented.

The focused command/event run `cargo nextest run --locked -p metis-core -p
metis-ipc -p metis-backend -p metis-frontend -p metis-web -p metis-app` passes
122/122 and covers the target descriptor, lifecycle guard and presentation
control state in addition to the earlier cases. It includes catalog round-trips,
version and malformed-entry rejection,
post-handshake service discovery, explicit unsupported-operation handling,
target descriptor round-trips and malformed-value rejection, remote event
envelope round-trips and bounds, synchronous send/receive,
event/response interleaving, identifier mismatch/replay rejection, typed plugin
invocation payloads and responses, bounded executor dispatch, scope and
unknown-plugin rejection, and the bounded `EventHub` tests for input-sensitive
fan-out, per-subscriber backpressure, unsubscribe and finite deadlines. Native
all-targets Clippy passes with `-D warnings`. The workspace
semver comparison rejected the exhaustive-enum extension under a minor
release, so the public `MessageType` change is classified as major and the
accepted ADR records that release classification; the manifests remain at
`0.1.0` until release authority assigns the next version.
The browser workbench renders the command catalog and target descriptor after
its authenticated handshake; the runtime trace remains a single Codex in-app
browser engine and does not close the cross-engine requirement. Core tests also validate typed
plugin manifest registration, duplicate and malformed metadata rejection,
operation-count limits and bounded registry capacity. The real backend
process-isolation test receives a backend-produced `clinical.result` event over
the synchronous memory transport after the correlated response and decodes its
typed body, identifier and rates. The authenticated Moirai WebSocket loopback
performs the same checks and then invokes a registered `websocket.increment`
plugin over the same framed transport, decoding its input-sensitive typed
response. Both client variants also exercise the typed target-discovery and
plugin invocation helpers and preserve peer error payloads. The asynchronous
client injection test also
rejects a canceled sequence and then receives a newer outstanding response;
browser-host late-response injection remains open alongside native desktop, OS
permission and cross-engine coverage.
The browser host maps local connection failures to `Disconnected` while keeping
remote handshake rejections in `SessionFailed` with their original wire code;
the mapping is covered by host-boundary unit tests. Lifecycle exhaustion is
terminal and rejects all completions from the exhausted generation.

The authenticated in-app browser trace also displayed `Registered frontend
extensions: workbench v1` after the Rust mount, alongside the host capability
catalog and `Remote events: none`. Submitting the default inputs rendered
`Remote event: clinical.result #3 (audit=3 rate=0.543750 ml/hr
drug=2.175000 mg/hr)`; the browser had decoded the typed event body and checked
it against the correlated response. A zero-weight submission displayed the
typed `Backend rejected request [0x3001]` state and
`Remote events: none (request rejected)`. Stopping replaced the root with the
stopped lifecycle state, and starting again recreated the form and plugin
metadata while surfacing `ERR_TRANSPORT_BROKEN` after the one-shot service had
exited. Restarting the service and starting the host again restored
`Authorized backend session ready`. The capture was made at a 1280x720 CSS
viewport with device scale 1.25; the engine version was unavailable. The tab's
console contained only the expected Moirai initialization log entries and no
warnings or errors. This is evidence for the single in-app browser engine and
does not close cross-engine or native desktop coverage.

<a id="visual-contract"></a>
## Visual and interaction contract

The software runner covers seven stable V01 states. CSV observations emitted by
the Rust example record actual inputs, actions, state, labels and text geometry
alongside independently supplied expected outcomes. The comparator binds these
to the current source/lock/compiler and rendering fixture, checks exact SVG bytes
and decoded BMP pixels, and compares reviewed semantic records. It rejects missing
captures, stale source mappings and wrong state even when an image appears valid.

Three deliberately altered renders change a label, geometry and color. Each must
produce a nonempty pixel difference against the initial form. They test the
comparator and never enter the application gallery. The contract below also
specifies the remaining real host work; the browser workbench now has local and
authenticated loopback runtime traces, while native desktop execution remains
unimplemented.

Every scenario has one application source and one declared input/action trace.
Tests assert application state, displayed values, layout/hit geometry and
accessibility semantics before accepting its pixels. Drive actual production
actions and backend requests; a mocked command result cannot prove an application
workflow. A renderer-only fixture remains labelled renderer-only.

Capture each meaningful stable state: initial, focused/selected, pending,
successful, edited-after-success, rejected, disconnected/cancelled and recovered,
where that state exists. Synchronize on application events or injected clocks;
do not use sleeps, retry flakes or disable errors to obtain a screenshot.

For deterministic software rendering, require exact pixels and exact generated
SVG bytes. Deliberately perturb a visible label, geometry and color in a test
fixture to prove the comparison detects each class. For browser/GPU/OS rendering,
pin fonts, locale, timezone, color scheme, reduced-motion setting, viewport,
device scale, engine/OS/driver versions and application revision. Keep separate
baselines where rasterization differs. Derive any perceptual tolerance from
repeated unchanged-reference captures under that configuration; record the
distribution and excluded regions with a reason. Never widen a threshold to hide
a changed label, misplaced control, missing glyph or clipped value. Independent
semantic/geometry failures always block even when a pixel difference is small.

For the software runner, `output/visual/latest/report.json` collects all seven
states and all three mutation probes; successful runs also write `manifest.json`.
Known reports rotate to `previous` at the next invocation. The gate invalidates
prior success before reading toolchain configuration, and records each command,
its deadline and expected exit status. `--help` does not start a run. Current
software metadata explicitly marks focus, accessibility, pointer dispatch and
responsive cancellation unsupported. The browser trace separately records DOM
focus and editing; browser cancellation and assistive-technology evidence remain
unsupported.

The complete host capture manifest records scenario ID, source/tree hash, action trace and
fixture hash, expected values, target/engine/driver, viewport/scale, fonts, image
hash and observed outcome. An expected/actual/difference image and semantic diff
must be available on failure. Include focus order and accessibility tree/action
results; screenshots cannot establish screen-reader operation or permission denial.

Run ordinary deterministic scenarios under the committed 30/60-second native
budgets, with zero retries; browser/native harnesses receive committed finite
budgets before introduction. Keep one shared build cache. Timing suites use a
separate committed total budget (300 seconds unless an independently derived
scenario requires another reviewed profile), on a controlled local machine;
CI keeps build/smoke and behavior checks, not wall-clock benchmark comparisons.
New output retention is bounded: current evidence uses fixed filenames in ignored
`output/`; expanded runner outputs keep latest plus one previous run per declared
scenario/target, evicting older derived runs. Only reviewed golden fixtures and
explicitly retained experiment evidence enter the repository.

The manual entry is part of each feature's Definition of Done: runnable source
and commands, prerequisites and target support, inputs/actions, expected values,
actual screenshots and relevant errors/recovery. Reuse the executable source,
not copied snippets. Regenerate the screenshots and validate links in the same
change; no mockup or staged prototype represents an unavailable application.
Instructions for the currently runnable case are in
[Inspect application output](manual/testing.md).

<a id="visual-scenarios"></a>
## Demonstration scenario matrix

These are acceptance specifications for development, not twelve completed apps.
Extend a few cohesive applications rather than create one binary per assertion.
The board owns implementation status and dependency order. Each target uses the
same scenario semantics with its real host implementation.

<a id="V01"></a>
### V01 — Isolated form state gallery

Current evidence: seven 800×600 software frames cover initial, success, edit,
rejection, same-session correction, disconnect and new-session recovery through
the production form. The backend runs on Moirai with bounded MemoryTransport;
this gallery does not prove OS process isolation or responsive host events. For inputs
60 kg, 2 mg/mL, 0.2 mcg/kg/min, the dimensional oracle is
`60 × 0.2 × 60 / 1000 = 0.72 mg/hour`, hence `0.36 mL/hour`; doubling weight
doubles those values. Use the numeric test suite's derived roundoff bound.
Assert labels carry submitted inputs and the correlated result. Edit after
success: old result/MAC cannot remain presented as current. Reject invalid input,
inject transport failure, then recover; assert typed status and cleared stale
values before capturing each state. The initial fixture must remain labelled
software output, not a working native control demonstration.

<a id="V02"></a>
### V02 — Browser and settings workbench

Load actual Rust/WASM and HTML/CSS in Chromium, Firefox and WebKit jobs. Exercise
pointer, keyboard, touch where supported, focus traversal, editable/selectable
controls, disabled state and a cancellable backend operation. Two input changes
must produce independently expected displayed results; confirm pending→cancel,
disconnect→recover and late-response rejection. Capture focus/error/success and
assert no remaining listeners, requests or tasks after teardown. A local-only
settings pane is allowed without backend authority; an authoritative calculation
must use the authenticated configured host/service and its real response.

<a id="V03"></a>
### V03 — Text and accessibility specimen

Use fixtures with combining accents, emoji sequences, mixed RTL/LTR text and CJK
composition. Assert grapheme-safe editing, exact committed strings, selection
and composition ranges, undo/clipboard and caret/line geometry. Capture preedit,
selection and committed text. Complete the form keyboard-only; assert semantic
roles/names/values/stable identities and focus actions. Exercise actual platform
screen readers and document results. Capture zoom/high-contrast/reduced-motion
states; an accessibility-tree snapshot alone does not prove usability.

<a id="V04"></a>
### V04 — Responsive layout and clipping

Exercise nested row/column and DOM flex/grid layouts, minimum/maximum sizes,
overflow/scrolling, alignment, theme and scaling. Use 360×640, 800×600 and
1440×900 CSS/logical viewports, at scale factors 1 and 2: these deliberately
partition narrow portrait, existing fixture and wide desktop, plus integer
high-DPI mapping. Assert box/clip/hit-target coordinates and no occluded required
controls. They are coverage points, not performance claims. Custom presentation
has explicit supported properties and returns `ERR_INVALID_CSS_STYLE` for
unknown or malformed declarations; browser semantics are not inferred from
similarly named custom enums. Add platform-specific fractional scale cases where
the declared host contract admits them.

<a id="V05"></a>
### V05 — Desktop lifecycle and isolation

Run actual Windows, macOS, Linux/Wayland and Linux/X11 hosts. Open two windows,
transfer focus, resize/move across display scales, enter text with native IME,
navigate allowed/denied content, close/reopen and terminate during an in-flight
request. Capture real windows and error states; assert process/handle/task drain,
positive authorized IPC and independent unauthorized file/network/process denial
probes. OS windows cannot be replaced by browser screenshots. Engine/driver
requirements are recorded per host; cross compilation is not runtime evidence.

<a id="V06"></a>
### V06 — Asset and graphics gallery

Render known-color/alpha geometry, images, vector paths, fonts, transformed and
nested-clipped content, with aspect-ratio/orientation oracles. Include malformed,
oversized and inaccessible assets and media load/playback failure/teardown.
Capture both successful content and diagnostic states. Test custom GPU output
against the software/analytical reference using the declared raster contract;
device loss/recreation is a lifecycle test, not an opportunity for silent fallback.

<a id="V07"></a>
### V07 — Result explorer

Use seeded rows with stable identities. Sort/filter/select, expand a tree, scroll
past the visible range, stream an update and remove a selected item. Assert exact
ordered IDs, values, selected identity and empty/error outcomes; capture top,
middle, selected and loading/error views. Measure allocated visible rows and
subscriptions against explicit capacity. Do not equate a screenshot of a large
list with virtualization or bounded-memory evidence.

<a id="V08"></a>
### V08 — Scoped services and recovery console

Exercise real file/dialog/store/audit, clipboard/menu/notification/deep-link,
network and sidecar operations as implemented. Pair every grant with a denial:
path/origin/argument escape, revoked grant, tampered record, expired request or
unsupported browser operation. Use temporary user-data fixtures and local test
servers. Capture user-visible outcomes and independently assert effects, audit
events, restart recovery and cleanup. No credentials or real patient data enter
the fixture, diagnostic trace or screenshot.

The authentication slice uses the Moirai provider with its TLS feature
disabled. `cargo tree --locked -p metis-core --edges normal` and the Metis
WASM check show the protocol graph contains only the provider's `hmac` and
`sha2` dependencies; `rustls` remains outside this graph. The frontend still
displays a received MAC without claiming it can verify that MAC. This is
dependency and value-semantic evidence, not a universal side-channel timing
proof or a security comparison with Tauri.

<a id="V09"></a>
### V09 — Migrated viewer and Tauri application

Pin a representative app and its command/event/channel/config/plugin inventory.
Run the same input traces against original and migrated builds, asserting
behavior and comparing relevant screens. Record the source/configuration delta,
retained JavaScript, Rust/WASM substitutions, unsupported target APIs and native
Metis implementations. Use a form/settings application and a document/result
explorer to exercise different native surfaces. Neither canvas resemblance nor
forwarding requests into Tauri proves migration completion.

The named viewer target is RITK's `ritk-snap`, inspected at
`341228ee3861c5e9a091dcf58de500510f948505`. It currently uses egui/eframe,
including the web canvas entrypoint; no Tauri dependency was found in its
manifest or workspace lock. It supplies the concrete egui migration scenario;
a separate real Tauri fixture still establishes Tauri API/configuration coverage.
Implementation and DICOM prerequisites belong to the
[RITK viewer item](../../ritk/backlog.md#RITK-SNAP-METIS-001) and its
[decision](../../ritk/docs/adr/0026-viewer-presentation-migration.md).
These are acceptance specifications, not a Métis viewer availability claim.

| Opening/display boundary | Required evidence |
| --- | --- |
| Actual input | Open a file, directory, DICOMDIR and browser-provided byte batch through their real host paths; select series explicitly. Mixed studies or tied series counts cannot silently combine. |
| Pixel decode | Known stored values, signedness, modality rescale and decoded color channels; pin the admitted transfer-syntax/photometric matrix, test each row and reject unsupported rows explicitly. A decoder's declared support is not runtime evidence. |
| Frames and geometry | Every expected frame is reachable; per-frame metadata is retained. Assert physical landmarks, spatial ordering, anisotropic spacing and oblique orientation; temporal frames are not treated as spatial slices. |
| Actual image | Axial/coronal/sagittal captures with known voxel/color landmarks, matching cursor readouts, orientation labels and aspect ratios; preserve RGB channels and admitted grayscale/VOI behavior. |
| Interaction and recovery | Series selection, slice/frame navigation, window/level, zoom/pan and linked cursor change the expected pixels/state. Failed replacement, cancellation, rapid study switching and close/reopen cannot display a stale study as current. |
| Host and resource bounds | Local file grants and invalid-reference denial, malformed/truncated data, unsupported syntax and bounded decode/task/buffer lifetime; real browser and native hosts are tested separately. |

Physical-coordinate oracles use DICOM PS3.3 2026c
[C.7.6.2](https://dicom.nema.org/medical/dicom/current/output/chtml/part03/sect_C.7.6.2.html),
including row/column spacing and patient-position/orientation mapping.
Grayscale oracles use
[C.11.2](https://dicom.nema.org/medical/dicom/current/output/chtml/part03/sect_C.11.2.html),
including distinct default LINEAR, LINEAR_EXACT and SIGMOID behavior where admitted.
The old renderer is a differential comparison, never the sole correctness oracle.

Required tests use small synthetic studies with known values and physical
landmarks; missing external datasets cannot turn a required test into success.
Source inspection found file-path dispatch, tied-series selection, frame-zero,
RGB display and grayscale coverage gaps, now owned by the RITK prerequisites.
The [RITK workflow manual](../../ritk/docs/manual/dicom-workflow.md) now records
required synthetic file/byte pixel and coordinate checks, explicit acquisition
selection, DICOMDIR membership, failed replacement and session restoration,
plus a real Windows egui/eframe viewport capture and invalid-study rejection.
RITK `8152f483` additionally verifies physical image proportions across layouts,
rotations and zoom, with explicit rejection of collapsed screen rectangles;
710 debug and 710 release viewer tests pass and the native capture is regenerated.
That is original-viewer baseline evidence. Remaining patient-coordinate fusion,
transformed measurements, media-directory semantics, resource bounds, frames,
color and grayscale gaps keep their RITK acceptance
items; none are established by a screenshot. Migrated-viewer captures must come
from actual Métis execution and use the same synthetic studies. Browser host
input and native host input retain separate verification requirements.

<a id="V10"></a>
### V10 — Developer and package lifecycle

`python scripts/verify.py` builds the distribution CLI and runs
`scripts/distribution.py` against its release executable. The workflow verifies
portable payload hashes, MSI inventory and two input-sensitive process sessions.
The application-entry acceptance additionally requires exactly one executable in
the demonstration payload while preserving distinct backend/frontend PIDs.
`python scripts/verify.py --install` additionally installs the generated per-user
MSI into a private directory, checks registration/shortcut and installed bytes,
runs both input cases and uninstalls while retaining a user-created sentinel.
The workflow retains one guarded `output/distribution/latest` directory and a
machine-readable report. Build commands have a 300-second bound; application and
installer commands have a 60-second bound; the complete workflow has a 720-second
bound. Native database tests use the ordinary nextest 30/60-second budgets.
These checks cover the Windows x64 MSI increment, not the remaining lifecycle
requirements below. Native system calls are covered by host tests, not Miri.


Create/build/run using the CLI, deliberately introduce a compile error, fix it
and exercise reload. Install locally built packages in isolated supported hosts;
verify launch, assets, update, interrupted/corrupt/signature-invalid update
rejection, recovery and uninstall without deleting user data. Capture real
installer/application/recovery screens with generated CLI help. Local package
tests do not authorize registry/app-store publication or production deployment.

<a id="V11"></a>
### V11 — Mobile interaction and permissions

On each supported Android/iOS target, rotate, change keyboard, edit with touch,
suspend/resume and revoke an application permission during an operation. Assert
state preservation or typed cancellation and actual permission enforcement;
capture portrait/landscape/keyboard/denial states. Record device/emulator and OS.
Desktop visual success never closes this scenario.
The pinned inventory identifies required Android/iOS capability pairs. An
explicit unsupported outcome documents a platform restriction only when that
pair is outside the admitted contract; it cannot close an unimplemented required
capability provided by the comparison target.

<a id="V12"></a>
### V12 — Resource, latency and fault evidence

Compare semantically and visually equivalent fixtures using pinned application
and framework revisions, equal assets/windows/traces and controlled host load.
Separate browser-engine-dependent costs from framework/application allocations.
Record total process-tree idle/active/peak memory, WASM committed/used memory,
retained handles/listeners, repeated lifecycle growth, startup, input-to-frame
and frame-time distributions. Record bundle and build-cache sizes separately.
Store baselines, machine/OS/engine/driver, sampling protocol and uncertainty.
Plot only measured values; report no universal framework ranking.

Input traces and workload sizes are fixed before comparison, chosen to exercise
the relevant working-set regimes under the committed budget. Profile production
paths before optimizing; preserve the instrument across comparisons. Inject
allocation exhaustion, disconnect and backend crash and assert bounded error
recovery. Permission-denial probes compare security under a matched threat model;
memory numbers and screenshots do not establish security superiority.

Nextest stores its bounded reports in `output/nextest/ci/junit.xml`; Cargo compilation continues to use Atlas's shared target directory. The native test timeout and retry policies remain in `.config/nextest.toml`.
