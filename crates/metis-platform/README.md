# metis-platform

Bounded software framebuffers, clipped rectangles, one-pixel lines, bounded
polyline strokes and bitmap text drawing, an application-supplied event queue,
and ANSI terminal previews.
On Windows, the
`native` module adapts Moirai's thread-owned Win32 window provider to the
framebuffer without bringing unsafe operating-system code into this crate.
The virtual `PlatformSurface` remains application-supplied and does not create
an operating-system window or provide a GUI isolation boundary.

```rust
let mut pixels = metis_platform::Framebuffer::new(32, 16)?;
metis_platform::fill_rect(
    &mut pixels,
    metis_platform::Rect::new(0, 0, 8, 8),
    metis_platform::Color::BLUE,
);
assert_eq!(pixels.get_pixel(2, 2), metis_platform::Color::BLUE);
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
source-over composition includes destination opacity. The font covers digits,
case-insensitive Latin letters, and selected punctuation; unsupported glyphs
produce a replacement box. It is not a Unicode shaping engine.

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

Applications that use the native pixel surface can share the bounded host loop
through `metis_platform::native::NativeApplication` and
`run_native_application`. The application owns its state and frame, applies
each complete `WindowEvent` batch and reports whether to repaint; the host owns
only finite waiting, initial presentation and terminal-window cleanup. The
contract is format-neutral: RITK keeps DICOM decoding and medical display
semantics in its own repository and can supply validated viewer frames to this
seam without adding a parser or a GUI dependency to Metis.
