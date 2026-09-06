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

- WASM portability: compile `metis-core`, `metis-platform` and `metis-ui-lang`
  libraries for `wasm32-unknown-unknown`. This does not run WASM, render in a
  browser, validate host bindings or establish Tauri compatibility.

- Types and compilation: frontend cannot import the backend through its declared dependency closure; validated policy fields cannot be overwritten externally.
- Behavioral tests: exact wire fixtures, canonical decoding, malformed corpus, scope/session/time rejection, audit event outcomes and bounded numerical error.
- Independent numeric evidence: dimensional infusion conversion and exact binary fixtures; arithmetic roundoff uses a stated gamma bound.
- Crypto evidence: published HMAC test vector and SHA-256 known answers plus streaming/padding regressions.
- Process evidence: separately built executables exchanging real pipes; PID and result checks. These do not prove OS least privilege.
- Visual evidence: software framebuffer generated from actual form state and inspected independently of compilation.

Use `python scripts/verify.py`. Resolving commands run with `--locked` outside
the Atlas overlay, following Atlas's standalone-lock workflow. Inside Atlas,
the gate preserves the configured shared build directory and profile budgets;
it never disables the shared overlay or creates another build cache. The
committed lock must describe Git sources, without local-overlay substitutions
or unused-patch records. Earlier `--stack` verification is superseded by this
standalone gate so publishing cannot ship an overlay-only dependency graph.

The user manual replaces a domain book. Its application snapshot is produced by
the Rust presentation example from actual framebuffer pixels and checked against
`docs/manual/images/form.svg`. `--update-snapshots` explicitly refreshes that file;
normal verification rejects drift and missing local manual links.

## Collected Windows evidence — 2026-09-05

The same pinned-toolchain gate also compiles `metis-core`, `metis-platform`,
`metis-ui-lang` and their Iris rendering dependency for
`wasm32-unknown-unknown`. No browser-runtime execution is claimed by this build.

The foundation gate passes on Rust 1.97.0,
`x86_64-pc-windows-msvc`: formatting, all-target Clippy with warnings denied,
82/82 debug tests, 82/82 release tests, ten doctests, documentation with warnings
denied, real-process demonstration and presentation rendering. The 800×600 BMP
is inspected: title/status and all form labels fit; viewport background is filled.
The gate writes exact source hashes and the lock hash to
`output/verification.json`, rejecting source changes during a run.

The resolved host metadata contains 43 packages, including 19 registry packages
through Atlas providers. No Metis package declares a registry dependency.
Separate upstream evidence includes 39/39
Moirai transport tests and 18/18 Atlas overlay-generator tests, with a real Cargo
fixture detecting duplicate local/Git type identities before the generator fix.

The negative executable oracle matches `ERR_NUMERIC_INSTABILITY`, the wire error
code, rather than expecting rejected numeric input in diagnostic text. This
corrects the initial test's message assumption without relaxing rejection.

Windows tests do not prove Linux or macOS behavior. Miri does not execute Windows
native system calls; those require targeted lifecycle tests and further platform
instrumentation. Moirai resolves from pushed commit `0514f11`, not local provider
edits. The public-repository increment passes the standalone gate against the
repaired lock. Comparative security/memory evidence against Tauri and browser
runtime tests remain required by [ADR 0002](adr/0002-web-application-contract.md).
Advisory scanning, coverage,
mutation analysis and cross-platform sandbox probes remain uncollected.

<a id="visual-contract"></a>
## Visual and interaction contract

The implementation today runs one exact software-frame comparison. The contract
below specifies the additional runner/host work owned by
[VISUAL](../backlog.md#METIS-VISUAL-001) and its dependent items. Those requirements
are not claims that browser/native tests already execute.

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

The capture manifest records scenario ID, source/tree hash, action trace and
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

Current evidence: only the initial 800×600 software frame exists. Extend the
existing form/presentation example through a real backend session. For inputs
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
has explicit supported properties; browser semantics are not inferred from
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

<a id="V09"></a>
### V09 — Migrated Tauri application

Pin a representative app and its command/event/channel/config/plugin inventory.
Run the same input traces against original and migrated builds, asserting
behavior and comparing relevant screens. Record the source/configuration delta,
retained JavaScript, Rust/WASM substitutions, unsupported target APIs and native
Metis implementations. Use a form/settings application and a document/result
explorer to exercise different native surfaces. Neither canvas resemblance nor
forwarding requests into Tauri proves migration completion.

<a id="V10"></a>
### V10 — Developer and package lifecycle

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
