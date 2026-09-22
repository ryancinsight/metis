use super::{MAX_PACKAGE_ATTEMPTS, assets};
use crate::invocation::WebViewTheme;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub(super) struct Package {
    pub(super) root: PathBuf,
}

#[derive(Clone, Copy)]
pub(super) enum Page {
    Form,
    PermissionProbe,
}

impl Page {
    pub(super) fn assets(self) -> (&'static str, &'static str) {
        match self {
            Self::Form => (assets::INDEX_HTML, assets::APP_JS),
            Self::PermissionProbe => (
                assets::PERMISSION_PROBE_INDEX_HTML,
                assets::PERMISSION_PROBE_APP_JS,
            ),
        }
    }
}

impl Package {
    pub(super) fn create(page: Page, initial_theme: Option<WebViewTheme>) -> io::Result<Self> {
        let base = std::env::temp_dir();
        let process = std::process::id();
        let (index, script) = page.assets();
        let index = initial_theme.map_or_else(
            || index.to_owned(),
            |theme| {
                index.replace(
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
                        fs::write(root.join("index.html"), index.as_bytes())?;
                        fs::write(root.join("styles.css"), assets::STYLES_CSS)?;
                        fs::write(root.join("app.js"), script)?;
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

    pub(super) fn entry_uri(&self) -> io::Result<String> {
        let entry = self.root.join("index.html").canonicalize()?;
        file_uri(&entry)
    }

    pub(super) fn cleanup(self) -> io::Result<()> {
        fs::remove_dir_all(self.root)
    }
}

pub(super) fn file_uri(path: &Path) -> io::Result<String> {
    let text = path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "WebView2 package path is not valid Unicode",
        )
    })?;
    let normalized = text.replace('\\', "/");
    let normalized = normalized
        .strip_prefix("//?/")
        .map_or(normalized.as_str(), |path| path);
    if normalized.starts_with('/') || normalized.starts_with("UNC/") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "WebView2 package path must use a local drive",
        ));
    }
    let mut uri = String::from("file:///");
    for byte in normalized.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/' | b':') {
            uri.push(char::from(byte));
        } else {
            uri.push('%');
            uri.push(char::from(b"0123456789ABCDEF"[(byte >> 4) as usize]));
            uri.push(char::from(b"0123456789ABCDEF"[(byte & 0x0f) as usize]));
        }
    }
    Ok(uri)
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
