//! `metis dev` for a browser application: the built page in a native window.
//!
//! The page is served from its build directory under a reserved `https`
//! host, so ES modules and WebAssembly load as they do from `metis serve`,
//! which a `file:///` page does not allow. With `--watch`, a source change
//! rebuilds the page and reloads the window; a failed compile keeps the
//! previous page and reports the error.
use super::DevMode;
use crate::{Result, manifest::WebApplication};
use std::path::Path;

#[cfg(windows)]
pub(super) fn run(application: &WebApplication, root: &Path, mode: DevMode) -> Result<()> {
    window::run(application, root, mode)
}

#[cfg(not(windows))]
pub(super) fn run(_application: &WebApplication, _root: &Path, _mode: DevMode) -> Result<()> {
    Err(
        "metis dev hosts a browser application in WebView2, which requires Windows; \
         use metis serve"
            .into(),
    )
}

#[cfg(windows)]
mod window {
    use super::super::{DevMode, snapshot, watcher};
    use crate::{Result, build, manifest::WebApplication};
    use metis_platform::native::{
        WebViewConfig, WebViewEvent, WebViewHostEvent, WebViewSurface, WindowConfig, WindowEvent,
        WindowVisibility,
    };
    use std::{path::Path, time::Duration};

    /// Reserved host (RFC 2606 `.example`), so the mapping cannot shadow a
    /// real site.
    const HOST: &str = "app.metis.example";
    const ENTRY: &str = "index.html";
    /// create-tauri-app's window size.
    const WIDTH: u32 = 800;
    const HEIGHT: u32 = 600;
    /// Window-event wait between source-change checks.
    const EVENT_WAIT: Duration = Duration::from_millis(100);
    const HOST_WAIT: Duration = Duration::from_secs(10);

    pub(super) fn run(application: &WebApplication, root: &Path, mode: DevMode) -> Result<()> {
        let served = build::web::application(application, root, None)?;
        let watcher = matches!(mode, DevMode::Watch)
            .then(|| watcher::Watcher::new(root))
            .transpose()?;
        let mut baseline = snapshot(root)?;
        let window = WindowConfig::with_visibility(
            &application.name,
            WIDTH,
            HEIGHT,
            WindowVisibility::Visible,
        )?;
        let config = WebViewConfig::folder(HOST, &served, ENTRY, HOST_WAIT)?;
        let entry = config.start_uri().to_owned();
        let mut surface = WebViewSurface::new(&window, config)?;
        eprintln!("metis dev: {} at {entry}", application.name);
        loop {
            for event in surface.wait_events(EVENT_WAIT)? {
                match event {
                    WebViewHostEvent::Window(WindowEvent::Destroyed) => return Ok(()),
                    WebViewHostEvent::Window(WindowEvent::Resized { width, height }) => {
                        surface.resize(width, height)?;
                    }
                    WebViewHostEvent::WebView(WebViewEvent::NavigationCompleted {
                        success: false,
                        ..
                    }) => eprintln!("metis dev: the page failed to load"),
                    WebViewHostEvent::Window(_) | WebViewHostEvent::WebView(_) => {}
                }
            }
            let Some(watcher) = &watcher else { continue };
            if !watcher.changed()? {
                continue;
            }
            let current = snapshot(root)?;
            if current == baseline {
                continue;
            }
            baseline = current;
            match build::web::application(application, root, None) {
                Ok(_) => {
                    eprintln!("metis dev: rebuilt; reloading");
                    surface.navigate(&entry)?;
                }
                Err(error) => eprintln!("metis dev: rebuild failed; keeping the page: {error}"),
            }
        }
    }
}
