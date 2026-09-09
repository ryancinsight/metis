# Verification

The owning gate is `python scripts/verify.py`. It records bounded logs under the
ignored `output/` directory and checks formatting, strict Clippy, debug and release
nextest suites, plan identifiers/dependencies/local links, the generated
`wasm-bindgen` browser assets, doctests, documentation, example execution,
dependency closure and the built `metis-rs` Python wheel test. It also runs the pinned cargo-deny advisory, source, license
and ban checks and records the reviewed Cargo build-link inventory.
Native tests use `.config/nextest.toml`: slow at 30 seconds, terminate at 60 seconds,
zero retries. The demonstration executable has a 60-second outer budget.

## Verification workflow definition — 2026-09-07

`.github/workflows/ci.yml` is the one hosted verification pipeline. Its Windows
job installs `cargo-nextest` 0.9.143 and `wasm-bindgen-cli` 0.2.128, then invokes
the same `python scripts/verify.py` gate used locally. The workflow records its
source hash through the gate, uploads `output/verification.json` and bounded
stage logs on failure, and never updates visual baselines. Pinned Atlas reusable
jobs check workflow syntax, the standalone Cargo lock, the strict ADR index and
public-API changes on ready pull requests.
The jobs run only for the actual `feat/process-foundation` default branch, pull
requests that are ready for review, and merge-queue events; no unsupported
full native-window or cross-platform host job is advertised.

The workflow contract is covered by the Python gate tests, including full
revision pinning, draft suppression, guard references and the Windows target.
This local check validates the committed definition; a hosted run is required
before reporting GitHub runner results or CI timing evidence.

## Registry publication workflow — 2026-09-08

`.github/workflows/rust-release.yml` delegates release validation and crates.io
publication to the Atlas reusable `semver-gate.yml` and `crates-publish.yml`
workflows at Atlas revision `c73c3dabe9573f09df7f1e2eacfccac17f685c6c`.
The caller triggers only on a published GitHub Release or an explicit
`workflow_dispatch`; it carries no registry secret and grants `id-token: write`
only to the release-only reusable publish job. Manual dispatch calls the
validation-only reusable job without `id-token: write`. The Atlas workflow obtains a short-lived
crates.io token through OIDC and gates it with the `crates-io` environment.
No private-key prompt or local signing step belongs to this release path. If a
developer's Git installation asks for a signing key, that prompt comes from
their local Git configuration and can be cancelled; the repository workflows
do not invoke it.

The local package inventory contains ten publishable Cargo packages and one
`publish = false` tooling package (`metis-cli`). `metis-python` builds the
`metis-rs` PyPI distribution and `.github/workflows/python-release.yml`
delegates wheel construction to Atlas's `python-wheels.yml`, then uploads
through PyPI Trusted Publishing with `id-token: write`. This source-level
check does not prove registry publisher registration, first publication,
release authority or package upload; those are external release actions.
Public branch inspection at revision `f266180` confirmed the crates.io and PyPI
callers expose OIDC permissions without registry-token, SSH, GPG or private-key
secrets. A live GitHub API inspection on 2026-09-09 found zero repository
Actions secrets or variables and two empty environments, `crates-io` and
`pypi`, with no protection rules. The environments provide the named release
boundary; trusted-publisher registration, protection rules and release
authority remain external to the local source gate.

`cargo package --locked --allow-dirty --list` succeeded for `metis-core`,
`metis-web` and the root `metis` package. A local
`cargo publish --locked --package metis-core --dry-run` could not reach the
crates.io index in this environment, so hosted package validation remains
unverified.

Fresh hosted runners prime the exact locked Git and registry sources with the
pinned Rust toolchain before invoking the gate. The gate then resolves offline,
so source acquisition is explicit while verification remains reproducible.
The workflow installs cargo-deny 0.20.2 and primes its advisory database before
the same offline policy check.

## Python binding verification — 2026-09-08

`metis-python` is the only Metis crate that depends directly on PyO3. The local
gate invokes `scripts/python_binding.py`, which builds a locked release wheel
with `maturin`, extracts that generated artifact into a temporary directory and
runs the provider-owned pytest suite against the extracted `metis._metis`
extension. The suite compares adult and pediatric results with the Rust
formula, checks input sensitivity and rejects non-finite, out-of-range and
envelope-violating values. The wheel contains the `metis` package,
`py.typed` marker and `_metis.pyi` stub. The gate's wheel build and pytest
stages are required; an import-only check does not close the binding contract.

Entry baseline: `cargo check --workspace --offline` passes with documentation and
source warnings. The original native test build fails with E0382 in the threaded
process-isolation test. No original OS sandbox or native-window evidence exists.

## Python presentation verification — 2026-09-09

