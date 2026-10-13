//! Surface ownership ends before caller-owned package/profile cleanup.

use super::package::Package;
use metis_core::error::Result;
use metis_platform::native::WebViewSurface;
use std::{fs, io, path::Path};

// Condition 1: this error-reporting boundary retains the concrete runtime
// failure across OS, protocol, native close and filesystem operations.
pub(super) fn finish(
    result: Result<Option<Vec<u8>>>,
    mut surface: WebViewSurface,
    package: Package,
    capture: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = package.root.clone();
    let profile = package.profile.clone();
    let mut errors = Vec::new();
    let captured = match result {
        Ok(bytes) => bytes,
        Err(error) => {
            errors.push(error.to_string());
            None
        }
    };
    if let Err(error) = surface.close() {
        errors.push(format!("WebView2 close failed: {error}"));
    }
    // Fresh-profile close observes the actual browser exit. Drop releases all
    // remaining native/interface owners before attempting directory deletion.
    drop(surface);
    if let Err(error) = package.cleanup() {
        errors.push(format!("WebView2 package cleanup failed: {error}"));
    }
    if !errors.is_empty() {
        return Err(errors.join("; ").into());
    }
    if root.try_exists()? || profile.try_exists()? {
        return Err(io::Error::other("Owned package or profile remains after cleanup").into());
    }
    tracing::info!(
        event = "page_package_cleaned",
        root = %root.display(),
        profile = %profile.display(),
        "Actual native owners released and profile/package removed"
    );
    match (capture, captured) {
        (Some(output), Some(bytes)) => {
            fs::write(output, &bytes)?;
            tracing::info!(event = "page_capture_published", path = %output.display(), bytes = bytes.len(), "Ready frame published after actual stop, destruction and cleanup");
            eprintln!(
                "webview_preview_path={} webview_preview_bytes={}",
                output.display(),
                bytes.len()
            );
            Ok(())
        }
        (None, None) => Ok(()),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Native capture ended without an admitted frame or destination",
        )
        .into()),
    }
}
