//! Gaussian-blurred outer box shadows.
//!
//! CSS Backgrounds and Borders Level 3 §6.1.2 defines the blur as a Gaussian
//! whose standard deviation is half the blur radius and accepts any image
//! whose pixels lie within 5% of that result. §6.1.1 casts the outer shadow
//! from the border box, offset but otherwise the same size and shape (this
//! subset has no spread distance), and clips it inside the border box.
//!
//! The Gaussian is separable. Rows outside the corner bands span the full
//! width, so both passes over them reduce to differences of the running sum of
//! a kernel whose taps are the Gaussian's mass over each pixel; for a
//! pixel-aligned edge that is exactly the continuous blur sampled at the pixel
//! centre. Rows crossing an arc are integrated as slabs with an exact profile
//! along x and the Gaussian's exact mass along y ([`field`]). Arc rows are kept
//! in a ring and columns are processed in tiles, so memory is bounded by the
//! blur and the tile rather than the radius, the shape or the surface.

mod field;
mod kernel;
mod mask;

use super::round_rect::CornerRadius;
use std::cell::RefCell;

use crate::framebuffer::{Color, Framebuffer, Rect};
use mask::{ShadowKey, ShadowMask, ShadowMasks};

thread_local! {
    /// Shadow alpha masks kept across repaints; see [`mask`].
    static SHADOWS: RefCell<ShadowMasks> = RefCell::default();
}

/// Visible columns evaluated together.
///
/// The arc-row ring holds `min(2K + 1, 2r) · S` rows of one tile. Its largest
/// product is `2K + 1 = 897` at the maximum blur, where `S = 1`, so a tile
/// bounds the ring at `897 · 1024 · 8` bytes, 7.3 MB, whatever the surface
/// width.
const TILE_WIDTH: i64 = 1024;

/// An outer shadow cast by a border box.
///
/// # Examples
///
/// ```
/// use metis_platform::framebuffer::Color;
/// use metis_platform::rasterizer::BoxShadow;
///
/// let shadow = BoxShadow::new(0, 4, 12, Color::rgba(0, 0, 0, 60)).expect("blur in range");
/// assert_eq!(shadow.blur(), 12);
/// assert!(BoxShadow::new(0, 0, BoxShadow::MAX_BLUR + 1, Color::BLACK).is_none());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxShadow {
    offset_x: i32,
    offset_y: i32,
    blur: u32,
    color: Color,
}

impl BoxShadow {
    /// Largest blur radius in device pixels.
    ///
    /// Work and scratch memory grow with the kernel reach, `1.75` pixels per
    /// blur pixel, so the bound caps both: at this radius the row ring holds
    /// 897 rows of the visible shadow width.
    pub const MAX_BLUR: u32 = 256;

    /// Describes a shadow, or `None` when `blur` exceeds [`Self::MAX_BLUR`].
    #[must_use]
    pub const fn new(offset_x: i32, offset_y: i32, blur: u32, color: Color) -> Option<Self> {
        if blur > Self::MAX_BLUR {
            return None;
        }
        Some(Self {
            offset_x,
            offset_y,
            blur,
            color,
        })
    }

    /// Horizontal offset of the shadow from the border box.
    #[must_use]
    pub const fn offset_x(self) -> i32 {
        self.offset_x
    }

    /// Vertical offset of the shadow from the border box.
    #[must_use]
    pub const fn offset_y(self) -> i32 {
        self.offset_y
    }

    /// Blur radius in device pixels; twice the Gaussian's standard deviation.
    #[must_use]
    pub const fn blur(self) -> u32 {
        self.blur
    }

    /// Straight RGBA shadow color.
    #[must_use]
    pub const fn color(self) -> Color {
        self.color
    }

    /// The device pixels [`draw_box_shadow`] can change for `border_box`:
    /// the offset box widened by the blur's reach on every side.
    ///
    /// # Panics
    ///
    /// Does not panic: the reach of a blur at most [`Self::MAX_BLUR`] fits
    /// `i32`.
    #[must_use]
    pub fn extent(self, border_box: Rect) -> Rect {
        let reach = i32::try_from(kernel::reach(self.blur))
            .expect("invariant: the reach of a bounded blur fits i32");
        Rect::new(
            border_box
                .x
                .saturating_add(self.offset_x)
                .saturating_sub(reach),
            border_box
                .y
                .saturating_add(self.offset_y)
                .saturating_sub(reach),
            border_box.width.saturating_add(reach.saturating_mul(2)),
            border_box.height.saturating_add(reach.saturating_mul(2)),
        )
    }
}

/// Paints an outer box shadow for `border_box` beneath its element.
///
/// The shadow is the border box offset by the shadow's offsets, rounded by
/// the same clamped radius and blurred by a Gaussian of standard deviation
/// `blur / 2`; pixels the border box covers are left untouched and pixels its
/// arcs cover partly receive the uncovered share. A zero blur paints the
/// antialiased offset shape exactly as [`super::fill_rect`] would.
///
/// # Panics
///
/// Does not panic for a valid [`Framebuffer`]; the conversion checks encode
/// the clipping invariant that every painted row and column lies on the
/// surface and every shape offset fits the `i32` extents it came from.
pub fn draw_box_shadow(
    fb: &mut Framebuffer,
    border_box: Rect,
    radius: CornerRadius,
    shadow: BoxShadow,
) {
    if shadow.color.a == 0 || border_box.width <= 0 || border_box.height <= 0 {
        return;
    }
    // A shadow wholly outside the clip is neither rendered nor retained.
    let extent = shadow.extent(border_box);
    let clip = fb.clip();
    let reaches = |start: i32, length: i32, low: u32, high: u32| {
        let (start, end) = (i64::from(start), i64::from(start) + i64::from(length));
        end > i64::from(low) && start < i64::from(high)
    };
    if !reaches(extent.x, extent.width, clip.left(), clip.right())
        || !reaches(extent.y, extent.height, clip.top(), clip.bottom())
    {
        return;
    }
    let radius = CornerRadius::clamped(radius.pixels(), border_box);
    let surface = (fb.width(), fb.height());
    let key = ShadowKey::new(border_box, radius, shadow, surface);
    SHADOWS.with_borrow_mut(|masks| {
        let mask = masks.get_or_render(key, || {
            ShadowMask::render(surface, border_box, radius, shadow)
        });
        if let Some(mask) = mask {
            mask.value().composite(fb, shadow.color);
        }
    });
}

#[cfg(test)]
#[path = "shadow_tests.rs"]
mod tests;
