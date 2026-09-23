//! Typeface parsing, outlines and text runs against an independent oracle.
//!
//! Reference values come from fontTools 4.61.1 reading the same files:
//! `getBestCmap` for glyph ids, `hmtx` for advances, and `AreaPen`, which
//! integrates each quadratic analytically, for outline areas.

use super::glyf::Transform;
use super::raster::{Canvas, Outline};
use super::*;
use crate::framebuffer::{Color, Framebuffer};

const REGULAR_BYTES: &[u8] = include_bytes!("../fonts/AtkinsonHyperlegible-Regular.ttf");

/// Character, glyph id, advance and absolute outline area in font units.
type Reference = (char, u16, u16, f64);

const REGULAR_REFERENCE: [Reference; 9] = [
    ('A', 15, 626, 143_318.5),
    ('a', 41, 526, 112_149.416_7),
    ('g', 47, 560, 150_106.708_3),
    ('O', 29, 727, 161_815.583_3),
    ('0', 5, 648, 184_858.666_7),
    ('é', 209, 539, 125_524.916_7),
    ('?', 110, 520, 83_622.291_7),
    ('%', 124, 927, 193_647.583_3),
    (' ', 3, 280, 0.0),
];

const BOLD_REFERENCE: [Reference; 9] = [
    ('A', 15, 690, 228_422.0),
    ('a', 41, 553, 169_444.666_7),
    ('g', 47, 597, 239_061.166_7),
    ('O', 29, 764, 261_389.833_3),
    ('0', 5, 652, 260_817.583_3),
    ('é', 209, 566, 187_719.333_3),
    ('?', 110, 614, 146_258.416_7),
    ('%', 124, 977, 289_735.0),
    (' ', 3, 320, 0.0),
];

#[test]
fn embedded_faces_report_their_metrics() {
    for face in [regular(), bold()] {
        assert_eq!(face.units_per_em(), 1000);
        assert_eq!(face.ascender(), 950);
        // Ascender 950, descender -290, no line gap.
        assert_eq!(face.line_height(), 1240);
        assert_eq!(face.glyph_count, 369);
    }
}

#[test]
fn characters_map_to_the_reference_glyphs_and_advances() {
    for (face, reference) in [(regular(), REGULAR_REFERENCE), (bold(), BOLD_REFERENCE)] {
        for (character, glyph, advance, _) in reference {
            let found = face.glyph(character);
            assert_eq!(found.index(), glyph, "{character:?}");
            assert_eq!(face.advance(found), advance, "{character:?}");
        }
        // A character the face lacks maps to the missing glyph.
        assert_eq!(face.glyph('\u{4E00}'), GlyphId::NOTDEF);
        assert_eq!(face.glyph('\u{1F600}'), GlyphId::NOTDEF);
    }
}

#[test]
fn every_glyph_of_the_embedded_faces_decodes() {
    for face in [regular(), bold()] {
        let mut outline = Outline::default();
        for index in 0..face.glyph_count {
            outline.clear();
            face.outline(
                GlyphId(index),
                &Transform::device(0.05, 3.0, 60.0),
                &mut outline,
            )
            .unwrap_or_else(|error| panic!("glyph {index}: {error}"));
        }
    }
}

/// Rasterizes one glyph and returns its coverage total and flattened
/// perimeter.
fn rasterized_area(
    face: &Typeface<'_>,
    glyph: GlyphId,
    size: f64,
    origin: (f64, f64),
) -> (f64, f64) {
    let scale = size / f64::from(face.units_per_em());
    let mut outline = Outline::default();
    face.outline(
        glyph,
        &Transform::device(scale, origin.0, origin.1),
        &mut outline,
    )
    .expect("reference glyph decodes");
    let Some(bounds) = outline.bounds() else {
        return (0.0, 0.0);
    };
    let mut canvas = Canvas::default();
    outline.rasterize(bounds, &mut canvas);
    (canvas.coverage.iter().sum(), outline.perimeter())
}

