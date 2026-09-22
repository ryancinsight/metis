//! Bounded RGBA software framebuffer and integer rectangle geometry.

use metis_core::error::{ErrorCode, MetisError, Result};
use std::fmt;

/// Maximum storage: 16,777,216 pixels, or 64 MiB of packed RGBA.
pub const MAX_PIXELS: usize = 16 * 1024 * 1024;

/// Straight (unpremultiplied) RGBA channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Opacity, from transparent zero to opaque 255.
    pub a: u8,
}

impl Color {
    /// Opaque black.
    pub const BLACK: Self = Self::rgba(0, 0, 0, 255);
    /// Opaque white.
    pub const WHITE: Self = Self::rgba(255, 255, 255, 255);
    /// Transparent black.
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);
    /// Red palette color.
    pub const RED: Self = Self::rgba(229, 62, 62, 255);
    /// Green palette color.
    pub const GREEN: Self = Self::rgba(56, 161, 105, 255);
    /// Blue palette color.
    pub const BLUE: Self = Self::rgba(49, 130, 206, 255);
    /// Gray palette color.
    pub const GRAY: Self = Self::rgba(113, 128, 150, 255);
    /// Light gray palette color.
    pub const LIGHT_GRAY: Self = Self::rgba(226, 232, 240, 255);
    /// Dark blue palette color.
    pub const DARK_BLUE: Self = Self::rgba(26, 54, 93, 255);

    /// Constructs explicit RGBA channels.
    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Constructs opaque RGB channels.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }

    /// Parses ASCII `#RGB`, `#RGBA`, `#RRGGBB`, or `#RRGGBBAA`.
    ///
    /// Malformed values, including non-ASCII characters, return `None`.
    #[must_use]
    pub fn from_hex(value: &str) -> Option<Self> {
        fn nibble(value: u8) -> Option<u8> {
            match value {
                b'0'..=b'9' => Some(value - b'0'),
                b'a'..=b'f' => Some(value - b'a' + 10),
                b'A'..=b'F' => Some(value - b'A' + 10),
                _ => None,
            }
        }
        fn pair(high: u8, low: u8) -> Option<u8> {
            Some(nibble(high)? * 16 + nibble(low)?)
        }
        let digits = value.trim().strip_prefix('#')?.as_bytes();
        match *digits {
            [r, g, b] => Some(Self::rgb(nibble(r)? * 17, nibble(g)? * 17, nibble(b)? * 17)),
            [r, g, b, a] => Some(Self::rgba(
                nibble(r)? * 17,
                nibble(g)? * 17,
                nibble(b)? * 17,
                nibble(a)? * 17,
            )),
            [r, rr, g, gg, b, bb] => Some(Self::rgb(pair(r, rr)?, pair(g, gg)?, pair(b, bb)?)),
            [r, rr, g, gg, b, bb, a, aa] => Some(Self::rgba(
                pair(r, rr)?,
                pair(g, gg)?,
                pair(b, bb)?,
                pair(a, aa)?,
            )),
            _ => None,
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#{:02X}{:02X}{:02X}{:02X}",
            self.r, self.g, self.b, self.a
        )
    }
}

/// Half-open integer rectangle; nonpositive dimensions describe an empty area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    /// Left coordinate.
    pub x: i32,
    /// Top coordinate.
    pub y: i32,
    /// Horizontal extent.
    pub width: i32,
    /// Vertical extent.
    pub height: i32,
}

impl Rect {
    /// Constructs a rectangle without restricting off-screen coordinates.
    #[must_use]
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Tests containment with widened integer endpoints to avoid overflow.
    #[must_use]
    pub fn contains(&self, px: i32, py: i32) -> bool {
        self.width > 0
            && self.height > 0
            && i64::from(px) >= i64::from(self.x)
            && i64::from(px) < i64::from(self.x) + i64::from(self.width)
            && i64::from(py) >= i64::from(self.y)
            && i64::from(py) < i64::from(self.y) + i64::from(self.height)
    }
}

/// Alpha denominator when the destination is opaque: `255 * 255`.
const OPAQUE_ALPHA: u32 = 255 * 255;

/// Source-over terms that depend only on the source color.
///
/// Compositing a span shares one set of these terms, so the per-source
/// multiplications leave the pixel loop while the arithmetic and its rounding
/// stay identical to compositing each pixel on its own.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceOver {
    packed: u32,
    alpha: u32,
    inverse_alpha: u32,
    numerators: [u32; 3],
}

