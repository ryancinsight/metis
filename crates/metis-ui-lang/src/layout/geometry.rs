use super::display::{DisplayCommand, DisplayList};
use crate::dom::{DomDocument, DomElement, DomNode};
use crate::parser::{MAX_DEPTH, MAX_INPUT_BYTES, MAX_NODES, copy_text, limit_error};
use crate::style::{ComputedStyle, Display, EdgeValues, FlexDirection, Size};
use metis_core::error::Result;
use metis_platform::DisplayScale;
use metis_platform::framebuffer::Rect;

/// Logical viewport dimensions and the host's device-pixel scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutViewport {
    width: i32,
    height: i32,
    scale: DisplayScale,
}

impl LayoutViewport {
    /// Creates a viewport at the default 96-DPI scale.
    #[must_use]
    pub const fn new(width: i32, height: i32) -> Self {
        Self::with_scale(width, height, DisplayScale::ONE)
    }

    /// Creates a viewport with an explicit validated device scale.
    #[must_use]
    pub const fn with_scale(width: i32, height: i32, scale: DisplayScale) -> Self {
        Self {
            width,
            height,
            scale,
        }
    }

    /// Horizontal physical pixel count supplied by the host.
    #[must_use]
    pub const fn width(self) -> i32 {
        self.width
    }

    /// Vertical physical pixel count supplied by the host.
    #[must_use]
    pub const fn height(self) -> i32 {
        self.height
    }

    /// Device scale used to map authored dimensions to physical pixels.
    #[must_use]
    pub const fn scale(self) -> DisplayScale {
        self.scale
    }
}

/// Computes a bounded display list for a nonnegative viewport.
///
/// Programmatic DOMs have the same depth, node, and text-byte limits as parsed DOMs.
/// Integer overflow is rejected rather than wrapped; percentage sizes round toward zero.
///
/// # Errors
/// Rejects negative viewport or explicit dimensions, non-finite percentages,
/// coordinate overflow, excessive tree resources, or failed allocations.
pub fn compute_layout(doc: &DomDocument, viewport: LayoutViewport) -> Result<DisplayList> {
    if viewport.width < 0 || viewport.height < 0 {
        return Err(limit_error("Viewport dimensions must be nonnegative"));
    }
    let mut remaining_nodes = MAX_NODES;
    let mut remaining_bytes = MAX_INPUT_BYTES;
    validate(&doc.root, 1, &mut remaining_nodes, &mut remaining_bytes)?;
    let mut list = DisplayList::default();
    list.element(
        &doc.root,
        Rect::new(0, 0, viewport.width, viewport.height),
        viewport.scale,
    )?;
    Ok(list)
}

fn validate(
    element: &DomElement,
    depth: usize,
    nodes: &mut usize,
    bytes: &mut usize,
) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(limit_error("Layout nesting limit exceeded"));
    }
    *nodes = nodes
        .checked_sub(1)
        .ok_or_else(|| limit_error("Layout node limit exceeded"))?;
    for child in &element.children {
        match child {
            DomNode::Element(child) => validate(child, depth + 1, nodes, bytes)?,
            DomNode::Text(text) => {
                *nodes = nodes
                    .checked_sub(1)
                    .ok_or_else(|| limit_error("Layout node limit exceeded"))?;
                *bytes = bytes
                    .checked_sub(text.len())
                    .ok_or_else(|| limit_error("Layout text byte limit exceeded"))?;
            }
        }
    }
    Ok(())
}

impl DisplayList {
    fn text(
        &mut self,
        text: &str,
        style: &crate::style::ComputedStyle,
        x: i32,
        y: i32,
        display_scale: DisplayScale,
    ) -> Result<(i32, i32)> {
        let scale = (style.font_size / 14).max(1);
        let effective_scale = display_scale.multiply(scale)?;
        let count = i32::try_from(text.chars().filter(|c| *c != '\n').count())
            .map_err(|_| limit_error("Text length exceeds coordinate range"))?;
        let text_width = effective_scale.scale_extent(mul(count, 8)?)?;
        let text_height = effective_scale.scale_extent(16)?;
        self.push(DisplayCommand::DrawText {
            text: copy_text(text)?,
            x,
            y,
            color: style.text_color,
            scale,
            display_scale,
        })?;
        Ok((text_width, text_height))
    }

    fn children_extent(&mut self, element: &DomElement, layout: ChildLayout<'_>) -> Result<i32> {
        let ChildLayout {
            style,
            content_x,
            content_y,
            content_width,
            available_height,
            row,
            gap,
            display_scale,
        } = layout;
        let mut child_x = content_x;
        let mut child_y = content_y;
        let mut cross_size = 0;
        let mut visible_children = 0;
        for child in &element.children {
            if matches!(child, DomNode::Element(child) if child.computed_style.display == Display::None)
            {
                continue;
            }
            if visible_children > 0 {
                if row {
                    child_x = add(child_x, gap)?;
                } else {
                    child_y = add(child_y, gap)?;
                }
            }
            let (child_width, child_height) = match child {
                DomNode::Element(child) => {
                    let available_width = if row {
                        sub(content_width, sub(child_x, content_x)?)?.max(0)
                    } else {
                        content_width
                    };
                    let child_rect = self.element(
                        child,
                        Rect::new(child_x, child_y, available_width, available_height),
                        display_scale,
                    )?;
                    (child_rect.width, child_rect.height)
                }
                DomNode::Text(text) => self.text(text, style, child_x, child_y, display_scale)?,
            };
            if row {
                child_x = add(child_x, child_width)?;
                cross_size = cross_size.max(child_height);
            } else {
                child_y = add(child_y, child_height)?;
                cross_size = cross_size.max(child_width);
            }
            visible_children += 1;
        }
        let content_height = if row {
            cross_size
        } else {
            sub(child_y, content_y)?
        };
        Ok(content_height)
    }

