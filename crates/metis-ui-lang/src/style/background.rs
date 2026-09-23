//! Background declarations: a color and the `linear-gradient()` subset of
//! CSS Images 3.

use super::{ComputedStyle, invalid_value, parse_color};
use metis_core::error::Result;
use metis_platform::rasterizer::{GradientStop, LinearGradient};

impl ComputedStyle {
    /// Applies `background`, `background-color` or `background-image`.
    ///
    /// The shorthand takes one layer, a hex color or a gradient, and resets
    /// the other, so a later `background` replaces an earlier color or
    /// gradient (CSS Backgrounds 3 section 3.10).
    pub(super) fn apply_background(&mut self, property: &str, value: &str) -> Result<()> {
        match property {
            "background-color" => self.background_color = Some(parse_color(property, value)?),
            "background-image" if value == "none" => self.background_gradient = None,
            "background-image" => {
                self.background_gradient = Some(parse_linear_gradient(property, value)?);
            }
            _ if value.starts_with('#') => {
                self.background_color = Some(parse_color(property, value)?);
                self.background_gradient = None;
            }
            _ => {
                self.background_gradient = Some(parse_linear_gradient(property, value)?);
                self.background_color = None;
            }
        }
        Ok(())
    }
}

/// Angle of each side keyword in `to <side>` (section 3.1).
const SIDES: [(&str, f64); 4] = [
    ("top", 0.0),
    ("right", 90.0),
    ("bottom", 180.0),
    ("left", 270.0),
];

/// Parses `linear-gradient([<angle> | to <side>,]? <stop>, <stop> {, <stop>})`
/// where an angle is `<number>deg`, a side is `top`, `right`, `bottom` or
/// `left`, and a stop is a hex color with an optional `<number>%` position.
///
/// The direction defaults to `to bottom`. Corner keywords, color hints,
/// length positions and other angle units are rejected rather than
/// approximated.
fn parse_linear_gradient(property: &str, value: &str) -> Result<LinearGradient> {
    let invalid = || invalid_value(property, value);
    let prefix = "linear-gradient(";
    let arguments = value
        .get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))
        .and_then(|_| value[prefix.len()..].strip_suffix(')'))
        .ok_or_else(invalid)?;
    let mut arguments = arguments.split(',').map(str::trim).peekable();
    let direction = match arguments.peek() {
        Some(first) => direction(property, value, first)?,
        None => None,
    };
    let degrees = match direction {
        Some(degrees) => {
            arguments.next();
            degrees
        }
        None => 180.0,
    };
    let stops = arguments
        .map(|argument| stop(property, value, argument))
        .collect::<Result<Vec<_>>>()?;
    LinearGradient::new(degrees, &stops).ok_or_else(invalid)
}

/// The angle a first argument names, or `None` when it is a color stop.
///
/// # Errors
/// A side or angle outside the subset is invalid for `property`.
fn direction(property: &str, value: &str, argument: &str) -> Result<Option<f64>> {
    let invalid = || invalid_value(property, value);
    // Keywords and units are ASCII case-insensitive.
    let argument = argument.to_ascii_lowercase();
    if let Some(side) = argument.strip_prefix("to ") {
        let side = side.trim();
        return SIDES
            .iter()
            .find(|(name, _)| side == *name)
            .map(|(_, degrees)| Some(*degrees))
            .ok_or_else(invalid);
    }
    let Some(number) = argument.strip_suffix("deg") else {
        return Ok(None);
    };
    number
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|degrees| degrees.is_finite())
        .map(Some)
        .ok_or_else(invalid)
}

fn stop(property: &str, value: &str, argument: &str) -> Result<GradientStop> {
    let invalid = || invalid_value(property, value);
    let mut parts = argument.split_whitespace();
    let color = parse_color(property, parts.next().ok_or_else(invalid)?)?;
    let position = parts
        .next()
        .map(|position| {
            position
                .strip_suffix('%')
                .and_then(|percent| percent.parse::<f64>().ok())
                .filter(|percent| percent.is_finite())
                .map(|percent| percent / 100.0)
                .ok_or_else(invalid)
        })
        .transpose()?;
    if parts.next().is_some() {
        return Err(invalid());
    }
    Ok(GradientStop { color, position })
}