impl SourceOver {
    /// Precomputes the terms for one straight RGBA source color.
    pub(crate) fn new(src: Color) -> Self {
        let alpha = u32::from(src.a);
        let scaled = alpha * 255;
        Self {
            packed: pack_color(src),
            alpha,
            inverse_alpha: 255 - alpha,
            numerators: [
                u32::from(src.r) * scaled,
                u32::from(src.g) * scaled,
                u32::from(src.b) * scaled,
            ],
        }
    }

    /// Reports a source that leaves the destination unchanged.
    pub(crate) const fn is_transparent(self) -> bool {
        self.alpha == 0
    }

    /// Reports a source that replaces the destination outright.
    pub(crate) const fn is_opaque(self) -> bool {
        self.alpha == 255
    }

    /// The packed replacement value for an opaque source.
    pub(crate) const fn packed(self) -> u32 {
        self.packed
    }

    /// Composites this source over one packed destination pixel.
    ///
    /// An opaque destination fixes the alpha denominator at `255 * 255`, so
    /// that case divides by a constant the compiler reduces to a multiply and
    /// shift. Both arms evaluate the same expression and agree channel for
    /// channel; the differential rasterizer test asserts it.
    pub(crate) fn apply(self, dst: u32) -> u32 {
        let dst = unpack_color(dst);
        if dst.a == 255 {
            let dest_weight = self.inverse_alpha * 255;
            let channel = |numerator: u32, dest: u8| {
                normalized_channel(
                    (numerator + u32::from(dest) * dest_weight + OPAQUE_ALPHA / 2) / OPAQUE_ALPHA,
                )
            };
            return pack_color(Color::rgba(
                channel(self.numerators[0], dst.r),
                channel(self.numerators[1], dst.g),
                channel(self.numerators[2], dst.b),
                255,
            ));
        }
        let dest_weight = u32::from(dst.a) * self.inverse_alpha;
        let alpha = self.alpha * 255 + dest_weight;
        let channel = |numerator: u32, dest: u8| {
            normalized_channel((numerator + u32::from(dest) * dest_weight + alpha / 2) / alpha)
        };
        pack_color(Color::rgba(
            channel(self.numerators[0], dst.r),
            channel(self.numerators[1], dst.g),
            channel(self.numerators[2], dst.b),
            normalized_channel((alpha + 127) / 255),
        ))
    }
}

/// Packed ARGB storage with immutable dimensions and checked allocation.
#[derive(Debug, Clone)]
pub struct Framebuffer {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}

impl Framebuffer {
    /// Allocates transparent storage bounded by [`MAX_PIXELS`].
    ///
    /// # Errors
    /// Rejects zero or non-coordinate-sized dimensions, excessive area, or allocation failure.
    pub fn new(width: u32, height: u32) -> Result<Self> {
        let count = u64::from(width) * u64::from(height);
        if width == 0
            || height == 0
            || width > i32::MAX as u32
            || height > i32::MAX as u32
            || count > MAX_PIXELS as u64
        {
            return Err(MetisError::ui(
                ErrorCode::SurfaceAllocationError,
                "Framebuffer dimensions exceed storage or coordinate limits",
            ));
        }
        let count = usize::try_from(count).map_err(|_| allocation_error())?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(count)
            .map_err(|_| allocation_error())?;
        pixels.resize(count, 0);
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Horizontal pixel count.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }
    /// Vertical pixel count.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }
    /// Contiguous row-major pixels packed as `0xAARRGGBB`.
    #[must_use]
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    /// Replaces every pixel with the supplied straight RGBA color.
    pub fn clear(&mut self, color: Color) {
        self.pixels.fill(pack_color(color));
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let x = u32::try_from(x).ok()?;
        let y = u32::try_from(y).ok()?;
        if x >= self.width || y >= self.height {
            return None;
        }
        usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()
    }

