//! Value-semantic tests for the bounded style subset.

use super::*;

#[test]
fn empty_and_trailing_declarations_keep_the_default_contract() {
    assert_eq!(
        ComputedStyle::parse("").expect("empty style"),
        ComputedStyle::default()
    );
    assert_eq!(
        ComputedStyle::parse("display: flex;")
            .expect("trailing declaration separator")
            .display,
        Display::Flex
    );
}

#[test]
fn parses_admitted_values() {
    let style = ComputedStyle::parse(
        "display: block; flex-direction: row; gap: 4px; width: 50%; height: 12px; \
         padding: 1px 2px 3px 4px; margin: 5px 6px; border-width: 1px; \
         border-color: #123456; background: #abcdef80; color: #fedcba; font-size: 10px;",
    )
    .expect("admitted style");

    assert_eq!(style.display, Display::Block);
    assert_eq!(style.flex_direction, FlexDirection::Row);
    assert_eq!(style.gap, 4);
    assert_eq!(style.width, Size::Percent(0.5));
    assert_eq!(style.height, Size::Px(12));
    assert_eq!(
        style.padding,
        EdgeValues {
            top: 1,
            right: 2,
            bottom: 3,
            left: 4
        }
    );
    assert_eq!(style.margin, EdgeValues::symmetric(5, 6));
    assert_eq!(style.border_color, Color::rgb(0x12, 0x34, 0x56));
    assert_eq!(
        style.background_color,
        Some(Color::rgba(0xab, 0xcd, 0xef, 0x80))
    );
    assert_eq!(style.justify_content, JustifyContent::FlexStart);
    assert_eq!(style.align_items, AlignItems::Stretch);
    assert_eq!(style.min_width, Size::Auto);
    assert_eq!(style.min_height, Size::Auto);
    assert_eq!(style.border_radius, 0);
    assert_eq!(style.font_weight, FontWeight::Normal);
}

#[test]
fn admits_border_radius_as_a_nonnegative_pixel_length() {
    let style = ComputedStyle::parse("border-radius: 12px").expect("admitted radius");
    assert_eq!(style.border_radius, 12);
    assert_eq!(
        ComputedStyle::parse("border-radius: 0px")
            .expect("zero radius")
            .border_radius,
        0
    );
    // A bare number is the same length grammar the sibling pixel properties
    // accept, so the radius follows `gap` and `border-width` rather than
    // inventing a stricter one.
    assert_eq!(
        ComputedStyle::parse("border-radius: 4")
            .expect("bare pixel length")
            .border_radius,
        4
    );
    for malformed in [
        "border-radius: -2px",
        "border-radius: auto",
        "border-radius: 50%",
    ] {
        let error = ComputedStyle::parse(malformed).expect_err("malformed radius");
        assert_eq!(error.code, ErrorCode::InvalidCssStyle, "{malformed}");
        assert!(error.message.contains("border-radius"), "{malformed}");
    }
}

#[test]
fn admits_the_bounded_font_weight_keywords_and_numerics() {
    for (value, expected) in [
        ("normal", FontWeight::Normal),
        ("400", FontWeight::Normal),
        ("bold", FontWeight::Bold),
        ("700", FontWeight::Bold),
    ] {
        let css = format!("font-weight: {value}");
        assert_eq!(
            ComputedStyle::parse(&css)
                .expect("admitted weight")
                .font_weight,
            expected,
            "{css}"
        );
    }
    // The subset is bounded: a weight the renderer cannot paint is a typed
    // error rather than a silent rounding to the nearest one it can.
    for value in ["500", "lighter", "bolder", "1000", ""] {
        let css = format!("font-weight: {value}");
        let error = ComputedStyle::parse(&css).expect_err("unsupported weight");
        assert_eq!(error.code, ErrorCode::InvalidCssStyle, "{css}");
        assert!(error.message.contains("font-weight"), "{css}");
    }
}

