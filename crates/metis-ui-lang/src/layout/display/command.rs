//! Fixed-size paint primitives in painter order.

use crate::image::ImagePlacement;
use crate::parser::limit_error;
use crate::style::{Color, LinearGradient};
use crate::transition::TransitionProperty;
use metis_core::error::Result;
use metis_platform::framebuffer::Rect;
use metis_platform::rasterizer::{BoxShadow, CornerRadius, LineCap, LineJoin, StrokeWidth};
use metis_platform::typeface::TextStyle;
use std::sync::Arc;

/// Primitive command in painter order.
///
/// Identifiers, text, gradients, polylines and image placements are shared
/// handles, so cloning a command copies a fixed-size value and requests no
/// heap memory; a display list is cloned to reuse a layout that did not
/// change. A const assertion bounds the size of the command.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DisplayCommand {
    /// Element rectangle metadata used by host hit testing.
    ElementRect {
        /// Authored element identifier.
        id: Arc<str>,
        /// Laid-out border rectangle in framebuffer coordinates.
        rect: Rect,
    },
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
    /// Linear gradient across a rectangle, painted over its background color.
    FillGradient {
        /// Target rectangle, which is also the gradient box.
        rect: Rect,
        /// Corner rounding; [`CornerRadius::SQUARE`] keeps square corners.
        radius: CornerRadius,
        /// Direction and color stops.
        gradient: Arc<LinearGradient>,
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
        points: Arc<[(i32, i32)]>,
        /// Positive device-space stroke width.
        width: StrokeWidth,
        /// Endpoint treatment.
        cap: LineCap,
        /// Interior vertex treatment.
        join: LineJoin,
        /// Straight RGBA stroke color.
        color: Color,
    },
    /// Single horizontal antialiased text run.
    DrawText {
        /// Unicode text; characters the face lacks display as its
        /// missing-glyph box.
        text: Arc<str>,
        /// Left edge of the line box.
        x: i32,
        /// Top edge of the line box.
        y: i32,
        /// Color, device-pixel size and weight.
        style: TextStyle,
    },
    /// Raster image crop composited with source-over alpha.
    DrawImage {
        /// Validated source and destination placement.
        placement: Arc<ImagePlacement>,
    },
}

const _: () = assert!(size_of::<DisplayCommand>() <= 56);

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
            Self::ElementRect { rect, .. }
            | Self::DrawShadow { rect, .. }
            | Self::FillRect { rect, .. }
            | Self::FillGradient { rect, .. }
            | Self::DrawBorder { rect, .. } => shift_rect(rect),
            Self::DrawLine { start, end, .. } => {
                *start = (shift(start.0, dx)?, shift(start.1, dy)?);
                *end = (shift(end.0, dx)?, shift(end.1, dy)?);
                Ok(())
            }
            Self::DrawPolyline { points, .. } => {
                *points = points
                    .iter()
                    .map(|point| Ok((shift(point.0, dx)?, shift(point.1, dy)?)))
                    .collect::<Result<_>>()?;
                Ok(())
            }
            Self::DrawText { x, y, .. } => {
                *x = shift(*x, dx)?;
                *y = shift(*y, dy)?;
                Ok(())
            }
            Self::DrawImage { placement } => Arc::make_mut(placement).translate(dx, dy),
        }
    }

    /// The color this command paints for `property`, when it paints one.
    pub(super) fn animated_color(&self, property: TransitionProperty) -> Option<Color> {
        match (self, property) {
            (Self::DrawText { style, .. }, TransitionProperty::Color) => Some(style.color),
            (Self::DrawBorder { color, .. }, TransitionProperty::BorderColor) => Some(*color),
            _ => None,
        }
    }

    /// The same color as a write target.
    pub(super) fn animated_color_mut(
        &mut self,
        property: TransitionProperty,
    ) -> Option<&mut Color> {
        match (self, property) {
            (Self::DrawText { style, .. }, TransitionProperty::Color) => Some(&mut style.color),
            (Self::DrawBorder { color, .. }, TransitionProperty::BorderColor) => Some(color),
            _ => None,
        }
    }
}
