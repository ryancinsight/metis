# ADR 0015: Native window provider boundary

Status: Accepted

Date: 2026-09-08

Driver: [METIS-DESKTOP-001](../../backlog.md#METIS-DESKTOP-001)

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
resize and DPI events. `WM_CHAR` UTF-16 code units are paired before a scalar is
emitted; malformed pairs produce the replacement character rather than silently
dropping input. All event values are copied into bounded Rust values before a
consumer sees them. `WM_PAINT` repaints the retained frame and never calls into
application code.

Metis adds a Windows adapter at its platform boundary. `NativeSurface` presents
the existing `Framebuffer` and exposes Moirai's complete `WindowEvent` values,
so focus, key-up, DPI and lifecycle information are not discarded by the
portable `PlatformEvent` vocabulary. It imports no Win32 types into frontend or
UI-language crates and owns no authorization, process or filesystem
capability. The `metis-app` entry remains the owner of role composition; a
later desktop slice will connect this surface to the frontend and broker.

This increment deliberately supplies a native software surface. WebView2 COM
hosting, HTML/CSS DOM embedding, OS file/network/process denial, accessibility
providers and native IME composition remain separate host increments with their
own contracts and captures.

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

The provider is Windows-only in this increment. Cross-platform native windows,
WebView2 integration, OS sandbox enforcement, native accessibility/IME and
actual two-window permission captures remain open under the linked backlog
items. A successful Windows build or off-screen frame does not close those
runtime requirements.

## Verification

Provider tests exercise configuration rejection, queue bounds, UTF-16 pairing,
event translation, resize/DPI values and retained-frame validation. A Windows
host test creates a real hidden window, pumps its lifecycle and destroys it
without retained callback state. The Metis adapter test presents the actual
framebuffer storage, observes the provider's resize event and closes the
window. `PlatformSurface` and its application-supplied `PlatformEvent` queue
remain unchanged on every target. Warning-denied Clippy, native tests and the
WASM library gate remain required; visual V05 evidence is added when a native
window can be driven by the host capture harness.
