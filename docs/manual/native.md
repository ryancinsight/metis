# Run the Windows native surface

The Windows platform crate now has a safe adapter over Moirai's thread-owned
Win32 provider. It owns one native window, translates its messages into bounded
`WindowEvent` values and presents the same row-major `0xAARRGGBB` pixels used by
`Framebuffer`. The adapter does not host HTML/CSS, grant file or network access,
or change the portable `PlatformEvent` queue.

## Verify the provider

Run the focused package gate on Windows:

```text
cargo nextest run --locked -p metis-platform
cargo clippy --locked -p metis-platform --all-targets -- -D warnings
```

The native test creates a real hidden `HWND`, presents a production framebuffer,
posts pointer, keyboard, text, resize and DPI messages through the provider,
observes the bounded event batch, and closes the window. The test does not use a
mock window or a default frame. A hidden test window is lifecycle evidence; it is
not a visible application capture.

## Connect a host

Create the validated configuration and keep all operations on the creating
thread:

```rust
use metis_platform::{Color, Framebuffer};
use metis_platform::native::{NativeSurface, WindowConfig, WindowEvent};

let config = WindowConfig::new("My Metis window", 800, 600)?;
let mut surface = NativeSurface::new(&config)?;
let mut frame = Framebuffer::new(800, 600)?;
frame.clear(Color::DARK_BLUE);
surface.present(&frame)?;

for event in surface.poll_events()? {
    match event {
        WindowEvent::CloseRequested | WindowEvent::Destroyed => surface.close()?,
        WindowEvent::Resized { width, height } => {
            // Rebuild the application framebuffer at the reported client size.
            let _ = (width, height);
        }
        _ => {}
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

Call `poll_events` from the host's bounded scheduler and present only validated
frames. `CloseRequested` is a policy signal; the host decides when to call
`close`. `WindowEvent` preserves focus, key-up, Unicode text and DPI values that
the portable application-supplied `PlatformEvent` type does not model.

## Current limits

This slice proves the Windows provider and Metis framebuffer boundary. The
`metis-app` demonstration still runs its process workflow without a visible
native window, and the browser path continues to use Moirai's HTML5/CSS host.
WebView2 composition, OS permission denial, native accessibility and IME
composition, macOS/Linux providers, two-window captures and the DICOM viewer
host remain V05 and migration work. Do not treat a successful Windows build or
the hidden-window test as cross-platform or security evidence.
