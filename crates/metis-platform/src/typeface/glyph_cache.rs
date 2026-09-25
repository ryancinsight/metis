//! Rasterized glyph coverage, memoized across repaints.
//!
//! Rasterizing an outline is where drawing text spends its time, and a
//! repaint redraws most glyphs at exactly the pixel positions it drew them
//! before. The key holds everything that determines a glyph's coverage —
//! face, glyph, scale and the exact device pen and baseline — so a hit
//! reproduces the rasterizer's output bit for bit rather than approximating
//! it at a quantized position. Eviction is the [`crate::memo`] generations'.

use super::GlyphId;
use super::raster::PixelBounds;
use super::text::GlyphWeight;
use crate::memo::{Footprint, GenerationalMemo};
use std::mem::size_of;

/// Coverage bytes admitted to one generation.
pub(super) const GENERATION_BYTES: usize = 2 * 1024 * 1024;

/// Everything that determines one glyph's rasterized coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct GlyphKey {
    weight: GlyphWeight,
    glyph: GlyphId,
    scale: u64,
    pen: u64,
    baseline: u64,
}

impl GlyphKey {
    pub(super) fn new(
        weight: GlyphWeight,
        glyph: GlyphId,
        scale: f64,
        pen: f64,
        baseline: f64,
    ) -> Self {
        Self {
            weight,
            glyph,
            scale: scale.to_bits(),
            pen: pen.to_bits(),
            baseline: baseline.to_bits(),
        }
    }
}

/// A rasterized glyph: its pixel bounds and row-major coverage, or no
/// bounds for a glyph without an outline, such as a space.
#[derive(Debug)]
pub(super) struct GlyphCoverage {
    pub(super) bounds: Option<PixelBounds>,
    pub(super) coverage: Box<[f64]>,
}

impl Footprint for GlyphCoverage {
    fn footprint(&self) -> usize {
        self.coverage.len() * size_of::<f64>() + size_of::<Self>()
    }
}

/// Glyph coverage for the current and previous generations.
pub(super) type GlyphCache = GenerationalMemo<GlyphKey, GlyphCoverage, GENERATION_BYTES>;

#[cfg(test)]
mod tests {
    use super::{GlyphCache, GlyphCoverage, GlyphKey};
    use crate::typeface::GlyphId;
    use crate::typeface::raster::PixelBounds;
    use crate::typeface::text::GlyphWeight;

    fn key(pen: f64) -> GlyphKey {
        GlyphKey::new(GlyphWeight::Regular, GlyphId::NOTDEF, 0.01, pen, 12.5)
    }

    fn entry(values: usize) -> GlyphCoverage {
        let width = i32::try_from(values).expect("small");
        GlyphCoverage {
            bounds: Some(PixelBounds {
                left: 0,
                top: 0,
                right: width,
                bottom: 1,
            }),
            coverage: vec![0.5; values].into_boxed_slice(),
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
