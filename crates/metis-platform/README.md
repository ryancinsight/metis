# metis-platform

Bounded software framebuffers, clipped and rounded rectangles, linear
gradients, box shadows, one-pixel lines, bounded polyline strokes and
antialiased TrueType text, an application-supplied event queue, and ANSI
terminal previews.
On Windows, the
`native` module adapts Moirai's thread-owned Win32 window provider to the
framebuffer without bringing unsafe operating-system code into this crate.
The virtual `PlatformSurface` remains application-supplied and does not create
an operating-system window or provide a GUI isolation boundary.

```rust
use metis_platform::rasterizer::CornerRadius;

let mut pixels = metis_platform::Framebuffer::new(32, 16)?;
let square = metis_platform::Rect::new(0, 0, 8, 8);
metis_platform::fill_rect(&mut pixels, square, CornerRadius::SQUARE, metis_platform::Color::BLUE);
assert_eq!(pixels.get_pixel(2, 2), metis_platform::Color::BLUE);

// The same entry point rounds its corners when given a radius. The radius is
// clamped to half the shorter side, the corner arcs are antialiased by
// coverage, and the straight edges stay exact.
let card = metis_platform::Rect::new(12, 2, 18, 12);
metis_platform::fill_rect(
    &mut pixels,
    card,
    CornerRadius::clamped(5, card),
    metis_platform::Color::WHITE,
);
assert_eq!(pixels.get_pixel(21, 8), metis_platform::Color::WHITE);
// The extreme corner falls outside the arc.
assert_eq!(pixels.get_pixel(12, 2), metis_platform::Color::TRANSPARENT);
# Ok::<(), metis_core::error::MetisError>(())
```

Line segments use the same clipped framebuffer and source-over contract. The
off-screen endpoints are clipped before traversal, so the work is bounded by
the visible surface rather than by the distance outside it.

Polyline strokes add a validated pixel width, endpoint cap and vertex join while
keeping one painter-order and source-over contract. Each visible pixel is
classified once, so overlapping translucent segments do not compound opacity.
The command retains at most 4,096 vertices and scans only the framebuffer
intersection of the path and its bounded miter envelope.

```rust
let mut pixels = metis_platform::Framebuffer::new(32, 16)?;
let width = metis_platform::StrokeWidth::new(3)?;
metis_platform::draw_polyline(
    &mut pixels,
    &[(4, 4), (12, 4), (12, 12)],
    width,
    metis_platform::LineCap::Round,
    metis_platform::LineJoin::Bevel,
    metis_platform::Color::BLUE,
);
assert_eq!(pixels.get_pixel(4, 4), metis_platform::Color::BLUE);
# Ok::<(), metis_core::error::MetisError>(())
```

```rust
let mut pixels = metis_platform::Framebuffer::new(8, 8)?;
metis_platform::draw_line(
    &mut pixels,
    (-4, -4),
    (12, 12),
    metis_platform::Color::RED,
);
assert_eq!(pixels.get_pixel(4, 4), metis_platform::Color::RED);
# Ok::<(), metis_core::error::MetisError>(())
```

Framebuffer allocation is limited to 16,777,216 pixels (64 MiB); the virtual
surface queue holds at most 1,024 events. Pixel storage uses straight alpha;
source-over composition includes destination opacity. Text uses the embedded
Atkinson Hyperlegible Regular and Bold faces (SIL Open Font License 1.1,
`fonts/OFL.txt`), rasterized unhinted with exact area coverage at fractional
positions; characters the faces lack draw the missing-glyph box. Glyphs
advance by their `hmtx` widths without kerning, and it is not a Unicode
shaping engine. Rasterized coverage is memoized per rendering thread, keyed on
face, glyph, scale and the exact device pen and baseline, so a repaint that
redraws a glyph where it was reproduces it bit for bit without rasterizing
again; two generations of at most 2 MiB each bound the memo.

`DisplayScale` is the validated native device-pixel ratio used by the layout
and rasterizer seams. It stores thousandths so 120 DPI and 144 DPI map to
`1.250x` and `1.500x` without floating-point coordinate drift. Use
`draw_text_scaled` for a direct text command; `metis-ui-lang::LayoutViewport`
uses the same scale for geometry, text and hit testing. A host reports its
current value through a `WindowEvent::DpiChanged` adapter.

