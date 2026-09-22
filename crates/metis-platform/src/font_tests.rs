//! Value-semantic tests for the bitmap glyph table.

use super::*;

/// The replacement bitmap an unsupported character falls through to.
fn replacement() -> [u8; 16] {
    get_glyph_bitmap('\u{1f4a5}')
}

/// The inclusive row band a bitmap actually occupies.
fn band(bitmap: [u8; 16]) -> Option<(usize, usize)> {
    let first = bitmap.iter().position(|row| *row != 0)?;
    let last = bitmap.iter().rposition(|row| *row != 0)?;
    Some((first, last))
}

#[test]
fn every_letter_renders_a_distinct_bitmap_in_each_case() {
    for upper in 'A'..='Z' {
        let lower = upper.to_ascii_lowercase();
        let upper_bitmap = get_glyph_bitmap(upper);
        let lower_bitmap = get_glyph_bitmap(lower);
        assert_ne!(
            upper_bitmap, lower_bitmap,
            "{lower} still renders the {upper} bitmap"
        );
        assert_ne!(
            lower_bitmap,
            replacement(),
            "{lower} falls through to the replacement box"
        );
        assert_ne!(lower_bitmap, [0; 16], "{lower} renders nothing");
    }
}

#[test]
fn lowercase_bands_follow_the_documented_baseline() {
    // Ascenders reach row 2; the x-height band starts at row 4; descenders run
    // past the row 9 baseline. Dotted letters start at their dot, not their stem.
    for letter in ['b', 'd', 'f', 'h', 'k', 'l', 't', 'i', 'j'] {
        let (first, _) = band(get_glyph_bitmap(letter)).expect("ascender is not blank");
        assert_eq!(first, 2, "{letter} does not start at the ascender row");
    }
    for letter in [
        'a', 'c', 'e', 'm', 'n', 'o', 'r', 's', 'u', 'v', 'w', 'x', 'z',
    ] {
        let (first, last) = band(get_glyph_bitmap(letter)).expect("x-height is not blank");
        assert_eq!(first, 4, "{letter} does not start at the x-height row");
        assert_eq!(last, 9, "{letter} does not rest on the baseline");
    }
    for letter in ['g', 'j', 'p', 'q', 'y'] {
        let (_, last) = band(get_glyph_bitmap(letter)).expect("descender is not blank");
        assert!(last > 9, "{letter} has no descender below the baseline");
        assert!(last <= 11, "{letter} descends past the glyph cell");
    }
}

#[test]
fn common_punctuation_no_longer_falls_through_to_the_box() {
    for mark in [
        ',', ';', '!', '?', '\'', '"', '*', '#', '&', '@', '<', '>', '$', '|', '~',
    ] {
        let bitmap = get_glyph_bitmap(mark);
        assert_ne!(bitmap, replacement(), "{mark} renders the replacement box");
        assert_ne!(bitmap, [0; 16], "{mark} renders nothing");
    }
}

#[test]
fn unsupported_characters_still_render_the_replacement_box() {
    for character in ['\u{1f4a5}', '\u{4e2d}', '\u{00e9}'] {
        assert_eq!(
            get_glyph_bitmap(character),
            replacement(),
            "{character} is not mapped to the replacement box"
        );
    }
    assert_eq!(get_glyph_bitmap(' '), [0; 16], "the space is not blank");
}

#[test]
fn bold_thickens_every_inked_glyph_without_leaving_its_cell() {
    let mut thickened = 0;
    for character in ('!'..='~').chain([' ']) {
        let regular = get_glyph_bitmap(character);
        let bold = regular.map(|row| GlyphWeight::Bold.apply(row));
        let regular_ink: u32 = regular.iter().map(|row| row.count_ones()).sum();
        let bold_ink: u32 = bold.iter().map(|row| row.count_ones()).sum();
        assert!(
            bold_ink >= regular_ink,
            "bold {character} lost ink: {bold_ink} < {regular_ink}"
        );
        if regular_ink > 0 {
            assert!(bold_ink > regular_ink, "bold {character} is not thicker");
            thickened += 1;
        }
        // The smear must not reach the leading column, or a bold glyph would
        // close the one-pixel gap at the cell advance.
        for row in bold {
            assert_eq!(
                row & 0x80,
                0,
                "bold {character} paints the leading column of its cell"
            );
        }
    }
    assert!(thickened > 80, "only {thickened} glyphs thickened");
    assert_eq!(GlyphWeight::default(), GlyphWeight::Regular);
    assert_eq!(GlyphWeight::Regular.apply(0x3C), 0x3C);
    assert_eq!(GlyphWeight::Bold.apply(0x3C), 0x3E);
}

#[test]
fn every_glyph_stays_inside_its_cell_width() {
    // The advance is FONT_WIDTH, so a glyph that set no bits would collide with
    // nothing and one that set all eight would touch its neighbour. The table
    // keeps at least the leftmost column clear.
    for character in ('!'..='~').chain([' ']) {
        for row in get_glyph_bitmap(character) {
            assert_eq!(
                row & 0x80,
                0,
                "{character} paints the leftmost column of its cell"
            );
        }
    }
}