#[test]
fn admits_minimum_sizes_on_the_extent_length_grammar() {
    let style =
        ComputedStyle::parse("min-width: 44px; min-height: 50%").expect("admitted minimum sizes");
    assert_eq!(style.min_width, Size::Px(44));
    assert_eq!(style.min_height, Size::Percent(0.5));
    assert_eq!(
        ComputedStyle::parse("min-width: auto")
            .expect("an explicit auto minimum")
            .min_width,
        Size::Auto
    );
    for malformed in ["min-width: -4px", "min-height: wide", "min-width: 10em"] {
        let error = ComputedStyle::parse(malformed).expect_err("malformed minimum");
        assert_eq!(error.code, ErrorCode::InvalidCssStyle, "{malformed}");
    }
}

#[test]
fn admits_the_bounded_alignment_keywords() {
    let style = ComputedStyle::parse("justify-content: space-between; align-items: center")
        .expect("admitted alignment");
    assert_eq!(style.justify_content, JustifyContent::SpaceBetween);
    assert_eq!(style.align_items, AlignItems::Center);
    for (css, expected) in [
        ("justify-content: flex-start", JustifyContent::FlexStart),
        ("justify-content: center", JustifyContent::Center),
        ("justify-content: flex-end", JustifyContent::FlexEnd),
    ] {
        assert_eq!(
            ComputedStyle::parse(css).expect("keyword").justify_content,
            expected,
            "{css}"
        );
    }
    // The subset is bounded: a keyword the renderer does not distribute is
    // a typed error rather than a silent fall back to the default.
    for css in [
        "justify-content: space-around",
        "justify-content: end",
        "align-items: baseline",
        "align-items: end",
    ] {
        let error = ComputedStyle::parse(css).expect_err("unsupported keyword");
        assert_eq!(error.code, ErrorCode::InvalidCssStyle, "{css}");
    }
}

#[test]
fn rejects_malformed_values_with_one_code() {
    for css in [
        "unknown: value",
        "display",
        "gap:",
        "display: column",
        "gap: -1px",
        "width: NaN%",
        "padding: 1px nope",
        "color: #xyz",
    ] {
        let error = ComputedStyle::parse(css).expect_err("invalid style");
        assert_eq!(error.code, ErrorCode::InvalidCssStyle, "{css}");
    }
}

#[test]
fn box_shadow_parses_offsets_blur_and_color_in_either_order() {
    let elevation = Shadow {
        offset_x: 0,
        offset_y: 4,
        blur: 12,
        color: Color::rgba(0x0f, 0x17, 0x2a, 0x28),
    };
    for css in [
        "box-shadow: 0 4px 12px #0f172a28",
        "box-shadow: #0f172a28 0px 4px 12px",
        "BOX-SHADOW:  0px   4px 12px   #0F172A28",
    ] {
        assert_eq!(
            ComputedStyle::parse(css).expect("shadow").box_shadow,
            Some(elevation),
            "{css}"
        );
    }
    // Offsets are signed and the blur defaults to a hard edge.
    assert_eq!(
        ComputedStyle::parse("box-shadow: -2px -3px #000")
            .expect("hard")
            .box_shadow,
        Some(Shadow {
            offset_x: -2,
            offset_y: -3,
            blur: 0,
            color: Color::BLACK,
        })
    );
    assert_eq!(ComputedStyle::default().box_shadow, None);
    assert_eq!(
        ComputedStyle::parse("box-shadow: 1px 1px #000; box-shadow: none")
            .expect("reset")
            .box_shadow,
        None
    );
}

#[test]
fn box_shadow_outside_the_subset_is_rejected() {
    for css in [
        // Spread distance, inset shadows and shadow lists are outside the subset.
        "box-shadow: 1px 2px 3px 4px #000",
        "box-shadow: inset 0 0 4px #000",
        "box-shadow: 0 0 2px #000, 1px 1px #fff",
        // A negative blur, a missing color or offset, and a named color.
        "box-shadow: 1px 2px -3px #000",
        "box-shadow: 1px 2px 3px",
        "box-shadow: 1px #000",
        "box-shadow: #000",
        "box-shadow: 1px 2px black",
    ] {
        let error = ComputedStyle::parse(css).expect_err("outside the subset");
        assert_eq!(error.code, ErrorCode::InvalidCssStyle, "{css}");
    }
}