The Windows adapter is a native pixel, event and `WebView2` boundary, not a
general application permission broker. Use
`metis_platform::native::NativeSurface` with a validated
`metis_platform::native::WindowConfig` for a real framebuffer HWND, or use
`WebViewSurface` with a validated `WebViewConfig` for a packaged `file:///`
entry. Both surfaces keep their callbacks and operating-system handles on the
creating thread; `WebView` navigation and messages remain bounded and
allowlisted. The `WebView2` provider denies every `PermissionRequested` callback
synchronously and reports the typed `WebViewEvent::PermissionDenied` with its
`WebViewPermission`; it never grants a page capability implicitly. Multiple
`NativeSurface` values can coexist on their creating thread; call `reopen` only
after `close` to reuse a surface's validated configuration. Native process and
file policy, application editing policy and accessibility remain host-level
workflows. Native applications that receive a host-authorized
`VerifiedHostCapability<{CapabilityScope::READ_FILE.0}>` can use
`ScopedFileProvider` for bounded reads below one trusted directory. Each read
uses Moirai's handle-anchored opener, rejects traversal and links, and is
limited to `MAX_SCOPED_FILE_BYTES`; the browser file provider remains the
format-neutral alternative. This provider does not parse DICOM or grant a
frontend unrestricted filesystem access.

Native sidecars use `ScopedProcessProvider` with a host-selected executable,
an argument-value allowlist, an explicit direct-child or process-tree policy
and a `RUN_PROCESS` capability witness. The provider starts the child with an
empty environment and direct operating-system arguments; shell strings and
browser-selected executable paths are not accepted. Moirai bounds the
process lifecycle and owns the optional stderr pipe. Metis drains stderr and
returns only its byte count, while stdout is capped at
`MAX_SCOPED_PROCESS_OUTPUT_BYTES`. A deadline requests finite termination and
reports cleanup failure instead of treating an unconfirmed exit as success.

Native network sidecars use `ScopedHttpProvider` with a host-owned HTTP(S) origin
allowlist and a NETWORK capability witness. Requests validate URL, method,
headers and body before reaching Moirai; hop-by-hop headers are rejected,
redirects are disabled, and response bytes remain bounded. Debug
representations redact URL, header values and payload bytes. The provider is
native-only and does not parse DICOM or expose a browser-controlled network
object.

`ScopedOpener` hands a URL to the user's default handler, as Tauri's opener
plugin does, behind an `OPEN_EXTERNAL` capability witness. The host fixes the
launcher (`OpenLauncher::platform_default()` finds `xdg-open`, `open` or the
Windows URL protocol handler at its standard path, never on `PATH`) and an
http(s) origin allowlist. The URL must be printable RFC 3986 ASCII with
complete percent escapes and no credentials, and reaches the launcher as one
argument. The launcher sees only a short list of desktop-session variables.
It is detached rather than contained, because the browser it starts must
outlive the request.

`WindowStateFile` saves a `WindowState` to one host-chosen absolute path and
loads it on the next launch; a missing file means no saved state. A save
writes a fresh sibling file and renames it over the old one, so an
interrupted save leaves the previous state intact. On Windows,
`native::placement_from_state` feeds `WindowConfig::with_placement`, which
restores the rectangle and maximized state before the window first appears,
and `NativeSurface::placement` or `WebViewSurface::placement` reads the
state to save when the window reports `CloseRequested`.

`native::GlobalShortcuts` registers `metis_core::input::Accelerator` values
as system-wide hotkeys on a native surface, like Tauri's global-shortcut
plugin: they fire while other applications have focus. A chord needs a
modifier, holding it reports one press, and a chord another application owns
is refused with the native error. The surface queues presses apart from
window events; `take_commands` turns them into the bound commands after each
event wait. Closing the surface releases every registration.

`native::TrayHost` shows a notification-area icon and notifications, like
Tauri's tray and notification plugins. The application draws a 16 or 32 pixel
icon into a `Framebuffer` with the Metis rasterizer and converts it with
`native::tray_image`. Notifications are shell balloons, which Windows 10 and
later present as toasts. Clicks, keyboard selection, notification clicks and
context requests arrive as `TrayEvent` values from `take_tray_events`; a
context request carries the screen position at which `show_popup_menu` opens
a native menu and returns the chosen item's index. Closing the surface
removes the icon.

`claim_or_forward` keeps one running instance per application, like Tauri's
single-instance plugin. The first launch becomes the `PrimaryInstance`; a later
launch forwards its arguments (at most `MAX_FORWARDED_ARGUMENTS`, UTF-8 without
NUL) and should exit. The primary polls `try_receive` from its event loop and
can pass each list to `metis_core::deep_link::DeepLink::from_arguments`. Moirai
holds the claim: an advisory lock and a socket in an owner-only directory on
Unix, or a session-scoped, local-only named pipe on Windows, released by the
operating system when the primary exits.

Applications that use the native pixel surface can share the bounded host loop
through `metis_platform::native::NativeApplication` and
`run_native_application`. The application owns its state and frame, applies
each complete `WindowEvent` batch and reports whether to repaint; the host owns
only finite waiting, initial presentation and terminal-window cleanup. The
contract is format-neutral: RITK keeps DICOM decoding and medical display
semantics in its own repository and can supply validated viewer frames to this
seam without adding a parser or a GUI dependency to Metis.
