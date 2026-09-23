//! CSS-inspired style declarations, box model, and layout properties.

mod background;

use metis_core::error::{ErrorCode, MetisError, Result};
pub use metis_platform::framebuffer::Color;
pub use metis_platform::rasterizer::LinearGradient;

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
    /// Alignment at the start edge.
    FlexStart,
    /// Centered alignment.
    Center,
    /// Alignment at the end edge.
    FlexEnd,
    /// Distribute free space between items.
    SpaceBetween,
}

/// Align items cross-axis alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignItems {
    /// Alignment at the start edge.
    FlexStart,
    /// Centered alignment.
    Center,
    /// Alignment at the end edge.
    FlexEnd,
    /// Stretch cross-axis items.
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
    /// Bold weight declaration.
    Bold,
}

/// An outer box shadow in authored pixels.
///
/// The subset is one shadow of two offsets, an optional blur radius and a
/// color: no `inset`, no spread distance and no comma-separated list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shadow {
    /// Horizontal offset; positive values move the shadow right.
    pub offset_x: i32,
    /// Vertical offset; positive values move the shadow down.
    pub offset_y: i32,
    /// Nonnegative blur radius: twice the Gaussian's standard deviation.
    pub blur: i32,
    /// Straight RGBA shadow color.
    pub color: Color,
}

/// Computed CSS style properties for a DOM node.
#[derive(Debug, Clone, PartialEq)]
pub struct ComputedStyle {
    /// Flow visibility and display declaration.
    pub display: Display,
    /// Sequential child layout direction.
    pub flex_direction: FlexDirection,
    /// Distribution of free main-axis space among the children.
    pub justify_content: JustifyContent,
    /// Placement of each child within the cross-axis extent.
    pub align_items: AlignItems,
    /// Space between adjacent children in pixels.
    pub gap: i32,
    /// Requested width.
    pub width: Size,
    /// Requested height.
    pub height: Size,
    /// Lower bound on the resolved width.
    pub min_width: Size,
    /// Lower bound on the resolved height.
    pub min_height: Size,
    /// Inner spacing.
    pub padding: EdgeValues,
    /// Outer spacing.
    pub margin: EdgeValues,
    /// Border edge widths; painting currently uses the top width uniformly.
    pub border_width: EdgeValues,
    /// Straight RGBA border color.
    pub border_color: Color,
    /// Corner radius, clamped to half the shorter side of the border box.
    pub border_radius: i32,
    /// Optional straight RGBA background fill.
    pub background_color: Option<Color>,
    /// Optional linear gradient painted over the background color, across
    /// the border box.
    pub background_gradient: Option<LinearGradient>,
    /// Straight RGBA text color.
    pub text_color: Color,
    /// Authored font size in CSS pixels per em; layout multiplies it by the
    /// host display scale to size the text in device pixels.
    pub font_size: u32,
    /// Face weight: the regular or the bold face.
    pub font_weight: FontWeight,
    /// Outer shadow painted beneath the background, if any.
    pub box_shadow: Option<Shadow>,
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
            background_gradient: None,
            text_color: Color::BLACK,
            font_size: 14,
            font_weight: FontWeight::Normal,
            box_shadow: None,
        }
    }
}

impl ComputedStyle {
    /// Parses an inline declaration list such as `display: flex; gap: 10px`.
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
                        "flex-start" => JustifyContent::FlexStart,
                        "center" => JustifyContent::Center,
                        "flex-end" => JustifyContent::FlexEnd,
                        "space-between" => JustifyContent::SpaceBetween,
                        _ => return Err(invalid_value(&key, val)),
                    }
                }
                "align-items" => {
                    style.align_items = match val {
                        "flex-start" => AlignItems::FlexStart,
                        "center" => AlignItems::Center,
                        "flex-end" => AlignItems::FlexEnd,
                        "stretch" => AlignItems::Stretch,
                        _ => return Err(invalid_value(&key, val)),
                    }
                }
                "min-width" => style.min_width = parse_size(&key, val)?,
                "min-height" => style.min_height = parse_size(&key, val)?,
                "font-weight" => {
                    style.font_weight = match val {
                        "normal" | "400" => FontWeight::Normal,
                        "bold" | "700" => FontWeight::Bold,
                        _ => return Err(invalid_value(&key, val)),
                    }
                }
                "box-shadow" => style.box_shadow = parse_shadow(&key, val)?,
                "border-radius" => style.border_radius = parse_nonnegative_px(&key, val)?,
                "gap" => style.gap = parse_nonnegative_px(&key, val)?,
                "width" => style.width = parse_size(&key, val)?,
                "height" => style.height = parse_size(&key, val)?,
                "padding" => style.padding = parse_edges(&key, val)?,
                "margin" => style.margin = parse_edges(&key, val)?,
                "border-width" => style.border_width = parse_edges(&key, val)?,
                "border-color" => {
                    style.border_color = parse_color(&key, val)?;
                }
                "background" | "background-color" | "background-image" => {
                    style.apply_background(&key, val)?;
                }
                "color" => style.text_color = parse_color(&key, val)?,
                "font-size" => {
                    let px = parse_nonnegative_px(&key, val)?.max(8);
                    style.font_size = u32::try_from(px).map_err(|_| invalid_value(&key, val))?;
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

/// Parses `none` or `<offset-x> <offset-y> [<blur>] <color>`, with the color
/// allowed first or last as in CSS.
fn parse_shadow(property: &str, value: &str) -> Result<Option<Shadow>> {
    if value == "none" {
        return Ok(None);
    }
    let tokens: Vec<&str> = value.split_whitespace().collect();
    let (color, lengths) = match tokens.as_slice() {
        [first, rest @ ..] if first.starts_with('#') => (*first, rest),
        [rest @ .., last] if last.starts_with('#') => (*last, rest),
        _ => return Err(invalid_value(property, value)),
    };
    let color = parse_color(property, color)?;
    let length = |token: &str| parse_px(token).ok_or_else(|| invalid_value(property, value));
    let (offset_x, offset_y, blur) = match lengths {
        [x, y] => (length(x)?, length(y)?, 0),
        [x, y, blur] => (
            length(x)?,
            length(y)?,
            parse_nonnegative_px(property, blur)?,
        ),
        _ => return Err(invalid_value(property, value)),
    };
    Ok(Some(Shadow {
        offset_x,
        offset_y,
        blur,
        color,
    }))
}

fn parse_color(property: &str, value: &str) -> Result<Color> {
    Color::from_hex(value).ok_or_else(|| invalid_value(property, value))
}

#[cfg(test)]
#[path = "style_tests.rs"]
mod tests;
