# 0003 — Framework conformance and application evidence

Status: Accepted

Date: 2026-09-05

Drivers: `METIS-GAPS-001`,
`METIS-VISUAL-001`,
`METIS-SVELTE-001`,
`METIS-SLINT-001`.

Revision 2026-10-09 (Slint comparator and comparator refresh): Slint 1.18.1
is added as a source-pinned comparator for declarative markup, observable row
models and embedded-grade rendering. Its `Model`/`ModelNotify` contract —
typed row changes (`row_changed`, `row_added`, `row_removed`, `reset`) shared
through one model handle [SL1] [SL2] — is now matched by
`metis_frontend::reactive::ListModel`, which delivers one typed `RowChange` per
mutation to drop-to-unsubscribe peers, begins each subscription with `Reset`,
refuses equal replacements, bounds rows at `MAX_MODEL_ROWS` (the `VirtualList`
bound) and cuts off runaway change cascades at `MAX_CASCADE` rounds. The same
re-inspection re-pins the other named comparators: Tauri core 2.12.0
(2026-09-26) exposes the wry permission-handler API and fixes origin-scoped
ACL denials, egui 0.36.2 (2026-09-08) adds `TextEdit::event_filter`, Axum is
unchanged at 0.8.9, and htmx 4.0.0 (2026-08-28) is a new major surface that
reopens the hypermedia attribute, header and extension inventory. No
comparator behavior is adopted without a Metis implementation and test;
virtualized widgets, the Slint tooling loop, gettext translation flow and
mobile/embedded targets remain open in their linked items.

Current Tauri native admission is pinned in [the capability inventory](../conformance/tauri.json)
and [the reviewed matrix](../manual/tauri.md). Rust browser ownership is
specified in [ADR 0056](0056-rust-browser-shell.md). Historical revision
evidence lives in [the history](0003-framework-conformance/history.md).

## Decision and scope

Use Tauri as the application-framework migration reference, egui/GPUI/Iced as
interaction, text, rendering and test-tooling references, Svelte/SvelteKit as
compiled DOM and web-deployment references, Slint as a declarative-markup,
row-model and embedded-renderer reference, Axum as a server and router
boundary reference, and htmx as a hypermedia boundary reference. Resolve all
gaps in
the audited capability matrix below through the linked development items.
This is a capability contract, not a promise to clone incompatible APIs,
every third-party extension, or future upstream releases. New upstream surfaces
reopen the inventory; none inherit a support claim without a test.

Retain [ADR 0002](0002-web-application-contract.md): browser DOM/HTML5/CSS and
system-WebView hosting provide the Tauri migration path; Rust/WASM owns portable
application behavior. Custom rendering has its own contract through Iris.
Moirai owns execution/transport; Metis owns application state, host integration
and permission policy. A GPU renderer is not a prerequisite for a DOM form.