    fn element(
        &mut self,
        element: &DomElement,
        available: Rect,
        display_scale: DisplayScale,
    ) -> Result<Rect> {
        let style = &element.computed_style;
        style.validate_renderer_support()?;
        if style.display == Display::None {
            return Ok(Rect::new(available.x, available.y, 0, 0));
        }
        let geometry = scaled_geometry(style, display_scale)?;
        let width = dimension(
            style.width,
            available.width,
            sub(
                available.width,
                add(geometry.margin.left, geometry.margin.right)?,
            )?
            .max(0),
            display_scale,
        )?;
        let x = add(available.x, geometry.margin.left)?;
        let y = add(available.y, geometry.margin.top)?;
        let content_x = add(add(x, geometry.padding.left)?, geometry.border.left)?;
        let content_y = add(add(y, geometry.padding.top)?, geometry.border.top)?;
        let horizontal_edges = add(
            add(geometry.padding.left, geometry.padding.right)?,
            add(geometry.border.left, geometry.border.right)?,
        )?;
        let content_width = sub(width, horizontal_edges)?.max(0);
        let row = style.flex_direction == FlexDirection::Row;
        // Reserve the parent's painter position before descendants; its auto height is
        // filled after child layout, so siblings never paint behind earlier siblings.
        let background_index = if let Some(color) = style.background_color {
            let index = self.commands.len();
            self.push(DisplayCommand::FillRect {
                rect: Rect::new(x, y, width, 0),
                color,
            })?;
            Some(index)
        } else {
            None
        };
        let content_height = self.children_extent(
            element,
            ChildLayout {
                style,
                content_x,
                content_y,
                content_width,
                available_height: available.height,
                row,
                gap: geometry.gap,
                display_scale,
            },
        )?;
        let vertical_edges = add(
            add(geometry.padding.top, geometry.padding.bottom)?,
            add(geometry.border.top, geometry.border.bottom)?,
        )?;
        let height = dimension(
            style.height,
            available.height,
            add(content_height, vertical_edges)?.max(0),
            display_scale,
        )?;
        let rect = Rect::new(x, y, width, height);
        if let Some(index) = background_index
            && let DisplayCommand::FillRect { rect: target, .. } = &mut self.commands[index]
        {
            *target = rect;
        }
        if geometry.border.top > 0 {
            self.push(DisplayCommand::DrawBorder {
                rect,
                width: geometry.border.top,
                color: style.border_color,
            })?;
        }
        Ok(rect)
    }
}

#[derive(Clone, Copy)]
struct ScaledGeometry {
    margin: EdgeValues,
    padding: EdgeValues,
    border: EdgeValues,
    gap: i32,
}

#[derive(Clone, Copy)]
struct ChildLayout<'style> {
    style: &'style ComputedStyle,
    content_x: i32,
    content_y: i32,
    content_width: i32,
    available_height: i32,
    row: bool,
    gap: i32,
    display_scale: DisplayScale,
}

fn scaled_geometry(
    style: &crate::style::ComputedStyle,
    display_scale: DisplayScale,
) -> Result<ScaledGeometry> {
    Ok(ScaledGeometry {
        margin: scale_edges(style.margin, display_scale)?,
        padding: scale_edges(style.padding, display_scale)?,
        border: scale_edges(style.border_width, display_scale)?,
        gap: display_scale.scale_coordinate(style.gap)?,
    })
}

fn scale_edges(edges: EdgeValues, display_scale: DisplayScale) -> Result<EdgeValues> {
    Ok(EdgeValues {
        top: display_scale.scale_coordinate(edges.top)?,
        right: display_scale.scale_coordinate(edges.right)?,
        bottom: display_scale.scale_coordinate(edges.bottom)?,
        left: display_scale.scale_coordinate(edges.left)?,
    })
}

fn add(left: i32, right: i32) -> Result<i32> {
    left.checked_add(right)
        .ok_or_else(|| limit_error("Layout coordinate addition overflow"))
}
fn sub(left: i32, right: i32) -> Result<i32> {
    left.checked_sub(right)
        .ok_or_else(|| limit_error("Layout coordinate subtraction overflow"))
}
fn mul(left: i32, right: i32) -> Result<i32> {
    left.checked_mul(right)
        .ok_or_else(|| limit_error("Layout text extent overflow"))
}

fn dimension(
    size: Size,
    available: i32,
    automatic: i32,
    display_scale: DisplayScale,
) -> Result<i32> {
    match size {
        Size::Auto => Ok(automatic),
        Size::Px(value) if value >= 0 => display_scale.scale_extent(value),
        Size::Percent(percent) if percent.is_finite() && percent >= 0.0 => {
            // The percentage contract is f32; multiplication remains in that precision.
            #[expect(
                clippy::cast_precision_loss,
                reason = "CSS percentage sizing uses f32 coordinates"
            )]
            let value = available as f32 * percent;
            #[expect(
                clippy::cast_precision_loss,
                reason = "i32::MAX rounds to the first excluded f32 coordinate"
            )]
            if !value.is_finite() || value >= i32::MAX as f32 {
                return Err(limit_error("Percentage size exceeds coordinate range"));
            }
            #[expect(
                clippy::cast_possible_truncation,
                reason = "Finite nonnegative value is checked below the i32 upper bound; fractional pixels truncate"
            )]
            Ok(value as i32)
        }
        _ => Err(limit_error("Size must be finite and nonnegative")),
    }
}
