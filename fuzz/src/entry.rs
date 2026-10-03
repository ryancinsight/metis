//! One entry point per `metis-cli` parser, for the fuzz targets.

use crate::{archive, svg};

/// Whether the packaging boundary admits `bytes` as an SVG resource.
#[must_use]
pub fn admits_svg(bytes: &[u8]) -> bool {
    svg::validate(bytes).is_ok()
}

/// Parses `bytes` as a Linux package archive and returns the number of
/// admitted files, or `None` when the archive is rejected.
///
/// # Panics
/// Panics when an admitted entry violates the archive contract: a path outside
/// the `usr/` prefix or a path the relative-path rule rejects.
#[must_use]
pub fn archive_file_count(bytes: &[u8]) -> Option<usize> {
    let entries = archive::parse(bytes).ok()?;
    for entry in &entries {
        assert!(
            entry.path.starts_with("usr/"),
            "an admitted archive path must lie under usr/"
        );
        assert!(
            archive::safe_relative(&entry.path).is_ok(),
            "an admitted archive path must satisfy the relative-path rule"
        );
        std::hint::black_box((entry.mode, entry.bytes));
    }
    Some(entries.len())
}
