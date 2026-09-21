use crate::image::ImagePlacement;
use crate::parser::limit_error;
use crate::style::Color;
use metis_core::error::Result;
use metis_platform::DisplayScale;
use metis_platform::framebuffer::{Framebuffer, Rect};
use metis_platform::rasterizer::{
    LineCap, LineJoin, MAX_STROKE_POINTS, StrokeWidth, draw_line, draw_polyline, draw_rect_outline,
    draw_text_scaled, fill_rect,
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
        /// Device scale reported by the host for this presentation.
        display_scale: DisplayScale,
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
                    display_scale,
                } => draw_text_scaled(fb, *x, *y, text, *color, *scale, *display_scale),
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
