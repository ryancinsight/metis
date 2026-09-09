# metis-platform

Bounded software framebuffers, clipped rectangle and bitmap text drawing, an
application-supplied event queue, and ANSI terminal previews. On Windows, the
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

Framebuffer allocation is limited to 16,777,216 pixels (64 MiB); the virtual
surface queue holds at most 1,024 events. Pixel storage uses straight alpha;
source-over composition includes destination opacity. The font covers digits,
case-insensitive Latin letters, and selected punctuation; unsupported glyphs
produce a replacement box. It is not a Unicode shaping engine.

The Windows adapter is a native pixel and event boundary, not a `WebView` or
permission broker. Use `metis_platform::native::NativeSurface` with a
validated `metis_platform::native::WindowConfig` for a real HWND; the adapter
now exposes bounded native IME composition phases through `WindowEvent`.
Multiple `NativeSurface` values can coexist on their creating thread; call
`reopen` only after `close` to reuse a surface's validated configuration.
Application editing policy, `WebView2`, OS permission enforcement and
accessibility remain host-level workflows.
