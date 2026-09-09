//! Windows `WebView2` consumer boundary over Moirai's bounded host.

use moirai_pal::windows::{
    webview::WebViewHost,
    window::{NativeWindow, WindowConfig, WindowVisibility},
};
use std::{io, time::Duration};

pub use moirai_pal::windows::webview::{
    MAX_WEBVIEW_EVENTS, MAX_WEBVIEW_MESSAGE_BYTES, MAX_WEBVIEW_MESSAGE_UNITS,
    MAX_WEBVIEW_URI_UNITS, MAX_WEBVIEW_WAIT_MILLISECONDS, WebViewConfig, WebViewEvent,
    WebViewHostEvent,
};

/// A Metis desktop `WebView2` surface with a Moirai-owned parent window.
///
/// The surface keeps the `WebView2` controller, its parent HWND and all callback
/// state on the creating thread. Navigation is restricted by [`WebViewConfig`]
/// to packaged `file:///` resources; the host does not grant filesystem,
/// network or process authority to page code. Application state and command
/// authorization stay above this platform boundary.
pub struct WebViewSurface {
    host: WebViewHost,
}

impl WebViewSurface {
    /// Creates a `WebView2` surface at the validated window dimensions.
    ///
    /// The `WebView2` host loads its configured entry page before returning. The
    /// controller is then resized to the parent client area and visibility is
    /// aligned with the [`WindowConfig`]. All operations remain thread-affine.
    ///
    /// # Errors
    /// Returns a native window, COM, `WebView2`, callback, navigation or bounds
    /// error. The installed `WebView2` runtime is required by the host.
    pub fn new(window: &WindowConfig, webview: WebViewConfig) -> io::Result<Self> {
        let native = NativeWindow::new(window)?;
        let mut host = WebViewHost::new(native, webview)?;
        host.resize(window.width(), window.height())?;
        host.set_visible(matches!(window.visibility(), WindowVisibility::Visible))?;
        Ok(Self { host })
    }

    /// Returns whether the `WebView2` controller and parent window are closed.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.host.is_closed()
    }

    /// Resizes the controller to the parent client area.
    ///
    /// # Errors
    /// Returns `InvalidInput` for zero or oversized dimensions, or a native
    /// `WebView2` error.
    pub fn resize(&mut self, width: u32, height: u32) -> io::Result<()> {
        self.host.resize(width, height)
    }

    /// Changes page visibility without changing the parent HWND visibility.
    ///
    /// # Errors
    /// Returns a native `WebView2` error when the controller rejects the change.
    pub fn set_visibility(&mut self, visibility: WindowVisibility) -> io::Result<()> {
        self.host
            .set_visible(matches!(visibility, WindowVisibility::Visible))
    }

    /// Navigates to another resource under the configured packaged prefix.
    ///
    /// # Errors
    /// Returns `PermissionDenied` for a URI outside that prefix, or a native
    /// `WebView2` error.
    pub fn navigate(&mut self, uri: impl AsRef<str>) -> io::Result<()> {
        self.host.navigate(uri)
    }

    /// Posts one bounded JSON message to the page.
    ///
    /// # Errors
    /// Returns `InvalidInput` for an oversized or NUL-containing message, or a
    /// native `WebView2` error.
    pub fn post_json(&mut self, json: impl AsRef<str>) -> io::Result<()> {
        self.host.post_json(json)
    }

    /// Pumps one bounded batch of parent-window and `WebView2` events.
    ///
    /// # Errors
    /// Returns queue overflow, callback decoding, native-window or `WebView2`
    /// errors.
    pub fn poll_events(&mut self) -> io::Result<Vec<WebViewHostEvent>> {
        self.host.poll_events()
    }

    /// Waits for one bounded batch of events for a finite duration.
    ///
    /// # Errors
    /// Returns an invalid duration, queue overflow, callback decoding,
    /// native-window or `WebView2` error.
    pub fn wait_events(&mut self, timeout: Duration) -> io::Result<Vec<WebViewHostEvent>> {
        self.host.wait_events(timeout)
    }

    /// Closes callbacks, the `WebView2` controller and the parent HWND.
    ///
    /// # Errors
    /// Returns the first teardown error after all cleanup steps are attempted.
    pub fn close(&mut self) -> io::Result<()> {
        self.host.close()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webview_config_rejects_non_packaged_entry() {
        let error = WebViewConfig::new("https://example.com/index.html")
            .expect_err("desktop surface requires a packaged file URI");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn webview_config_accepts_a_bounded_packaged_entry() {
        let config =
            WebViewConfig::new("file:///C:/Metis/app/index.html").expect("bounded packaged URI");
        assert_eq!(config.start_uri(), "file:///C:/Metis/app/index.html");
    }

    #[test]
    #[ignore = "requires an installed WebView2 runtime and a visible Windows host"]
    fn installed_runtime_loads_packaged_page_and_closes_surface() {
        let root = std::env::temp_dir().join(format!("metis-webview-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("temporary package directory");
        let entry = root.join("index.html");
        std::fs::write(
            &entry,
            r"<!doctype html><script>window.chrome.webview.postMessage({ready:true});</script>",
        )
        .expect("temporary packaged entry");
        let path = entry
            .canonicalize()
            .expect("canonical package entry")
            .to_string_lossy()
            .replace('\\', "/");
        let path = path.strip_prefix("//?/").unwrap_or(&path);
        let uri = format!("file:///{path}");
        let window = WindowConfig::with_visibility(
            "Metis WebView2 adapter",
            640,
            480,
            WindowVisibility::Hidden,
        )
        .expect("bounded window configuration");
        let config = WebViewConfig::new(uri).expect("bounded WebView configuration");
        let mut surface = WebViewSurface::new(&window, config).expect("installed WebView2 host");
        let mut events = surface.poll_events().expect("initial WebView2 events");
        if !events.iter().any(|event| {
            matches!(
                event,
                WebViewHostEvent::WebView(WebViewEvent::NavigationCompleted { success: true, .. })
            )
        }) || !events.iter().any(|event| {
            matches!(
                event,
                WebViewHostEvent::WebView(WebViewEvent::Message { json, .. })
                    if json.contains("\"ready\"")
            )
        }) {
            events.extend(
                surface
                    .wait_events(Duration::from_secs(1))
                    .expect("WebView2 events"),
            );
        }
        assert!(events.iter().any(|event| matches!(
            event,
            WebViewHostEvent::WebView(WebViewEvent::NavigationCompleted { success: true, .. })
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            WebViewHostEvent::WebView(WebViewEvent::Message { json, .. })
                if json.contains("\"ready\"")
        )));
        assert_eq!(
            surface
                .navigate("https://example.test/blocked")
                .expect_err("external navigation must be denied")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        let denied = surface.poll_events().expect("denied navigation event");
        assert!(denied.iter().any(|event| matches!(
            event,
            WebViewHostEvent::WebView(WebViewEvent::NavigationStarting { allowed: false, .. })
        )));
        surface.close().expect("WebView2 close");
        assert!(surface.is_closed());
        std::fs::remove_dir_all(root).expect("temporary package cleanup");
    }
}
