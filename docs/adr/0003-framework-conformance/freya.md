# Freya comparison

Comparison date: 2026-10-22. Metis source resolved at `13542f6` on
`origin/main`, where every source path inspected here is identical to the
comparison working tree; that tree also carries one inherited uncommitted
`Cargo.toml` dependency bump (consus, eunomia, hermes) unrelated to any claim
here, and no Freya executable, benchmark or matched fixture was run. This
document is a companion to [ADR 0003](../0003-framework-conformance.md) and
follows its evidence rule: "provided" means documented in the pinned version,
not independently executed, and a comparator capability closes no Metis gap.

## Baseline

[Freya 0.4.3][F0], released 2026-08-30, read from its versioned crate
documentation. The 0.5.0 pre-release line (`0.5.0-rc.10`, 2026-10-11;
`0.5.0-beta.12`, 2026-10-18) reopens the inventory under the rule that applies
to every comparator: no support claim is inherited from an unreleased surface.

Freya is a declarative, native-only GUI library rendered through Skia by its
own crate family: `freya-core` for elements, `freya-engine` for painting,
`torin` for layout and `freya-animation` for interpolation, with
optional surfaces such as `router`, `i18n`, `webview`, `devtools`, `testing` and
the media crates carried as separate optional dependencies [F9]. It is the
closest comparator for Metis's software-rendered native path — one Rust element
tree, hook-owned state, GPU painting, no DOM — and the furthest from its browser
path, which Freya does not build for.

## Capability comparison

Statuses are Metis's, resolved at the pinned revision: **Delivered** (the
boundary exists and is exercised), **Partial** (a real boundary exists with a
named open residual), **Open** (no boundary), **Not adopted** (decided against).

