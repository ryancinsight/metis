//! Existing display/input scale selection at the native host boundary.

/// Pins the `WebView2` browser's device scale factor to the display scale.
///
/// Chromium maps physical pointer input to CSS coordinates with its device
/// scale factor while the host rasterizes the page at the monitor scale. On
/// a display scaled above 96 dpi the two mappings disagree inside a
/// process-unaware host, so a trusted click on a rendered element hit-tests
/// away from the page's layout (measured 2026-10-12: the proportionally
/// scaled submit point hit-tested the packaged page at `dpi/96` times the
/// client point). `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` carries
/// `--force-device-scale-factor`, which pins the browser's input and
/// rasterization mapping to one scale and restores a 1:1 input mapping over
/// the rendered layout. `WebView2` reads the variable only when it creates
/// the browser environment, so it must be in place before
/// [`WebViewSurface::new`].
#[cfg(windows)]
#[expect(
    unsafe_code,
    reason = "Minimal user32 DPI probe and loader env boundary"
)]
pub(super) fn force_webview_input_scale() {
    use std::ffi::c_void;

    // Windows pseudo-handle for DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2.
    const PER_MONITOR_AWARE_V2: *mut c_void = std::ptr::without_provenance_mut(usize::MAX - 3);
    const VARIABLE: &str = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";

    #[link(name = "user32")]
    unsafe extern "system" {
        fn SetThreadDpiAwarenessContext(context: *mut c_void) -> *mut c_void;
        fn GetDpiForSystem() -> u32;
    }

    // SAFETY: the requested context is an operating-system constant and the
    // call affects only the current thread; the prior context is restored on
    // every return path below.
    let previous = unsafe { SetThreadDpiAwarenessContext(PER_MONITOR_AWARE_V2) };
    if previous.is_null() {
        return;
    }
    // SAFETY: GetDpiForSystem only reads the thread context set above.
    let dpi = unsafe { GetDpiForSystem() };
    // SAFETY: the context handle returned above belongs to this thread and
    // remains valid for restoration here.
    unsafe { SetThreadDpiAwarenessContext(previous) };

    let scale = if dpi == 0 { 1.0 } else { f64::from(dpi) / 96.0 };
    if (scale - 1.0).abs() <= f64::EPSILON {
        return;
    }
    let argument = format!("--force-device-scale-factor={scale:.4}");
    let existing = std::env::var(VARIABLE).unwrap_or_default();
    let joined = if existing.is_empty() {
        argument
    } else {
        format!("{existing} {argument}")
    };
    // SAFETY: this runs on the single window-creating thread before this
    // process spawns any other reader of the environment, so the write
    // cannot race a concurrent environment read.
    unsafe { std::env::set_var(VARIABLE, joined) };
}
