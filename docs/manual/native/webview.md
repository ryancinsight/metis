> Windows build/run commands require the [current Rust asset preparation](../../../crates/metis-app/src/frontend/webview/manual.md) before compilation. Capture records retain their cited revisions.

## Embed packaged HTML and CSS with WebView2

`metis_platform::native::WebViewSurface` embeds the Moirai WebView2 provider in
the same thread-owned HWND boundary. The entry page must be a validated
`file:///` URI; navigation is restricted to that entry directory, new-window
requests are denied and page messages are bounded JSON values. The surface
does not grant page code filesystem, network or process authority. The
WebView2 runtime must be installed on the Windows machine. The Windows
`metis-platform` target explicitly enables Moirai's `webview2` feature; other
Metis targets do not pull the optional COM binding. The standalone Cargo.lock currently pins Moirai revision
`0c2f752e28c6683d91440783a8b5967a78fa909d`, which includes the AccessKit
provider, the UI Automation value-action mapping, WebView2 0.39.1 bindings and
the surrogate-preservation correction. The current lock retains the provider feature,
bounded host implementation, stable browser canvas extents, the content-box
mapping used by the RITK consumer and the bounded `CapturePreview` PNG path;
it also contains the later WebGPU provider recreation. The capture hash and
runtime provenance remain tied to the revision that produced that artifact.
WebGPU is a browser-only opt-in surface; native WebView2 packaging does not
claim GPU presentation evidence.

The application executable includes a complete supervised form path over this
boundary:

```powershell
cargo run --locked -p metis-app -- --metis-webview 60 2 0.2
```

The host writes a bounded temporary package containing the form, stylesheet and
bridge script, then removes it after the window closes. Submitting the form
crosses the WebView2 message callback, the unprivileged frontend's private pipe
and the backend's existing capability and audit checks before the result is
posted back to the page. Its content-security policy admits only the package's
own script and stylesheet; images, fonts, media, network connections, objects,
frames, workers, manifests and forms are denied. The page script uses only the
bounded `chrome.webview.postMessage` bridge; WebView2 host objects and process
APIs are not registered. The provider separately rejects arbitrary navigation
and new-window requests. These are package/provider restrictions, not a claim
that the operating system sandbox has been proven. The Moirai provider also
handles every WebView2 `PermissionRequested` callback, sets the state to deny
before profile or operating-system prompting, and forwards a bounded
`WebViewEvent::PermissionDenied` snapshot with the permission kind and
user-initiation bit. `metis-app` renders that event as a typed
`permission_denied` page message with `ErrorCode::PermissionDenied`. The
separate permission-probe page requests geolocation, camera, microphone and
notifications sequentially. Each row records host denial, browser-level
rejection or an unavailable API; the calculation page requests no capability,
so its existing form capture remains unchanged. Escape or the close button
ends the finite five-minute session.

### Verify the installed WebView2 adapter

On Windows with WebView2 installed, run the ignored adapter smoke from the
workspace root:

```powershell
cargo nextest run --locked -p metis-platform --all-targets --run-ignored all installed_runtime_loads_packaged_page_and_closes_surface
```

The smoke uses a hidden native host, loads a temporary packaged page, checks a
successful navigation, rejects an external HTTPS navigation and observes the
denied-navigation event, then closes the surface. It passed against WebView2
runtime `152.0.4191.66`. The companion
`installed_runtime_denies_geolocation_permission` smoke was also attempted on
the currently installed runtime `153.0.4234.32`, but its file-origin page did
not emit a callback before the bounded wait; that result is retained as a
provider residual, not a pass claim. The visible application capture below
does receive and render the typed denial. Both tests are ignored unless the
WebView2 runtime is installed. A hidden smoke is not a visual or accessibility
capture; the visible form workflow is recorded below.

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


[Windows native manual](../native.md) · [Current Rust page contract](../../../crates/metis-app/src/frontend/webview/manual.md)