#[test]
fn glyph_coverage_matches_the_analytic_outline_area() {
    for (face, reference) in [(regular(), REGULAR_REFERENCE), (bold(), BOLD_REFERENCE)] {
        for size in [11.0, 16.0, 37.5] {
            for origin in [(0.0, 40.0), (0.37, 40.61), (5.5, 50.25)] {
                for (character, glyph, _, area) in reference {
                    let scale = size / 1000.0;
                    let expected = area * scale * scale;
                    let (total, perimeter) = rasterized_area(face, GlyphId(glyph), size, origin);
                    // Rasterized area is exact for the flattened outline;
                    // chords sit within the flattening tolerance of the curve,
                    // so the area moves by at most tolerance times perimeter.
                    // The reference areas are rounded to 1e-4 units squared.
                    let bound = perimeter / 256.0 + 1e-4 * scale * scale + 1e-9;
                    assert!(
                        (total - expected).abs() <= bound,
                        "{character:?} at {size}px: {total} against {expected} (bound {bound})"
                    );
                }
            }
        }
    }
}

#[test]
fn truncated_fonts_fail_with_typed_errors() {
    for length in (0..REGULAR_BYTES.len()).step_by(97).chain(0..64) {
        let truncated = &REGULAR_BYTES[..length];
        // Parsing may succeed when the cut falls in a table read lazily; any
        // later read must still fail cleanly.
        if let Ok(face) = Typeface::parse(truncated) {
            let mut outline = Outline::default();
            for index in 0..face.glyph_count {
                outline.clear();
                let _decoded = face.outline(
                    GlyphId(index),
                    &Transform::device(0.016, 0.0, 16.0),
                    &mut outline,
                );
            }
            for character in ['A', 'é', '\u{FFFF}'] {
                let _ = face.advance(face.glyph(character));
            }
        }
    }
    assert!(matches!(
        Typeface::parse(&REGULAR_BYTES[..3]),
        Err(TypefaceError::Truncated { .. })
    ));
    assert!(matches!(
        Typeface::parse(b"OTTO\0\0\0\0\0\0\0\0"),
        Err(TypefaceError::Invalid {
            field: "sfntVersion",
            ..
        })
    ));
}

#[test]
fn mutated_fonts_never_panic() {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut bytes = REGULAR_BYTES.to_vec();
    let mut outline = Outline::default();
    for _ in 0..300 {
        bytes.copy_from_slice(REGULAR_BYTES);
        for _ in 0..8 {
            let at =
                usize::try_from(next() % u64::try_from(bytes.len()).expect("fits")).expect("fits");
            bytes[at] = u8::try_from(next() & 0xff).expect("masked");
        }
        let Ok(face) = Typeface::parse(&bytes) else {
            continue;
        };
        for index in (0..face.glyph_count).step_by(7) {
            outline.clear();
            let _decoded = face.outline(
                GlyphId(index),
                &Transform::device(0.016, 0.0, 16.0),
                &mut outline,
            );
        }
        for character in ['A', 'z', 'é', '%'] {
            let _ = face.advance(face.glyph(character));
        }
    }
}

#[test]
fn text_advance_is_the_sum_of_glyph_advances() {
    let size = TextSize::new(14.0).expect("valid size");
    let style = TextStyle::new(Color::BLACK, size);
    // A, a, g and a space: 626 + 526 + 560 + 280 units at 14 px per 1000.
    assert!((style.advance("Aag ") - 1992.0 * 0.014).abs() < 1e-12);
    assert!((style.advance("A\na") - 1152.0 * 0.014).abs() < 1e-12);
    let bold = style.with_weight(GlyphWeight::Bold);
    assert!((bold.advance("Aag ") - (690.0 + 553.0 + 597.0 + 320.0) * 0.014).abs() < 1e-12);
    assert!((style.line_height() - 1240.0 * 0.014).abs() < 1e-12);
}

