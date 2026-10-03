//! Entry points shared by the Metis fuzz targets.
//!
//! `metis-cli` is a binary crate, so its parsers cannot be linked as a
//! dependency. This library includes the exact source files of those parsers,
//! under the crate-root module names they use, and exposes one entry point per
//! parser, so every target exercises the code that ships rather than a copy.

mod entry;

pub use entry::{admits_svg, archive_file_count};

/// The CLI's boxed-error result, named through `crate::Result` by the
/// included sources.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[path = "../../crates/metis-cli/src/build/install/archive.rs"]
#[expect(
    dead_code,
    reason = "fuzz input calls archive parsing, not file loading"
)]
mod archive;
#[path = "../../crates/metis-cli/src/bounded_read.rs"]
#[expect(
    dead_code,
    reason = "the parsers under test take bytes, not files or readers"
)]
mod bounded_read;
#[path = "../../crates/metis-cli/src/manifest/payload_path.rs"]
mod payload_path;
#[path = "../../crates/metis-cli/src/manifest/svg.rs"]
#[expect(
    dead_code,
    reason = "fuzz input calls byte validation, not file loading"
)]
mod svg;

/// The path rules the included archive sources read through `manifest::`.
mod manifest {
    pub(crate) use crate::payload_path::{PAYLOAD_LIMIT, relative};
}
