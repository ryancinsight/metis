//! LibFuzzer target for `Typeface::parse`, the TrueType reader for
//! application-supplied fonts, including the lazily read `cmap` and `hmtx`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use metis_platform::typeface::Typeface;

/// Characters that reach each `cmap` range and both planes the reader supports.
const PROBES: [char; 4] = ['\u{3b1}', '\u{2603}', '\u{1f600}', '\u{10ffff}'];

fuzz_target!(|data: &[u8]| {
    if let Ok(face) = Typeface::parse(data) {
        // Lookups read the character map and metrics tables that `parse` only
        // bounds-checks, so malformed table bodies surface here.
        for character in ('\0'..='\u{17f}').chain(PROBES) {
            let glyph = face.glyph(character);
            std::hint::black_box(face.advance(glyph));
        }
        std::hint::black_box(face.line_height());
    }
});