The presentation-surface increment adds Rust-owned `RasterImage`, `Rect` and
`Canvas` values to the same wheel. The extracted-wheel suite compares exact
row-major RGBA bytes after asymmetric nearest-neighbor scaling and destination
clipping, verifies source-over alpha over white, and rejects invalid crops,
dimensions and byte lengths with stable error codes. `Canvas.to_rgba()` is an
explicit cold-boundary copy; the wheel exposes no Python renderer, native
window, filesystem path or DICOM decoder. The image fixture and visual
semantics are shared with the inspected [software raster image evidence](#software-raster-image-evidence--2026-09-09).

## Windows native provider and host evidence — 2026-09-08

Moirai PR #286 merged at `c91e2cdd` added bounded native IME composition events,
and PR #287 merged at `7ad8eeee` closed the empty-composition cancellation edge.
That evidence revision used `metis-platform::native::NativeSurface` to present
the production `Framebuffer` pixels while returning the provider's bounded
`WindowEvent` values. On `x86_64-pc-windows-msvc`, the provider suite
passes 61/61 with strict Clippy; the native tests create a real hidden HWND,
present a production frame, observe input/IME/resize/DPI lifecycle events,
validate bounded UTF-16 composition decoding, verify retained initial readiness
and an overlong-wait rejection, prove a posted event wakes the finite wait, and
close the window. A lifecycle regression test rejects reopening a live surface,
closes it, and creates a fresh hidden HWND from the same validated configuration;
pending events from the destroyed provider are not retained. A two-window test
creates independent hidden HWNDs with different dimensions, presents separate
frames, verifies each event batch and confirms closing one leaves the other live.

The same `metis-app` executable now composes that surface with the production
frontend and supervised private IPC under `--metis-native-window`. The focused
frontend/platform/application suite passes 26/26 with strict Clippy. Its tests
cover role selection, bounded Unicode editing, transient composition and commit
through the ordinary patient-field transition, an authored submit hit region,
resize replacement with rollback on invalid dimensions, input-sensitive process
results and child cleanup. The interactive role uses a finite five-minute
watchdog; the headless role retains the ten-second budget.

This establishes provider, lifecycle, bounded native IME event production and
code-level host composition evidence. The committed Windows host captures are
recorded below. An installed CJK/other IME keyboard journey, WebView2
composition, OS permission denial, accessibility behavior, two-window captures,
and macOS/Linux support remain open under [V05](#V05) and the linked backlog
items; a hidden-window test and a passing build cannot replace real visual,
assistive-technology or denial-probe evidence.

### WebView2 consumer seam — 2026-09-09

Metis `metis-platform::native::WebViewSurface` now consumes Moirai main revision
`a58344b00ccc4a062c71659a463dd188d67bf1f4` through a safe, thread-affine
adapter. The adapter creates the provider-owned HWND, sizes and maps visibility
for the controller, forwards combined window/WebView events and preserves the
provider's packaged-URI, new-window and bounded-JSON policies. The workspace
lock records one Moirai revision for every direct provider crate during this
co-evolution increment. Consumer configuration tests pass on
`x86_64-pc-windows-msvc`. On a Windows host with WebView2 runtime
`152.0.4191.66`, the ignored adapter smoke was run outside the sandbox with
`cargo nextest` and passed; it loads a packaged page, observes its ready bridge
message, verifies successful navigation, rejects an external HTTPS navigation
with a denied-navigation event and closes the surface. The direct Moirai
requirements no longer carry a temporary revision pin; the standalone lock
records `a58344b00ccc4a062c71659a463dd188d67bf1f4`. The test stays
ignored in the ordinary suite because the runtime and native host are not
available on every target. This is lifecycle and bridge evidence; the visible
form and bridge-result captures are recorded below.

The application now exposes `--metis-webview`, which uses the same executable
and supervised private pipe as the software-rendered native role. Its child
creates a bounded temporary HTML/CSS package, receives typed submit messages
through the provider callback, sends the existing capability-authorized
calculation to the backend and posts the value-semantic result back to the page.
The package applies a no-network CSP and is removed after the bounded session.
The source and parser tests cover the application bridge contract. The visible
`--metis-webview` initial and submit journey is now captured below; OS permission
denial and assistive-technology evidence remain unverified.

### Windows visible host captures — 2026-09-09

The application capture increment is recorded in
[`docs/manual/images/native-captures.json`](manual/images/native-captures.json)
and is bound to Metis revision `0c8bcc32911c087bf686588cd4a7c56a29d0b92e`.
The executable ran on `x86_64-pc-windows-msvc` with WebView2 runtime
`152.0.4191.66`. The native framebuffer capture has an 800×600 client area in
an 816×639 outer window; its initial and trusted-Enter states show the waiting
form and `0.360 mL/hr (0.72 MG/hr)` at audit sequence 2. The WebView2 capture
has a 1024×768 client area in a 1040×807 outer window; its initial page reports
the connected host bridge and no submitted calculation, and a trusted
operating-system pointer click produces `Rate 0.36 mL/hour; drug 0.72 mg/hour;
audit 2`.

The four PNGs are committed beside the manual:
[`native-form.png`](manual/images/native-form.png),
[`native-form-success.png`](manual/images/native-form-success.png),
[`webview-form.png`](manual/images/webview-form.png) and
[`webview-form-success.png`](manual/images/webview-form-success.png). The
supervisor's runtime environment policy clears the child environment and admits
only nine operating-system path variables needed by WebView2; application
settings, registry tokens and the `github-cli` environment name are not passed.
The focused regression test verifies that a runtime child has no `PATH` and that
the allowlist does not contain `github-cli`. The capture parent and child were
closed after each workflow. This evidence does not establish physical
resize/DPI, native accessibility, installed-IME, OS permission denial,
two-window visual or macOS/Linux behavior.

## Parser and diagnostic safeguards — 2026-09-08

The `METIS-QUALITY-001` increment is implemented in commit
`866822016d8ee02b2b38149efee2e26783c77bb2` and adds bounded property-style and mutation
coverage to the public wire decoders using a deterministic Rust generator.
`metis-ipc` runs 128 generated arbitrary-byte cases
with payload lengths capped at 1024 bytes, plus Unicode and IEEE-754 round-trip
properties, deterministic truncation and bit-flip mutations across every
payload, and explicit oversized-length rejection. The focused native suite
passes 96/96 tests and strict Clippy; the standalone `fuzz/Cargo.toml` manifest
passes `cargo check --locked` and carries a LibFuzzer target for the same
decoder boundary. A runtime campaign was attempted with nightly cargo-fuzz on
the Windows MSVC host but cannot link `clang_rt.asan_dynamic_runtime_thunk`;
therefore no LibFuzzer execution result is claimed for this host.

`MetisError` now has a structured redacted `Debug` representation and an
explicit `redacted()` view containing only `ErrorCode` and `trace_id`.
`ErrorResponsePayload` similarly redacts its message under `Debug`. Unit tests
assert that untrusted path, authorization and patient text is absent while
the typed code remains present. `Display` remains the user-facing full
diagnostic contract.

### Bounded decoder mutation evidence — 2026-09-08

The committed `scripts/mutation.py` runner pins cargo-mutants 27.1.0 and the
1.97.0 workspace toolchain. It mutates
`crates/metis-core/src/protocol/payload.rs` at `decode`, runs the `metis-ipc`
integration tests through nextest, uses `--locked --offline` with two jobs,
and bounds each test, build and suite at 30, 120 and 300 seconds. The
`--test-package metis-ipc` selection is material: a package-only run cannot
reach the decoder integration contract.

At revision
`6af70734965ceb5eb94dd4ce1d663a50e4f532ab`, the run generated 14 mutants,
caught all 6 viable mutants, and reported 0 missed, 0 timed-out and 8
unviable mutants (viable score 1.0). The exact command, source SHA-256,
toolchain and counts are recorded in the derived
`output/mutation/latest/manifest.json`. The report is a scoped decoder result,
not a workspace-wide mutation score. The Windows MSVC sanitizer limitation is
covered by the hosted Ubuntu campaign in the single scheduled and manually
dispatchable verification workflow. That job selects `nightly-2026-08-01`,
checks the locked fuzz manifest, and runs the combined protocol/SVG target with
a 300-second campaign, 2 GiB RSS limit and 25-second input timeout.

The hosted run at
<https://github.com/ryancinsight/metis/actions/runs/34293187709> completed the
`LibFuzzer parser campaign` job successfully at source revision
`0998e63748faa2c76f963574353374658329e20` (job
<https://github.com/ryancinsight/metis/actions/runs/34293187709/job/102289671960>).
Crash reproducers are uploaded from `fuzz/artifacts/` only when the campaign
fails; this run produced no failure artifact.

The runner follows cargo-mutants' [nextest integration](https://mutants.rs/nextest.html)
and [workspace test-package selection](https://mutants.rs/workspaces.html)
contracts; the pinned tool installation is documented in the
[testing manual](manual/testing.md#run-bounded-mutation-analysis).

## Evidence classes

- WASM portability: compile `metis-core`, `metis-platform`, `metis-ui-lang` and
  `metis-web` libraries for `wasm32-unknown-unknown`. Compilation is static
  evidence; it does not by itself validate host bindings or establish Tauri
  compatibility. The browser workbench has a separate runtime trace below.

- Types and compilation: frontend cannot import the backend through its declared dependency closure; validated policy fields cannot be overwritten externally.
- Behavioral tests: exact wire fixtures, canonical decoding, malformed corpus, scope/session/time rejection, audit event outcomes and bounded numerical error.
- Independent numeric evidence: dimensional infusion conversion and exact binary fixtures; arithmetic roundoff uses a stated gamma bound.
- Crypto evidence: Moirai `d879779247c8cfc5870f62f99a5364cbbf2d3c58` publishes the shared HMAC/SHA-256 and
  fixed-width comparison primitives; independent vectors and streaming/padding
  regressions run upstream, while Metis capability, audit, result-signature and
  CLI tests exercise those functions at their real boundaries.
- Process acceptance: separate instances of one application executable exchange real pipes; PID, result and standalone relocation checks distinguish executable packaging from process state. These do not prove OS least privilege.
- Visual evidence: software framebuffer generated from actual form state and inspected independently of compilation.
- Supply-chain evidence: cargo-deny checks the locked graph against the committed
  source/license/advisory policy; `output/build-links.json` inventories packages
  that declare Cargo build-link contracts. This inventory does not prove the
  absence of unsafe code in dependencies.

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
the standalone lock to merged provider
`d879779247c8cfc5870f62f99a5364cbbf2d3c58`; comparative security/memory evidence
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
path. The delayed-response stop/remount trace below closes service-boundary
stale DOM delivery; post-drop resource counts, cross-engine behavior,
accessibility technology support and OS permission isolation remain open in
[METIS-BROWSER-001](../backlog.md#METIS-BROWSER-001),
[METIS-MEMORY-001](../backlog.md#METIS-MEMORY-001),
[METIS-PERF-001](../backlog.md#METIS-PERF-001),
[METIS-SERVICES-001](../backlog.md#METIS-SERVICES-001) and
[METIS-DESKTOP-001](../backlog.md#METIS-DESKTOP-001).

<a id="browser-delegated-control-evidence"></a>
## Browser delegated-control and text-boundary evidence — 2026-09-08

At Metis revision `c08dcbd`, the generated workbench was rebuilt and loaded in
the Codex in-app browser at a 1280×720 CSS-pixel viewport and device scale 1.25.
The browser engine version was unavailable. With no service endpoint configured,
the trace exercised only the local Rust/WASM host and its semantic DOM state.

Changing **Weight (kg)** from `72.5` to `80` produced an accessibility update for
the same `weight-kg` control and changed the Rust-owned result to `80.00 kg`.
Changing **Result scale** from `100` to `120` updated the same `result-scale`
slider and the `options-state` text to `scale 120%`. Changing **Theme** from
system preference to **Dark** selected the existing `theme-mode` control and
updated `options-state` to `theme dark`. These three transitions arrived through
the delegated `input`/`change` listeners on `#metis-app`; the semantic control
identities and surrounding form remained present after each render.

The trace then selected **Clinical note**, replaced its value with the literal
`<img src=x onerror=alert(1)>`, and typed it through the browser input path. The
accessibility tree and screenshot showed the exact characters in the textarea and
the preview, with no added image or other element. This verifies the text-only
rendering boundary for that input; it does not claim cross-engine or
assistive-technology coverage.

<a id="browser-svg-asset-evidence"></a>
## Browser SVG asset evidence — 2026-09-08

At Metis revision `c1bc89f`, `python scripts/browser.py build` copied the local
`metis-mark.svg` (368 bytes), PNG alternate and ICO into the generated browser
directory. The Codex in-app browser loaded that exact output at a 1280×720
CSS-pixel viewport and device scale 1.25. Its accessibility tree exposed the
header image as `Métis mark`, and the inspected screenshot showed the teal and
indigo vector mark in the header while the surrounding form remained intact.

The focused `metis-cli` suite passed 19/19 tests, including the project SVG and
rejections for XML expansion, external references, remote paints, missing paths,
trailing markup and an oversized viewport. The browser asset suite passed 9/9
tests and checks the same-origin SVG favicon, PNG alternate, manifest resource
and copied output. This proves the local asset admission and generated browser
presentation for one engine; browser decode behavior in other engines, installed
MSI rendering and font/media resource lifecycles remain open V06 evidence.

## Browser lifecycle evidence — 2026-09-07

After rebuilding the generated artifacts from the standalone lock at Moirai
`d879779247c8cfc5870f62f99a5364cbbf2d3c58`, the Codex in-app browser loaded
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
`d879779247c8cfc5870f62f99a5364cbbf2d3c58`, including the bounded HTTP/WebSocket
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
session. It does not close post-drop JavaScript allocation, TLS,
accessibility/IME, cross-engine or native desktop/OS permission scenarios.

## Browser control evidence — 2026-09-07

The control model adds semantic HTML5 checkbox, radio, range and select inputs to the
same Rust/WASM workbench. `cargo nextest run --locked -p metis-web` passes
10/10, including a regression that applies visibility, display-unit, scale and
result-detail changes to a successful response without clearing that response. Native
warning-denied Clippy, the WASM-target check and WASM-target Clippy pass for
`metis-web`; `python scripts/browser.py build` regenerates the loader and WASM
from the standalone lock at Moirai
`d879779247c8cfc5870f62f99a5364cbbf2d3c58`.

In the authenticated service trace, the Codex in-app browser exposed the
semantic control names and values at a 1280×720 CSS-pixel viewport and device
scale 1.25; the engine version was unavailable. Pointer activation of **Drug
mass rate** selected the radio and changed the rendered metric to
`Drug mass rate: 2.175000 mg/hr` while retaining `Backend result received` and
the correlated event. Pointer activation of **Show remote events** changed the
event text to `Remote events: hidden by preference` while retaining the metric.
Two keyboard **Right** presses on **Result scale** changed its accessibility
value to `120`, updated `View options: ... scale 120%`, and left the visible
focus ring on the range. Selecting **Audit detail** changed the select value and
the result annotation to `Audit detail: sequence 4` while retaining
`Drug mass rate: 2.175000 mg/hr`. The browser console contained only expected
Moirai initialization entries and no warnings or errors.

The same trace verified the disabled-control lifecycle introduced by the
Moirai `WebElement::disabled`/`set_disabled` seam. Before the authenticated
handshake, the accessibility tree marked **Submit to authorized backend** as
disabled. The control became enabled when the bridge reported ready. With a
four-second response delay, submitting changed the status to **Request in
progress** and disabled the control; after the correlated response it became
enabled again and the metric was `Volume rate: 0.543750 mL/hr`. A disconnected
workbench rejected activation of its disabled submit control; the status and
accessibility tree stayed unchanged.

This closes the checkbox/radio/range/select and disabled-submit browser slices
with real pointer and keyboard demonstrations. Drag/drop policy,
multi-touch/pinch interpretation, IME, accessibility technology, cross-engine
parity, post-drop allocation and native-window input remain open under the
linked backlog items.

## Browser dialog evidence — 2026-09-07

The dialog increment consumes Moirai `WebElement::dialog_open`, `show_modal`,
`close_dialog` and `focus` from merged revision
`8f02b8b7de6cf6361b519bd79759d8508568fbdb`. `metis-web` passes 10/10 native
nextest tests, native warning-denied Clippy, the WASM-target check and
WASM-target Clippy; `python scripts/browser.py build` produces the generated
loader and WASM from that standalone lock.

The authenticated Codex in-app browser trace used the same 1280×720 CSS-pixel
viewport and device scale 1.25. **Session details** appeared as a semantic
dialog opener. Activating it opened the native HTML dialog; the provider-backed
open-state read returned the `open` attribute, and the accessibility tree
exposed **Authorized session details**, the ready status, the full capability
summary and **Close**. The captured modal screenshot showed the dialog backdrop,
the bounded panel and the visible focus ring on **Close**.

Activating **Close** called the Rust `close_dialog` seam. The browser `close`
event then called the provider `focus` seam, and the accessibility tree returned
to the workbench with focus on **Session details**. Pressing **Escape** closed
the modal through the browser's native cancel path; the dialog had no `open`
attribute afterward and focus again returned to the opener. The browser console
contained only expected Moirai initialization entries and no warnings or errors.

This closes the menu/dialog portion of `METIS-INPUT-001`. Drag/drop policy,
multi-touch/pinch interpretation, IME, accessibility technology, cross-engine
parity, post-drop allocation and native-window input remain open.

## Browser pointer-capture evidence — 2026-09-07

The pointer increment consumes Moirai `WebEvent::pointer_id` and the
`WebElement::set_pointer_capture`, `has_pointer_capture` and
`release_pointer_capture` seams from merged revision
`5a5e4b1540eff39bc3f082c6907f0c82fa14dcc8`. `metis-web` passes 10/10 native
nextest tests, native warning-denied Clippy, the WASM-target check and
WASM-target Clippy; `python scripts/browser.py build` produces the generated
loader and WASM from the updated standalone lock.

The Codex in-app browser trace used the generated page at a 1280×720 CSS-pixel
viewport and device scale 1.25, served from a separate loopback HTTP origin
without a backend bridge. The accessibility tree exposed the **Pointer
capture** heading and a group named **Pointer capture surface**. Activating
the surface delivered pointer ID `1`; Rust called `set_pointer_capture`, read
`has_pointer_capture` as true, and the `pointerup` listener called
`release_pointer_capture`. The status changed to `Pointer capture: released
(1)`, the browser tree retained the semantic surface, and the full-page
screenshot showed the pointer card beside the unchanged backend-result panel.
The same release path is registered for `pointercancel`, and a second active
pointer is rejected while the mounted surface owns its first identifier.

This closes the browser pointer-capture portion of `METIS-INPUT-001`. Drag/drop
policy, multi-touch/pinch interpretation, IME, accessibility technology,
cross-engine parity, post-drop allocation and native-window input remain open.

## Browser pointer-metadata evidence — 2026-09-07

The metadata increment consumes Moirai `WebEvent::pointer_metadata` and the
`PointerMetadata` value seam from merged revision
`a3c86cd183a18edc35db30f1d35e79fe80092df4`. `metis-web` passes 10/10 native
nextest tests, native warning-denied Clippy, the WASM-target check and
WASM-target Clippy; `python scripts/browser.py build` produces the generated
loader and WASM from the updated standalone lock.

The Codex in-app browser trace used the generated page at a 1280×720 CSS-pixel
viewport and device scale 1.25, served from a separate loopback HTTP origin
without a backend bridge. The accessibility tree exposed the **Pointer
capture** heading and named **Pointer capture surface** group. A normal click
ended with `Pointer capture: released (1) — mouse at (386, 519), button 0,
buttons 0, modifiers none, primary`. A Shift-click ended with
`mouse at (386, 540), button 0, buttons 0, modifiers Shift, primary`, proving
modifier state is read from the browser event. A right-click ended with
`mouse at (386, 540), button 2, buttons 0, modifiers none, primary`, proving
the changed-button value is input-sensitive. The full-page screenshot showed
the metadata status beside the unchanged backend-result panel.

Captured `pointermove` listeners render the same metadata record while the
surface owns the pointer, and `pointercancel` uses the same release path. This
closes the provider metadata portion of `METIS-INPUT-001`; drag/drop policy,
multi-touch/pinch interpretation, IME, accessibility technology, cross-engine
parity, post-drop allocation and native-window input remain open.

## Browser wheel metadata evidence — 2026-09-07

The wheel increment consumes Moirai `WebEvent::wheel_metadata` and the
`WheelMetadata` value seam from merged provider revision
`f634b3a802ec0355da22f111ed01067d2435c5cb`. `metis-web` passes 10/10 native
nextest tests, native warning-denied Clippy, the WASM-target check and
WASM-target Clippy; `python scripts/browser.py build` produces the generated
loader and WASM from the updated standalone lock.

The Codex in-app browser trace used the generated page at a 1280×720 CSS-pixel
viewport and device scale 1.25, served from a separate loopback HTTP origin
without a backend bridge. The accessibility tree retained the named **Pointer
capture surface** group and the semantic **Wheel** status. An upward scroll
action ended with `Wheel: delta (0.00, -129.60, 0.00) pixel at (386, 580),
modifiers none`; a rightward action ended with `Wheel: delta (426.40, 0.00,
0.00) pixel at (386, 580), modifiers none`. The status changed with the
scroll direction, proving that the deltas are input-sensitive, and the
full-page screenshot showed the wheel record beside the unchanged backend
result panel.

The scroll action is automation-generated browser input. The CUA surface does
not expose the browser event's hardware `isTrusted` flag, so this trace does
not claim physical-wheel or cross-engine parity. The provider's WASM
compile/clippy checks cover the browser binding; native provider nextest
remains 47/47. This closes the wheel metadata transport portion of
`METIS-INPUT-001`; the gesture policy is recorded below. Drag/drop,
multi-touch/pinch interpretation, IME, accessibility technology, cross-engine
parity, post-drop allocation and native-window input remain open.

## Browser gesture policy evidence — 2026-09-07

The gesture increment moves the pan/zoom state machine into the target-
independent `gesture_policy` module so its bounded behavior is tested on the
native target as well as compiled into WASM. `cargo nextest run --locked
--offline -p metis-web` passes 13/13 tests, including pointer ownership,
line/page normalization, zoom bounds and non-finite rejection. Native
warning-denied Clippy, the WASM-target check and WASM-target Clippy pass;
`python scripts/browser.py build` produces the generated loader and WASM from
the standalone lock.

The Codex in-app browser trace used
`http://127.0.0.1:8093/?cache=gesture-20260907` at a 1280×720 CSS-pixel
viewport and device scale 1.25, served from a loopback HTTP origin without a
backend bridge. An upward scroll at the pointer surface rendered
`Gesture: wheel pan; pan (0.0, -129.6) CSS px; zoom 100%`; a rightward scroll
then rendered `Gesture: wheel pan; pan (426.4, -129.6) CSS px; zoom 100%`.
Dragging from `(300, 590)` to `(420, 620)` after those scrolls rendered
`Gesture: release; pan (546.4, -99.6) CSS px; zoom 100%`. The screenshot showed
the translated content and the semantic gesture status beside the unchanged
backend result panel.

The trace is automation-generated. CUA does not expose the browser event's
hardware `isTrusted` flag, physical touch or IME injection, or another browser
engine, so it claims the observed Rust/WASM event flow and rendered transform
only. The native policy tests cover Ctrl-wheel zoom behavior and its 50–300%
bound; the live trace does not claim physical-input or cross-engine parity.

## Browser pinch gesture evidence — 2026-09-08

The pinch increment extends the Rust-owned capture and gesture policy to two
distinct pointer identifiers. A second pointer establishes a finite baseline;
centroid movement updates bounded pan and the distance ratio updates bounded
zoom. Duplicate and third-pointer presses are rejected, a zero-distance pair
waits for a valid baseline, and releasing either pointer clears the pinch
state. Commit `16e14afaf3ee8c993e840a06a539dfd3bb0ff5bc` carries this
implementation and documentation. `cargo nextest run -p metis-web --locked
--offline` passes 30/30;
native warning-denied Clippy, the WASM-target check and WASM-target Clippy
pass for the same package.

The generated browser workbench renders the pointer surface instruction for
one-pointer drag and two-pointer pinch. The policy's input-sensitive transform
is covered by native tests because the Codex in-app browser can inject mouse
and wheel actions but cannot provide trusted physical touch or expose the
browser `isTrusted` flag. The screenshot and source evidence therefore claim
the Rust/WASM policy and semantic surface only; physical touch and
cross-engine parity remain open.

## Browser file-drop evidence — 2026-09-08

The file-drop consumer now captures Moirai `DropFiles` at merged revision
`5c8a9e8be32ad6beac14ed263c2f11c3663b87cb`. The provider bounds one event to 64
files, validates names/media types and owns each browser `File` handle without
exposing a filesystem path. Metis revalidates the copied metadata, retains a
bounded `Box<[FileDropEntry]>` for presentation and starts one cancellable task
for the accepted batch. Each file is read to its declared end through 64 KiB
continuations, with a 64 MiB per-file and 256 MiB batch budget. The first
payload is classified at the DICOM Part 10 marker at offsets 128–131; the
completed named bytes are exposed through one `FileDropBatch` handoff slot.
Moirai rejects any individual read larger than 1 MiB, and the consumer stays
below that provider bound.

The focused evidence against the updated standalone lock is:

```text
cargo nextest run --locked -p metis-web --profile default — 28/28 passed
cargo clippy --locked -p metis-web --all-targets -- -D warnings — passed
cargo check --locked -p metis-web --target wasm32-unknown-unknown — passed
cargo clippy --locked -p metis-web --target wasm32-unknown-unknown -- -D warnings — passed
```

The full repository gate passed against Metis commit
`eee0cd14bc8be102526b045ff46b4445a0314509`; every configured stage passed,
including the generated browser visual run.

The native policy tests cover empty names, NUL and oversized metadata, empty
and 65-file drops, DICOM media/extension classification, UTF-8-safe display
truncation, the Part 10 marker, payload budget edges, batch ownership,
ownership-consuming allocation preservation and bounded read-status
transitions. The
generated HTML5/CSS page renders the **DICOM file drop** card, both semantic
status regions and the focusable **DICOM file drop zone**; the visual capture
records the idle state. CUA does not expose `isTrusted`, cannot attach a local
operating-system file to a synthetic browser event and cannot establish the
browser engine version, so this evidence does not claim a trusted live byte
read or DICOM opening. The handoff is ready for the RITK adapter; full dataset
parsing and study decoding remain RITK responsibilities.

The ownership increment adds `FileDropBatch::into_files` and
`FileDropPayload::into_parts`. The test records the collection and byte-buffer
addresses before and after the moves, then asserts the names, media type and
bytes; this is evidence of move-only handoff, not evidence of a decoded DICOM
dataset.

## Browser text and composition evidence — 2026-09-08

The text increment consumes Moirai's merged browser text contract at
`0862716265d657b8069d5a47fd1e77ae26ddd006`. `moirai-pal` bounds browser text,
event data, input-operation names and composition locales before copying them
into owned values; selection snapshots retain UTF-16 code-unit offsets and a
direction enum. `metis-web` keeps its own bounded `TextState`, validates
selection ranges against the current value and rejects offsets inside a
UTF-16 surrogate pair, and handles `input`, `select`,
`compositionstart`, `compositionupdate`, `compositionend` and
`compositioncancel` through Rust-owned listener guards.

The focused commands against the updated standalone lock are:

```text
cargo nextest run --locked -p metis-web — 32/32 passed
cargo clippy --locked -p metis-web --all-targets -- -D warnings — passed
cargo check --locked -p metis-web --target wasm32-unknown-unknown — passed
cargo clippy --locked -p metis-web --target wasm32-unknown-unknown -- -D warnings — passed
python -m unittest discover -s scripts/tests — passed
python scripts/browser.py build — passed
```

The native policy suite covers an accented character and emoji UTF-16 span,
scalar-boundary rejection, selection ordering and bounds,
forward/backward/unknown direction labels,
input metadata rejection without state mutation, and composition start/update,
commit and cancellation. The generated page renders a labelled textarea,
separate text/composition/selection status regions, a bounded value preview and
selection/composition data attributes. A CUA trace can inspect those semantic
nodes and the focus ring; it cannot synthesize a trusted operating-system IME
or expose the browser `isTrusted` flag. The trace therefore establishes the
HTML/WASM rendering and listener surface only. Grapheme segmentation, bidi
shaping, fallback-font metrics, clipboard/undo, assistive technology and native
IME evidence remain open under `METIS-TEXT-001` and `METIS-A11Y-001`.

The 2026-09-08 CUA trace opened
`http://127.0.0.1:8095/?cache=text-clean-20260908` at 1280×720 CSS pixels and
device scale 1.25. The initial accessibility tree and inspected screenshot
showed `Résumé — 東京 / 影像` with a `16`-unit caret. Browser `typeText` input
appended ` typedX`; the observed value became `Résumé — 東京 / 影像 typedX`,
`text-status` reported `Text: input insertText applied; data X`, and
`selection-status` reported a `23`-unit forward caret. This validates ordinary
browser input and Rust/WASM state updates; it does not claim native IME,
trusted hardware input, or cross-engine behavior.

## Browser responsive-layout evidence — 2026-09-08

The browser stylesheet now constrains the page to `width: 100%` with a
`960px` bound, uses `minmax(0, 1fr)` columns above the `700px` breakpoint, and
stacks the form and options below that breakpoint. Shared box sizing, zero
minimum grid items and `overflow-wrap: anywhere` keep long status and clinical
strings inside their cards. Option rows and the result-scale slider use a
`44px` CSS hit target. The static browser contract suite checks the responsive
declarations.

The Browser viewport capability captured the generated page at
`360×640`, `800×600` and `1440×900` CSS pixels with device scale `1`. The
runtime manifest and JPEG captures are committed under
`docs/manual/images/browser-layout-*`; its source hash binds the measurements to
`examples/browser/styles.css`. The narrow capture resolves to one `312.8px`
grid column; the fixture resolves to two `348.4px` columns; the wide capture
resolves to two `436px` columns. All three have no horizontal overflow and every
required card or target ends inside the viewport. The full Metis gate passes on
the delivered revision.

The viewport capability does not expose a device-scale override. Scale `2`,
and platform fractional-scale cases remain open under V04; these captures do not
claim those paths.

## Software style diagnostic evidence — 2026-09-09

`metis-ui-lang` now rejects `justify-content`, `align-items`, `min-width`,
`min-height`, `border-radius` and `font-weight` declarations because the
software renderer has no layout or paint semantics for them. The parser returns
`ERR_INVALID_CSS_STYLE` with the property name, and layout applies the same
check to programmatically constructed DOMs before emitting a display list. The
presentation fixture was migrated to the admitted subset, preserving its
software-rendered geometry and pixels. Focused strict Clippy and nextest cover
the parser and programmatic-layout paths; the full local gate is the acceptance
oracle for the synchronized documentation and visual fixtures.

## Browser accessibility presentation evidence — 2026-09-08

The browser asset contract now checks semantic group names, polite atomic live
regions, dynamic `aria-busy` wiring for form/result loading, non-positive focus
order and the responsive presentation preferences.
The stylesheet honors `prefers-reduced-motion: reduce` by removing scroll and
transition motion, and `forced-colors: active` by mapping surfaces, controls
and focus outlines to system colors. The keyboard order follows the active
document path from **Session details** through the form, view options, pointer
surface, DICOM drop zone and clinical note; the closed dialog remains outside
that path.

The focused native suite and full Metis gate pass on the committed revision.
The Rust view sets `aria-busy` on the form, primary status, result status and
result explorer from the same pending/loading states that drive the visible
messages, so assistive technology receives the same lifecycle as the visual
surface.
The available CUA browser can inspect the semantic tree and visible focus ring
at 1280×720 CSS pixels and device scale 1.25. It cannot change the browser's
reduced-motion or forced-colors media preferences, expose spoken screen-reader
output, or provide an operating-system accessibility bridge. Runtime captures
under those preferences, supported screen-reader traversal, zoom-scale
geometry and native host accessibility remain open under `METIS-A11Y-001`.

The fresh CUA trace started from **Start host**, advanced to **Stop host** and
**Session details**, then traversed **Patient reference**, **Weight (kg)**,
**Drug concentration (mg/mL)**, **Target dose (mcg/kg/min)**, **Show remote
events**, **Volume rate**, **Result scale**, **Result detail**, **Pointer
capture surface**, **DICOM file drop zone** and **Clinical note**. The disabled
submit control and the unselected radio option were correctly skipped by the
browser's tab sequence. Refocusing **Clinical note** produced the visible
yellow focus outline in the inspected screenshot. This is one-engine browser
evidence; it does not establish screen-reader speech, forced-colors rendering
or native host integration.

The service-backed CUA trace used the real loopback service with
`--response-delay-ms 4000`. Immediately after activating **Submit to authorized
backend**, the accessibility tree announced **Request in progress** and all
four busy targets (`metis-status`, `metis-form`, `result-state` and
`explorer-table`) returned `aria-busy="true"`; the button was disabled. After
the delayed response, those attributes returned to `"false"`, the tree
announced **Backend result received**, and the retained result row appeared
with the enabled submit control. This is one-engine lifecycle evidence from
the Rust view and real backend exchange; it does not establish spoken
screen-reader output or a native accessibility bridge.

## Browser theme and starter asset evidence — 2026-09-08

The browser host now exposes four Rust-owned modes: system preference, light,
dark and high contrast. Rendering writes the selected value to the document
body and application root; the external stylesheet maps it to semantic
`--metis-*` variables. The header and favicon use the local
`examples/browser/assets/metis-mark.png`; the browser build also copies the
multi-resolution `metis-mark.ico` required by native packaging.

The focused `metis-web` unit suite and `scripts.tests.test_browser_assets` pass
for the working tree. The complete `python scripts/verify.py` gate also passed
with 155 resolved packages, including the WASM build, strict Clippy, debug and
release nextest suites, docs and visual baselines. The manual contains the
starter mark and the exact mode-by-mode capture procedure. A 2026-09-08 CUA
trace at 1280×720 CSS pixels
and device scale 1.25 selected light, high-contrast and dark explicitly; each
accessibility snapshot reported the matching mode and the inspected screenshots
showed the expected palette with the unchanged local mark. A DOM read after
dark selection found `data-metis-theme="dark"` on both the body and
`#metis-app`, dark page/text colors and a loaded mark. This is one-engine
explicit-mode evidence; system media preference, forced-colors and the V04
viewport/scale matrix remain open. Native packaging additionally validates the
ICO header, PNG chunk CRCs, dimensions and non-overlapping ranges, then stores
the stream in `Icon` and references it from `Shortcut.Icon_`; the focused CLI
suite and distribution workflow cover those rows.

## Native icon asset evidence — 2026-09-08

`examples/browser/assets/metis-mark.ico` is a local seven-entry PNG-in-ICO
asset generated from the project mark. Entries cover 16×16, 24×24, 32×32,
48×48, 64×64, 128×128 and 256×256 resolutions. `metis-cli` reads at most the
1 MiB icon budget, rejects invalid ICO headers, table/range overlap, truncated
PNG chunks, CRC mismatches and declared/decoded dimension mismatches, and only
then persists an MSI package. The package test reads `Icon.Name = MetisIcon`
and `Shortcut.Icon_ = MetisIcon`; the portable inventory includes
`assets/metis-mark.ico` as an explicit resource. Windows shell rendering is
exercised when `python scripts/verify.py --install` is run on Windows x64. The
install probe reads the real `.lnk` through `WScript.Shell` and requires its
`IconLocation` to end in the cached `MetisIcon,0` reference; the 2026-09-08
run produced `...\\MetisIcon,0` and then removed the exact test ProductCode.

## Software raster image evidence — 2026-09-09

`metis-ui-lang` now exposes a bounded `RasterImage` and `ImagePlacement` display
command. Construction rejects empty, oversized and mismatched row-major RGBA
storage; placement rejects empty or out-of-bounds source crops and nonpositive
destinations. Rendering clips the destination before iterating, maps pixels with
nearest-neighbor sampling and composites source-over alpha into the existing
framebuffer without allocating in the draw loop.

The focused `metis-ui-lang` suite passes 20/20 tests, including exact asymmetric
scaling, off-screen clipping, painter order and alpha-over-background checks. The
bounded `examples/image.rs` workflow renders a 3×2 fixture into a 240×180
surface, asserts six source-color positions and the untouched background, and
writes `output/image-placement.svg` plus `output/image-placement.bmp`. The SVG
generated by that run is committed as the [manual image artifact](manual/images/image-placement.svg)
and was inspected through the BMP rendering; it is derived from the example,
not hand-authored. This closes the software raster image subset of V06. Browser
and native format decoding, orientation metadata, font/media lifecycles and GPU
vector paths remain open.

## Browser stale-response evidence — 2026-09-07

The service conformance host now accepts `--response-delay-ms` with a bounded
1–30,000 millisecond value. The delay is implemented by Moirai's asynchronous
timer in `AsyncIpcServer` and applies only to a successful clinical response;
handshake, rejection and event frames remain immediate. The probe was run at
revision `a8cc67c` with the standalone lock resolving Moirai to
`d879779247c8cfc5870f62f99a5364cbbf2d3c58`:

```text
cargo run --locked -p metis-app -- --metis-browser-service http://127.0.0.1:8080 8765 66666666666666666666666666666666 --response-delay-ms 4000
```

The Codex in-app browser used the configured workbench URL at a 1280×720
CSS-pixel viewport and device scale 1.25; its engine version was unavailable.
After observing `Authorized backend session ready`, the trace activated
`Submit to authorized backend` and observed `Request in progress`, then
activated `Stop host` before the four-second response deadline. The stopped DOM
contained only `Metis browser host stopped.`. It then activated `Start host`
while the old response was pending. The new generation rendered
`Backend unavailable [ERR_TRANSPORT_BROKEN]`, `Host capabilities: unavailable`
and `Remote events: none`. After waiting 4.5 seconds, the accessibility tree
was unchanged and contained no old result, event or request completion. The
service session ended when the dropped peer could no longer receive the delayed
frame; no stale response reached the remounted DOM.

The screenshot captured after the wait showed the remounted CSS form with the
typed disconnected status, default controls and no prior clinical result. This
is lifecycle evidence for one in-app browser engine and the real loopback
service. Browser engine comparison, post-drop JavaScript allocation counts,
TLS and native host permission evidence remain open.

## Final gate evidence — 2026-09-07

The delivered revision passes `python scripts/verify.py`. The gate reports zero
exit status for compiler identity, dependency metadata, revision and fixture
freshness, formatting, visual tests, WASM library checks, Clippy, debug and
release builds, distribution, debug and release nextest suites, doctests,
documentation, the runnable example, presentation checks and visual capture
comparison. The deliberate capture-failure probe exits 1 as its negative oracle;
the gate records that result as expected and still passes overall.

The debug and release native suites each run 197 tests with zero failures or
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
client injection test also rejects a canceled sequence and then receives a
newer outstanding response. The browser-host delayed-response stop/remount
probe is recorded in [Browser stale-response evidence](#browser-stale-response-evidence--2026-09-07);
native desktop, OS permission and cross-engine coverage remain open.
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
Repository text and lock digests use the committed LF representation, so a CRLF
working-tree checkout cannot create a different fixture identity.

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
disconnect→recover and delayed-response stop/remount rejection. Capture focus/error/success and
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

Current browser evidence covers the Rust-owned textarea, bounded Unicode value,
UTF-16 selection transport and composition lifecycle. It does not yet close
grapheme segmentation, bidi/layout metrics, clipboard/undo, native IME or
assistive-technology acceptance; those remain explicit residuals.

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

<a id="result-explorer-evidence--2026-09-08"></a>
### Result explorer evidence — 2026-09-08

The bounded explorer landed at `e34830f9cac9553dbe2357edcbed88eba0a94633` after the
implementation commit `56aae2b` and rustdoc fix `f93a556`. The shared
frontend contract owns typed rows, bounded patient labels and filters, exact
ordering, stable selection, group disclosure and an eight-entry visible page.

The exact-revision focused run `cargo nextest run -p metis-frontend --locked`
passed 10/10 tests, including the four explorer value-semantic cases. The
browser asset and plan checks passed 14/14 tests. The full `python scripts/verify.py`
gate passed on the pinned 1.97.0 Windows toolchain: supply-chain, format,
visual, plan, WASM libraries, browser assets, clippy, workspace build and
tests, Python binding, release build/tests, doctests, docs, example and the
capture-failure probe all returned their required outcomes.

The software visual report has seven existing form captures with zero changed
pixels and zero semantic differences; its fixture digest is `7fd54e7f34e0adc241bbc3710576b1f7aa07a78d02f528b2d6240b1c1219ed3f`.
A separate CUA trace inspected the generated browser page at
`http://127.0.0.1:8095/?cache=explorer-20260908` in a 1280×720 CSS-pixel viewport
at device scale 1.25. It showed the empty result card, live status, filter,
order select, table caption and disabled pager; changing the filter to `PT`
and `X` returned `Entries 0 of 0`, and keyboard deletion cleared it.

The CUA page had no configured backend bridge, so this capture does not prove
live-row rendering. Connected-service rows, native host rendering, and the
RITK DICOM result-history workflow remain required under V09.

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

Metis does not implement a DICOM parser or volume model. Its browser file-drop
tests stop at the bounded named-byte handoff; RITK owns the subsequent scan,
decode, geometry, and medical display contracts. The RITK
[DICOM workflow](../../ritk/docs/manual/dicom-workflow.md) is the authoritative
visual demonstration: it runs the real byte and filesystem loaders, asserts
exact pixels and physical landmarks, and compares all three slice captures to
reviewed images. A future Métis viewer capture must consume that RITK result
through the presentation seam and must not duplicate the DICOM workflow.

The format-neutral handoff follow-up removes the remaining browser-side DICOM
candidate and Part 10 marker decisions. Metis now reports bounded file metadata
and byte progress only; a RITK adapter receives the named bytes before any
format-specific scan or decode decision.

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
