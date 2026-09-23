//! Fills whose color may vary across the surface.

use crate::framebuffer::{Color, Framebuffer, SourceOver};

/// What a shape is filled with: a straight color at each pixel.
///
/// Shape rasterization hands fully covered runs to [`Paint::fill_run`] and
/// scales [`Paint::color_at`] by coverage for partially covered pixels, so a
/// solid color keeps its span path and a varying paint decides per run how
/// to write its pixels.
pub(crate) trait Paint {
    /// Whether every pixel of the paint is fully transparent.
    fn is_transparent(&self) -> bool;

    /// Composites the paint over the fully covered columns `left..right` of
    /// `row`, which the caller has clipped to the surface.
    fn fill_run(&self, fb: &mut Framebuffer, row: u32, left: u32, right: u32);

    /// The paint's straight color at the pixel `(column, row)`.
    fn color_at(&self, column: u32, row: u32) -> Color;
}

impl Paint for Color {
    fn is_transparent(&self) -> bool {
        SourceOver::new(*self).is_transparent()
    }

    fn fill_run(&self, fb: &mut Framebuffer, row: u32, left: u32, right: u32) {
        let source = SourceOver::new(*self);
        if source.is_opaque() {
            fb.row_span_mut(row, left, right).fill(source.packed());
        } else if !source.is_transparent() {
            fb.composite_span(row, left, right, source);
        }
    }

    fn color_at(&self, _column: u32, _row: u32) -> Color {
        *self
    }
}
