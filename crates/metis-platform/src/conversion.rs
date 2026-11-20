//! Checked conversions into device-pixel coordinates.

#![expect(
    clippy::cast_possible_truncation,
    reason = "the finite whole value is checked against the i32 range"
)]

use metis_core::error::{ErrorCode, MetisError, Result};

/// Rounds a finite nonnegative device-pixel extent up to a whole pixel.
///
/// # Errors
/// Returns [`ErrorCode::LayoutOverflow`] when `extent` is negative, non-finite,
/// or rounds outside the nonnegative `i32` range.
///
/// # Examples
///
/// ```
/// use metis_platform::whole_pixels;
///
/// assert_eq!(whole_pixels(12.01)?, 13);
/// # Ok::<(), metis_core::error::MetisError>(())
/// ```
pub fn whole_pixels(extent: f64) -> Result<i32> {
    if !extent.is_finite() || extent < 0.0 {
        return Err(layout_error("Text extent must be finite and nonnegative"));
    }
    pixel(extent.ceil()).ok_or_else(|| layout_error("Text extent exceeds coordinate range"))
}

/// Converts a finite whole-pixel value in the `i32` range.
pub(crate) fn pixel(value: f64) -> Option<i32> {
    if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return None;
    }
    Some(value as i32)
}

pub(crate) fn layout_error(message: &'static str) -> MetisError {
    MetisError::ui(ErrorCode::LayoutOverflow, message)
}