egui, GPUI, Iced, Tauri, Svelte, SvelteKit, Slint, Axum and htmx are comparison
subjects, not newly
adopted dependencies. The conditional Axum boundary is specified in [ADR
0025](0025-axum-server-boundary.md); the admitted loopback demonstration is
not a public deployment or a DICOM service.
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
| egui ecosystem | [0.36.1 release][E0], [commit](https://github.com/emilk/egui/commit/4c1f2fae95475a40e524884ebb298bcb1714b08e), 2026-08-07; re-inspected 2026-10-09 at [0.36.2][E8], 2026-09-08 | Versioned crate docs where available; IME/extras `latest` resolved to 0.36.1; accessibility/template `main` pages are dated observations. The 0.36.2 delta is `TextEdit::event_filter` and hit-test/text-layout bugfixes, which reopen the text-editing row without closing it. |
| GPUI / Zed | Official `main` sources read on the inspection date; observed head [commit](https://github.com/zed-industries/zed/commit/5a9b9558db01a6b906cec2fb70a797affdc58cdd) | Source inventory, not a checked-out build or proof every API is in the published GPUI crate. |
| Tauri | [tauri-v2.11.5 release][T0], 2026-07-01; v2 documentation read on inspection date; core re-inspected 2026-10-09 at [2.12.0][T8], 2026-09-26 | Documentation can describe newer integrations than a release; pin application/driver revisions when building comparison fixtures. The 2.12.0 delta exposes the wry permission-handler API (`PermissionResponse::{Allow, Deny, Default}`), app-bound navigation limits and an origin-scoped ACL denial fix; these reopen the authority and migration inventory without inheriting support claims. |
| Iced | [0.14.0 crate and API docs][I0], released 2025-12-07; official examples and release notes [I1] [I2] | Versioned docs describe Windows/macOS/Linux/Web, Elm-style state/messages/view/update, async tasks, native rendering and wgpu/tiny-skia paths. The former DOM runtime is archived [I3]; DOM reuse is not inferred from current Iced. |
| Svelte / SvelteKit | Official Svelte 5 [overview][S0], [runes][S1] and [custom-elements][S2] docs, plus SvelteKit [introduction][S3] and [project types][S4], inspected 2026-09-18 | Current documentation describes compiler-generated JavaScript/CSS components, rune-based reactivity, custom-element output and configurable SSR/CSR/prerendering. It is a web-component/deployment reference, not evidence for Rust/WASM memory, native-window, permission or installer parity. |
| Dioxus | [0.7.10 crates][D0] (`dioxus`, `dioxus-desktop`, `-signals`, `-stores`, `-hooks`, `-router`, `-fullstack`, `-native`, `blitz-*` 0.2) fetched from crates.io and read on 2026-09-24 | Published crate sources and READMEs, not the website (unreachable from the inspection host) or an executed application. Dioxus renders one `rsx!` component tree to web (`web-sys`), desktop and mobile WebViews (`wry`/`tao`), server-side HTML, liveview and the experimental wgpu `dioxus-native`/Blitz renderer. |
| Axum | [0.8.9 API documentation][A0], inspected 2026-09-10; re-inspected 2026-10-09 with no newer release | Server/router reference for typed routes, state, extraction, middleware and response conversion. Metis keeps Moirai as the transport owner and implements the admitted loopback boundary without an Axum dependency. |
| Slint | `slint` 1.18.1 crate API [SL0] [SL1] [SL2], released 2026-09-21 [SL3]; repository architecture, tooling and licensing [SL4]; translation guide [SL5]; all inspected 2026-10-09 | Published crate API, release artifacts and repository documentation, not a checked-out build or an executed Slint application. docs.rs `latest` resolved to 1.18.1 on the inspection date. |
| htmx | [htmx 4.0.0 release][H3], 2026-08-28; docs [H0]–[H2] re-read 2026-10-09 | Hypermedia attribute/request/swap reference. The major release reopens the attribute, response-header and extension inventory; none of it is a Metis contract until a typed dispatcher test covers it. |

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
| Application state and controls | Immediate-mode widgets and responses [E1] | Entities, views, actions [G1] | Frontend framework supplies widgets/state [T1] | One form with Rust-owned checkbox, radio, range, select, dialog and pointer-capture dispatch; general component lifecycle remains open. STATE (`METIS-STATE-001`), [INPUT](../../backlog.md#METIS-INPUT-001). |
| Layout, themes, resizing | Panels, scrolling, logical-point sizing [E1] | Styled element layout; not browser CSS [G1] | Host HTML/CSS and DOM [T1] | Sequential layout; unsupported custom declarations return typed diagnostics. LAYOUT (`METIS-LAYOUT-001`). |
| Text editing and IME | Text editing plus integration IME contract [E4] | Selection/composition input example [G4] | Browser text/IME, subject to host integration | Browser text and native preedit/commit/cancel events are bounded; post-default navigation selections are validated on keyup; the software renderer draws unshaped TrueType text in one embedded Latin face without selection ([ADR 0047](0047-truetype-text.md)). [TEXT](../../backlog.md#METIS-TEXT-001). |
| Accessibility | AccessKit integration; custom widget semantics required [E5] | AccessKit roles/identity/actions in current source [G3] | Semantic frontend plus WebView/OS accessibility | Browser markup now exposes named groups, polite atomic live regions, dynamic `aria-busy` state and a bounded Chromium/Edge native accessibility-tree trace; the Windows custom-renderer path now projects the validated tree through Moirai AccessKit and routes focus/activation actions. Screen-reader speech, host preference enablement, WebView/OS traversal and non-Windows bridges remain open. [A11Y](../../backlog.md#METIS-A11Y-001). |
| Pointer, keyboard, touch, focus | Backend input, sensitivity and viewports [E1] | Platform events and actions [G1] | Web frontend and native window events [T1] | Browser text, checkbox, radio, range and pointer surface use semantic keyboard/pointer targets; Moirai owns browser pointer ID/capture/release, pointer metadata, bounded file-drop metadata and bounded browser file access, while Metis applies bounded single-pointer drag pan, two-pointer centroid/distance pinch pan/zoom, wheel pan, Ctrl+wheel zoom and format-neutral file-drop state. The Windows `NativeSurface` now returns provider pointer, key, focus, text, bounded IME composition and accessibility-action events; configured chooser runs pass Chromium and Firefox with trusted keyboard/cine traces, while trusted physical-drop evidence, installed IME journeys, screen-reader traversal, Safari/WebKit read authorization and OS pump integration remain open. RITK owns DICOM format decisions after the byte handoff. [INPUT](../../backlog.md#METIS-INPUT-001), desktop items. |
| Browser/WASM execution | eframe canvas host with WASM bindings [E2] | Current `gpui_web`: canvas, WebGPU/WebGL2 [G2] | Web frontend can target browser; native APIs need a host [T1] | `metis-web` loads generated WASM into an HTML5/CSS DOM host and connects through a bounded Moirai WebSocket service; target-surface discovery, lifecycle generation guards, semantic checkbox/radio/range controls and loopback success/rejection/recovery pass. Its borrowed canvas seam accepts RITK-owned `PresentationFrame` pixels and bounded input through Moirai, with explicit asynchronous WebGPU constructors and provider recovery now available beside the raster path; RITK's hosted chooser run passes exact three-plane canvas and file-transfer oracles on Chromium 152 and Firefox 155, while Safari 26.6.2 rejects the first bounded read. Trusted physical input, real GPU output/recovered pixels and full-window capture remain open. BROWSER (`METIS-BROWSER-001`), ASYNC (`METIS-ASYNC-001`), [GRAPHICS](../../backlog.md#METIS-GRAPHICS-001). |
| Hypermedia actions and fragments | No native HTML request/target/swap contract | WebView/browser concern; response markup and script policy remain application-owned | HTML forms and links run in the system WebView; fragment behavior depends on the frontend/runtime | The htmx event→request→target→swap model [H0] [H1] [H2] informs Metis's generation-bound typed Rust/WASM action. Metis invokes the scoped `ui` plugin over the authenticated binary transport and preflights bounded text/attribute patches against an allowlisted target set. Its loopback `metis-app --metis-http-service` role carries the same authenticated, allowlisted fragment contract over Moirai; it does not ship htmx, arbitrary markup or DICOM behavior. [ADR 0022](0022-typed-browser-actions.md), BROWSER (`METIS-BROWSER-001`), FRAGMENT (`METIS-FRAGMENT-001`), AXUM (`METIS-AXUM-001`). htmx 4.0.0 [H3] reopens the attribute and response-header inventory (htmx comparison below). |
| Existing HTML5/CSS frontend reuse | Canvas UI is not DOM compatibility [E2] | Canvas UI is not DOM compatibility [G2] | WebView presentation is the core model [T1] | Custom markup does not preserve DOM/CSS applications. BROWSER (`METIS-BROWSER-001`), [MIGRATION](../../backlog.md#METIS-MIGRATION-001). |
| Native windows and platform lifecycle | eframe/backend-dependent viewports [E1] [E2] | macOS, Windows, Wayland/X11 platform code [G1] | Desktop system WebViews [T1] | Moirai's Windows PAL plus `metis-platform::native::NativeSurface` create a real thread-owned HWND, present the Metis framebuffer, return bounded pointer/key/text/IME events and install/update the validated AccessKit tree before visibility; the generic `NativeApplication` loop owns finite waiting, initial presentation, accessibility updates and terminal cleanup while `metis-app --metis-native-window` composes the software-rendered frontend and private IPC. `metis-app --metis-webview` composes a packaged HTML/CSS form through `WebViewSurface` and the same supervised pipe. The installed WebView2 navigation/bridge smoke and Windows initial/submit captures pass, and RITK's pathless native session now consumes the folder picker and renders a real saved MRI study. Installed IME journey, WebView2 composition, screen-reader traversal, permission probes, physical resize/DPI and macOS/Linux hosts remain open. [WINDOWS](../../backlog.md#METIS-DESKTOP-001), [MACOS](../../backlog.md#METIS-MACOS-001), [LINUX](../../backlog.md#METIS-LINUX-001). |
| Async commands, events, cancellation | Application/host concern | Executor and action facilities [G1] | Commands, events and channels [T2] [T3] | Async client/server, bounded correlation, request cancellation, browser task handle and pre-response Origin validation exist. A versioned capability catalog, target-surface descriptor, bounded local event hub, versioned remote event envelope, typed plugin invocation and host-local plugin registry now cover command discovery and delivery metadata; lifecycle generation guards and the delayed-response stop/remount trace prevent stale browser completions, and the hosted typed fragment matrix passes on Chromium, Firefox and WebKit. COMMANDS (`METIS-COMMANDS-001`), BROWSER (`METIS-BROWSER-001`), FRAGMENT (`METIS-FRAGMENT-001`). |
| Scoped native authority | Tauri-like broker not established by toolkit docs | Tauri-like broker not established by toolkit docs | Capability scopes and host boundaries [T4] | `HostPolicy` enforces exact origin/window/session binding and host-bound HMAC associated data; the live service validates Origin before `101`; OS permission enforcement remains open. AUTHORITY (`METIS-AUTHORITY-001`), desktop items. |
| Images, vector content and media | Extras loaders; renderer integrations [E6] | Image/list examples and GPU elements [G1] | Browser assets/media and host permissions | Validated raster image crops, discrete pixel-grid orientation transforms, normalized affine image placement, clipped one-pixel line segments and bounded width/cap/join polylines now render through the software display list with one painter order and source-over alpha; the browser has an explicit opt-in WebGPU RGBA surface and provider recovery, while arbitrary affine vector paths, font, media, browser decode and GPU visual/resource evidence remain open. [ASSETS](../../backlog.md#METIS-ASSETS-001), [GRAPHICS](../../backlog.md#METIS-GRAPHICS-001). |
| Large lists, tables and reactive updates | Extras tables [E6] | Elements support large list views [G1] | Frontend framework/browser concern | `VirtualList` windows uniform and variable-extent lists (constant-time and Fenwick-tree logarithmic queries, overscan, scroll-to alignment); the result explorer regroups and orders rows in linear passes. No virtualized widget is built on the primitive yet; `metis_frontend::reactive` supplies Svelte-style writable and derived stores with drop-to-unsubscribe handles and bounded re-entrant delivery, plus two-source `derived2`, field `project`ions and stale-safe async `resource`s (the Dioxus memo, store and resource counterparts). `ListModel` peers hear typed `RowChange` values for the rows that moved, the Slint `Model`/`ModelNotify` counterpart. DATA (`METIS-DATA-001`), STATE (`METIS-STATE-001`). |
| Files, persistence and dialogs | Host/application concern | Platform services; browser restrictions [G5] | Official plugin surfaces [T5] | Browser drops now have bounded provider-owned handles and a named-byte batch handoff. Windows exposes a bounded native folder-selection adapter consumed by RITK's saved-study and Ctrl+O workflows; the real 94-file MRI selection, decode, cancellation and prior-frame preservation are recorded in the RITK provenance. Native `ScopedFileProvider` reads now require `CapabilityScope::READ_FILE`, remain below a trusted root and enforce a 64 MiB bound; RITK owns DICOM scanning/decoding. Persistent stores, broader native permission policy, non-Windows providers and audit persistence remain open. [FILES](../../backlog.md#METIS-FILES-001), AUDIT (`METIS-AUDIT-001`), [INPUT](../../backlog.md#METIS-INPUT-001). |
| Clipboard, menus, tray, shortcuts, deep links | Host/integration concern | Platform APIs; web limits differ [G5] | Core/plugin APIs [T5] | Browser clipboard read/write and a Rust-owned command menu exist on the browser, WebView2 and native hosts. Application-scoped keyboard accelerators parse Tauri's `CmdOrCtrl` syntax, resolve browser and Windows key events through one vocabulary and are announced through `aria-keyshortcuts`; `ScopedOpener` is the capability-witnessed counterpart of Tauri's opener plugin for allowlisted http(s) URLs; manifest `url_schemes` are registered by the Linux, macOS and Windows installers and parsed by `metis_core::deep_link`, like Tauri's deep-link plugin. `metis_core::window_state` with `WindowStateFile` saves and restores window geometry and the maximized state, like Tauri's window-state plugin; the Windows surfaces apply it before first show. `native::GlobalShortcuts` registers accelerators as system-wide hotkeys, like Tauri's global-shortcut plugin, and `native::TrayHost` shows a tray icon, its native context menu and toast notifications, like its tray and notification plugins, and `claim_or_forward` forwards a later launch's arguments to the running instance, like its single-instance plugin; manifest `file_associations` register document types with every installer, like its bundler's `fileAssociations`; `Autostart` registers per-user login items, like its autostart plugin. Installed-OS click-through evidence remains open. [INTEGRATION](../../backlog.md#METIS-INTEGRATION-001). |
| Network, shell and sidecars | Application/host concern | Host APIs do not establish a capability broker | Scoped plugins and sidecar support [T5] | `ScopedProcessProvider` is a capability-scoped, direct-argument host boundary with bounded stdout, private stderr draining and finite cleanup; it does not parse shell strings or expose browser-selected executables. Network providers, OS sandboxing and broader sidecar lifecycle remain open. [SERVICES](../../backlog.md#METIS-SERVICES-001). |
| Configuration, API/plugin migration | Separate API and hosting model | Separate API and hosting model | Commands/plugins/configuration/tooling [T1] [T5] | Custom wire protocol only; no import/mapping diagnostics. [MIGRATION](../../backlog.md#METIS-MIGRATION-001). |
| Packaging, signing and updates | eframe template covers app/web build [E7] | Tauri-like distribution contract not established | Platform bundling and signing [T6]; updater plugin [T5] | Application manifest, one application executable serving two process roles (ADR 0006), portable bundle and Windows per-user MSI are the current distribution surface (ADR 0005). RITK's locked package workflow verifies inventory, executable/MSI hashes and `--help`; local replay and install/uninstall evidence show the packaged executable opening the real CT study. Signing, [other platform formats](../../backlog.md#METIS-DISTRIBUTION-003) and [update recovery](../../backlog.md#METIS-DISTRIBUTION-004) remain gaps. DISTRIBUTION (`METIS-DISTRIBUTION-001`), [RELEASE](../../backlog.md#METIS-RELEASE-001). |
| Mobile/touch lifecycle | Target-specific integrations; parity not inferred | Complete mobile product support not established | Android/iOS target and plugin support [T1] [T5] | No mobile host/probes. [MOBILE](../../backlog.md#METIS-MOBILE-001). |
| Semantic and visual tests | egui_kittest interaction/AccessKit/snapshots [E3] | TestAppContext and platform-dependent rendering [G1] | WebDriver routes differ by integration/platform [T7] | Seven software captures plus a local browser semantic/screenshot trace; RITK's hosted chooser run adds real Chromium and Firefox three-canvas galleries with exact pixels, file hashes, bounded rejections, cine/repeat actions and teardown. Safari read authorization remains open. VISUAL (`METIS-VISUAL-001`), QUALITY (`METIS-QUALITY-001`). |
| Memory, latency and growth | Rendering model alone proves no advantage | GPU model alone proves no advantage | Small bundle does not prove low process memory | Three-run eframe and native Métis lifecycle records now bind public CT/MRI captures and process-tree uncertainty; the browser runner can add an explicit user-agent aggregate-memory estimate when the host exposes the secure, cross-origin-isolated API, but surface and process boundaries differ and no matched GPUI/Tauri, WASM allocator/allocation, compositor/input latency, repeated-growth or security evidence exists. [PERF](../../backlog.md#METIS-PERF-001), MEMORY (`METIS-MEMORY-001`). |
| Assurance, provenance and recovery | Application responsibility | Application responsibility | Capabilities, audits and distribution controls [T4] [T6] | MAC vectors/bounded IPC exist; durable audit, supply-chain and operational evidence incomplete. CRYPTO (`METIS-CRYPTO-001`), AUDIT (`METIS-AUDIT-001`), QUALITY (`METIS-QUALITY-001`). |
| Runnable user documentation | Demos and eframe template [E1] [E7] | Source examples [G1] [G4] | Guides and test examples [T7] | The manual now links real RITK package CT, native-picker MRI and hosted Chromium/Firefox MRI captures alongside the software form states and browser workbench. Installed host input, permission, Safari/WebKit and cross-platform evidence remains open. MANUAL (`METIS-MANUAL-001`), VISUAL (`METIS-VISUAL-001`); every new item carries a manual demonstration. |

## Iced-specific comparison

Iced is a comparator, not a Metis dependency. The rows below separate what the
versioned Iced contract supplies from the application-framework and security
contracts Metis still owns. A documented Iced capability closes a Metis gap only
after the corresponding Metis implementation and target evidence pass.

| Capability | Iced 0.14 evidence | Metis consequence and closing work |
| --- | --- | --- |
| State and control flow | Elm-style state, messages, `update` and `view`; `Task` and `Subscription` support asynchronous work [I0] [I1] | Use the state/message split as a design reference; Metis's typed form state, command catalog, bounded async lifecycle and authority boundary are implemented for the current browser/native workflows. General component lifecycle remains open. STATE (`METIS-STATE-001`), COMMANDS (`METIS-COMMANDS-001`), ASYNC (`METIS-ASYNC-001`) |
| Layout and widgets | Responsive layout, built-in text inputs and scrollables, and custom widgets are documented [I0] [I1] | Metis must implement or reject each admitted CSS/layout property and provide reusable DOM controls. LAYOUT (`METIS-LAYOUT-001`), [INPUT](../../backlog.md#METIS-INPUT-001), DATA (`METIS-DATA-001`). |
| Text, IME and accessibility | Iced documents text input/widgets; the comparator does not establish Metis's DOM IME or assistive-technology contract | Keep DOM text, composition, selection, semantic roles and OS bridge in Metis's target items. [TEXT](../../backlog.md#METIS-TEXT-001), [A11Y](../../backlog.md#METIS-A11Y-001). |
| Native windows | The native runtime manages windows and events on supported desktop targets [I0] | Moirai's Windows provider and Metis `NativeSurface` establish the HWND/frame/event boundary; the restricted visible application host is composed and its initial/submit captures are recorded, while native process/permission probes and macOS/Linux providers remain required. The installed WebView2 navigation/bridge smoke is recorded in [VERIFICATION](../VERIFICATION.md#webview2-consumer-seam--2026-09-09). [DESKTOP](../../backlog.md#METIS-DESKTOP-001), [MACOS](../../backlog.md#METIS-MACOS-001), [LINUX](../../backlog.md#METIS-LINUX-001). |
| Browser and WebAssembly | Iced examples run on native and web; current renderer direction uses the browser canvas/GPU path [I1] [I2] | This establishes a useful renderer comparator but does not provide an HTML/CSS DOM replacement. Metis now has a local DOM host; authenticated lifecycle and cross-engine evidence remain. BROWSER (`METIS-BROWSER-001`), ASYNC (`METIS-ASYNC-001`). |
| HTML/CSS reuse | The old `iced_web` DOM runtime is archived and read-only [I3] | Do not claim DOM compatibility from Iced. Metis's DOM route remains an owned implementation with CSS semantics and migration diagnostics. BROWSER (`METIS-BROWSER-001`), [MIGRATION](../../backlog.md#METIS-MIGRATION-001). |
| Renderers and assets | Native renderer abstraction includes wgpu and tiny-skia; current docs identify WebGPU/WebGL-oriented browser rendering [I0] [I4] | Compare renderer correctness and resource bounds through Iris and Metis fixtures; do not add an Iced dependency or duplicate a renderer. [GRAPHICS](../../backlog.md#METIS-GRAPHICS-001), [ASSETS](../../backlog.md#METIS-ASSETS-001), [PERF](../../backlog.md#METIS-PERF-001). |
| Files and authority | Iced provides application/runtime facilities, not Tauri's capability scopes or Metis's deny-by-default broker | Metis must enforce session/origin/window grants around files, network, processes and persistence. AUTHORITY (`METIS-AUTHORITY-001`), [FILES](../../backlog.md#METIS-FILES-001), [SERVICES](../../backlog.md#METIS-SERVICES-001). |
| Packaging and updates | The versioned Iced docs describe application execution and rendering, not Tauri-style installers, signing or updater recovery | Keep packaging, install/uninstall preservation, signing and update recovery in Metis's distribution items. DISTRIBUTION (`METIS-DISTRIBUTION-001`), [RELEASE](../../backlog.md#METIS-RELEASE-001). |
| Semantic and visual tests | Iced 0.14 release notes identify headless mode and first-class E2E testing; exact test APIs require a pinned fixture before adoption [I2] | Metis keeps its own semantic/raster snapshots and must add browser/native capture providers. VISUAL (`METIS-VISUAL-001`), QUALITY (`METIS-QUALITY-001`). |
| Memory and performance | A renderer/framework description does not establish memory reduction or latency parity | Measure Metis against matched workloads and process boundaries after live browser/native apps exist. MEMORY (`METIS-MEMORY-001`), [PERF](../../backlog.md#METIS-PERF-001), [CONFORMANCE](../../backlog.md#METIS-CONFORMANCE-001). |

## Dioxus comparison

Dioxus is the closest Rust comparator: one component tree in Rust, HTML/CSS
styling, and hosts for the browser, desktop and mobile WebViews, the server and
a native wgpu renderer. It is a comparator, not a dependency; the rows name what
the published 0.7.10 crates supply and what Metis owns or still lacks.

| Capability | Dioxus 0.7.10 evidence | Metis consequence and closing work |
| --- | --- | --- |
| Components and markup | `rsx!` elements and function components returning `Element`; props via derive [D0] [D1] | Metis keeps Rust-owned state, a typed command catalog and custom markup rather than a JSX-like macro; a general component lifecycle remains open. STATE (`METIS-STATE-001`), COMMANDS (`METIS-COMMANDS-001`) |
| Fine-grained state | Copy `Signal`s, `use_memo` over any read signals, global signals and `Store` field projections [D2] [D3] [D4] | `metis_frontend::reactive` supplies `Writable`, `derived`, two-source `derived2` and `Writable::project` field views that notify only when their field changes. Automatic dependency tracking is not adopted: dependencies are explicit, so a store never subscribes to what it happened to read. STATE (`METIS-STATE-001`) |
| Async data | `use_resource`, `use_future`, `use_coroutine` and suspense boundaries [D4] | `reactive::resource` holds `Pending`/`Ready`/`Failed` for each source value and ignores completions for superseded values, on any executor. Suspense-style rendering boundaries remain open. ASYNC (`METIS-ASYNC-001`) |
| Routing | Typed `Routable` enum router with nested layouts and history [D5] | `metis_core::route::Router` maps literal, `:parameter` and `*rest` patterns to application values with order-independent precedence, conflict refusal and `href` building; paths decode under the deep-link rule, so deep links route through their segments. `metis_frontend::navigation::Navigator` exposes the current route as a store, and `metis_web::BrowserNavigator` keeps it in step with same-origin history (Moirai `WebHistory`), pushing entries only for matched paths. Nested layouts remain open. [MIGRATION](../../backlog.md#METIS-MIGRATION-001) |
| Desktop integration | `dioxus-desktop` wraps `wry`/`tao` with `tray-icon`, `muda` menus, `global-hotkey`, `rfd` dialogs and window-state persistence [D6] | Metis has first-party, bounded counterparts: tray and popup menus, system-wide shortcuts, native dialogs, window state, single instance, deep links, file associations and autostart through Moirai, and `native::MenuBarHost` attaches native menu bars whose item labels show the bound shortcuts. [INTEGRATION](../../backlog.md#METIS-INTEGRATION-001) |
| Server and fullstack | Axum-based server functions, SSR with hydration, websockets, streaming and forms [D7] | Metis keeps Moirai as the transport owner with a bounded loopback service and typed plugin invocation; SSR and hydration are not provided. See the Axum comparison below. COMMANDS (`METIS-COMMANDS-001`), BROWSER (`METIS-BROWSER-001`) |
| Native rendering and accessibility | `dioxus-native` renders HTML/CSS through Blitz on wgpu, with AccessKit behind a feature [D8] | Metis renders its software display list natively with an AccessKit tree on Windows and an HTML/CSS DOM in the browser; GPU presentation evidence remains open. [GRAPHICS](../../backlog.md#METIS-GRAPHICS-001), [A11Y](../../backlog.md#METIS-A11Y-001) |
| Tooling | `dx` CLI: templates, `serve` with RSX/asset hot reload and experimental Rust hot patching (`subsecond`), `bundle` for web, desktop and mobile [D0] [D9] | `metis init`, `metis dev` (portable watch) and `metis build` cover templates, rebuild-on-change and installers; code hot patching and mobile bundles are not provided. DISTRIBUTION (`METIS-DISTRIBUTION-001`), [MOBILE](../../backlog.md#METIS-MOBILE-001) |
| Authority | Server functions and desktop APIs are callable by app code; no Tauri-style capability scopes are documented in the crates | Metis's deny-by-default broker and capability witnesses remain the differentiator to preserve. AUTHORITY (`METIS-AUTHORITY-001`) |

## Svelte and SvelteKit comparison

Svelte is a compiler-driven web-component framework. Its current reference
surface is useful for testing whether a Rust-owned DOM host can preserve the
same HTML/CSS integration points without importing a JavaScript application
runtime. SvelteKit is evaluated separately as an application and deployment
framework. Documentation describes capabilities; it does not substitute for a
matched application trace, resource measurement or security test.

| Capability | Svelte/SvelteKit reference | Metis consequence and closing work |
| --- | --- | --- |
| Component state and reactivity | Svelte 5 runes are compiler syntax; `$state` creates reactive state and `$derived`/`$effect` describe dependent work [S0] [S1] | Preserve Rust-owned state, commands and event envelopes. `metis_frontend::reactive` supplies the subscription contract, with `Writable` and `derived` stores whose `Subscription` handles unsubscribe on drop, in the manner of Svelte stores. A full component lifecycle remains a `STATE`/`COMMANDS` item, and no JavaScript state runtime is admitted. STATE (`METIS-STATE-001`), COMMANDS (`METIS-COMMANDS-001`) |
| HTML, CSS and component output | The Svelte compiler turns declarative HTML, CSS and JavaScript components into JavaScript; the overview describes the generated output as optimized JavaScript [S0] | The browser host continues to own HTML5/CSS while Rust owns behavior and the typed style contract. DOM/CSS migration diagnostics must remain explicit; compiled Svelte output is an external consumer contract, not a Metis dependency. BROWSER (`METIS-BROWSER-001`), LAYOUT (`METIS-LAYOUT-001`), [MIGRATION](../../backlog.md#METIS-MIGRATION-001) |
| Custom elements and shadow DOM | Svelte can compile a component as a custom element with `<svelte:options>`, `$host`, properties and configurable shadow-root behavior [S2] | Interoperability is limited to precompiled, allowlisted custom elements at the DOM boundary. Metis must type-check element identity, properties and events before dispatch and must not accept arbitrary component scripts or turn a custom element into an authority boundary. BROWSER (`METIS-BROWSER-001`), AUTHORITY (`METIS-AUTHORITY-001`), FRAGMENT (`METIS-FRAGMENT-001`) |
| Rendering and deployment | SvelteKit documents configurable SSR, CSR, prerendering, SPA, separate-backend and offline project types [S3] [S4] | These modes guide deployment comparison only. Rust/WASM browser execution, native windows, Moirai transport and Métis installers remain the supported seams; no SSR, hydration or SvelteKit adapter claim is made. WEB (`METIS-WEB-001`), BROWSER (`METIS-BROWSER-001`), DISTRIBUTION (`METIS-DISTRIBUTION-001`) |
| Security and host authority | The Svelte/SvelteKit reference describes component compilation and deployment, not a Tauri-style capability broker or OS permission model [S2] [S3] | Keep `HostPolicy`, Origin/session binding, capability HMAC and OS permission probes as Metis-owned contracts. A Svelte component, custom element or SvelteKit adapter never grants file, network, process or installer authority. AUTHORITY (`METIS-AUTHORITY-001`), [SERVICES](../../backlog.md#METIS-SERVICES-001) |
| Evidence and performance | Official documentation gives no matched DICOM, native-window, allocator-used or compositor/input-latency measurement | Extend the existing visual and resource protocols only with an equal consumer fixture, surface, host boundary, input trace and workload. Until then, Svelte is a design comparator and no memory, latency or framework ranking is claimed. VISUAL (`METIS-VISUAL-001`), MEMORY (`METIS-MEMORY-001`), [PERF](../../backlog.md#METIS-PERF-001), [CONFORMANCE](../../backlog.md#METIS-CONFORMANCE-001) |

The Svelte-derived implementation order is therefore: retain the Rust/Moirai
state and authority seams; close the generic DOM component lifecycle and CSS
migration diagnostics; specify typed custom-element interoperation only after
those seams are tested; and measure any external Svelte consumer against the
existing equal-fixture protocol. SvelteKit SSR, hydration, adapters and desktop
wrappers remain explicit comparison or integration requirements rather than
new framework dependencies. This keeps the Tauri replacement path compatible
with ordinary HTML5/CSS applications without moving DICOM or application state
into Metis.

## Slint comparison

Slint is the closest declarative-markup comparator: one `.slint` document
declares components, properties, bindings and repeated elements, compiled
ahead of time to Rust or loaded through the interpreter, and rendered by
selectable backends that include a dependency-free software renderer for
microcontrollers [SL0] [SL4]. It is a comparator, not a dependency; the rows
separate its published 1.18.x contract from what Metis owns or still lacks.

| Capability | Slint 1.18 evidence | Metis consequence and closing work |
| --- | --- | --- |
| Declarative markup and bindings | The `.slint` language declares components, properties, expressions and property bindings; `slint-build` compiles files in a build script, the `slint!` macro embeds them and the interpreter loads them at run time [SL0] [SL4] | `metis-ui-lang` keeps its bounded markup subset with typed rejection of unsupported declarations. A compiled binding language, run-time loading and custom expressions remain outside the admitted contract. LAYOUT (`METIS-LAYOUT-001`), [MIGRATION](../../backlog.md#METIS-MIGRATION-001) |
| Row models and large lists | `Model`/`ModelNotify` deliver `row_changed`, `row_added`, `row_removed` and `reset` to attached peers through shared `ModelRc`/`VecModel` handles, and `for` repetitions bind to models [SL1] [SL2] | `metis_frontend::reactive::ListModel` now delivers the same row vocabulary as typed `RowChange` values to drop-to-unsubscribe peers, begins each subscription with `Reset`, refuses equal replacements and bounds rows and cascades. Dependencies stay explicit as in the Dioxus decision: nothing tracks which row a listener read. A virtualized widget over `VirtualList` remains open. DATA (`METIS-DATA-001`), STATE (`METIS-STATE-001`) |
| Widgets and layout | Standard widget set, box/grid layouts and list views are documented toolkit surfaces [SL0] [SL4] | Metis's reusable controls live in the DOM hosts and the semantic tree; the general component lifecycle and the virtualized list widget remain open. [INPUT](../../backlog.md#METIS-INPUT-001), LAYOUT (`METIS-LAYOUT-001`) |
| Text and accessibility | Text items and accessible role/label properties feed platform accessibility behind the `accessibility` feature [SL0] [SL3] | Keep DOM text, composition and selection and the projected `SemanticTree`/AccessKit contract in Metis's own items; a Slint runtime does not establish screen-reader or IME evidence for Metis. [TEXT](../../backlog.md#METIS-TEXT-001), [A11Y](../../backlog.md#METIS-A11Y-001) |
| Rendering and resource bounds | Configurable renderers: femtovg (OpenGL ES 2.0), Skia and a dependency-free software renderer, with an optional Qt style [SL4] | Compare renderer correctness and bounds through Iris and the software display list; do not adopt a second renderer. Lightweight-memory claims are design goals, not matched measurements. [GRAPHICS](../../backlog.md#METIS-GRAPHICS-001), MEMORY (`METIS-MEMORY-001`), [PERF](../../backlog.md#METIS-PERF-001) |
| Targets and platform APIs | Documented desktop, web (WASM), mobile and microcontroller targets, with Android backends and MCU board templates in the release artifacts [SL3] [SL4] | That breadth reopens the mobile inventory without closing any pair: Metis has no mobile or embedded host, and unsupported operations return explicit outcomes. [MOBILE](../../backlog.md#METIS-MOBILE-001), [WINDOWS](../../backlog.md#METIS-DESKTOP-001) |
| Tooling loop | LSP with live preview, `slint-viewer` auto-reload, an online editor and a Figma import plugin [SL4] | `metis dev --watch` is the current developer loop; design-tool import and live preview remain unimplemented tooling, not application runtime gaps. DISTRIBUTION (`METIS-DISTRIBUTION-001`), [MIGRATION](../../backlog.md#METIS-MIGRATION-001) |
| Translations | `@tr()` marks strings with formatting, plurals and contexts; `slint-tr-extractor` generates gettext templates, and runtime gettext or bundled catalogs select the locale [SL5] | `metis_ui_lang::catalog` compiles keys and named slots at build time and fails closed to the source string; routing consumer display text through it remains open. [I18N](../../backlog.md#METIS-I18N-001) |
| Authority and licensing | Slint ships under GPL-3.0, royalty-free or commercial terms and documents no capability scopes or OS permission model [SL4] | Keep `HostPolicy`, capability witnesses and OS permission probes as Metis-owned contracts; a `.slint` document or generated component never grants file, network or process authority. AUTHORITY (`METIS-AUTHORITY-001`), [SERVICES](../../backlog.md#METIS-SERVICES-001) |
| Evidence and performance | Documentation, demos and release artifacts, not matched Metis fixtures | Extend the equal-fixture protocol only with the same consumer workload, surface and process boundary; no memory, latency or framework ranking is claimed from toolkit descriptions. VISUAL (`METIS-VISUAL-001`), [PERF](../../backlog.md#METIS-PERF-001), [CONFORMANCE](../../backlog.md#METIS-CONFORMANCE-001) |

The Slint-derived implementation order is therefore: keep the row-model
notifications and explicit-dependency stores; close the virtualized widget and
general component lifecycle on top of them; treat the `.slint` tooling loop and
translation flow as migration references for their Metis counterparts; and
measure any external Slint consumer only against the equal-fixture protocol.
Slint's embedded and mobile targets re-open the mobile inventory without
closing a single pair, and its license terms are not a Metis dependency.

## Axum server and router comparison

Axum is evaluated as a server boundary, not as a GUI renderer or a replacement
for Moirai. Its `Router` composes routes and state, extractors validate request
inputs, middleware wraps the request lifecycle, and `IntoResponse` converts a
typed result into an HTTP response ([A0]–[A4]). These are useful contract
shapes for a future server deployment, but documentation inspection is not
runtime or security evidence.

| Server concern | Axum reference surface | Metis consequence and closing work |
| --- | --- | --- |
| Route composition | `Router` and nested routes [A0] | The admitted loopback service uses a closed `/health`, `/v1/session` and `/v1/fragments` route table over Moirai; public deployment remains out of scope. AXUM (`METIS-AXUM-001`) |
| State and extraction | `State<S>`, `FromRef` and extractors [A1] [A2] | Service state must remain Rust-owned and typed, with bounded body sizes and no frontend authority; the existing browser transport remains Moirai-owned. COMMANDS (`METIS-COMMANDS-001`), AUTHORITY (`METIS-AUTHORITY-001`) |
| Middleware and deadlines | Request middleware layers [A3] | A server increment must enforce origin/session authorization, deadlines, cancellation and backpressure before dispatching a command. AXUM (`METIS-AXUM-001`), [SERVICES](../../backlog.md#METIS-SERVICES-001) |
| Typed responses and fragments | `IntoResponse` [A4] | Responses must be versioned typed envelopes or allowlisted text/attribute patches; arbitrary HTML, scripts and DICOM data remain outside Metis. FRAGMENT (`METIS-FRAGMENT-001`), AXUM (`METIS-AXUM-001`) |
| Operational evidence | Axum docs describe APIs, not deployment behavior | The loopback target has real handshake/fragment, denial, malformed/oversized, deadline and teardown tests; public deployment and cross-engine captures remain separate work. AXUM (`METIS-AXUM-001`), VISUAL (`METIS-VISUAL-001`) |

The selected design is a first-party boundary over the Atlas transport stack.
The current target is a loopback demonstration for typed fragments; adding Axum
would duplicate transport ownership and would make a third-party dependency
look like a security boundary. This decision does not authorize public
deployment and does not move DICOM parsing, metadata, geometry or viewer state
out of RITK.

## htmx and HTML5 hypermedia comparison

htmx is the hypermedia comparator: HTML attributes trigger a request, choose a
target and select a swap, and the response is markup the browser applies
[H0]–[H2]. Its 4.0.0 release [H3] is a new major surface, so the attribute,
response-header and extension inventory reopens; none of it is a Metis
contract without a typed test. Metis keeps the model's shape and rejects its
authority: an event becomes a generation-bound typed action over the
authenticated transport, and the response is an allowlisted text/attribute
patch, never arbitrary markup.

| Concern | htmx/HTML5 reference | Metis consequence and closing work |
| --- | --- | --- |
| Event→request→target→swap | Trigger, target and swap attributes route DOM events to requests and apply the response [H0] [H1] [H2] | The browser workbench delegates ordinary input and control events at the mounted root and dispatches generation-bound typed actions through the scoped `ui` plugin; every patch is preflighted against an allowlisted target set before it applies. FRAGMENT (`METIS-FRAGMENT-001`), [ADR 0022](0022-typed-browser-actions.md) |
| Response content | Any HTML fragment can be swapped into the target, and extensions add behaviors | Metis admits bounded text and attribute patches only. Arbitrary markup, scripts, extension code and DICOM data stay outside the admitted contract. FRAGMENT (`METIS-FRAGMENT-001`), BROWSER (`METIS-BROWSER-001`) |
| Headers and history | Response headers drive redirects, out-of-band swaps and event triggers [H0] | The versioned typed envelope and the bounded unsolicited event surface carry the same information; header-driven commands are not parsed from HTTP responses. COMMANDS (`METIS-COMMANDS-001`), [ADR 0012](0012-command-event-contract.md) |
| Server surface | A hypermedia application needs only HTML responses from any server | The loopback `metis-app --metis-http-service` role serves the same authenticated, allowlisted fragment contract over Moirai; it is not a public deployment, a template engine or a DICOM service. AXUM (`METIS-AXUM-001`), [ADR 0025](0025-axum-server-boundary.md) |
| HTML5 form semantics | Native form validation, file inputs and submission semantics belong to the host document | `metis-web` keeps bounded Rust-owned controls with typed invalid-input rejection over the host's native text and IME behavior; migrating an existing htmx frontend remains a mapped task, not an implied compatibility. [MIGRATION](../../backlog.md#METIS-MIGRATION-001), BROWSER (`METIS-BROWSER-001`) |

## Concrete findings driving priority

The form-state findings are resolved by [ADR 0004](0004-form-state.md): private
inputs and a single outcome clear stale results on edits and local/peer failures.
Real backend traces and seven software captures verify the transitions. This
2026-09-05 revision replaces the original stale-result finding; responsive host
events remain dependent on asynchronous transport and browser/native hosting.
The [style contract](../../crates/metis-ui-lang/README.md) admits a bounded set
of declarations and rejects unknown, malformed or unsupported custom-renderer
properties with a typed diagnostic. It never silently accepts browser-like
syntax with different behavior; [ADR 0013](0013-strict-style-contract.md) owns
the parser contract.

The portable [event surface](../../crates/metis-platform/src/event.rs) remains
application-supplied; the Windows [native adapter](../../crates/metis-platform/src/native/mod.rs)
supplies a real event producer over Moirai's HWND provider, including bounded
native IME composition phases. The
[transport](../../crates/metis-ipc/src/transport.rs) still blocks on receipt.
Moirai's merged `be87d009cd0e877beef719b47bdcbadc45659069` browser PAL and HTTP
service own DOM/event callbacks, bounded WebSocket receipt and pre-response
upgrade validation. The follow-up provider revision
`d879779247c8cfc5870f62f99a5364cbbf2d3c58` adds checked and disabled DOM state;
`5a5e4b1540eff39bc3f082c6907f0c82fa14dcc8` adds pointer IDs and capture. The
Metis browser host uses those providers, including cancellable local tasks, and
the live service composes the trusted host policy.
The Windows event producer, native IME event path, visible application host and
initial/submit capture are delivered boundaries. The hosted chooser matrix now
has exact Chromium and Firefox consumer evidence; Safari/WebKit read
authorization and OS permission probes remain closure requirements, not reasons
to add another runtime. Consumer checks are against the pushed provider
revision, not local provider edits.
Iris's current lending rendering seam is sufficient for the software path and
does not block a DOM host.

## Current plan revision — 2026-09-20

The RITK viewport slice is now a delivered consumer boundary: zoom and pan are
validated in RITK, pointer coordinates use the inverse transform, and the real
94-file MRI replay keeps the same pixel and teardown oracles across Chromium and
Firefox. The hosted record is [run 35512369724](https://github.com/ryancinsight/ritk/actions/runs/35512369724),
with Safari selected-file reads and Chromium WebGPU adapter availability recorded
as capability failures. This evidence is bound to the lock-pinned Metis revision
`ab239d70bb561cbe665f852cc55ecbe4275be349`; it does not establish a current-main
framework benchmark.

The remaining implementation order is: finish screen-reader/IME and OS
capability-denial evidence; establish one equal GPUI/Tauri fixture with the same
study, three-plane surface, input trace and process boundary; exercise WebGPU on
a host with an adapter and preserve the no-fallback failure path; then complete
non-Windows packaging/update recovery before the final conformance audit. Each
slice keeps DICOM decoding and clinical presentation in RITK and supplies a
revision-bound manual capture.

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
   update recovery. The admitted loopback HTTP boundary in
   `METIS-AXUM-001` supplies only typed
   presentation routes; mobile and public deployment are separate targets with
   explicit capability limits.
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

Use the [visual scenario matrix](../VERIFICATION.md#visual-scenarios). Each
closing item provides real source, input traces, semantic assertions, target
captures and a user-manual section from the same revision. The current gallery
contains software states from real backend sessions plus RITK's package, native
picker and Chromium/Firefox consumer captures; do not add mockups for
unimplemented applications.
An unsupported browser OS operation closes only its explicit target restriction,
not the equivalent desktop requirement. Apply the same rule to mobile: required
Android/iOS capability pairs come from the pinned inventory, and unimplemented
admitted pairs remain open. The finite quality infrastructure item (`METIS-QUALITY-001`)
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
[E8]: https://github.com/emilk/egui/releases/tag/0.36.2
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
[T8]: https://v2.tauri.app/release/tauri/
[I0]: https://docs.rs/crate/iced/0.14.0
[I1]: https://docs.rs/crate/iced/0.14.0/source/examples/README.md
[I2]: https://github.com/iced-rs/iced/releases
[I3]: https://github.com/iced-rs/iced_web
[I4]: https://github.com/iced-rs/iced/blob/master/Cargo.toml
[H0]: https://htmx.org/docs/
[H1]: https://htmx.org/attributes/hx-target/
[H2]: https://htmx.org/attributes/hx-swap/
[H3]: https://github.com/bigskysoftware/htmx/releases/tag/v4.0.0
[SL0]: https://docs.rs/slint/latest/slint/
[SL1]: https://docs.rs/slint/latest/slint/trait.Model.html
[SL2]: https://docs.rs/slint/latest/slint/struct.ModelNotify.html
[SL3]: https://github.com/slint-ui/slint/releases
[SL4]: https://raw.githubusercontent.com/slint-ui/slint/master/README.md
[SL5]: https://docs.slint.dev/latest/docs/slint/guide/development/translations/
[S0]: https://svelte.dev/docs/svelte/overview
[S1]: https://svelte.dev/docs/svelte/what-are-runes
[S2]: https://svelte.dev/docs/svelte/custom-elements
[S3]: https://svelte.dev/docs/kit/introduction
[S4]: https://svelte.dev/docs/kit/project-types
[D0]: https://crates.io/crates/dioxus/0.7.10
[D1]: https://crates.io/crates/dioxus-core/0.7.10
[D2]: https://crates.io/crates/dioxus-signals/0.7.10
[D3]: https://crates.io/crates/dioxus-stores/0.7.10
[D4]: https://crates.io/crates/dioxus-hooks/0.7.10
[D5]: https://crates.io/crates/dioxus-router/0.7.10
[D6]: https://crates.io/crates/dioxus-desktop/0.7.10
[D7]: https://crates.io/crates/dioxus-fullstack/0.7.10
[D8]: https://crates.io/crates/dioxus-native/0.7.10
[D9]: https://crates.io/crates/subsecond/0.7.10
[A0]: https://docs.rs/axum/0.8.9/axum/struct.Router.html
[A1]: https://docs.rs/axum/0.8.9/axum/extract/struct.State.html
[A2]: https://docs.rs/axum/0.8.9/axum/extract/index.html
[A3]: https://docs.rs/axum/0.8.9/axum/middleware/index.html
[A4]: https://docs.rs/axum/0.8.9/axum/response/trait.IntoResponse.html
