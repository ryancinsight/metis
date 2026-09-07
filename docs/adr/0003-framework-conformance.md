# 0003 — Framework conformance and application evidence

Status: Accepted

Date: 2026-09-05

Drivers: [METIS-GAPS-001](../../backlog.md#METIS-GAPS-001),
[METIS-VISUAL-001](../../backlog.md#METIS-VISUAL-001).

Revision 2026-09-05: the software V01 runner now binds seven real backend states
to recorded semantics, exact raster comparisons and source/fixture provenance.
Deliberate text, geometry and color changes exercise regression detection;
browser/native host evidence remains with the host items.

Revision 2026-09-06: the user names `ritk-snap` as the concrete viewer migration
target. [METIS-MIGRATION-001](../../backlog.md#METIS-MIGRATION-001) now includes
its egui/eframe surface and [V09](../VERIFICATION.md#V09) DICOM opening/display
oracles. RITK retains format, geometry and medical-display ownership; this does
not replace the distinct Tauri fixture or claim a working Métis viewer.

Revision 2026-09-06: Iced 0.14.0 is added as a source-pinned comparator. Its
Elm-style state/update/view model, native runtime, WebAssembly renderer path,
async tasks and headless/E2E testing direction inform Metis's state, host and
visual contracts. The archived `iced_web` DOM runtime is not treated as current
Iced behavior, so it does not close Metis's HTML5/CSS reuse gap.

Revision 2026-09-06: the browser slice adds `metis-web`, a real HTML5/CSS host
that loads generated WASM and drives Rust-owned controls through Moirai's DOM
handles. The local trace proves editable state, typed invalid-input rejection
and explicit missing-bridge failure. It does not close the authenticated live
service, origin/session, accessibility or cross-engine rows.

Revision 2026-09-07: Moirai `16a1b88` adds cancellable browser-local tasks.
Metis request-table cancellation and `metis_stop` listener teardown now have
native and browser lifecycle evidence; authenticated live-service task wiring,
allocation measurement and cross-engine rows remain open.

Revision 2026-09-07: Metis adds [ADR 0011](0011-host-authority-policy.md),
which defines exact origin/window/session admission, host-bound capability HMAC
associated data and the strict external-asset CSP/navigation policy. Core
substitution tests and browser asset checks close the local authority-kernel
increment; service-side Origin validation, OS permissions and live bridge
evidence remain open.

Revision 2026-09-07: Moirai PR #269 added the bounded native WebSocket service
and PR #270 merged at `be87d009cd0e877beef719b47bdcbadc45659069`. Metis now
validates the browser Origin before `101 Switching Protocols`, runs the framed
`AsyncIpcServer` under a trusted host context, and exercises the live loopback
bridge in the browser. Post-drop allocation, TLS, cross-engine and desktop
host evidence remain open.

Revision 2026-09-07: Moirai `9e86e1dd3b9be39b03bacfd830c5a364a33f4f9d` adds the
checked-state DOM seam. Metis now demonstrates Rust-owned checkbox, radio and
range controls while select/menu/dialog, pointer capture, IME, accessibility
technology and native-window input remain open.

## Decision and scope

Use Tauri as the application-framework migration reference and egui/GPUI/Iced
as interaction, text, rendering and test-tooling references. Resolve all gaps in
the audited capability matrix below through the linked development items.
This is a capability contract, not a promise to clone four incompatible APIs,
every third-party extension, or future upstream releases. New upstream surfaces
reopen the inventory; none inherit a support claim without a test.

Retain [ADR 0002](0002-web-application-contract.md): browser DOM/HTML5/CSS and
system-WebView hosting provide the Tauri migration path; Rust/WASM owns portable
application behavior. Custom rendering has its own contract through Iris.
Moirai owns execution/transport; Metis owns application state, host integration
and permission policy. A GPU renderer is not a prerequisite for a DOM form.

egui, GPUI, Iced and Tauri are comparison subjects, not newly adopted
dependencies.
Their companion crates are named separately. Native GPU rendering, mobile
support and distribution are separate increments, not reasons to delay the
first working browser and Windows applications. Unsupported target operations
must return explicit outcomes; a silent no-op never closes a gap.

## Baselines and evidence limits

Initial inspection date: 2026-09-06. Metis source baseline was
`fbda109`; its standalone Windows gate passed
82 debug tests, 82 release tests, ten doctests, WASM library compilation and the
initial software snapshot. The initial comparison ran no competitor executable,
comparative benchmark or native Metis window. The follow-up browser-host trace
is recorded in [VERIFICATION](../VERIFICATION.md) and does not change those
initial comparison limits.

| Reference | Inspected baseline | Qualification |
| --- | --- | --- |
| egui ecosystem | [0.36.1 release][E0], `4c1f2fae95475a40e524884ebb298bcb1714b08e`, 2026-08-07 | Versioned crate docs where available; IME/extras `latest` resolved to 0.36.1; accessibility/template `main` pages are dated observations. |
| GPUI / Zed | Official `main` sources read on the inspection date; observed head `5a9b9558db01a6b906cec2fb70a797affdc58cdd` | Source inventory, not a checked-out build or proof every API is in the published GPUI crate. |
| Tauri | [tauri-v2.11.5 release][T0], 2026-07-01; v2 documentation read on inspection date | Documentation can describe newer integrations than a release; pin application/driver revisions when building comparison fixtures. |
| Iced | [0.14.0 crate and API docs][I0], released 2025-12-07; official examples and release notes [I1] [I2] | Versioned docs describe Windows/macOS/Linux/Web, Elm-style state/messages/view/update, async tasks, native rendering and wgpu/tiny-skia paths. The former DOM runtime is archived [I3]; DOM reuse is not inferred from current Iced. |

“Provided” below means documented or present in inspected source, not
independently executed in the initial comparison. “Host” means the browser/OS or
a named companion supplies the behavior. “Not established” is limited to
inspected sources, not an assertion
that no ecosystem solution exists. No numeric parity percentage is meaningful
until the test inventory and supported target set are fixed.

## Capability gap matrix

Each row names its closing items; acceptance belongs in the
[development board](../../backlog.md), with visual methodology in
[verification](../VERIFICATION.md#visual-contract). All open rows remain gaps.

| Capability | egui ecosystem | GPUI | Tauri | Metis state and closing items |
| --- | --- | --- | --- | --- |
| Application state and controls | Immediate-mode widgets and responses [E1] | Entities, views, actions [G1] | Frontend framework supplies widgets/state [T1] | One form with Rust-owned checkbox, radio and range dispatch; select/menu/dialog and general component lifecycle remain open. [STATE](../../backlog.md#METIS-STATE-001), [INPUT](../../backlog.md#METIS-INPUT-001). |
| Layout, themes, resizing | Panels, scrolling, logical-point sizing [E1] | Styled element layout; not browser CSS [G1] | Host HTML/CSS and DOM [T1] | Sequential layout; several accepted styles do nothing. [LAYOUT](../../backlog.md#METIS-LAYOUT-001). |
| Text editing and IME | Text editing plus integration IME contract [E4] | Selection/composition input example [G4] | Browser text/IME, subject to host integration | Bitmap Latin subset; no composition/selection. [TEXT](../../backlog.md#METIS-TEXT-001). |
| Accessibility | AccessKit integration; custom widget semantics required [E5] | AccessKit roles/identity/actions in current source [G3] | Semantic frontend plus WebView/OS accessibility | No semantic tree/adapter. [A11Y](../../backlog.md#METIS-A11Y-001). |
| Pointer, keyboard, touch, focus | Backend input, sensitivity and viewports [E1] | Platform events and actions [G1] | Web frontend and native window events [T1] | Browser text, checkbox, radio and range controls use semantic keyboard/pointer targets; native event production, touch, capture and OS pump remain open. [INPUT](../../backlog.md#METIS-INPUT-001), desktop items. |
| Browser/WASM execution | eframe canvas host with WASM bindings [E2] | Current `gpui_web`: canvas, WebGPU/WebGL2 [G2] | Web frontend can target browser; native APIs need a host [T1] | `metis-web` loads generated WASM into an HTML5/CSS DOM host and connects through a bounded Moirai WebSocket service; target-surface discovery, lifecycle generation guards, semantic checkbox/radio/range controls and loopback success/rejection/recovery pass, while cross-engine runs remain. [BROWSER](../../backlog.md#METIS-BROWSER-001), [ASYNC](../../backlog.md#METIS-ASYNC-001). |
| Existing HTML5/CSS frontend reuse | Canvas UI is not DOM compatibility [E2] | Canvas UI is not DOM compatibility [G2] | WebView presentation is the core model [T1] | Custom markup does not preserve DOM/CSS applications. [BROWSER](../../backlog.md#METIS-BROWSER-001), [MIGRATION](../../backlog.md#METIS-MIGRATION-001). |
| Native windows and platform lifecycle | eframe/backend-dependent viewports [E1] [E2] | macOS, Windows, Wayland/X11 platform code [G1] | Desktop system WebViews [T1] | Headless Windows-contained process workflow. [WINDOWS](../../backlog.md#METIS-DESKTOP-001), [MACOS](../../backlog.md#METIS-MACOS-001), [LINUX](../../backlog.md#METIS-LINUX-001). |
| Async commands, events, cancellation | Application/host concern | Executor and action facilities [G1] | Commands, events and channels [T2] [T3] | Async client/server, bounded correlation, request cancellation, browser task handle and pre-response Origin validation exist. A versioned capability catalog, target-surface descriptor, bounded local event hub, versioned remote event envelope, typed plugin invocation and host-local plugin registry now cover command discovery and delivery metadata; lifecycle generation guards prevent stale browser completions, while delayed-response injection and cross-engine service traces remain. [COMMANDS](../../backlog.md#METIS-COMMANDS-001), [BROWSER](../../backlog.md#METIS-BROWSER-001). |
| Scoped native authority | Tauri-like broker not established by toolkit docs | Tauri-like broker not established by toolkit docs | Capability scopes and host boundaries [T4] | `HostPolicy` enforces exact origin/window/session binding and host-bound HMAC associated data; the live service validates Origin before `101`; OS permission enforcement remains open. [AUTHORITY](../../backlog.md#METIS-AUTHORITY-001), desktop items. |
| Images, vector content and media | Extras loaders; renderer integrations [E6] | Image/list examples and GPU elements [G1] | Browser assets/media and host permissions | Rectangle/border/bitmap-text commands only. [ASSETS](../../backlog.md#METIS-ASSETS-001), [GRAPHICS](../../backlog.md#METIS-GRAPHICS-001). |
| Large lists, tables and reactive updates | Extras tables [E6] | Elements support large list views [G1] | Frontend framework/browser concern | No virtualized controls or reusable subscriptions. [DATA](../../backlog.md#METIS-DATA-001), [STATE](../../backlog.md#METIS-STATE-001). |
| Files, persistence and dialogs | Host/application concern | Platform services; browser restrictions [G5] | Official plugin surfaces [T5] | In-memory audit only; no user file/store APIs. [FILES](../../backlog.md#METIS-FILES-001), [AUDIT](../../backlog.md#METIS-AUDIT-001). |
| Clipboard, menus, tray, shortcuts, deep links | Host/integration concern | Platform APIs; web limits differ [G5] | Core/plugin APIs [T5] | No desktop integration services. [INTEGRATION](../../backlog.md#METIS-INTEGRATION-001). |
| Network, shell and sidecars | Application/host concern | Host APIs do not establish a capability broker | Scoped plugins and sidecar support [T5] | Process supervision is not a public permission-scoped shell/network API. [SERVICES](../../backlog.md#METIS-SERVICES-001). |
| Configuration, API/plugin migration | Separate API and hosting model | Separate API and hosting model | Commands/plugins/configuration/tooling [T1] [T5] | Custom wire protocol only; no import/mapping diagnostics. [MIGRATION](../../backlog.md#METIS-MIGRATION-001). |
| Packaging, signing and updates | eframe template covers app/web build [E7] | Tauri-like distribution contract not established | Platform bundling and signing [T6]; updater plugin [T5] | Application manifest, one application executable serving two process roles (ADR 0006), portable bundle and Windows per-user MSI are the current distribution surface (ADR 0005); signing, [other platform formats](../../backlog.md#METIS-DISTRIBUTION-003) and [update recovery](../../backlog.md#METIS-DISTRIBUTION-004) remain gaps. [DISTRIBUTION](../../backlog.md#METIS-DISTRIBUTION-001), [RELEASE](../../backlog.md#METIS-RELEASE-001). |
| Mobile/touch lifecycle | Target-specific integrations; parity not inferred | Complete mobile product support not established | Android/iOS target and plugin support [T1] [T5] | No mobile host/probes. [MOBILE](../../backlog.md#METIS-MOBILE-001). |
| Semantic and visual tests | egui_kittest interaction/AccessKit/snapshots [E3] | TestAppContext and platform-dependent rendering [G1] | WebDriver routes differ by integration/platform [T7] | Seven software captures plus a local browser semantic/screenshot trace; committed cross-engine and live-state capture providers remain. [VISUAL](../../backlog.md#METIS-VISUAL-001), [QUALITY](../../backlog.md#METIS-QUALITY-001). |
| Memory, latency and growth | Rendering model alone proves no advantage | GPU model alone proves no advantage | Small bundle does not prove low process memory | No matched baseline or resource telemetry. [PERF](../../backlog.md#METIS-PERF-001), [MEMORY](../../backlog.md#METIS-MEMORY-001). |
| Assurance, provenance and recovery | Application responsibility | Application responsibility | Capabilities, audits and distribution controls [T4] [T6] | MAC vectors/bounded IPC exist; durable audit, supply-chain and operational evidence incomplete. [CRYPTO](../../backlog.md#METIS-CRYPTO-001), [AUDIT](../../backlog.md#METIS-AUDIT-001), [QUALITY](../../backlog.md#METIS-QUALITY-001). |
| Runnable user documentation | Demos and eframe template [E1] [E7] | Source examples [G1] [G4] | Guides and test examples [T7] | Seven real-session software captures and a browser workbench walkthrough; native and authenticated browser evidence remains open. [MANUAL](../../backlog.md#METIS-MANUAL-001), [VISUAL](../../backlog.md#METIS-VISUAL-001); every new item carries a manual demonstration. |

## Iced-specific comparison

Iced is a comparator, not a Metis dependency. The rows below separate what the
versioned Iced contract supplies from the application-framework and security
contracts Metis still owns. A documented Iced capability closes a Metis gap only
after the corresponding Metis implementation and target evidence pass.

| Capability | Iced 0.14 evidence | Metis consequence and closing work |
| --- | --- | --- |
| State and control flow | Elm-style state, messages, `update` and `view`; `Task` and `Subscription` support asynchronous work [I0] [I1] | Use the state/message split as a design reference, while keeping Metis's typed command and authority boundary. [STATE](../../backlog.md#METIS-STATE-001), [COMMANDS](../../backlog.md#METIS-COMMANDS-001), [ASYNC](../../backlog.md#METIS-ASYNC-001) remain open for the DOM and broker contracts. |
| Layout and widgets | Responsive layout, built-in text inputs and scrollables, and custom widgets are documented [I0] [I1] | Metis must implement or reject each admitted CSS/layout property and provide reusable DOM controls. [LAYOUT](../../backlog.md#METIS-LAYOUT-001), [INPUT](../../backlog.md#METIS-INPUT-001), [DATA](../../backlog.md#METIS-DATA-001). |
| Text, IME and accessibility | Iced documents text input/widgets; the comparator does not establish Metis's DOM IME or assistive-technology contract | Keep DOM text, composition, selection, semantic roles and OS bridge in Metis's target items. [TEXT](../../backlog.md#METIS-TEXT-001), [A11Y](../../backlog.md#METIS-A11Y-001). |
| Native windows | The native runtime manages windows and events on supported desktop targets [I0] | Metis still needs restricted Windows, macOS and Linux hosts with real event, process and permission probes. [DESKTOP](../../backlog.md#METIS-DESKTOP-001), [MACOS](../../backlog.md#METIS-MACOS-001), [LINUX](../../backlog.md#METIS-LINUX-001). |
| Browser and WebAssembly | Iced examples run on native and web; current renderer direction uses the browser canvas/GPU path [I1] [I2] | This establishes a useful renderer comparator but does not provide an HTML/CSS DOM replacement. Metis now has a local DOM host; authenticated lifecycle and cross-engine evidence remain. [BROWSER](../../backlog.md#METIS-BROWSER-001), [ASYNC](../../backlog.md#METIS-ASYNC-001). |
| HTML/CSS reuse | The old `iced_web` DOM runtime is archived and read-only [I3] | Do not claim DOM compatibility from Iced. Metis's DOM route remains an owned implementation with CSS semantics and migration diagnostics. [BROWSER](../../backlog.md#METIS-BROWSER-001), [MIGRATION](../../backlog.md#METIS-MIGRATION-001). |
| Renderers and assets | Native renderer abstraction includes wgpu and tiny-skia; current docs identify WebGPU/WebGL-oriented browser rendering [I0] [I4] | Compare renderer correctness and resource bounds through Iris and Metis fixtures; do not add an Iced dependency or duplicate a renderer. [GRAPHICS](../../backlog.md#METIS-GRAPHICS-001), [ASSETS](../../backlog.md#METIS-ASSETS-001), [PERF](../../backlog.md#METIS-PERF-001). |
| Files and authority | Iced provides application/runtime facilities, not Tauri's capability scopes or Metis's deny-by-default broker | Metis must enforce session/origin/window grants around files, network, processes and persistence. [AUTHORITY](../../backlog.md#METIS-AUTHORITY-001), [FILES](../../backlog.md#METIS-FILES-001), [SERVICES](../../backlog.md#METIS-SERVICES-001). |
| Packaging and updates | The versioned Iced docs describe application execution and rendering, not Tauri-style installers, signing or updater recovery | Keep packaging, install/uninstall preservation, signing and update recovery in Metis's distribution items. [DISTRIBUTION](../../backlog.md#METIS-DISTRIBUTION-001), [RELEASE](../../backlog.md#METIS-RELEASE-001). |
| Semantic and visual tests | Iced 0.14 release notes identify headless mode and first-class E2E testing; exact test APIs require a pinned fixture before adoption [I2] | Metis keeps its own semantic/raster snapshots and must add browser/native capture providers. [VISUAL](../../backlog.md#METIS-VISUAL-001), [QUALITY](../../backlog.md#METIS-QUALITY-001). |
| Memory and performance | A renderer/framework description does not establish memory reduction or latency parity | Measure Metis against matched workloads and process boundaries after live browser/native apps exist. [MEMORY](../../backlog.md#METIS-MEMORY-001), [PERF](../../backlog.md#METIS-PERF-001), [CONFORMANCE](../../backlog.md#METIS-CONFORMANCE-001). |

## Concrete findings driving priority

The form-state findings are resolved by [ADR 0004](0004-form-state.md): private
inputs and a single outcome clear stale results on edits and local/peer failures.
Real backend traces and seven software captures verify the transitions. This
2026-09-05 revision replaces the original stale-result finding; responsive host
events remain dependent on asynchronous transport and browser/native hosting.
The [style contract](../../crates/metis-ui-lang/README.md) admits a bounded set
of declarations, stores some properties whose renderer semantics are pending,
and rejects unknown or malformed declarations with a typed diagnostic. It never
silently accepts browser-like syntax with different behavior; [ADR 0013](0013-strict-style-contract.md)
owns the parser contract.

The [event surface](../../crates/metis-platform/src/event.rs) has no native event
producer; [transport](../../crates/metis-ipc/src/transport.rs) blocks on receipt.
Moirai's merged `be87d009cd0e877beef719b47bdcbadc45659069` browser PAL and HTTP
service own DOM/event callbacks, bounded WebSocket receipt and pre-response
upgrade validation. The follow-up provider revision
`9e86e1dd3b9be39b03bacfd830c5a364a33f4f9d` adds checked DOM state. The Metis
browser host uses those providers, including cancellable local tasks, and the
live service composes the trusted host policy.
The native event producer, cross-engine runtime matrix and OS host remain
closure requirements, not reasons to add another runtime. Consumer checks are
against the pushed provider revision, not local provider edits.
Iris's current lending rendering seam is sufficient for the software path and
does not block a DOM host.

## Dependency order and ownership

The board owns the exact dependency graph and acceptance. Work proceeds through:

1. Correct form-state/limit defects and connect real state transitions to visual
   evidence; preserve the existing native behavior tests.
2. Complete bounded async delivery and authority policy, then a real browser form
   and Windows WebView host. Browser local UI and the Windows host may advance
   independently once their respective contracts exist.
3. Input, text, accessibility, responsive layout, assets and data views; complete
   macOS/Linux containment and host probes alongside platform integrations.
4. Tauri command/configuration migration, scoped native services, packaging and
   update recovery; mobile is a separate target with explicit capability limits.
5. Close measured performance/security claims and all target-specific quality
   evidence. Measurement instrumentation begins with the first live application,
   not after implementation choices become fixed.

Reuse verified first-party providers. Metis owns DOM/window/input glue, frontend
state and the broker contract; Moirai owns lifecycle/async/process mechanisms;
Iris owns rendering/color/view seams, not a new browser or windowing engine.
Allocation, storage, crypto and GPU additions require the Atlas ownership search
at their item boundary; implement a missing shared capability upstream. Existing
provider quarantine is removed only after published-source consumer verification.

## Demonstrations and closure

Use the [visual scenario matrix](../VERIFICATION.md#visual-scenarios). Each closing
item provides real source, input traces, semantic assertions, target captures and
a user-manual section from the same revision. The current gallery contains seven
software states from real backend sessions; do not add mockups for unimplemented
applications.
An unsupported browser OS operation closes only its explicit target restriction,
not the equivalent desktop requirement. Apply the same rule to mobile: required
Android/iOS capability pairs come from the pinned inventory, and unimplemented
admitted pairs remain open. The finite [quality infrastructure item](../../backlog.md#METIS-QUALITY-001)
precedes hosts; [final conformance](../../backlog.md#METIS-CONFORMANCE-001) depends
on all scenario providers and alone closes the framework-wide evidence matrix.

Do not adopt the union of all toolkit APIs as one framework abstraction. Preserve
the minimal current shell/render seams, add behavior where a demonstrator needs
it, and test the admitted target set. Do not retain Tauri forwarding wrappers as
“migration support.” A feature passes when its native Metis implementation and
documented migration succeed on the specified targets.

Failure modes include stale comparisons, canvas mistaken for DOM compatibility,
toolkit presence mistaken for security, visual baselines that preserve defects,
and unsupported platforms hidden by compile-only checks. The board's per-item
oracles, [risk register](../../gap_audit.md), and independent review close these
failure modes. No security superiority, memory reduction or regulatory assurance
is established by this planning decision.

## Sources

All links below were resolved/read for this comparison. Mutable pages are dated
observations; future implementation fixtures must pin the actual dependencies.

[E0]: https://github.com/emilk/egui/releases/tag/0.36.1
[E1]: https://docs.rs/egui/0.36.1/egui/
[E2]: https://docs.rs/eframe/0.36.1/eframe/
[E3]: https://docs.rs/egui_kittest/0.36.1/egui_kittest/
[E4]: https://docs.rs/egui/latest/egui/enum.ImeEvent.html
[E5]: https://github.com/emilk/egui/blob/main/docs/accessibility.md
[E6]: https://docs.rs/egui_extras/latest/egui_extras/
[E7]: https://github.com/emilk/eframe_template
[G1]: https://raw.githubusercontent.com/zed-industries/zed/main/crates/gpui/README.md
[G2]: https://raw.githubusercontent.com/zed-industries/zed/main/crates/gpui_web/src/gpui_web.rs
[G3]: https://raw.githubusercontent.com/zed-industries/zed/main/crates/gpui/src/_accessibility.rs
[G4]: https://raw.githubusercontent.com/zed-industries/zed/main/crates/gpui/examples/input.rs
[G5]: https://raw.githubusercontent.com/zed-industries/zed/main/crates/gpui_web/src/platform.rs
[T0]: https://github.com/tauri-apps/tauri/releases/tag/tauri-v2.11.5
[T1]: https://v2.tauri.app/start/
[T2]: https://v2.tauri.app/develop/calling-rust/
[T3]: https://v2.tauri.app/develop/calling-frontend/
[T4]: https://v2.tauri.app/security/capabilities/
[T5]: https://v2.tauri.app/plugin/
[T6]: https://v2.tauri.app/distribute/
[T7]: https://v2.tauri.app/develop/tests/webdriver/
[I0]: https://docs.rs/crate/iced/0.14.0
[I1]: https://docs.rs/crate/iced/0.14.0/source/examples/README.md
[I2]: https://github.com/iced-rs/iced/releases
[I3]: https://github.com/iced-rs/iced_web
[I4]: https://github.com/iced-rs/iced/blob/master/Cargo.toml
