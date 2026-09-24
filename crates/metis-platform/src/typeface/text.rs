//! Horizontal text runs in the embedded faces.

use super::Typeface;
use super::faces;
use super::glyf::Transform;
use super::raster::{Canvas, Outline};
use crate::framebuffer::{Color, Framebuffer, SourceOver};

/// Glyph stroke weight, selecting the embedded face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlyphWeight {
    /// The regular face.
    #[default]
    Regular,
    /// The bold face.
    Bold,
}

impl GlyphWeight {
    /// The embedded face for this weight.
    #[must_use]
    pub fn face(self) -> &'static Typeface<'static> {
        match self {
            Self::Regular => faces::regular(),
            Self::Bold => faces::bold(),
        }
    }
}

/// Text size in device pixels per em, finite and within
/// `(0, TextSize::MAX]`.
///
/// # Examples
///
/// ```
/// use metis_platform::typeface::TextSize;
///
/// assert_eq!(TextSize::new(14.0).map(TextSize::pixels), Some(14.0));
/// assert!(TextSize::new(0.0).is_none());
/// assert!(TextSize::new(f64::NAN).is_none());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct TextSize(f64);

impl TextSize {
    /// Largest size in device pixels. A glyph's coverage buffers span its
    /// bounding box, about one million pixels for the largest glyph of the
    /// embedded faces at this size.
    pub const MAX: f64 = 1024.0;

    /// Validates a size, or `None` when it is not a positive finite number
    /// of pixels at most [`Self::MAX`].
    #[must_use]
    pub fn new(pixels: f64) -> Option<Self> {
        (pixels.is_finite() && pixels > 0.0 && pixels <= Self::MAX).then_some(Self(pixels))
    }

    /// Pixels per em.
    #[must_use]
    pub const fn pixels(self) -> f64 {
        self.0
    }
}

/// Color, size and weight of one text run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    /// Straight RGBA text color.
    pub color: Color,
    /// Size in device pixels per em.
    pub size: TextSize,
    /// Face weight.
    pub weight: GlyphWeight,
}

impl TextStyle {
    /// A regular-weight style.
    #[must_use]
    pub const fn new(color: Color, size: TextSize) -> Self {
        Self {
            color,
            size,
            weight: GlyphWeight::Regular,
        }
    }

    /// The same style at `weight`.
    #[must_use]
    pub const fn with_weight(self, weight: GlyphWeight) -> Self {
        Self { weight, ..self }
    }

    /// Device pixels per font unit.
    fn scale(self, face: &Typeface<'_>) -> f64 {
        self.size.pixels() / f64::from(face.units_per_em())
    }

    /// Width of `text` in device pixels: the sum of its glyph advances.
    /// Newlines have no advance.
    #[must_use]
    pub fn advance(self, text: &str) -> f64 {
        let face = self.weight.face();
        let units: f64 = text
            .chars()
            .filter(|character| *character != '\n')
            .map(|character| f64::from(face.advance(face.glyph(character))))
            .sum();
        units * self.scale(face)
    }

    /// Height of one line box in device pixels.
    #[must_use]
    pub fn line_height(self) -> f64 {
        let face = self.weight.face();
        f64::from(face.line_height()) * self.scale(face)
    }
}

/// Renders one horizontal text run whose line box has its top-left corner at
/// `(x, y)`; newline characters have no advance.
///
/// Glyphs are placed at fractional pen positions and antialiased by exact
/// area coverage, so a run measures what [`TextStyle::advance`] reports.
/// Characters the face lacks draw its missing-glyph box.
///
/// # Panics
///
/// Does not panic: every glyph of the embedded faces decodes, as the face
/// tests prove, and drawing clips to the surface.
pub fn draw_text(fb: &mut Framebuffer, x: i32, y: i32, text: &str, style: TextStyle) {
    let color = style.color;
    if SourceOver::new(color).is_transparent() {
        return;
    }
    let face = style.weight.face();
    let scale = style.scale(face);
    let baseline = f64::from(face.ascender()).mul_add(scale, f64::from(y));
    let surface_width = f64::from(fb.width());
    let mut pen = f64::from(x);
    let mut outline = Outline::default();
    let mut canvas = Canvas::default();
    // No glyph reaches further left of its pen than the face's bounding box,
    // so once that edge is past the surface nothing later can be visible.
    let overhang = f64::from(face.min_x()) * scale;
    for character in text.chars().filter(|character| *character != '\n') {
        if pen + overhang >= surface_width {
            break;
        }
        let glyph = face.glyph(character);
        let advance = f64::from(face.advance(glyph)) * scale;
        outline.clear();
        face.outline(
            glyph,
            &Transform::device(scale, pen, baseline),
            &mut outline,
        )
        .expect("invariant: every glyph of the embedded faces decodes");
        pen += advance;
        let Some(bounds) = outline.bounds() else {
            continue;
        };
        let visible = |start: i32, end: i32, limit: u32| {
            i64::from(end) > 0 && i64::from(start) < i64::from(limit)
        };
        if !visible(bounds.left, bounds.right, fb.width())
            || !visible(bounds.top, bounds.bottom, fb.height())
        {
            continue;
        }
        outline.rasterize(bounds, &mut canvas);
        composite(fb, &canvas.coverage, bounds, color);
    }
}

/// Composites a coverage bitmap at `bounds`, clipped to the surface.
fn composite(
    fb: &mut Framebuffer,
    coverage: &[f64],
    bounds: super::raster::PixelBounds,
    color: Color,
) {
    let width = bounds.width();
    for (row, values) in coverage.chunks_exact(width).enumerate() {
        let Some(y) = surface_coordinate(bounds.top, row, fb.height()) else {
            continue;
        };
        for (column, value) in values.iter().enumerate() {
            if *value <= 0.0 {
                continue;
            }
            let Some(x) = surface_coordinate(bounds.left, column, fb.width()) else {
                continue;
            };
            let source = SourceOver::covering(color, *value);
            if !source.is_transparent() {
                fb.composite_span(y, x, x + 1, source);
            }
        }
    }
}

/// The surface coordinate `origin + index`, if it lies on a surface of
/// `limit` pixels.
fn surface_coordinate(origin: i32, index: usize, limit: u32) -> Option<u32> {
    let coordinate = i64::from(origin) + i64::try_from(index).ok()?;
    u32::try_from(coordinate)
        .ok()
        .filter(|value| *value < limit)
}