#[test]
fn text_clipped_by_the_surface_paints_only_its_visible_part() {
    let size = TextSize::new(18.0).expect("valid size");
    let style = TextStyle::new(Color::BLACK, size);
    let mut whole = Framebuffer::new(120, 40).expect("surface");
    whole.clear(Color::WHITE);
    draw_text(&mut whole, 5, 5, "Clipped", style);
    // The same run started off the top-left corner paints the shifted part.
    let mut shifted = Framebuffer::new(120, 40).expect("surface");
    shifted.clear(Color::WHITE);
    draw_text(&mut shifted, -15, -6, "Clipped", style);
    for y in 0..29 {
        for x in 0..100 {
            assert_eq!(
                shifted.get_pixel(x, y),
                whole.get_pixel(x + 20, y + 11),
                "({x}, {y})"
            );
        }
    }
    // Far off-surface runs and transparent color paint nothing.
    let mut untouched = Framebuffer::new(40, 20).expect("surface");
    untouched.clear(Color::WHITE);
    let reference = untouched.clone();
    draw_text(&mut untouched, i32::MIN, i32::MIN, "x", style);
    draw_text(&mut untouched, 5000, 5, "x", style);
    draw_text(
        &mut untouched,
        2,
        2,
        "x",
        TextStyle::new(Color::rgba(0, 0, 0, 0), size),
    );
    assert_eq!(untouched.pixels(), reference.pixels());
}

#[test]
fn text_sizes_are_validated() {
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY, TextSize::MAX + 1.0] {
        assert!(TextSize::new(invalid).is_none(), "{invalid}");
    }
    assert_eq!(
        TextSize::new(TextSize::MAX).map(TextSize::pixels),
        Some(TextSize::MAX)
    );
}

/// Darkness of black text on white: the alpha each pixel composited, in
/// units of full coverage.
fn darkness(fb: &Framebuffer) -> f64 {
    fb.pixels()
        .iter()
        .map(|packed| f64::from(255 - packed.to_be_bytes()[1]) / 255.0)
        .sum()
}

#[test]
fn inked_darkness_equals_the_reference_outline_areas() {
    // Spaces keep neighbouring glyphs' ink apart, so no pixel composites two
    // glyphs and the darkness is the sum of their coverage.
    let text = "A a g 0 ? %";
    let size = 20.0;
    let scale = size / 1000.0;
    for (weight, reference) in [
        (GlyphWeight::Regular, REGULAR_REFERENCE),
        (GlyphWeight::Bold, BOLD_REFERENCE),
    ] {
        let style = TextStyle::new(Color::BLACK, TextSize::new(size).expect("valid size"))
            .with_weight(weight);
        let mut fb = Framebuffer::new(200, 40).expect("surface");
        fb.clear(Color::WHITE);
        draw_text(&mut fb, 6, 5, text, style);
        let face = weight.face();
        let mut pen = 6.0;
        let (mut expected, mut perimeter) = (0.0, 0.0);
        for character in text.chars() {
            let glyph = face.glyph(character);
            let area = reference
                .iter()
                .find(|entry| entry.0 == character)
                .map_or(0.0, |entry| entry.3);
            expected += area * scale * scale;
            perimeter += rasterized_area(face, glyph, size, (pen, 24.0)).1;
            pen += f64::from(face.advance(glyph)) * scale;
        }
        let inked = fb
            .pixels()
            .iter()
            .filter(|packed| packed.to_be_bytes()[1] != 255)
            .count();
        // Flattening moves the area by at most tolerance times perimeter, and
        // rounding each composited alpha to a byte moves a pixel by at most
        // half a level.
        let bound = perimeter / 256.0 + f64::from(u32::try_from(inked).expect("small")) / 510.0;
        let measured = darkness(&fb);
        assert!(
            (measured - expected).abs() <= bound,
            "{weight:?}: darkness {measured} against area {expected} (bound {bound})"
        );
    }
}

