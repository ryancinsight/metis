//! Validated device-pixel scale derived from a native display capability.

use metis_core::error::{ErrorCode, MetisError, Result};
use std::fmt;

const MILLIS_PER_SCALE: u64 = 1_000;
const DPI_PER_CSS_INCH: u64 = 96;

/// Fixed-point device scale with one-thousandth precision.
///
/// The value is the ratio between physical device pixels and authored CSS
/// pixels. A scale of `1.25` is stored as `1_250`, which keeps layout and text
/// rasterization deterministic without floating-point coordinate drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DisplayScale {
    milli: u32,
}

impl DisplayScale {
    /// The default CSS-to-device mapping at 96 DPI.
    pub const ONE: Self = Self { milli: 1_000 };

    /// Constructs a scale from thousandths of a device pixel per CSS pixel.
    ///
    /// # Errors
    /// Returns [`ErrorCode::InvalidWindow`] when the scale is zero.
    pub fn from_milli(milli: u32) -> Result<Self> {
        if milli == 0 {
            return Err(MetisError::ui(
                ErrorCode::InvalidWindow,
                "Display scale must be greater than zero",
            ));
        }
        Ok(Self { milli })
    }

    /// Converts a native 96-DPI-relative value to a validated device scale.
    ///
    /// Native providers report DPI as an integer. The conversion rounds to the
    /// nearest thousandth so common values such as 120 DPI and 144 DPI map to
    /// `1.250` and `1.500` exactly.
    ///
    /// # Errors
    /// Returns [`ErrorCode::InvalidWindow`] for zero DPI or when the converted
    /// value cannot be represented by this type.
    pub fn from_dpi(dpi: u32) -> Result<Self> {
        if dpi == 0 {
            return Err(MetisError::ui(
                ErrorCode::InvalidWindow,
                "Native display DPI must be greater than zero",
            ));
        }
        let milli = (u64::from(dpi) * MILLIS_PER_SCALE + DPI_PER_CSS_INCH / 2) / DPI_PER_CSS_INCH;
        let milli = u32::try_from(milli).map_err(|_| {
            MetisError::ui(
                ErrorCode::InvalidWindow,
                "Native display DPI exceeds the display scale range",
            )
        })?;
        Self::from_milli(milli)
    }

    /// Returns the fixed-point scale in thousandths.
    #[must_use]
    pub const fn milli(self) -> u32 {
        self.milli
    }

    /// Multiplies the display scale by a positive integer font multiplier.
    ///
    /// # Errors
    /// Returns [`ErrorCode::LayoutOverflow`] for zero or an unrepresentable
    /// result.
    pub fn multiply(self, factor: u32) -> Result<Self> {
        if factor == 0 {
            return Err(MetisError::ui(
                ErrorCode::LayoutOverflow,
                "Text scale multiplier must be greater than zero",
            ));
        }
        let milli = u64::from(self.milli) * u64::from(factor);
        let milli = u32::try_from(milli).map_err(|_| {
            MetisError::ui(
                ErrorCode::LayoutOverflow,
                "Effective text scale exceeds the representable range",
            )
        })?;
        Self::from_milli(milli)
    }

    /// Scales a signed coordinate and rounds to the nearest device pixel.
    ///
    /// # Errors
    /// Returns [`ErrorCode::LayoutOverflow`] when the scaled coordinate does
    /// not fit in an `i32`.
    pub fn scale_coordinate(self, value: i32) -> Result<i32> {
        let product = i128::from(value) * i128::from(self.milli);
        let rounded = round_ratio(product, i128::from(MILLIS_PER_SCALE));
        i32::try_from(rounded).map_err(|_| {
            MetisError::ui(
                ErrorCode::LayoutOverflow,
                "Display-scaled coordinate exceeds the layout range",
            )
        })
    }

    /// Scales a nonnegative extent and keeps a positive source extent visible.
    ///
    /// # Errors
    /// Returns [`ErrorCode::LayoutOverflow`] for negative input or an extent
    /// outside the `i32` coordinate range.
    pub fn scale_extent(self, value: i32) -> Result<i32> {
        if value < 0 {
            return Err(MetisError::ui(
                ErrorCode::LayoutOverflow,
                "Display-scaled extent must be nonnegative",
            ));
        }
        let scaled = self.scale_coordinate(value)?;
        Ok(if value > 0 { scaled.max(1) } else { 0 })
    }
}

impl Default for DisplayScale {
    fn default() -> Self {
        Self::ONE
    }
}

impl fmt::Display for DisplayScale {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}.{:03}x",
            self.milli / 1_000,
            self.milli % 1_000
        )
    }
}

fn round_ratio(value: i128, denominator: i128) -> i128 {
    let adjustment = if value >= 0 {
        denominator / 2
    } else {
        -(denominator / 2)
    };
    (value + adjustment) / denominator
}

#[cfg(test)]
mod tests {
    use super::DisplayScale;
    use metis_core::ErrorCode;

    #[test]
    fn dpi_conversion_preserves_common_windows_scales() {
        assert_eq!(
            DisplayScale::from_dpi(96).expect("96 DPI"),
            DisplayScale::ONE
        );
        assert_eq!(DisplayScale::from_dpi(120).expect("120 DPI").milli(), 1_250);
        assert_eq!(DisplayScale::from_dpi(144).expect("144 DPI").milli(), 1_500);
    }

    #[test]
    fn invalid_or_unrepresentable_dpi_is_typed() {
        assert_eq!(
            DisplayScale::from_dpi(0).expect_err("zero DPI").code,
            ErrorCode::InvalidWindow
        );
        assert_eq!(
            DisplayScale::from_milli(0).expect_err("zero scale").code,
            ErrorCode::InvalidWindow
        );
    }

    #[test]
    fn coordinates_and_extents_round_at_fractional_scale() {
        let scale = DisplayScale::from_milli(1_250).expect("fractional scale");
        assert_eq!(scale.scale_coordinate(20).expect("padding"), 25);
        assert_eq!(scale.scale_coordinate(-20).expect("negative offset"), -25);
        assert_eq!(scale.scale_extent(1).expect("visible extent"), 1);
        assert_eq!(scale.multiply(2).expect("font multiplier").milli(), 2_500);
    }
}
