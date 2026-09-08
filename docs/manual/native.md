# Run the Windows native surface

The Windows platform crate now has a safe adapter over Moirai's thread-owned
Win32 provider. It owns one native window, translates its messages into bounded
`WindowEvent` values and presents the same row-major `0xAARRGGBB` pixels used by
`Framebuffer`. The adapter does not host HTML/CSS, grant file or network access,
or change the portable `PlatformEvent` queue.

## Run the visible form

The application entry composes the adapter with the existing frontend and the
supervised backend process. On Windows, run:

```powershell
cargo run --locked -p metis-app -- --metis-native-window 60 2 0.2
```

The `Metis native form` window renders the production software framebuffer.
While the window is focused, typed Unicode characters extend the patient
reference and Backspace removes its last scalar; every edit clears a prior
calculation through `FrontendApp::set_inputs`. Press **Enter** or click the
blue **[ SUBMIT CALCULATION TO BACKEND ]** surface to send the exact numeric
inputs through the private pipe. The result and audit sequence are painted by
the same frontend state machine as the headless workflow. Resize the window to
exercise framebuffer replacement; DPI, focus and close events are consumed by
the host. **Escape** or the window close control ends the child cleanly.

The parent keeps this interactive session under a finite five-minute watchdog
budget so an abandoned window cannot leave a process tree running forever.
This role is Windows-only. Native IME start/update/commit/cancel phases are
consumed by the host; preedit text stays transient and committed UTF-8 text uses
the same bounded patient-field transition as ordinary text input. WebView2,
file/network/device permissions and accessibility semantics remain host gaps.

## Verify the provider

Run the focused package gate on Windows:

```text
cargo nextest run --locked -p metis-platform
cargo clippy --locked -p metis-platform --all-targets -- -D warnings
```

The native test creates a real hidden `HWND`, presents a production framebuffer,
posts pointer, keyboard, text, IME composition, resize and DPI messages through
the provider,
observes the bounded event batch, and closes the window. The test does not use a
mock window or a default frame. A hidden test window is lifecycle evidence; it is
not a visible application capture.

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
implemented. A committed native screenshot and keyboard/IME journey are still
required for V05 visual acceptance; the hidden provider test and host unit tests
are lifecycle evidence, not visual evidence. WebView2 composition, OS
permission denial, native accessibility, an installed CJK or other IME journey,
macOS/Linux providers, two-window captures and the DICOM viewer host remain V05
and migration work. Do not treat a successful Windows build or hidden-window test
as cross-platform or security evidence.