    /// Overwrites an in-bounds pixel; off-screen writes are clipped.
    pub fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        if let Some(index) = self.index(x, y) {
            self.pixels[index] = pack_color(color);
        }
    }

    /// Composites straight RGBA using source-over, rounding to the nearest channel.
    ///
    /// The output alpha numerator is `sa * 255 + da * (255 - sa)`.
    /// Each color numerator includes destination alpha before normalization.
    /// Span filling shares this arithmetic through the same precomputed
    /// source terms, so a single pixel and a filled run composite identically.
    pub fn blend_pixel(&mut self, x: i32, y: i32, src: Color) {
        let Some(index) = self.index(x, y) else {
            return;
        };
        let source = SourceOver::new(src);
        if source.is_transparent() {
            return;
        }
        self.pixels[index] = source.apply(self.pixels[index]);
    }

    /// Borrows the visible pixels of row `y` between the `left` and `right`
    /// column bounds.
    ///
    /// Bounds outside the surface yield an empty slice, so a caller traverses
    /// only visible work and the pixel loop needs no per-pixel bounds test.
    pub(crate) fn row_span_mut(&mut self, y: u32, left: u32, right: u32) -> &mut [u32] {
        if y >= self.height {
            return &mut [];
        }
        let left = left.min(self.width);
        let right = right.min(self.width);
        if right <= left {
            return &mut [];
        }
        let fits = "invariant: bounded framebuffer offsets fit usize";
        let stride = usize::try_from(self.width).expect(fits);
        let start = usize::try_from(y).expect(fits) * stride + usize::try_from(left).expect(fits);
        let end = start + usize::try_from(right - left).expect(fits);
        &mut self.pixels[start..end]
    }

    /// Composites one precomputed source over the visible part of a row span.
    pub(crate) fn composite_span(&mut self, y: u32, left: u32, right: u32, source: SourceOver) {
        for pixel in self.row_span_mut(y, left, right) {
            *pixel = source.apply(*pixel);
        }
    }

    /// Reads a pixel, returning transparent black for clipped coordinates.
    #[must_use]
    pub fn get_pixel(&self, x: i32, y: i32) -> Color {
        self.index(x, y)
            .map_or(Color::TRANSPARENT, |index| unpack_color(self.pixels[index]))
    }
}

pub(crate) fn allocation_error() -> MetisError {
    MetisError::ui(
        ErrorCode::SurfaceAllocationError,
        "Unable to reserve bounded pixel or presentation storage",
    )
}

const fn pack_color(c: Color) -> u32 {
    u32::from_be_bytes([c.a, c.r, c.g, c.b])
}
const fn unpack_color(value: u32) -> Color {
    let [a, r, g, b] = value.to_be_bytes();
    Color::rgba(r, g, b, a)
}

fn normalized_channel(value: u32) -> u8 {
    u8::try_from(value).expect("invariant: normalized composite channel is at most 255")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_handles_unicode_and_expands_short_channels() {
        for value in ["#a€aa", "#€aaa", "#💥aaaa", "#１２３", "#gggggg"] {
            assert_eq!(Color::from_hex(value), None);
        }
        assert_eq!(
            Color::from_hex("#f08c"),
            Some(Color::rgba(255, 0, 136, 204))
        );
        assert_eq!(
            Color::from_hex("#12345678"),
            Some(Color::rgba(18, 52, 86, 120))
        );
    }

    #[test]
    fn allocation_rejects_unbounded_or_empty_surfaces() {
        for (width, height) in [(0, 1), (1, 0), (u32::MAX, u32::MAX), (4097, 4096)] {
            assert_eq!(
                Framebuffer::new(width, height)
                    .expect_err("invalid dimensions")
                    .code,
                ErrorCode::SurfaceAllocationError
            );
        }
    }

    #[test]
    fn alpha_over_transparent_preserves_straight_color() {
        let mut fb = Framebuffer::new(2, 2).expect("small surface");
        fb.blend_pixel(0, 0, Color::rgba(200, 80, 10, 128));
        assert_eq!(fb.get_pixel(0, 0), Color::rgba(200, 80, 10, 128));
        fb.set_pixel(1, 0, Color::WHITE);
        fb.blend_pixel(1, 0, Color::rgba(0, 0, 0, 128));
        assert_eq!(fb.get_pixel(1, 0), Color::rgb(127, 127, 127));
        fb.set_pixel(1, 1, Color::rgba(0, 100, 0, 128));
        fb.blend_pixel(1, 1, Color::rgba(200, 0, 0, 128));
        // Alpha numerator 48896; channel numerators 6528000 and 1625600.
        assert_eq!(fb.get_pixel(1, 1), Color::rgba(134, 33, 0, 192));
        fb.set_pixel(i32::MAX, i32::MIN, Color::RED);
        assert_eq!(fb.get_pixel(0, 1), Color::TRANSPARENT);
        assert!(Rect::new(i32::MAX - 1, 0, 10, 1).contains(i32::MAX, 0));
    }
}