| Capability | Freya 0.4.3 evidence | Metis resolved status at `13542f6` |
| --- | --- | --- |
| Elements and components | Elements are functions returning typed builders (`rect()`, `label()`, `paragraph()`, `image()`), composed with `.child()`/`.children()` and reconciled by `.key()`; reusable UI is a plain function or a `Component` whose `render` reruns when a read state changes or its `PartialEq` props change [F1] [F4] | **Open.** `metis-ui-lang` parses a bounded markup subset into a DOM and paints a display list, but there is no component trait, no keyed reconciliation and no props-equality subtree skip; the clinical form and starter compose through `FrontendApp`'s bespoke path. METIS-COMPONENT-001 |
| State and reactivity | `use_state` returns a `State<T>`; a component subscribes to whatever it reads during render; app-wide state uses Radio stations, channels and `DataReducer`; `Readable`/`Writable` erase the source type; `Memo` and Query cover derived and cached async data [F3] | **Delivered (stores), Partial (component binding).** `metis_frontend::reactive` supplies `Writable`, `Readable`, `derived`, `derived2`, field `project`, `resource` and `ListModel`, with explicit dependencies, drop-to-unsubscribe handles and a bounded cascade that is cut off and reported. Automatic read-tracking and a channel-selecting global station are deliberately not adopted. METIS-COMPONENT-001 |
| Layout and styling | Element-level style methods (`Size::fill`/`px`, `Gaps`, padding, background, color) resolved by the `torin` layout crate; the inspected documentation describes no CSS cascade, selector or stylesheet model [F1] [F4] [F9] | **Delivered (admitted subset).** `metis-ui-lang::layout` computes flex layout, popover placement and virtual-list windows from `style` declarations; unknown or unsupported declarations are typed diagnostics ([ADR 0013](../0013-strict-style-contract.md)), and rounded rects, shadows, gradients and focus rings have their own ADRs. No `overflow`/scroll container (METIS-LIST-002) and no animated property (METIS-ANIMATION-001) |
| Text, selection and IME | Skia-backed text with alignment, decoration and overflow style data, a text-editing module (`freya-edit`) with selection, and IME preedit events [F1] [F8] | **Partial.** [ADR 0047](../0047-truetype-text.md) parses TrueType and draws antialiased glyphs for one embedded Latin face, unshaped and unselectable; the native host forwards bounded IME preedit/commit/cancel and browser text belongs to the host. Shaping, fallback faces, bidi, selection and an installed IME journey remain open. METIS-TEXT-001 |
| Accessibility | Every element carries accessibility data projected into an `accesskit` node, editable through `a11y_builder`, with roles, ids, group membership, focus strategies and a `ScreenReader` surface [F7] [F8] | **Partial.** `metis-ui-lang::semantics` builds a bounded role/name/state/action projection that is explicitly not an OS bridge; the Windows host projects it through Moirai's AccessKit mapping and routes focus and activation. Screen-reader speech and non-Windows bridges remain open. METIS-A11Y-001 |
| Native windows and platforms | winit windows; Linux and Windows paint through Vulkan with an OpenGL fallback, macOS through Metal, Android through OpenGL and described as "highly experimental"; an optional `freya-webview` crate adds a web-content component, but no web/WASM build target is listed [F2] [F9]. Multi-window applications share state through a global Radio station [F3] | **Partial (Windows only).** `metis-platform::native` is compiled only under `#[cfg(windows)]` and owns the HWND surface, native application loop, WebView2, tray, menus, global shortcuts and dialogs; a regression test drives two independent surfaces. macOS and Linux are METIS-MACOS-001 and METIS-LINUX-001; application-level multi-window composition is in METIS-DESKTOP-001. Metis's cross-platform route is the DOM host, which Freya lacks |
| Rendering | Skia through `skia-safe`; the documented backends are Vulkan, OpenGL and Metal, with no software rasterizer [F2] | **Deliberate difference.** Iris paints a software display list on every host, with an opt-in browser WebGPU surface and provider recovery; native GPU presentation evidence remains open. METIS-GRAPHICS-001 |
| Async work and cancellation | Its own executor: `spawn` is tied to the component scope and cancels on unmount, `spawn_forever` attaches to the root scope, `use_future` exposes Pending/Loading/Fulfilled, and `TaskHandle`/`OwnedTaskHandle` cancel explicitly [F5] | **Delivered (transport and listener scoped), Open (component scoped).** Moirai owns the executor and transport; the IPC client and server correlate bounded asynchronous requests, cancel them and guard browser lifecycle generations, and `reactive::resource` ignores superseded completions. Moirai's `LocalTaskHandle` sits in a per-listener slot in the browser host, so cancellation follows a listener; no scope cancels everything a view started on unmount. METIS-COMPONENT-001 |
| Testing | `freya-testing` runs an application headless, injects pointer events, exposes `TestingNode` queries and asserts state [F6] | **Open.** No in-crate headless UI driver exists; `metis-frontend` tests exercise modules, and host evidence comes from the software, browser and native capture runners under `scripts/`. A shared component test set across the software and DOM hosts is METIS-COMPONENT-001's acceptance |
| Integration surface | Tray through `tray-icon`, clipboard, router, i18n, animation, multi-window, hot reload through `dx`, devtools, a performance overlay and zoom shortcuts [F1] | **Delivered except animation and hot patching.** First-party tray, native menu bar, global shortcuts, dialogs, window state, single instance, deep links, file associations and autostart (METIS-INTEGRATION-001); capability-witnessed clipboard; `metis_core::route` with `Navigator` and `BrowserNavigator`; a message catalog with validated BCP-47 locales (`metis-ui-lang::catalog`) whose display-text routing is METIS-I18N-001; `metis dev --watch` rebuilds but does not hot-patch. Animation is METIS-ANIMATION-001. Devtools are disabled in the WebView host by policy |
| Media and asset surfaces | Image, SVG and GIF viewers, video, camera, terminal, markdown, a code editor and a plotters backend, plus Material Design and icon packs, all as crate features [F1] | **Not adopted.** Metis renders validated raster crops, orientation transforms, affine placement, strokes and polylines through the display list; it has no SVG, video, camera, terminal, markdown, editor or plot surface. A media or terminal surface belongs to an application or Iris, not to the presentation framework, and admitting one requires a new decision and an Atlas ownership search |
| Authority and host security | The inspected documentation describes rendering, state, hooks and platform backends; it documents no capability broker, origin/session grant or OS permission model [F1] [F2] | **Delivered (broker), Open (OS enforcement).** `HostPolicy` binds grants to origin, window and session with host-bound HMAC ([ADR 0011](../0011-host-authority-policy.md)), the browser service validates Origin before the upgrade, and the file, network and process providers require capability witnesses. OS permission enforcement remains METIS-DESKTOP-001 |
| Packaging, distribution and mobile | The crate surface documents no bundler, installer, signing or updater; Android builds require the Android SDK, the NDK and `cargo-ndk`; no iOS target appears in the supported table [F1] [F2] | **Partial.** Manifest, one executable with two process roles, portable bundle and per-user MSI ([ADR 0005](../0005-application-distribution.md)), plus Apple bundles ([ADR 0055](../0055-apple-application-bundles.md)); signing, other formats and update recovery are METIS-DISTRIBUTION-003 and METIS-DISTRIBUTION-004; there is no mobile host (METIS-MOBILE-001) |

