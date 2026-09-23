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

fn gradient(degrees: f64, stops: &[(Color, Option<f64>)]) -> LinearGradient {
    let stops: Vec<_> = stops
        .iter()
        .map(
            |(color, position)| metis_platform::rasterizer::GradientStop {
                color: *color,
                position: *position,
            },
        )
        .collect();
    LinearGradient::new(degrees, &stops).expect("valid expected gradient")
}

#[test]
fn linear_gradients_parse_to_their_direction_and_stops() {
    let navy = Color::rgb(0x1a, 0x36, 0x5d);
    let blue = Color::rgb(0x2c, 0x52, 0x82);
    let cases = [
        (
            "background-image: linear-gradient(135deg, #1a365d, #2c5282)",
            gradient(135.0, &[(navy, None), (blue, None)]),
        ),
        // The direction defaults to `to bottom`.
        (
            "background-image: linear-gradient(#1a365d, #2c5282)",
            gradient(180.0, &[(navy, None), (blue, None)]),
        ),
        (
            "background-image: LINEAR-GRADIENT(TO Right, #1A365D 10%, #2c5282 75.5%)",
            gradient(90.0, &[(navy, Some(0.1)), (blue, Some(0.755))]),
        ),
        (
            "background-image: linear-gradient(to top, #1a365d, #fff 50%, #2c5282)",
            gradient(
                0.0,
                &[(navy, None), (Color::WHITE, Some(0.5)), (blue, None)],
            ),
        ),
        (
            "background-image: linear-gradient( -45.5DEG , #1a365d,#2c5282 )",
            gradient(-45.5, &[(navy, None), (blue, None)]),
        ),
        (
            "background-image: linear-gradient(to left, #1a365d, #2c5282)",
            gradient(270.0, &[(navy, None), (blue, None)]),
        ),
    ];
    for (css, expected) in cases {
        let style = ComputedStyle::parse(css).expect("admitted gradient");
        assert_eq!(style.background_gradient, Some(expected), "{css}");
        assert_eq!(style.background_color, None, "{css}");
    }
}

#[test]
fn background_layers_follow_the_shorthand_reset() {
    let navy = Color::rgb(0x1a, 0x36, 0x5d);
    let ramp = gradient(180.0, &[(navy, None), (Color::WHITE, None)]);
    let parse = |css: &str| ComputedStyle::parse(css).expect("admitted background");
    // The longhands set one layer each and keep the other.
    let both = parse("background-color: #1a365d; background-image: linear-gradient(#1a365d, #fff)");
    assert_eq!(both.background_color, Some(navy));
    assert_eq!(both.background_gradient, Some(ramp.clone()));
    // The shorthand sets its layer and resets the other.
    let shorthand = parse("background-color: #fff; background: linear-gradient(#1a365d, #fff)");
    assert_eq!(shorthand.background_color, None);
    assert_eq!(shorthand.background_gradient, Some(ramp));
    let replaced = parse("background: linear-gradient(#1a365d, #fff); background: #1a365d");
    assert_eq!(replaced.background_color, Some(navy));
    assert_eq!(replaced.background_gradient, None);
    let cleared = parse("background-image: linear-gradient(#1a365d, #fff); background-image: none");
    assert_eq!(cleared.background_gradient, None);
    assert_eq!(ComputedStyle::default().background_gradient, None);
}

#[test]
fn gradients_outside_the_subset_are_rejected() {
    for value in [
        // Corner keywords, other angle units, color hints and length stops.
        "linear-gradient(to top right, #000, #fff)",
        "linear-gradient(0.25turn, #000, #fff)",
        "linear-gradient(1rad, #000, #fff)",
        "linear-gradient(#000, 30%, #fff)",
        "linear-gradient(#000 10px, #fff)",
        "linear-gradient(#000 10% 20%, #fff)",
        // Other gradient functions and malformed syntax.
        "radial-gradient(#000, #fff)",
        "repeating-linear-gradient(#000, #fff)",
        "linear-gradient(#000, #fff",
        "linear-gradient(#000,, #fff)",
        "linear-gradient(to middle, #000, #fff)",
        "linear-gradient(infdeg, #000, #fff)",
        "linear-gradient(#000 NaN%, #fff)",
        "linear-gradient(#000, black)",
        // One stop, no stops, and more than the stop bound.
        "linear-gradient(#000)",
        "linear-gradient(90deg)",
        "linear-gradient(#000, #111, #222, #333, #444, #555, #666, #777, #888)",
    ] {
        for property in ["background", "background-image"] {
            let css = format!("{property}: {value}");
            let error = ComputedStyle::parse(&css).expect_err("outside the subset");
            assert_eq!(error.code, ErrorCode::InvalidCssStyle, "{css}");
        }
    }
    // The bound admits exactly eight stops.
    assert!(
        ComputedStyle::parse(
            "background: linear-gradient(#000, #111, #222, #333, #444, #555, #666, #777)"
        )
        .expect("eight stops")
        .background_gradient
        .is_some()
    );
    let error = ComputedStyle::parse("background-color: linear-gradient(#000, #fff)")
        .expect_err("a color longhand takes no gradient");
    assert_eq!(error.code, ErrorCode::InvalidCssStyle);
}
