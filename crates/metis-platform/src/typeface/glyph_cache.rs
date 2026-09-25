//! Rasterized glyph alpha, memoized across repaints.
//!
//! Rasterizing an outline is where drawing text spends its time, and a
//! repaint redraws most glyphs at exactly the pixel positions it drew them
//! before. A glyph is held as the source alpha of each pixel: its coverage
//! already scaled by the text color's alpha and rounded to a byte, which is
//! what compositing consumes. The key holds everything that determines those
//! bytes — face, glyph, scale, the exact device pen and baseline, and the
//! color's alpha — so a hit reproduces the direct path's pixels bit for bit
//! rather than approximating them at a quantized position. Eviction is the
//! [`crate::memo`] generations'.

use super::GlyphId;
use super::raster::PixelBounds;
use super::text::GlyphWeight;
use crate::memo::{Footprint, GenerationalMemo};
use std::mem::size_of;

/// Mask bytes admitted to one generation: one byte per pixel, so about 3,500
/// glyphs of 24 by 24 pixels.
pub(super) const GENERATION_BYTES: usize = 2 * 1024 * 1024;

/// Everything that determines one glyph's alpha mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct GlyphKey {
    weight: GlyphWeight,
    glyph: GlyphId,
    scale: u64,
    pen: u64,
    baseline: u64,
    alpha: u8,
}

impl GlyphKey {
    pub(super) fn new(
        weight: GlyphWeight,
        glyph: GlyphId,
        scale: f64,
        pen: f64,
        baseline: f64,
        alpha: u8,
    ) -> Self {
        Self {
            weight,
            glyph,
            scale: scale.to_bits(),
            pen: pen.to_bits(),
            baseline: baseline.to_bits(),
            alpha,
        }
    }
}

/// A rasterized glyph: its pixel bounds and row-major source alpha, or no
/// bounds for a glyph without an outline, such as a space.
#[derive(Debug)]
pub(super) struct GlyphMask {
    pub(super) bounds: Option<PixelBounds>,
    pub(super) alphas: Box<[u8]>,
}

impl Footprint for GlyphMask {
    fn footprint(&self) -> usize {
        self.alphas.len() + size_of::<Self>()
    }
}

/// Glyph masks for the current and previous generations.
pub(super) type GlyphCache = GenerationalMemo<GlyphKey, GlyphMask, GENERATION_BYTES>;

#[cfg(test)]
mod tests {
    use super::{GlyphCache, GlyphKey, GlyphMask};
    use crate::typeface::GlyphId;
    use crate::typeface::raster::PixelBounds;
    use crate::typeface::text::GlyphWeight;

    fn key(pen: f64) -> GlyphKey {
        GlyphKey::new(GlyphWeight::Regular, GlyphId::NOTDEF, 0.01, pen, 12.5, 255)
    }

    fn entry(values: usize) -> GlyphMask {
        let width = i32::try_from(values).expect("small");
        GlyphMask {
            bounds: Some(PixelBounds {
                left: 0,
                top: 0,
                right: width,
                bottom: 1,
            }),
            alphas: vec![128; values].into_boxed_slice(),
        }
    }

    #[test]
    fn hits_skip_rendering_and_keys_are_exact() {
        let mut cache = GlyphCache::default();
        let mut renders = 0;
        for _ in 0..3 {
            cache.get_or_render(key(1.0), || {
                renders += 1;
                Some(entry(4))
            });
        }
        assert_eq!(renders, 1);
        // The next representable pen position is a different key.
        cache.get_or_render(key(f64::from_bits(1.0_f64.to_bits() + 1)), || {
            renders += 1;
            Some(entry(4))
        });
        assert_eq!(renders, 2);
    }
}
