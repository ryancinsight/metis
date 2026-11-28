//! Selects the packaged browser application from explicit page metadata.
use std::io;

#[cfg(target_arch = "wasm32")]
const ENTRY_ATTRIBUTE: &str = "data-metis-entry";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Entry {
    Workbench,
    Http,
}

fn select(marker: Option<&str>) -> io::Result<Option<Entry>> {
    match marker {
        None => Ok(None),
        Some("workbench") => Ok(Some(Entry::Workbench)),
        Some("http") => Ok(Some(Entry::Http)),
        Some(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Browser page has an unsupported Metis entry marker",
        )),
    }
}

fn select_available(marker: io::Result<Option<String>>) -> io::Result<Option<Entry>> {
    match marker {
        Ok(marker) => select(marker.as_deref()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::Unsupported | io::ErrorKind::NotFound
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

#[cfg(target_arch = "wasm32")]
fn mount() -> io::Result<()> {
    use moirai_pal::wasm::WebDocument;

    let marker = WebDocument::current()
        .and_then(|document| document.body())
        .map(|body| body.attribute(ENTRY_ATTRIBUTE));
    match select_available(marker)? {
        Some(Entry::Workbench) => crate::metis_browser_start(),
        Some(Entry::Http) => crate::metis_http_start(),
        None => {}
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn report(error: &io::Error) {
    use moirai_pal::wasm::WebDocument;

    if let Ok(document) = WebDocument::current()
        && let Some(status) = document
            .get_element_by_id("metis-status")
            .or_else(|| document.get_element_by_id("metis-app"))
    {
        status.set_text(&format!("Browser host error: {error}"));
    }
}

/// Starts the page-selected application for this WebAssembly module instance.
///
/// A document without `data-metis-entry` retains the import-only behavior of
/// the library: initialization installs no application state or listeners.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start, private)]
fn initialize() {
    if let Err(error) = mount() {
        report(&error);
    }
}

#[cfg(test)]
mod tests {
    use super::{Entry, select, select_available};
    use std::io;

    #[test]
    fn known_markers_select_their_application() {
        assert!(matches!(
            select(Some("workbench")),
            Ok(Some(Entry::Workbench))
        ));
        assert!(matches!(select(Some("http")), Ok(Some(Entry::Http))));
    }

    #[test]
    fn absent_marker_preserves_import_only_initialization() {
        assert!(matches!(select(None), Ok(None)));
    }

    #[test]
    fn unavailable_document_or_body_preserves_import_only_initialization() {
        for kind in [io::ErrorKind::Unsupported, io::ErrorKind::NotFound] {
            let marker = Err(io::Error::new(kind, "unavailable"));
            assert!(matches!(select_available(marker), Ok(None)));
        }
    }

    #[test]
    fn other_document_errors_remain_visible() {
        let marker = Err(io::Error::other("unreadable"));
        let Err(error) = select_available(marker) else {
            panic!("document error was hidden");
        };
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(error.to_string(), "unreadable");
    }

    #[test]
    fn unknown_marker_is_rejected() {
        let Err(error) = select(Some("canvas")) else {
            panic!("unknown marker was admitted");
        };
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            error.to_string(),
            "Browser page has an unsupported Metis entry marker"
        );
    }
}
