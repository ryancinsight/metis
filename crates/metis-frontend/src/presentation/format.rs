//! Number formatting for the form's output fields.

use std::fmt;
use std::fmt::Write;

/// Shortest scientific f64 notation fits 24 glyphs: sign, 17 significant digits,
/// decimal point, exponent marker/sign and three exponent digits.
pub const NUMBER_GLYPHS: usize = 24;

/// Appends `value` rounded to `decimals` places.
pub(crate) fn write_result_number(out: &mut String, value: f64, decimals: usize) -> fmt::Result {
    let start = out.len();
    write!(out, "{value:.decimals$}")?;
    let rounded = &out[start..];
    // Scientific notation keeps a nonzero mantissa when fixed decimal rounding
    // would erase every significant digit, with the same displayed precision.
    if rounded.len() > NUMBER_GLYPHS
        || value.is_subnormal()
        || (value.is_normal() && rounded.chars().all(|c| matches!(c, '0' | '.' | '-')))
    {
        out.truncate(start);
        write!(out, "{value:.decimals$e}")?;
    }
    Ok(())
}

/// Appends `value` in shortest notation with at least `minimum_decimals`
/// decimal places.
pub(crate) fn write_input_number(
    out: &mut String,
    value: f64,
    minimum_decimals: usize,
) -> fmt::Result {
    // Shortest scientific f64 notation fits 24 glyphs: sign, 17 significant
    // digits, decimal point, exponent marker/sign and three exponent digits.
    // Preserve those digits instead of rounding accepted small inputs to zero.
    let start = out.len();
    write!(out, "{value}")?;
    if out.len() - start > NUMBER_GLYPHS {
        out.truncate(start);
        return write!(out, "{value:e}");
    }
    if value.is_finite() {
        let decimals = out[start..]
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len());
        if decimals < minimum_decimals {
            if decimals == 0 {
                out.push('.');
            }
            for _ in decimals..minimum_decimals {
                out.push('0');
            }
        }
    }
    Ok(())
}