#[test]
fn ink_stays_within_the_glyph_bounds_it_draws() {
    // 'j' overhangs left of its pen, 'q' right of its advance, and the
    // combining acute has no advance at all; ink may leave the advance box,
    // as CSS text does, but never the glyphs' declared bounds.
    let text = "j a\u{301} \u{c5}jq";
    for weight in [GlyphWeight::Regular, GlyphWeight::Bold] {
        let size = 24.0;
        let style = TextStyle::new(Color::BLACK, TextSize::new(size).expect("valid size"))
            .with_weight(weight);
        let face = weight.face();
        let scale = size / 1000.0;
        let (x, y) = (40, 10);
        let baseline = f64::from(face.ascender()).mul_add(scale, f64::from(y));
        let mut fb = Framebuffer::new(240, 60).expect("surface");
        fb.clear(Color::WHITE);
        draw_text(&mut fb, x, y, text, style);
        let mut pen = f64::from(x);
        let mut allowed = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for character in text.chars() {
            let glyph = face.glyph(character);
            if let Some([x_min, y_min, x_max, y_max]) = face.glyph_bounds(glyph).expect("decodes") {
                allowed = [
                    allowed[0].min(f64::from(x_min).mul_add(scale, pen)),
                    allowed[1].min(f64::from(y_max).mul_add(-scale, baseline)),
                    allowed[2].max(f64::from(x_max).mul_add(scale, pen)),
                    allowed[3].max(f64::from(y_min).mul_add(-scale, baseline)),
                ];
            }
            pen += f64::from(face.advance(glyph)) * scale;
        }
        let mut ink = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
        for row in 0..60 {
            for column in 0..240 {
                if fb.get_pixel(column, row) != Color::WHITE {
                    ink = [
                        ink[0].min(column),
                        ink[1].min(row),
                        ink[2].max(column),
                        ink[3].max(row),
                    ];
                }
            }
        }
        // A pixel is inked when its square meets the outline, so inked
        // columns lie in [floor(left), ceil(right)).
        assert!(
            f64::from(ink[0]) >= allowed[0].floor(),
            "{weight:?} left {ink:?} {allowed:?}"
        );
        assert!(
            f64::from(ink[1]) >= allowed[1].floor(),
            "{weight:?} top {ink:?} {allowed:?}"
        );
        assert!(
            f64::from(ink[2]) < allowed[2].ceil(),
            "{weight:?} right {ink:?} {allowed:?}"
        );
        assert!(
            f64::from(ink[3]) < allowed[3].ceil(),
            "{weight:?} bottom {ink:?} {allowed:?}"
        );
        // The overhangs are real: ink reaches left of the pen and past the
        // run's advance.
        assert!(ink[0] < x, "{weight:?}: no left overhang in {ink:?}");
        assert!(
            f64::from(ink[2]) >= f64::from(x) + style.advance(text),
            "{weight:?}: {ink:?}"
        );
        // The face's bounding box lies inside its line box, so every run's
        // ink does too.
        let line_height = style.line_height();
        assert!(ink[1] >= y && f64::from(ink[3]) < f64::from(y) + line_height);
    }
}

#[test]
fn glyphs_overhanging_the_right_edge_still_paint() {
    let style = TextStyle::new(Color::BLACK, TextSize::new(200.0).expect("valid size"));
    // At 200 px 'j' reaches 11.6 px left of its pen, so a pen exactly at the
    // right edge of a 100-pixel surface still paints visible ink.
    let mut narrow = Framebuffer::new(100, 260).expect("surface");
    narrow.clear(Color::WHITE);
    draw_text(&mut narrow, 100, 0, "j", style);
    let mut wide = Framebuffer::new(200, 260).expect("surface");
    wide.clear(Color::WHITE);
    draw_text(&mut wide, 100, 0, "j", style);
    let mut visible = 0;
    for row in 0..260 {
        for column in 0..100 {
            assert_eq!(narrow.get_pixel(column, row), wide.get_pixel(column, row));
            visible += usize::from(narrow.get_pixel(column, row) != Color::WHITE);
        }
    }
    assert!(visible > 0, "the overhang painted nothing");
}
