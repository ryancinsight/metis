//! The typefaces embedded in the renderer.
//!
//! Atkinson Hyperlegible, designed by the Braille Institute of America for
//! legibility and distributed under the SIL Open Font License 1.1
//! (`fonts/OFL.txt`). The faces are parsed once on first use.

use std::sync::LazyLock;

use super::Typeface;

static REGULAR: LazyLock<Typeface<'static>> = LazyLock::new(|| {
    Typeface::parse(include_bytes!(
        "../../fonts/AtkinsonHyperlegible-Regular.ttf"
    ))
    .expect("invariant: the embedded regular face parses, as its unit test proves")
});

static BOLD: LazyLock<Typeface<'static>> = LazyLock::new(|| {
    Typeface::parse(include_bytes!("../../fonts/AtkinsonHyperlegible-Bold.ttf"))
        .expect("invariant: the embedded bold face parses, as its unit test proves")
});

/// The embedded regular face.
#[must_use]
pub fn regular() -> &'static Typeface<'static> {
    &REGULAR
}

/// The embedded bold face.
#[must_use]
pub fn bold() -> &'static Typeface<'static> {
    &BOLD
}
