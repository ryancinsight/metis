//! Embedded monospace bitmap font renderer (8x16 baseline).
//!
//! Provides pure-Rust ASCII glyph rendering with zero external font files or dependencies.

use crate::DisplayScale;
use crate::framebuffer::{Color, Framebuffer};

mod glyph;
pub use glyph::get_glyph_bitmap;

/// 8x16 font baseline constants.
pub const FONT_WIDTH: u32 = 8;
/// Glyph cell height in pixels at scale one.
pub const FONT_HEIGHT: u32 = 16;

/// Presentation of one bitmap text run.
///
/// Bundling the run's presentation keeps one text entry point instead of a
/// `_scaled` sibling per dimension: the device scale and the stroke weight are
/// parameters of a run, not separate functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextStyle {
    /// Straight RGBA color.
    pub color: Color,
    /// Authored bitmap multiplier; zero is treated as one.
    pub scale: u32,
    /// Device scale reported by the host for this presentation.
    pub display_scale: DisplayScale,
    /// Stroke weight.
    pub weight: GlyphWeight,
}

impl TextStyle {
    /// A run at the host's unit device scale and regular weight.
    #[must_use]
    pub fn new(color: Color, scale: u32) -> Self {
        Self {
            color,
            scale,
            display_scale: DisplayScale::ONE,
            weight: GlyphWeight::Regular,
        }
    }

    /// Returns the run at a validated device scale.
    #[must_use]
    pub const fn with_display_scale(self, display_scale: DisplayScale) -> Self {
        Self {
            display_scale,
            ..self
        }
    }

    /// Returns the run at the given stroke weight.
    #[must_use]
    pub const fn with_weight(self, weight: GlyphWeight) -> Self {
        Self { weight, ..self }
    }
}

/// Stroke weight applied to a bitmap glyph.
///
/// Bold smears each row one column toward the trailing edge of the cell rather
/// than carrying a second glyph table. Bits pushed past the last column are
/// dropped, so a bold glyph cannot reach into the next cell; the leading column
/// stays clear in both weights, which keeps a one-pixel gap at the
/// [`FONT_WIDTH`] advance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlyphWeight {
    /// The glyph exactly as the table authors it.
    #[default]
    Regular,
    /// One column of horizontal smear, thickening every stroke.
    Bold,
}

impl GlyphWeight {
    /// Applies the weight to one glyph row.
    #[must_use]
    pub const fn apply(self, row: u8) -> u8 {
        match self {
            Self::Regular => row,
            Self::Bold => row | (row >> 1),
        }
    }
}

/// Draws a glyph with clipping before scaled pixels are traversed.
///
/// Scale zero means one. Work is bounded by the visible surface even for `u32::MAX` scale.
pub fn draw_glyph(fb: &mut Framebuffer, x: i32, y: i32, c: char, style: TextStyle) {
    let TextStyle {
        color,
        scale,
        display_scale,
        weight,
    } = style;
    let effective_milli = u64::from(scale.max(1)) * u64::from(display_scale.milli());
    draw_glyph_cells(fb, x, y, c, color, effective_milli, weight);
}

/// Draws a glyph using a validated fractional device scale.
pub(crate) fn draw_glyph_cells(
    fb: &mut Framebuffer,
    x: i32,
    y: i32,
    c: char,
    color: Color,
    effective_milli: u64,
    weight: GlyphWeight,
) {
    for (row, byte) in (0_u32..16).zip(get_glyph_bitmap(c).map(|row| weight.apply(row))) {
        if byte == 0 {
            continue;
        }
        let top = i64::from(y).saturating_add(scaled_offset(row, effective_milli));
        let bottom = i64::from(y).saturating_add(scaled_offset(row + 1, effective_milli));
        // Adjacent set bits describe one horizontal run. Scaled column offsets
        // are monotone, so the run covers exactly the cells the per-bit fills
        // covered, each pixel once.
        let mut col = 0_u32;
        while col < 8 {
            if byte & (0x80 >> col) == 0 {
                col += 1;
                continue;
            }
            let start = col;
            while col < 8 && byte & (0x80 >> col) != 0 {
                col += 1;
            }
            let left = i64::from(x).saturating_add(scaled_offset(start, effective_milli));
            let right = i64::from(x).saturating_add(scaled_offset(col, effective_milli));
            crate::rasterizer::fill_bounds(fb, left, top, right, bottom, color);
        }
    }
}

fn scaled_offset(index: u32, effective_milli: u64) -> i64 {
    let numerator = u128::from(index) * u128::from(effective_milli);
    let rounded = (numerator + 500) / 1_000;
    i64::try_from(rounded.min(u128::from(u64::MAX / 2)))
        .expect("invariant: clipped glyph offset fits i64")
}

#[cfg(test)]
#[path = "font_tests.rs"]
mod tests;
