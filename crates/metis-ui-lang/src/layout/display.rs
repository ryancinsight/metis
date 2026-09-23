use crate::image::ImagePlacement;
use crate::parser::limit_error;
use crate::style::Color;
use metis_core::error::Result;
use metis_platform::DisplayScale;
use metis_platform::framebuffer::{Framebuffer, Rect};
use metis_platform::rasterizer::{
    BoxShadow, CornerRadius, LineCap, LineJoin, MAX_STROKE_POINTS, StrokeWidth, draw_box_shadow,
    draw_line, draw_polyline, draw_rect_outline, draw_text, fill_rect,
};
use metis_platform::{GlyphWeight, TextStyle};

/// Primitive command in painter order.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DisplayCommand {
    /// Blurred outer shadow of a border box, clipped inside that box.
    DrawShadow {
        /// Border box casting the shadow.
        rect: Rect,
        /// Corner rounding shared with the border box.
        radius: CornerRadius,
        /// Offsets, blur and color in device pixels.
        shadow: BoxShadow,
    },
    /// Rectangle fill.
    FillRect {
        /// Target rectangle.
        rect: Rect,
        /// Corner rounding; [`CornerRadius::SQUARE`] keeps square corners.
        radius: CornerRadius,
        /// Straight RGBA color.
        color: Color,
    },
    /// Uniform inward border following the fill it encloses.
    DrawBorder {
        /// Outer rectangle.
        rect: Rect,
        /// Border width.
        width: i32,
        /// Corner rounding; [`CornerRadius::SQUARE`] keeps square corners.
        radius: CornerRadius,
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
        /// Device scale reported by the host for this presentation.
        display_scale: DisplayScale,
        /// Stroke weight; [`GlyphWeight::Regular`] paints the authored glyph.
        weight: GlyphWeight,
    },
    /// Raster image crop composited with source-over alpha.
    DrawImage {
        /// Validated source and destination placement.
        placement: ImagePlacement,
    },
}

impl DisplayCommand {
    /// Moves every coordinate this command carries.
    ///
    /// Alignment redistributes free space after a child has already painted,
    /// so the child's commands move rather than being emitted twice. Every
    /// variant is matched without elision: a command kind added later must
    /// state how it moves, or a laid-out child would tear.
    pub(crate) fn translate(&mut self, dx: i32, dy: i32) -> Result<()> {
        let shift = |value: i32, delta: i32| {
            value
                .checked_add(delta)
                .ok_or_else(|| limit_error("Display coordinate exceeds coordinate range"))
        };
        let shift_rect = |rect: &mut Rect| -> Result<()> {
            *rect = Rect::new(
                shift(rect.x, dx)?,
                shift(rect.y, dy)?,
                rect.width,
                rect.height,
            );
            Ok(())
        };
        match self {
            Self::DrawShadow { rect, .. }
            | Self::FillRect { rect, .. }
            | Self::DrawBorder { rect, .. } => shift_rect(rect),
            Self::DrawLine { start, end, .. } => {
                *start = (shift(start.0, dx)?, shift(start.1, dy)?);
                *end = (shift(end.0, dx)?, shift(end.1, dy)?);
                Ok(())
            }
            Self::DrawPolyline { points, .. } => {
                for point in points.iter_mut() {
                    *point = (shift(point.0, dx)?, shift(point.1, dy)?);
                }
                Ok(())
            }
            Self::DrawText { x, y, .. } => {
                *x = shift(*x, dx)?;
                *y = shift(*y, dy)?;
                Ok(())
            }
            Self::DrawImage { placement } => placement.translate(dx, dy),
        }
    }
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
                DisplayCommand::DrawShadow {
                    rect,
                    radius,
                    shadow,
                } => draw_box_shadow(fb, *rect, *radius, *shadow),
                DisplayCommand::FillRect {
                    rect,
                    radius,
                    color,
                } => fill_rect(fb, *rect, *radius, *color),
                DisplayCommand::DrawBorder {
                    rect,
                    width,
                    radius,
                    color,
                } => {
                    draw_rect_outline(fb, *rect, *width, *radius, *color);
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
                    display_scale,
                    weight,
                } => draw_text(
                    fb,
                    *x,
                    *y,
                    text,
                    TextStyle::new(*color, *scale)
                        .with_display_scale(*display_scale)
                        .with_weight(*weight),
                ),
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

    pub(super) fn push(&mut self, command: DisplayCommand) -> Result<()> {
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
