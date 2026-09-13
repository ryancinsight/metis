//! Sequential row/column box layout and display-list generation.
//!
//! Supports explicit or automatic sizes, margins, padding, backgrounds, text, and
//! uniform square borders. Unsupported browser layout declarations are rejected
//! before a display list is emitted so programmatic DOMs cannot silently diverge.

use crate::dom::{DomDocument, DomElement, DomNode};
use crate::image::ImagePlacement;
use crate::parser::{MAX_DEPTH, MAX_INPUT_BYTES, MAX_NODES, copy_text, limit_error};
use crate::style::{Color, Display, FlexDirection, Size};
use metis_core::error::Result;
use metis_platform::framebuffer::Framebuffer;
pub use metis_platform::framebuffer::Rect;
pub use metis_platform::rasterizer::{LineCap, LineJoin, StrokeWidth};
use metis_platform::rasterizer::{
    MAX_STROKE_POINTS, draw_line, draw_polyline, draw_rect_outline, draw_text, fill_rect,
};

/// Primitive command in painter order.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DisplayCommand {
    /// Rectangle fill.
    FillRect {
        /// Target rectangle.
        rect: Rect,
        /// Straight RGBA color.
        color: Color,
    },
    /// Uniform inward square border.
    DrawBorder {
        /// Outer rectangle.
        rect: Rect,
        /// Border width.
        width: i32,
        /// Straight RGBA color.
        color: Color,
    },
    /// One-pixel line segment clipped to the framebuffer.
    DrawLine {
        /// Inclusive start coordinate.
        start: (i32, i32),
        /// Inclusive end coordinate.
        end: (i32, i32),
        /// Straight RGBA stroke color.
        color: Color,
    },
    /// Bounded multi-segment stroke with explicit width, caps and joins.
    DrawPolyline {
        /// Integer-coordinate path vertices in painter order.
        points: Vec<(i32, i32)>,
        /// Positive device-space stroke width.
        width: StrokeWidth,
        /// Endpoint treatment.
        cap: LineCap,
        /// Interior vertex treatment.
        join: LineJoin,
        /// Straight RGBA stroke color.
        color: Color,
    },
    /// Single horizontal bitmap text run.
    DrawText {
        /// Unicode text; unsupported glyphs display as a box.
        text: String,
        /// Horizontal origin.
        x: i32,
        /// Vertical origin.
        y: i32,
        /// Straight RGBA color.
        color: Color,
        /// Integer bitmap scale.
        scale: u32,
    },
    /// Raster image crop composited with source-over alpha.
    DrawImage {
        /// Validated source and destination placement.
        placement: ImagePlacement,
    },
}

/// Drawing commands emitted by bounded layout.
#[derive(Debug, Clone, Default)]
pub struct DisplayList {
    /// Commands in painter order.
    pub commands: Vec<DisplayCommand>,
}

impl DisplayList {
    /// Executes the display list with framebuffer clipping.
    pub fn render_to(&self, fb: &mut Framebuffer) {
        for command in &self.commands {
            match command {
                DisplayCommand::FillRect { rect, color } => fill_rect(fb, *rect, *color),
                DisplayCommand::DrawBorder { rect, width, color } => {
                    draw_rect_outline(fb, *rect, *width, *color);
                }
                DisplayCommand::DrawLine { start, end, color } => {
                    draw_line(fb, *start, *end, *color);
                }
                DisplayCommand::DrawPolyline {
                    points,
                    width,
                    cap,
                    join,
                    color,
                } => draw_polyline(fb, points, *width, *cap, *join, *color),
                DisplayCommand::DrawText {
                    text,
                    x,
                    y,
                    color,
                    scale,
                } => draw_text(fb, *x, *y, text, *color, *scale),
                DisplayCommand::DrawImage { placement } => placement.render_to(fb),
            }
        }
    }

    /// Appends a validated image command in painter order.
    ///
    /// # Errors
    /// Returns [`metis_core::error::ErrorCode::LayoutOverflow`] when the
    /// display command storage cannot grow.
    pub fn append_image(&mut self, placement: ImagePlacement) -> Result<()> {
        self.push(DisplayCommand::DrawImage { placement })
    }

    /// Appends a clipped one-pixel line command in painter order.
    ///
    /// Endpoints may be outside the target surface; the renderer clips them
    /// before traversing the segment.
    ///
    /// # Errors
    ///
    /// Returns [`metis_core::error::ErrorCode::LayoutOverflow`] when the
    /// display command storage cannot grow.
    pub fn append_line(&mut self, start: (i32, i32), end: (i32, i32), color: Color) -> Result<()> {
        self.push(DisplayCommand::DrawLine { start, end, color })
    }

