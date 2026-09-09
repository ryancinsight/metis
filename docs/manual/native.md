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

A host that owns the validated `WindowConfig` can call
`NativeSurface::reopen` after `close` to create a fresh HWND with the same
bounded configuration; reopening a live surface is rejected.

The parent keeps this interactive session under a finite five-minute watchdog
budget so an abandoned window cannot leave a process tree running forever.
This role is Windows-only. Native IME start/update/commit/cancel phases are
consumed by the host; preedit text stays transient and committed UTF-8 text uses
the same bounded patient-field transition as ordinary text input. WebView2
composition remains a separate host role.

## Embed packaged HTML and CSS with WebView2

`metis_platform::native::WebViewSurface` embeds the Moirai WebView2 provider in
the same thread-owned HWND boundary. The entry page must be a validated
`file:///` URI; navigation is restricted to that entry directory, new-window
requests are denied and page messages are bounded JSON values. The surface
does not grant page code filesystem, network or process authority. The
WebView2 runtime must be installed on the Windows machine.

The application executable includes a complete supervised form path over this
boundary:

```powershell
cargo run --locked -p metis-app -- --metis-webview 60 2 0.2
```

The host writes a bounded temporary package containing the form, stylesheet and
bridge script, then removes it after the window closes. Submitting the form
crosses the WebView2 message callback, the unprivileged frontend's private pipe
and the backend's existing capability and audit checks before the result is
posted back to the page. The page has no network or arbitrary navigation
permission. Escape or the close button ends the finite five-minute session.

### Verify the installed WebView2 adapter

On Windows with WebView2 installed, run the ignored adapter smoke from the
workspace root:

```powershell
cargo nextest run --locked -p metis-platform --all-targets --run-ignored all installed_runtime_loads_packaged_page_and_closes_surface
```

The smoke uses a hidden native host, loads a temporary packaged page, checks a
successful navigation, rejects an external HTTPS navigation and observes the
denied-navigation event, then closes the surface. It passed against WebView2
runtime `152.0.4191.66`. A hidden smoke is
not a visual or accessibility capture; the visible form and those user journeys
remain required for desktop acceptance.

```rust
use metis_platform::native::{
    WebViewConfig, WebViewHostEvent, WebViewSurface, WindowConfig, WindowVisibility,
};
use std::time::Duration;

let window = WindowConfig::with_visibility(
    "My Metis web application",
    1024,
    768,
    WindowVisibility::Visible,
)?;
let page = WebViewConfig::new("file:///C:/Apps/MyMetis/index.html")?;
let mut surface = WebViewSurface::new(&window, page)?;

for event in surface.wait_events(Duration::from_millis(250))? {
    match event {
        WebViewHostEvent::Window(_) | WebViewHostEvent::WebView(_) => {}
    }
}
surface.close()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Resize the surface when the parent window reports a new client size. Use
`set_visibility` for explicit visibility transitions and `post_json` for a
bounded page bridge. Keep command authorization in the host or service layer;
the browser page is an untrusted renderer.

## Verify the provider

Run the focused package gate on Windows:

```text
cargo nextest run --locked -p metis-platform
cargo clippy --locked -p metis-platform --all-targets -- -D warnings
```

The native tests create real hidden `HWND` values, present production
framebuffers, post pointer, keyboard, text, IME composition, resize and DPI
messages through the provider, observe bounded event batches, and close the
windows. The two-window test confirms each surface retains its own dimensions
and that closing one leaves the other live. The tests do not use mock windows or
default frames. Hidden test windows are lifecycle evidence; they are not visible
application captures.

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
a live surface is rejected. A committed native screenshot and keyboard/IME
journey are still
required for V05 visual acceptance; the hidden provider test and host unit tests
are lifecycle evidence, not visual evidence. The WebView2 consumer and provider
configuration tests compile and enforce URI/message bounds; the installed
runtime smoke passes when WebView2 is present. A visible WebView2 capture,
page-to-host bridge journey, OS permission denial, native accessibility, an
installed CJK or other IME journey, macOS/Linux providers, two-window captures
and the DICOM viewer host remain V05 and migration work. Do not treat a
successful Windows build or hidden-window test as cross-platform or security
evidence.