## Capability resolution at `13542f6`

Each Metis status above was measured at this revision rather than restated
from the matrix in ADR 0003. Three findings follow.

1. **The missing seam is the element/component lifecycle, not the renderer.**
   METIS-COMPONENT-001 already names Freya's hook state as a comparator; the
   comparison adds that `Component: PartialEq` subtree skipping and `.key()`
   reconciliation are the two behaviours a Metis lifecycle must adopt or
   reject explicitly, and that nothing in the tree can test either today.
   `freya-testing` is the model for the driver that item's acceptance requires.

2. **Animation has no owner.** No transition, animation or easing declaration
   exists in `metis-ui-lang`'s style or layout at this revision, and
   `freya-animation` is a shipped crate in the comparator. The gap is recorded
   as METIS-ANIMATION-001 with the admission decision it needs, rather than
   left as prose in a comparison table.

3. **The earlier matrix's closing items no longer resolve.** Fourteen ids cited
   by ADR 0003 — METIS-STATE-001, METIS-LAYOUT-001, METIS-COMMANDS-001,
   METIS-QUALITY-001, METIS-VISUAL-001, METIS-BROWSER-001, METIS-AUTHORITY-001,
   METIS-MANUAL-001, METIS-DATA-001, METIS-MEMORY-001, METIS-DISTRIBUTION-001,
   METIS-AXUM-001, METIS-FRAGMENT-001 and METIS-WEB-001 — have neither a
   heading nor an anchor in `backlog.md` at this revision. Every row above
   cites a live anchor instead. METIS-RESIDUAL-001 owns the repair of the
   earlier rows; this document adds no new dangling reference.

## What this comparison does not establish

Freya leads on shaped and selectable text, animation, a headless test driver
and GPU rendering breadth. Metis leads on DOM/HTML/CSS reuse, a system WebView,
a capability broker, installers and painting without a GPU. Those are different
target sets, not a ranking: no competitor executable, matched workload, process
boundary, allocation count or latency was measured, and the ADR's existing
"no numeric parity percentage" rule applies unchanged.

## Sources

[F0]: https://crates.io/crates/freya/0.4.3
[F1]: https://docs.rs/freya/0.4.3/freya/
[F2]: https://docs.rs/freya/0.4.3/freya/_docs/platforms/index.html
[F3]: https://docs.rs/freya/0.4.3/freya/_docs/state_management/index.html
[F4]: https://docs.rs/freya/0.4.3/freya/_docs/ui_and_components/index.html
[F5]: https://docs.rs/freya/0.4.3/freya/_docs/_async/index.html
[F6]: https://docs.rs/freya-testing/0.4.3/freya_testing/
[F7]: https://docs.rs/freya/0.4.3/freya/elements/extensions/trait.AccessibilityExt.html
[F8]: https://docs.rs/freya/0.4.3/freya/all.html
[F9]: https://crates.io/api/v1/crates/freya/0.4.3/dependencies
