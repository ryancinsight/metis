> Windows build/run commands require the [current Rust asset preparation](../../../crates/metis-app/src/frontend/webview/manual.md) before compilation. Capture records retain their cited revisions.

## Connect a host

Create the validated configuration and keep all operations on the creating
thread:

```rust
use metis_platform::{Color, Framebuffer};
use metis_platform::native::{NativeSurface, WindowConfig, WindowEvent};
use std::time::Duration;

let config = WindowConfig::new("My Metis window", 800, 600)?;
let mut surface = NativeSurface::new(&config)?;
let mut frame = Framebuffer::new(800, 600)?;
frame.clear(Color::DARK_BLUE);
surface.present(&frame)?;

for event in surface.wait_events(Duration::from_millis(250))? {
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

Call `wait_events` with a finite duration from the host's bounded scheduler and
present only validated frames. `CloseRequested` is a policy signal; the host
decides when to call `close`. `WindowEvent` preserves focus, key-up, Unicode
text and DPI values that the portable application-supplied `PlatformEvent` type
does not model.

## Current limits

The visible `metis-app` composition and its private-IPC workflow are now
implemented. The provider tests exercise two independent hidden HWNDs and close
and reopen one only after close, reusing its validated configuration; reopening
a live surface is rejected. The host trace and framebuffer image above verify
the format-neutral frame/event seam, the four original OS-window captures
establish the visible native and WebView2 initial/submit journeys, and the
resize pair proves a real client-size transition with a rebuilt frame. A native
keyboard/IME journey is still required for V05 input acceptance. The software
mapping for fractional display scales is covered by deterministic component
tests, while a physical monitor transition, broader OS permission enforcement,
native accessibility, an installed CJK or other IME journey, macOS/Linux
providers, two-window captures and the viewer host remain V05 and migration
work. WebView2 provider denial and the four-row permission-probe capture are
covered; broader Windows sandbox enforcement remains open. Do not treat a
successful Windows build or a hidden-window test as cross-platform,
assistive-technology or broad OS-permission evidence.


[Windows native manual](../native.md) · [Current Rust page contract](../../../crates/metis-app/src/frontend/webview/manual.md)
