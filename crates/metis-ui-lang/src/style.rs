//! CSS-inspired style declarations, box model, and layout properties.

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
    /// Unknown properties and malformed declaration syntax are ignored. Invalid
    /// dimensions resolve to `Auto`; malformed edge lists resolve to zero edges.
    /// Coordinate limits and finite percentages are checked by layout.
    #[must_use]
    pub fn parse(css: &str) -> Self {
        let mut style = Self::default();
        for declaration in css.split(';') {
            let part = declaration.trim();
            if part.is_empty() {
                continue;
            }
            let Some((key, val)) = part.split_once(':') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let val = val.trim();

            match key.as_str() {
                "display" => match val {
                    "flex" => style.display = Display::Flex,
                    "block" => style.display = Display::Block,
                    "inline" => style.display = Display::Inline,
                    "none" => style.display = Display::None,
                    _ => {}
                },
                "flex-direction" => match val {
                    "row" => style.flex_direction = FlexDirection::Row,
                    "column" => style.flex_direction = FlexDirection::Column,
                    _ => {}
                },
                "justify-content" => match val {
                    "flex-start" | "start" => style.justify_content = JustifyContent::FlexStart,
                    "center" => style.justify_content = JustifyContent::Center,
                    "flex-end" | "end" => style.justify_content = JustifyContent::FlexEnd,
                    "space-between" => style.justify_content = JustifyContent::SpaceBetween,
                    _ => {}
                },
                "align-items" => match val {
                    "flex-start" | "start" => style.align_items = AlignItems::FlexStart,
                    "center" => style.align_items = AlignItems::Center,
                    "flex-end" | "end" => style.align_items = AlignItems::FlexEnd,
                    "stretch" => style.align_items = AlignItems::Stretch,
                    _ => {}
                },
                "gap" => {
                    if let Some(px) = parse_px(val) {
                        style.gap = px;
                    }
                }
                "width" => {
                    style.width = parse_size(val);
                }
                "height" => {
                    style.height = parse_size(val);
                }
                "min-width" => {
                    style.min_width = parse_size(val);
                }
                "min-height" => {
                    style.min_height = parse_size(val);
                }
                "padding" => {
                    style.padding = parse_edges(val);
                }
                "margin" => {
                    style.margin = parse_edges(val);
                }
                "border-width" => {
                    style.border_width = parse_edges(val);
                }
                "border-color" => {
                    if let Some(c) = Color::from_hex(val) {
                        style.border_color = c;
                    }
                }
                "border-radius" => {
                    if let Some(px) = parse_px(val) {
                        style.border_radius = px;
                    }
                }
                "background-color" | "background" => {
                    if let Some(c) = Color::from_hex(val) {
                        style.background_color = Some(c);
                    }
                }
                "color" => {
                    if let Some(c) = Color::from_hex(val) {
                        style.text_color = c;
                    }
                }
                "font-size" => {
                    if let Some(px) = parse_px(val) {
                        style.font_size = px.max(8).unsigned_abs();
                    }
                }
                "font-weight" => match val {
                    "bold" | "700" => style.font_weight = FontWeight::Bold,
                    _ => style.font_weight = FontWeight::Normal,
                },
                _ => {}
            }
        }
        style
    }
}

fn parse_px(s: &str) -> Option<i32> {
    let s = s.trim().trim_end_matches("px");
    s.parse::<i32>().ok()
}

fn parse_size(s: &str) -> Size {
    let s = s.trim();
    if s == "auto" {
        return Size::Auto;
    }
    if let Some(pct) = s.strip_suffix('%')
        && let Ok(val) = pct.trim().parse::<f32>()
    {
        return Size::Percent(val / 100.0);
    }
    if let Some(px) = parse_px(s) {
        return Size::Px(px);
    }
    Size::Auto
}

fn parse_edges(s: &str) -> EdgeValues {
    let mut parts = s.split_whitespace();
    let Some(top) = parts.next().and_then(parse_px) else {
        return EdgeValues::default();
    };
    let Some(right_text) = parts.next() else {
        return EdgeValues::all(top);
    };
    let Some(right) = parse_px(right_text) else {
        return EdgeValues::default();
    };
    let Some(bottom_text) = parts.next() else {
        return EdgeValues::symmetric(top, right);
    };
    let Some(bottom) = parse_px(bottom_text) else {
        return EdgeValues::default();
    };
    let Some(left_text) = parts.next() else {
        return EdgeValues {
            top,
            right,
            bottom,
            left: right,
        };
    };
    let Some(left) = parse_px(left_text) else {
        return EdgeValues::default();
    };
    if parts.next().is_some() {
        return EdgeValues::default();
    }
    EdgeValues {
        top,
        right,
        bottom,
        left,
    }
}
