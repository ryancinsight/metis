//! CSS-inspired style declarations, box model, and layout properties.

use metis_core::error::{ErrorCode, MetisError, Result};
pub use metis_platform::framebuffer::Color;

/// Display flow mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    /// Sequential flex-like flow (without browser flex distribution).
    Flex,
    /// Block flow, using the configured direction.
    Block,
    /// Inline declaration, currently laid out as sequential flow.
    Inline,
    /// Excluded from painting and flow.
    None,
}

/// Flex direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection {
    /// Lay children out horizontally.
    Row,
    /// Lay children out vertically.
    Column,
}

/// Justify content alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JustifyContent {
    /// Alignment at the start edge (stored only).
    FlexStart,
    /// Centered alignment (stored only).
    Center,
    /// Alignment at the end edge (stored only).
    FlexEnd,
    /// Distribute free space between items (stored only).
    SpaceBetween,
}

/// Align items cross-axis alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignItems {
    /// Alignment at the start edge (stored only).
    FlexStart,
    /// Centered alignment (stored only).
    Center,
    /// Alignment at the end edge (stored only).
    FlexEnd,
    /// Stretch cross-axis items (stored only).
    Stretch,
}

/// Edge dimensions (top, right, bottom, left) in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EdgeValues {
    /// Top edge in pixels.
    pub top: i32,
    /// Right edge in pixels.
    pub right: i32,
    /// Bottom edge in pixels.
    pub bottom: i32,
    /// Left edge in pixels.
    pub left: i32,
}

impl EdgeValues {
    /// Sets all four edges to the same pixel value.
    #[must_use]
    pub const fn all(val: i32) -> Self {
        Self {
            top: val,
            right: val,
            bottom: val,
            left: val,
        }
    }

    /// Sets vertical and horizontal edge pairs.
    #[must_use]
    pub const fn symmetric(vertical: i32, horizontal: i32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

/// Size dimension specification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Size {
    /// Dimension derived from available width or content height.
    Auto,
    /// Explicit pixel dimension.
    Px(i32),
    /// Fraction of the available dimension; 1.0 represents 100 percent.
    Percent(f32),
}

/// Font weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    /// Normal weight.
    Normal,
    /// Bold weight declaration (stored only).
    Bold,
}

/// Computed CSS style properties for a DOM node.
#[derive(Debug, Clone, PartialEq)]
pub struct ComputedStyle {
    /// Flow visibility and display declaration.
    pub display: Display,
    /// Sequential child layout direction.
    pub flex_direction: FlexDirection,
    /// Main-axis alignment declaration (stored only).
    pub justify_content: JustifyContent,
    /// Cross-axis alignment declaration (stored only).
    pub align_items: AlignItems,
    /// Space between adjacent children in pixels.
    pub gap: i32,
    /// Requested width.
    pub width: Size,
    /// Requested height.
    pub height: Size,
    /// Minimum width declaration (stored only).
    pub min_width: Size,
    /// Minimum height declaration (stored only).
    pub min_height: Size,
    /// Inner spacing.
    pub padding: EdgeValues,
    /// Outer spacing.
    pub margin: EdgeValues,
    /// Border edge widths; painting currently uses the top width uniformly.
    pub border_width: EdgeValues,
    /// Straight RGBA border color.
    pub border_color: Color,
    /// Corner radius declaration (stored only).
    pub border_radius: i32,
    /// Optional straight RGBA background fill.
    pub background_color: Option<Color>,
    /// Straight RGBA text color.
    pub text_color: Color,
    /// Requested font size; bitmap scale is max(1, size / 14).
    pub font_size: u32,
    /// Weight declaration (stored only).
    pub font_weight: FontWeight,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Stretch,
            gap: 0,
            width: Size::Auto,
            height: Size::Auto,
            min_width: Size::Auto,
            min_height: Size::Auto,
            padding: EdgeValues::default(),
            margin: EdgeValues::default(),
            border_width: EdgeValues::default(),
            border_color: Color::TRANSPARENT,
            border_radius: 0,
            background_color: None,
            text_color: Color::BLACK,
            font_size: 14,
            font_weight: FontWeight::Normal,
        }
    }
}

