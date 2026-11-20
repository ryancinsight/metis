use crate::whole_pixels;
use metis_core::error::ErrorCode;

fn assert_layout_error(extent: f64, message: &str) {
    let error = whole_pixels(extent).expect_err("invalid extent must fail");
    assert_eq!(error.code, ErrorCode::LayoutOverflow);
    assert_eq!(error.trace_id, "REQ-METIS-UI-004");
    assert_eq!(error.message, message);
}

#[test]
fn whole_pixel_extents_round_up() {
    for (extent, expected) in [
        (-0.0, 0),
        (0.0, 0),
        (0.01, 1),
        (1.0, 1),
        (1.01, 2),
        (f64::from(i32::MAX) - 0.25, i32::MAX),
        (f64::from(i32::MAX), i32::MAX),
    ] {
        assert_eq!(whole_pixels(extent), Ok(expected), "{extent:?}");
    }
}

#[test]
fn whole_pixel_extents_reject_invalid_values() {
    for extent in [
        -f64::from_bits(1),
        -f64::EPSILON,
        -0.25,
        -1.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        assert_layout_error(extent, "Text extent must be finite and nonnegative");
    }
    assert_layout_error(
        f64::from(i32::MAX) + 0.25,
        "Text extent exceeds coordinate range",
    );
}
