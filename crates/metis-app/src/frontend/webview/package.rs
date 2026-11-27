use super::{MAX_PACKAGE_ATTEMPTS, assets};
use crate::invocation::WebViewTheme;
use metis_platform::native::{MAX_WEBVIEW_WAIT_MILLISECONDS, WebViewConfig};
use std::{fs, io, path::PathBuf};

pub(super) struct Package {
    pub(super) root: PathBuf,
}

#[derive(Clone, Copy)]
pub(super) enum Page {
    Form,
    PermissionProbe,
}

impl Page {
    pub(super) fn assets(self) -> &'static str {
        match self {
            Self::Form => assets::INDEX_HTML,
            Self::PermissionProbe => assets::PERMISSION_PROBE_INDEX_HTML,
        }
    }
}

impl Package {
    pub(super) fn create(page: Page, initial_theme: Option<WebViewTheme>) -> io::Result<Self> {
        let base = std::env::temp_dir();
        let process = std::process::id();
        let index = initial_theme.map_or_else(
            || page.assets().to_owned(),
            |theme| {
                page.assets().replace(
                    "data-metis-theme=\"system\"",
                    &format!("data-metis-theme=\"{}\"", theme.query_value()),
                )
            },
        );
        for attempt in 0..MAX_PACKAGE_ATTEMPTS {
            let root = base.join(format!("metis-webview-{process}-{attempt}"));
            match fs::create_dir(&root) {
                Ok(()) => {
                    let result = (|| {
                        let files = root.join("assets");
                        fs::create_dir(&files)?;
                        fs::create_dir(root.join("profile"))?;
                        fs::write(files.join("index.html"), &index)?;
                        fs::write(files.join("styles.css"), assets::STYLES_CSS)?;
                        fs::write(files.join("page.js"), assets::PAGE_JS)?;
                        fs::write(files.join("page_bg.wasm"), assets::PAGE_WASM)?;
                        Ok(())
                    })();
                    return match result {
                        Ok(()) => Ok(Self { root }),
                        Err(error) => Err(cleanup_error(root, error)),
                    };
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "bounded WebView2 package names are exhausted",
        ))
    }

    pub(super) fn config(&self) -> io::Result<WebViewConfig> {
        WebViewConfig::folder(
            "metis.invalid",
            self.root.join("assets"),
            "index.html",
            std::time::Duration::from_millis(u64::from(MAX_WEBVIEW_WAIT_MILLISECONDS)),
        )?
        .with_user_data_folder(self.root.join("profile"))
    }

    pub(super) fn cleanup(self) -> io::Result<()> {
        fs::remove_dir_all(self.root)
    }
}

pub(super) fn package_failure(package: Package, error: io::Error) -> io::Error {
    cleanup_error(package.root, error)
}

pub(super) fn cleanup_error(path: PathBuf, error: io::Error) -> io::Error {
    match fs::remove_dir_all(path) {
        Ok(()) => error,
        Err(cleanup) => io::Error::new(
            error.kind(),
            format!("{error}; WebView2 package cleanup failed: {cleanup}"),
        ),
    }
}