impl ComputedStyle {
    /// Parses an inline declaration list such as `display: flex; gap: 10px`.
    ///
    ///
    /// # Errors
    /// Returns [`ErrorCode::InvalidCssStyle`] for unknown properties, malformed
    /// declarations, invalid values, negative spacing, malformed edge lists or
    /// non-finite and negative dimensions. Coordinate limits are checked by
    /// layout after parsing.
    pub fn parse(css: &str) -> Result<Self> {
        let mut style = Self::default();
        for declaration in css.split(';') {
            let part = declaration.trim();
            if part.is_empty() {
                continue;
            }
            let (key, val) = part
                .split_once(':')
                .ok_or_else(|| invalid_style("Style declaration requires ':'"))?;
            let key = key.trim();
            if key.is_empty() {
                return Err(invalid_style("Style property name is empty"));
            }
            let key = key.to_ascii_lowercase();
            let val = val.trim();
            if val.is_empty() {
                return Err(invalid_value(&key, val));
            }

            match key.as_str() {
                "display" => {
                    style.display = match val {
                        "flex" => Display::Flex,
                        "block" => Display::Block,
                        "inline" => Display::Inline,
                        "none" => Display::None,
                        _ => return Err(invalid_value(&key, val)),
                    }
                }
                "flex-direction" => {
                    style.flex_direction = match val {
                        "row" => FlexDirection::Row,
                        "column" => FlexDirection::Column,
                        _ => return Err(invalid_value(&key, val)),
                    }
                }
                "justify-content" => {
                    style.justify_content = match val {
                        "flex-start" | "start" => JustifyContent::FlexStart,
                        "center" => JustifyContent::Center,
                        "flex-end" | "end" => JustifyContent::FlexEnd,
                        "space-between" => JustifyContent::SpaceBetween,
                        _ => return Err(invalid_value(&key, val)),
                    }
                }
                "align-items" => {
                    style.align_items = match val {
                        "flex-start" | "start" => AlignItems::FlexStart,
                        "center" => AlignItems::Center,
                        "flex-end" | "end" => AlignItems::FlexEnd,
                        "stretch" => AlignItems::Stretch,
                        _ => return Err(invalid_value(&key, val)),
                    }
                }
                "gap" => style.gap = parse_nonnegative_px(&key, val)?,
                "width" => style.width = parse_size(&key, val)?,
                "height" => style.height = parse_size(&key, val)?,
                "min-width" => style.min_width = parse_size(&key, val)?,
                "min-height" => style.min_height = parse_size(&key, val)?,
                "padding" => style.padding = parse_edges(&key, val)?,
                "margin" => style.margin = parse_edges(&key, val)?,
                "border-width" => style.border_width = parse_edges(&key, val)?,
                "border-color" => {
                    style.border_color = parse_color(&key, val)?;
                }
                "border-radius" => style.border_radius = parse_nonnegative_px(&key, val)?,
                "background-color" | "background" => {
                    style.background_color = Some(parse_color(&key, val)?);
                }
                "color" => style.text_color = parse_color(&key, val)?,
                "font-size" => {
                    let px = parse_nonnegative_px(&key, val)?.max(8);
                    style.font_size = u32::try_from(px).map_err(|_| invalid_value(&key, val))?;
                }
                "font-weight" => {
                    style.font_weight = match val {
                        "normal" | "400" => FontWeight::Normal,
                        "bold" | "700" => FontWeight::Bold,
                        _ => return Err(invalid_value(&key, val)),
                    };
                }
                _ => {
                    return Err(MetisError::ui(
                        ErrorCode::InvalidCssStyle,
                        format!("Unsupported CSS style property '{key}'"),
                    ));
                }
            }
        }
        Ok(style)
    }
}

fn invalid_style(message: &str) -> MetisError {
    MetisError::ui(ErrorCode::InvalidCssStyle, message)
}

fn invalid_value(property: &str, value: &str) -> MetisError {
    MetisError::ui(
        ErrorCode::InvalidCssStyle,
        format!("Invalid value for CSS property '{property}': '{value}'"),
    )
}

fn parse_px(s: &str) -> Option<i32> {
    let s = s.trim();
    let number = s.strip_suffix("px").unwrap_or(s).trim();
    (!number.is_empty())
        .then(|| number.parse::<i32>().ok())
        .flatten()
}

fn parse_nonnegative_px(property: &str, value: &str) -> Result<i32> {
    let px = parse_px(value).ok_or_else(|| invalid_value(property, value))?;
    if px < 0 {
        return Err(invalid_value(property, value));
    }
    Ok(px)
}

fn parse_size(property: &str, value: &str) -> Result<Size> {
    let s = value.trim();
    if s == "auto" {
        return Ok(Size::Auto);
    }
    if let Some(pct) = s.strip_suffix('%')
        && let Ok(val) = pct.trim().parse::<f32>()
        && val.is_finite()
        && val >= 0.0
    {
        return Ok(Size::Percent(val / 100.0));
    }
    parse_nonnegative_px(property, s).map(Size::Px)
}

fn parse_edges(property: &str, value: &str) -> Result<EdgeValues> {
    let mut values = [0; 4];
    let mut count = 0;
    for part in value.split_whitespace() {
        let slot = values
            .get_mut(count)
            .ok_or_else(|| invalid_value(property, value))?;
        *slot = parse_nonnegative_px(property, part)?;
        count += 1;
    }
    if count == 0 {
        return Err(invalid_value(property, value));
    }
    let [top, right, bottom, left] = values;
    Ok(match count {
        1 => EdgeValues::all(top),
        2 => EdgeValues::symmetric(top, right),
        3 => EdgeValues {
            top,
            right,
            bottom,
            left: right,
        },
        4 => EdgeValues {
            top,
            right,
            bottom,
            left,
        },
        _ => return Err(invalid_value(property, value)),
    })
}

fn parse_color(property: &str, value: &str) -> Result<Color> {
    Color::from_hex(value).ok_or_else(|| invalid_value(property, value))
}

#[cfg(test)]
mod tests {
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
    fn parses_admitted_values_and_stored_properties() {
        let style = ComputedStyle::parse(
            "display: block; flex-direction: row; justify-content: center; align-items: end; \
             gap: 4px; width: 50%; height: 12px; min-width: auto; min-height: 3; \
             padding: 1px 2px 3px 4px; margin: 5px 6px; border-width: 1px; \
             border-color: #123456; border-radius: 2px; background: #abcdef80; \
             color: #fedcba; font-size: 10px; font-weight: 700;",
        )
        .expect("admitted style");

        assert_eq!(style.display, Display::Block);
        assert_eq!(style.flex_direction, FlexDirection::Row);
        assert_eq!(style.justify_content, JustifyContent::Center);
        assert_eq!(style.align_items, AlignItems::FlexEnd);
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
        assert_eq!(style.font_weight, FontWeight::Bold);
    }

    #[test]
    fn rejects_unsupported_and_malformed_values_with_one_code() {
        for css in [
            "unknown: value",
            "display",
            "gap:",
            "display: column",
            "gap: -1px",
            "width: NaN%",
            "padding: 1px nope",
            "color: #xyz",
            "font-weight: 500",
        ] {
            let error = ComputedStyle::parse(css).expect_err("invalid style");
            assert_eq!(error.code, ErrorCode::InvalidCssStyle, "{css}");
        }
    }
}
