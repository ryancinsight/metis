//! Bounded RGBA software framebuffer and integer rectangle geometry.

use metis_core::error::{ErrorCode, MetisError, Result};
use std::fmt;

mod clip;
mod composite;

pub use clip::Clip;
use clip::ClipScope;
pub(crate) use composite::SourceOver;

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

/// Packed ARGB storage with immutable dimensions and checked allocation.
///
/// Writes land only inside the current [`Clip`], the whole surface unless a
/// [`Self::render_clipped`] call narrows it; reads see every pixel.
#[derive(Debug, Clone)]
pub struct Framebuffer {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
    clip: Clip,
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
            clip: Clip::new(0, 0, width, height),
        })
    }

    pub(crate) const fn surface(&self) -> Clip {
        Clip::new(0, 0, self.width, self.height)
    }

    pub(crate) const fn set_clip(&mut self, clip: Clip) {
        self.clip = clip;
    }

    /// The region writes may currently change.
    #[must_use]
    pub const fn clip(&self) -> Clip {
        self.clip
    }

    /// Runs `draw` with writes confined to `region` on the surface, then
    /// restores the whole-surface clip.
    ///
    /// Every pixel outside `region` keeps its value, and every pixel inside
    /// it receives exactly what an unclipped draw would give it: rasterizers
    /// evaluate pixels independently, so narrowing the clip changes which
    /// pixels are computed, never their values.
    ///
    /// # Panics
    ///
    /// Panics only if `draw` does; the whole-surface clip is restored first.
    pub fn render_clipped<R>(&mut self, region: Rect, draw: impl FnOnce(&mut Self) -> R) -> R {
        let bound = |start: i32, extent: i32, limit: u32| {
            let low = i64::from(start).clamp(0, i64::from(limit));
            let high = (i64::from(start) + i64::from(extent.max(0))).clamp(low, i64::from(limit));
            let fits = "invariant: a coordinate clamped to the surface fits u32";
            (
                u32::try_from(low).expect(fits),
                u32::try_from(high).expect(fits),
            )
        };
        let (left, right) = bound(region.x, region.width, self.width);
        let (top, bottom) = bound(region.y, region.height, self.height);
        let scope = ClipScope::narrow(self, Clip::new(left, top, right, bottom));
        draw(scope.surface)
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

    /// Mutable view of the same contiguous storage.
    ///
    /// A whole-frame producer — a software renderer writing every pixel — can
    /// then fill this buffer directly instead of rendering into a scratch
    /// buffer and copying per pixel through [`Self::set_pixel`], which would
    /// cost one bounds-checked call and one clip test per pixel. Dimensions are
    /// not reachable from this view, so a caller cannot resize the buffer
    /// underneath the surface; only the pixel values can change.
    pub fn pixels_mut(&mut self) -> &mut [u32] {
        &mut self.pixels
    }

    /// Replaces every pixel inside the clip with the supplied straight RGBA
    /// color.
    pub fn clear(&mut self, color: Color) {
        let packed = pack_color(color);
        if self.clip == self.surface() {
            self.pixels.fill(packed);
            return;
        }
        let clip = self.clip;
        for row in clip.top()..clip.bottom() {
            self.row_span_mut(row, clip.left(), clip.right())
                .fill(packed);
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let x = u32::try_from(x).ok()?;
        let y = u32::try_from(y).ok()?;
        if x >= self.width || y >= self.height {
            return None;
        }
        usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()
    }

    /// The storage index of a pixel writes may change.
    fn writable_index(&self, x: i32, y: i32) -> Option<usize> {
        let index = self.index(x, y)?;
        let fits = "invariant: an indexed pixel's coordinates are nonnegative";
        self.clip
            .contains(u32::try_from(x).expect(fits), u32::try_from(y).expect(fits))
            .then_some(index)
    }

    /// Overwrites a pixel inside the clip; other writes are dropped.
    pub fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        if let Some(index) = self.writable_index(x, y) {
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
        let Some(index) = self.writable_index(x, y) else {
            return;
        };
        let source = SourceOver::new(src);
        if source.is_transparent() {
            return;
        }
        self.pixels[index] = source.apply(self.pixels[index]);
    }

    /// Borrows the writable pixels of row `y` between the `left` and `right`
    /// column bounds.
    ///
    /// The span is clamped to the clip, and a row outside it yields an empty
    /// slice, so the pixel loop needs no per-pixel bounds test. A caller that
    /// pairs columns with the returned pixels bounds `left` by the clip first.
    pub(crate) fn row_span_mut(&mut self, y: u32, left: u32, right: u32) -> &mut [u32] {
        if y < self.clip.top() || y >= self.clip.bottom() {
            return &mut [];
        }
        let left = left.clamp(self.clip.left(), self.clip.right());
        let right = right.clamp(left, self.clip.right());
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
    ///
    /// A span lying wholly on opaque pixels, the common case once a surface
    /// is cleared, takes the branch-free opaque kernel; the check is one
    /// read pass the compiler vectorizes.
    pub(crate) fn composite_span(&mut self, y: u32, left: u32, right: u32, source: SourceOver) {
        let span = self.row_span_mut(y, left, right);
        if span.iter().all(|pixel| pixel >> 24 == 0xFF) {
            for pixel in span {
                *pixel = source.over_opaque(*pixel);
            }
        } else {
            for pixel in span {
                *pixel = source.apply(*pixel);
            }
        }
    }

    /// Composites `color` scaled by a row of antialiasing `coverage` values
    /// whose first value lies at column `x` of row `y`, clipped to the
    /// surface and clip rectangle.
    ///
    /// Each pixel receives exactly the source a one-pixel
    /// [`composite_span`](Self::composite_span) would give it; resolving the
    /// row once removes the per-pixel bounds and clip checks.
    pub(crate) fn composite_coverage_row(
        &mut self,
        y: u32,
        x: i64,
        coverage: &[f64],
        color: Color,
    ) {
        let Ok(length) = i64::try_from(coverage.len()) else {
            return;
        };
        let start = x.max(i64::from(self.clip.left()));
        let end = x.saturating_add(length).min(i64::from(self.clip.right()));
        let (Ok(left), Ok(right)) = (u32::try_from(start), u32::try_from(end)) else {
            return;
        };
        if left >= right {
            return;
        }
        let span = self.row_span_mut(y, left, right);
        let skip = usize::try_from(start - x).unwrap_or(usize::MAX);
        let Some(values) = coverage.get(skip..) else {
            return;
        };
        for (pixel, value) in span.iter_mut().zip(values) {
            if *value <= 0.0 {
                continue;
            }
            let source = SourceOver::covering(color, *value);
            if !source.is_transparent() {
                *pixel = source.apply(*pixel);
            }
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

pub(crate) const fn pack_color(c: Color) -> u32 {
    u32::from_be_bytes([c.a, c.r, c.g, c.b])
}
pub(crate) const fn unpack_color(value: u32) -> Color {
    let [a, r, g, b] = value.to_be_bytes();
    Color::rgba(r, g, b, a)
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
