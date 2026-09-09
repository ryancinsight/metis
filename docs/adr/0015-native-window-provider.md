# ADR 0015: Native window provider boundary

Status: Accepted

Date: 2026-09-08

Driver: [METIS-DESKTOP-001](../../backlog.md#METIS-DESKTOP-001)

Revision 2026-09-08: Moirai PR #283 merged at `3ae43143` adds a finite native
event wait. Metis now composes the provider with the visible `metis-app` form,
including supervised private IPC, resize and input transitions.

Revision 2026-09-08: Moirai PR #284 merged at `7f5ddf80` makes the finite wait
drain retained lifecycle events before blocking on the operating-system queue.
Metis advances its lock so initial window readiness is preserved by the same
native adapter path.

Revision 2026-09-08: Moirai PR #286 merged at `c91e2cdd` adds bounded native
IME start, preedit, commit and cancellation events. PR #287 merged at
`7ad8eeee` closes the empty-composition cancellation edge. Metis consumes the
phases through the same native adapter path.

Revision 2026-09-09: Moirai branch `arch/webview2-provider` at `0310280a`
adds the thread-affine WebView2 host below the existing HWND boundary. It
restricts navigation to a validated packaged `file:///` prefix, bounds bridge
messages, denies new windows and removes every callback before teardown. The
provider's installed-runtime smoke and the Metis adapter's packaged-page
navigation, bridge and external-denial smoke pass on WebView2
`152.0.4191.66`; the visible Metis bundle capture was open at this revision. The provider
does not require a registry or signing key. See the
[Moirai ADR](../../moirai/docs/adr/0052-bounded-webview2-host.md).

Revision 2026-09-09: Metis now exposes `native::WebViewSurface`, which creates
the provider-owned HWND, sizes the controller to the validated client area and
maps visibility, event pumping, navigation and bounded JSON messaging without
leaking WebView2 or Win32 types into the frontend. The Moirai revision remains
an explicit co-evolution pin until its branch merges; application composition
and installed-runtime visual evidence were open at this revision.

Revision 2026-09-09: the supervised WebView2 role admits only the operating-system
path variables required to create the runtime; application settings and
credentials remain cleared. A real Windows capture records native and WebView2
initial/submit states and the trusted keyboard/pointer bridge result in
[`native-captures.json`](../manual/images/native-captures.json), on runtime
`152.0.4191.66`.

## Context

The framework comparison in [ADR 0003](0003-framework-conformance.md) leaves a
native host gap. `PlatformSurface` owns a bounded software framebuffer and an
application-supplied event queue, but it does not create a window or receive
operating-system messages. egui, GPUI, Iced and Tauri each obtain a native
window/event loop from a platform or toolkit provider; a browser screenshot is
not equivalent evidence for that contract.

Moirai already owns Atlas process lifecycle, asynchronous scheduling, browser
DOM callbacks and the Windows system boundary. A second GUI runtime would
duplicate those ownership and teardown models. The native provider must remain
outside the pure Metis framebuffer and state crates, while the application keeps
its existing Rust-owned state and IPC authority.

## Decision

Moirai's Windows PAL exposes a thread-owned native-window provider. It creates a
real Win32 window from a validated title and bounded client size, translates the
window procedure into an owned `WindowEvent` queue, and presents the last
validated ARGB frame through the window's device context. The queue has a fixed
capacity and reports overflow; the frame buffer has a fixed pixel bound and is
reused when dimensions match. The handle is destroyed by `Drop` on its owning
thread, and the callback state remains valid until the window has completed
`WM_NCDESTROY`.

The provider emits close-request, destroyed, focus, pointer, key, Unicode text,
bounded IME composition phases, resize and DPI events. `WM_CHAR` UTF-16 code
units are paired before a scalar is emitted; malformed pairs produce the
replacement character rather than silently dropping input. IME buffers are
bounded by `MAX_COMPOSITION_UNITS`, validated as UTF-16 and canceled when the
composition message carries no string or the composition ends. All event values
are copied into bounded Rust values before a consumer sees them. `WM_PAINT`
repaints the retained frame and never calls into application code.

Metis adds a Windows adapter at its platform boundary. `NativeSurface` presents
the existing `Framebuffer`, exposes Moirai's complete `WindowEvent` values and
waits for input with a finite timeout, so focus, key-up, DPI and lifecycle
information are not discarded by the portable `PlatformEvent` vocabulary. It
imports no Win32 types into frontend or UI-language crates and owns no
authorization, process or filesystem capability. The `metis-app` entry owns
role composition: its native role presents the frontend framebuffer, applies
text and IME composition transitions, handles resize, and sends calculation
requests over the same
supervised private pipe as the headless role.

The same boundary exposes `WebViewSurface` for packaged HTML/CSS applications.
It owns one Moirai `WebViewHost`, aligns its controller bounds and visibility
with `WindowConfig`, forwards the combined window/WebView event batch and keeps
navigation and JSON bridge policy in the provider. The adapter exposes no
filesystem, network, process or authorization capability to page code. The
`metis-app --metis-webview` role composes this surface with the existing
supervised private-pipe frontend and backend, using a bounded temporary page
package for the end-to-end form workflow.

This increment deliberately supplies a native software surface, provider-owned
IME event production, the WebView2 consumer seam and the application bridge.
OS file/network/process denial, accessibility providers, an installed IME
journey, physical resize/DPI and consumer editing policy remain separate host
increments with their own contracts and captures.

## Alternatives

Adding winit, tao, egui, GPUI, Iced or Tauri would add a second event-loop and
window ownership model without enforcing the Metis authority boundary. Calling
Win32 directly from `metis-platform` would place unsafe system code beside a
crate whose safety contract is `forbid(unsafe_code)` and would make another
consumer repeat the provider. Using a browser canvas or a software snapshot
would leave the native event producer unimplemented. Each alternative leaves a
material gap in the requested drop-in host path.

## Threat model and limits

Window messages and frame dimensions are external input. Configuration rejects
empty or NUL-containing titles, zero dimensions and sizes beyond the provider's
pixel and coordinate limits. Event storage is bounded; text conversion never
allocates from an unbounded UTF-16 stream. The Win32 callback catches no panic
and performs no fallible application operation across the ABI boundary. The
presenter treats pixels as data only and does not grant file, network, process or
WebView authority.

The provider and visible host are Windows-only in this increment. Cross-platform
native windows, OS sandbox enforcement, native accessibility, an installed CJK
or other IME journey, and actual two-window permission captures remain open
under the linked backlog items. The committed capture covers only the initial
and successful form states;
a successful Windows build or off-screen frame does not close the remaining
runtime requirements.

## Verification

Provider tests exercise configuration rejection, queue bounds, UTF-16 pairing,
finite waiting, event translation, resize/DPI values and retained-frame
validation. A Windows host test creates a real hidden window, pumps its
lifecycle and destroys it without retained callback state. The Metis adapter
and frontend host tests present actual framebuffer storage, derive the submit
hit region from the authored display list, exercise bounded text and preserve
the old surface across an invalid resize. Warning-denied Clippy, native tests
and the WASM library gate remain required. The capture manifest and four PNGs
provide visual V05 evidence for the initial and successful native/WebView2 form
journeys; installed-IME, accessibility, permission, physical resize/DPI and
cross-platform host evidence remain required.