    /// Appends a bounded polyline stroke in painter order.
    ///
    /// At most [`MAX_STROKE_POINTS`] vertices are retained. The points are
    /// copied once into the display list so callers may release their input
    /// after this method returns.
    ///
    /// # Errors
    ///
    /// Returns [`metis_core::error::ErrorCode::LayoutOverflow`] for an empty
    /// or oversized path or a failed bounded allocation. Construct the
    /// [`StrokeWidth`] before calling this method to reject zero.
    pub fn append_polyline(
        &mut self,
        points: &[(i32, i32)],
        width: StrokeWidth,
        cap: LineCap,
        join: LineJoin,
        color: Color,
    ) -> Result<()> {
        if points.is_empty() || points.len() > MAX_STROKE_POINTS {
            return Err(limit_error("Polyline point limit exceeded"));
        }
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(points.len())
            .map_err(|_| limit_error("Polyline point allocation failed"))?;
        owned.extend_from_slice(points);
        self.push(DisplayCommand::DrawPolyline {
            points: owned,
            width,
            cap,
            join,
            color,
        })
    }

    fn push(&mut self, command: DisplayCommand) -> Result<()> {
        self.commands
            .try_reserve(1)
            .map_err(|_| limit_error("Display command allocation failed"))?;
        self.commands.push(command);
        Ok(())
    }
}

impl iris::render::RenderBackend<DisplayList> for Framebuffer {
    type Error = std::convert::Infallible;
    type Frame<'frame> = &'frame [u32];

    fn render<'frame>(
        &'frame mut self,
        view: &DisplayList,
    ) -> std::result::Result<Self::Frame<'frame>, Self::Error> {
        view.render_to(self);
        Ok(self.pixels())
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
pub fn compute_layout(doc: &DomDocument, viewport_w: i32, viewport_h: i32) -> Result<DisplayList> {
    if viewport_w < 0 || viewport_h < 0 {
        return Err(limit_error("Viewport dimensions must be nonnegative"));
    }
    let mut remaining_nodes = MAX_NODES;
    let mut remaining_bytes = MAX_INPUT_BYTES;
    validate(&doc.root, 1, &mut remaining_nodes, &mut remaining_bytes)?;
    let mut list = DisplayList::default();
    list.element(&doc.root, Rect::new(0, 0, viewport_w, viewport_h))?;
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
    ) -> Result<(i32, i32)> {
        let scale = (style.font_size / 14).max(1);
        let scale_pixels =
            i32::try_from(scale).map_err(|_| limit_error("Font scale exceeds coordinate range"))?;
        let count = i32::try_from(text.chars().filter(|c| *c != '\n').count())
            .map_err(|_| limit_error("Text length exceeds coordinate range"))?;
        let text_width = mul(mul(count, 8)?, scale_pixels)?;
        let text_height = mul(16, scale_pixels)?;
        self.push(DisplayCommand::DrawText {
            text: copy_text(text)?,
            x,
            y,
            color: style.text_color,
            scale,
        })?;
        Ok((text_width, text_height))
    }

    fn element(&mut self, element: &DomElement, available: Rect) -> Result<Rect> {
        let style = &element.computed_style;
        style.validate_renderer_support()?;
        if style.display == Display::None {
            return Ok(Rect::new(available.x, available.y, 0, 0));
        }
        let width = dimension(
            style.width,
            available.width,
            sub(available.width, add(style.margin.left, style.margin.right)?)?.max(0),
        )?;
        let x = add(available.x, style.margin.left)?;
        let y = add(available.y, style.margin.top)?;
        let content_x = add(add(x, style.padding.left)?, style.border_width.left)?;
        let content_y = add(add(y, style.padding.top)?, style.border_width.top)?;
        let horizontal_edges = add(
            add(style.padding.left, style.padding.right)?,
            add(style.border_width.left, style.border_width.right)?,
        )?;
        let content_width = sub(width, horizontal_edges)?.max(0);
        let row = style.flex_direction == FlexDirection::Row;
        let mut child_x = content_x;
        let mut child_y = content_y;
        let mut cross_size = 0;
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
        let mut visible_children = 0;
        for child in &element.children {
            if matches!(child, DomNode::Element(child) if child.computed_style.display == Display::None)
            {
                continue;
            }
            if visible_children > 0 {
                if row {
                    child_x = add(child_x, style.gap)?;
                } else {
                    child_y = add(child_y, style.gap)?;
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
                        Rect::new(child_x, child_y, available_width, available.height),
                    )?;
                    (child_rect.width, child_rect.height)
                }
                DomNode::Text(text) => self.text(text, style, child_x, child_y)?,
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
        let vertical_edges = add(
            add(style.padding.top, style.padding.bottom)?,
            add(style.border_width.top, style.border_width.bottom)?,
        )?;
        let height = dimension(
            style.height,
            available.height,
            add(content_height, vertical_edges)?.max(0),
        )?;
        let rect = Rect::new(x, y, width, height);
        if let Some(index) = background_index
            && let DisplayCommand::FillRect { rect: target, .. } = &mut self.commands[index]
        {
            *target = rect;
        }
        if style.border_width.top > 0 {
            self.push(DisplayCommand::DrawBorder {
                rect,
                width: style.border_width.top,
                color: style.border_color,
            })?;
        }
        Ok(rect)
    }
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

fn dimension(size: Size, available: i32, automatic: i32) -> Result<i32> {
    match size {
        Size::Auto => Ok(automatic),
        Size::Px(value) if value >= 0 => Ok(value),
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

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
