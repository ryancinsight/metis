//! `flex-grow`: an element's weight in its container's free main-axis space.

use super::invalid_value;
use metis_core::error::Result;

/// Nonnegative `flex-grow` factor, held in thousandths.
///
/// Fixed point keeps [`ComputedStyle`](super::ComputedStyle) free of floating
/// point and makes the share arithmetic exact: a factor is an integer weight,
/// and a thousand of them is one unit. Factors above [`FlexGrow::MAX`] are
/// rejected rather than clamped, as every other out-of-range value is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct FlexGrow(u32);

impl FlexGrow {
    /// The default: the element does not grow.
    pub const NONE: Self = Self(0);
    /// Thousandths in one unit of `flex-grow`.
    pub const ONE: u32 = 1000;
    /// Largest admitted factor, `flex-grow: 1000`.
    pub const MAX: Self = Self(1_000 * Self::ONE);

    /// Factor from thousandths, when within [`FlexGrow::MAX`].
    #[must_use]
    pub const fn from_thousandths(thousandths: u32) -> Option<Self> {
        if thousandths <= Self::MAX.0 {
            Some(Self(thousandths))
        } else {
            None
        }
    }

    /// The factor in thousandths.
    #[must_use]
    pub const fn thousandths(self) -> u32 {
        self.0
    }

    /// Whether the element takes any share of free space.
    #[must_use]
    pub const fn grows(self) -> bool {
        self.0 > 0
    }
}

/// Parses a CSS `<number>`, rounded to the nearest thousandth.
pub(super) fn parse_flex_grow(property: &str, value: &str) -> Result<FlexGrow> {
    let factor = value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|factor| factor.is_finite() && *factor >= 0.0)
        .ok_or_else(|| invalid_value(property, value))?;
    let thousandths = (factor * f64::from(FlexGrow::ONE)).round();
    if thousandths > f64::from(FlexGrow::MAX.0) {
        return Err(invalid_value(property, value));
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a finite, nonnegative, rounded value at most FlexGrow::MAX fits u32 exactly"
    )]
    let thousandths = thousandths as u32;
    Ok(FlexGrow(thousandths))
}
