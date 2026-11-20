use super::{GlyphWeight, TextSize, TextStyle, draw_text};
use crate::framebuffer::{Color, Framebuffer, Rect};

fn text_style(pixels: u32, weight: GlyphWeight) -> TextStyle {
    TextStyle::new(
        Color::BLACK,
        TextSize::new(f64::from(pixels)).expect("positive test size is valid"),
    )
    .with_weight(weight)
}

fn expected_centered_top(bounds: Rect, pixels: u32) -> i32 {
    // Both embedded faces declare a 1,240-unit line box per 1,000-unit em.
    // Integer ceiling keeps this oracle independent of the production float
    // conversion and centering implementation.
    let numerator = 1_240_u64 * u64::from(pixels);
    let line_height = i64::try_from(numerator.div_ceil(1_000)).expect("test height fits");
    let offset = (i64::from(bounds.height) - line_height).div_euclid(2);
    i32::try_from(i64::from(bounds.y) + offset).expect("test coordinate fits")
}

#[test]
fn line_boxes_center_in_control_bounds() {
    let cases = [
        (10, Rect::new(3, 40, 80, 30)),
        (10, Rect::new(3, 40, 80, 31)),
        (13, Rect::new(3, 40, 80, 30)),
        (13, Rect::new(3, 40, 80, 32)),
        (25, Rect::new(3, 40, 80, 30)),
        (26, Rect::new(3, -20, 80, 30)),
    ];
    for weight in [GlyphWeight::Regular, GlyphWeight::Bold] {
        for (pixels, bounds) in cases {
            assert_eq!(
                text_style(pixels, weight).centered_line_box_top(bounds),
                Ok(expected_centered_top(bounds, pixels)),
                "{weight:?}, {pixels}px, {bounds:?}"
            );
        }
    }
}

#[test]
fn line_box_placement_rejects_invalid_bounds_and_overflow() {
    let style = text_style(13, GlyphWeight::Regular);
    for bounds in [
        Rect::new(0, 0, 0, 30),
        Rect::new(0, 0, -1, 30),
        Rect::new(0, 0, 80, 0),
        Rect::new(0, 0, 80, -1),
    ] {
        let error = style
            .centered_line_box_top(bounds)
            .expect_err("nonpositive bounds must fail");
        assert_eq!(error.code, metis_core::error::ErrorCode::LayoutOverflow);
        assert_eq!(error.trace_id, "REQ-METIS-UI-004");
        assert_eq!(error.message, "Text control bounds must be positive");
    }
    for (style, bounds) in [
        (style, Rect::new(0, i32::MAX, 80, 30)),
        (
            text_style(1_024, GlyphWeight::Bold),
            Rect::new(0, i32::MIN, 80, 1),
        ),
    ] {
        let error = style
            .centered_line_box_top(bounds)
            .expect_err("coordinate overflow must fail");
        assert_eq!(error.code, metis_core::error::ErrorCode::LayoutOverflow);
        assert_eq!(error.trace_id, "REQ-METIS-UI-004");
        assert_eq!(
            error.message,
            "Centered text coordinate exceeds layout range"
        );
    }
}

#[test]
fn centered_control_text_ink_stays_inside_bounds() {
    for (text, weight) in [("100", GlyphWeight::Regular), ("mm", GlyphWeight::Bold)] {
        for height in [30, 32] {
            let bounds = Rect::new(8, 20, 80, height);
            let style = text_style(13, weight);
            let y = style
                .centered_line_box_top(bounds)
                .expect("control bounds are representable");
            let mut fb = Framebuffer::new(100, 70).expect("test surface");
            fb.clear(Color::WHITE);
            draw_text(&mut fb, bounds.x + 4, y, text, style);

            let mut ink_rows = Vec::new();
            let surface_height = i32::try_from(fb.height()).expect("test surface height fits");
            let surface_width = i32::try_from(fb.width()).expect("test surface width fits");
            for row in 0..surface_height {
                for column in 0..surface_width {
                    if fb.get_pixel(column, row) != Color::WHITE {
                        ink_rows.push(row);
                    }
                }
            }
            assert!(!ink_rows.is_empty(), "{text:?}, {weight:?}, {height}px");
            assert!(
                ink_rows
                    .iter()
                    .all(|row| (bounds.y..bounds.y + bounds.height).contains(row)),
                "{text:?}, {weight:?}, {height}px: {ink_rows:?}"
            );
        }
    }
}
