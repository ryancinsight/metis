//! Pure page-domain logic: bounded themes, the JavaScript-compatible number
//! and error renderings, the announced shortcut vocabulary and the retired
//! `Number()` input conversion. This module compiles on every target so the
//! contracts carry native unit tests; the DOM lifecycle in `page` (compiled
//! for the `wasm32` page target) composes them.

use std::string::String;

/// The bounded page themes, matching the packaged page's select options.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    /// Follow the system preference.
    System,
    /// The light palette.
    Light,
    /// The dark palette.
    Dark,
    /// The forced high-contrast palette.
    HighContrast,
}

impl Theme {
    /// Parses one admitted theme attribute value.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "high-contrast" => Some(Self::HighContrast),
            _ => None,
        }
    }

    /// The rendered description inside the theme status line.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "system preference",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::HighContrast => "high contrast",
        }
    }

    /// The `data-metis-theme` attribute value driving the palette.
    #[must_use]
    pub fn attribute(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::HighContrast => "high-contrast",
        }
    }
}

/// Formats one number the way the retired script's template rendered numbers:
/// shortest round-trip digits with JavaScript's exponential switch outside
/// the inclusive `[1e-6, 1e21)` band.
#[must_use]
pub fn number_text(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    if !value.is_finite() {
        return if value.is_nan() {
            "NaN".to_owned()
        } else if value < 0.0 {
            "-Infinity".to_owned()
        } else {
            "Infinity".to_owned()
        };
    }
    let magnitude = value.abs();
    if (1e-6..1e21).contains(&magnitude) {
        format!("{value}")
    } else {
        exponential_text(value)
    }
}

/// JavaScript `Number.prototype.toString` spelling of an exponential value:
/// one mantissa digit group, a sign only on positive exponents, no zero fill.
fn exponential_text(value: f64) -> String {
    let rendered = format!("{value:e}");
    match rendered.split_once('e') {
        Some((mantissa, exponent)) => match exponent.parse::<i32>() {
            Ok(power) if power >= 0 => format!("{mantissa}e+{power}"),
            Ok(power) => format!("{mantissa}e{power}"),
            Err(_) => rendered,
        },
        None => rendered,
    }
}

/// Converts an input's text the way the retired `Number()` argument did: an
/// empty or whitespace-only value is zero and an unparseable rest is NaN,
/// which the host then rejects as non-finite.
#[must_use]
pub fn number_argument(text: &str) -> f64 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    trimmed.parse::<f64>().unwrap_or(f64::NAN)
}

/// Renders the typed error line exactly as the retired script did: a lowecase
/// hexadecimal error code padded to four digits.
#[must_use]
pub fn error_line(status: &str, message: &str, error_code: u16) -> String {
    format!("{status}: {message} [0x{error_code:04x}]")
}

/// One browser key event reading: the physical code plus the four modifiers
/// the announced-shortcut vocabulary uses.
#[derive(Clone, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the browser KeyboardEvent carries exactly four independent modifier flags"
)]
pub struct KeyPress {
    /// The physical browser code, such as `KeyP`.
    pub code: String,
    /// Whether Control is held.
    pub control: bool,
    /// Whether Alt is held.
    pub alt: bool,
    /// Whether Shift is held.
    pub shift: bool,
    /// Whether Meta is held.
    pub meta: bool,
}

/// Builds the `Control+Alt+Shift+Meta+Letter` spelling from one browser key
/// event reading, matching `metis_core::input` formatting and the retired
/// `Key[A-Z]` regex: only physical letter codes produce a shortcut.
#[must_use]
pub fn shortcut_text(press: &KeyPress) -> Option<String> {
    let letter = press.code.strip_prefix("Key")?;
    let first = letter.chars().next()?;
    if letter.len() != 1 || !first.is_ascii_uppercase() {
        return None;
    }
    let mut held = String::new();
    for (name, down) in [
        ("Control", press.control),
        ("Alt", press.alt),
        ("Shift", press.shift),
        ("Meta", press.meta),
    ] {
        if down {
            if !held.is_empty() {
                held.push('+');
            }
            held.push_str(name);
        }
    }
    if !held.is_empty() {
        held.push('+');
    }
    held.push_str(letter);
    Some(held)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themes_round_trip_the_four_admitted_values() {
        for theme in [
            (Theme::System, "system", "system preference"),
            (Theme::Light, "light", "light"),
            (Theme::Dark, "dark", "dark"),
            (Theme::HighContrast, "high-contrast", "high contrast"),
        ] {
            let parsed = Theme::parse(theme.1).expect("admitted theme");
            assert_eq!(parsed, theme.0);
            assert_eq!(parsed.attribute(), theme.1);
            assert_eq!(parsed.label(), theme.2);
        }
        assert_eq!(Theme::parse("gallery"), None);
    }

    #[test]
    fn number_text_matches_the_retired_template_rendering() {
        assert_eq!(number_text(0.0), "0");
        assert_eq!(number_text(12.0), "12");
        assert_eq!(number_text(-2.5), "-2.5");
        assert_eq!(number_text(0.000_001), "0.000001");
        assert_eq!(number_text(1e-7), "1e-7");
        assert_eq!(number_text(1.25e-8), "1.25e-8");
        assert_eq!(number_text(1e21), "1e+21");
        assert_eq!(number_text(1.5e21), "1.5e+21");
        assert_eq!(number_text(f64::NAN), "NaN");
        assert_eq!(number_text(f64::INFINITY), "Infinity");
        assert_eq!(number_text(f64::NEG_INFINITY), "-Infinity");
    }

    #[test]
    fn number_argument_matches_the_retired_number_conversion() {
        // Exact bit patterns: the conversion is deterministic, so the test
        // pins the precise value rather than an epsilon band.
        assert_eq!(number_argument("60").to_bits(), 60.0_f64.to_bits());
        assert_eq!(number_argument(" 60 ").to_bits(), 60.0_f64.to_bits());
        assert_eq!(number_argument("").to_bits(), 0.0_f64.to_bits());
        assert_eq!(number_argument("   ").to_bits(), 0.0_f64.to_bits());
        assert!(number_argument("60px").is_nan());
    }

    #[test]
    fn error_line_pads_the_code_to_four_hex_digits() {
        assert_eq!(
            error_line("rejected", "out of bounds", 0x270e),
            "rejected: out of bounds [0x270e]"
        );
        assert_eq!(
            error_line("failed", "no pipe", 0x12),
            "failed: no pipe [0x0012]"
        );
    }

    #[test]
    fn shortcut_text_follows_the_modifier_order_contract() {
        let press = |code: &str, control: bool, alt: bool, shift: bool, meta: bool| KeyPress {
            code: code.to_owned(),
            control,
            alt,
            shift,
            meta,
        };
        assert_eq!(
            shortcut_text(&press("KeyP", false, true, true, false)),
            Some("Alt+Shift+P".to_owned())
        );
        assert_eq!(
            shortcut_text(&press("KeyD", false, true, true, false)),
            Some("Alt+Shift+D".to_owned())
        );
        assert_eq!(
            shortcut_text(&press("KeyJ", true, false, false, false)),
            Some("Control+J".to_owned())
        );
        assert_eq!(
            shortcut_text(&press("KeyW", true, true, true, true)),
            Some("Control+Alt+Shift+Meta+W".to_owned())
        );
        assert_eq!(
            shortcut_text(&press("Digit1", false, true, true, false)),
            None
        );
        assert_eq!(
            shortcut_text(&press("KeySpace", false, true, true, false)),
            None
        );
        assert_eq!(shortcut_text(&press("key", false, true, true, false)), None);
    }
}
