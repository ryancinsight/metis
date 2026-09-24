//! A bounded memo of rasterized glyph coverage.
//!
//! Rasterizing an outline is where drawing text spends its time, and a
//! repaint redraws most glyphs at exactly the pixel positions it drew them
//! before. The cache keys a glyph's coverage on everything that determines
//! it — face, glyph, scale and the exact device pen and baseline — so a hit
//! reproduces the rasterizer's output bit for bit rather than approximating
//! it at a quantized position.
//!
//! Memory is bounded by two generations of at most
//! [`GENERATION_BYTES`] each. When the current generation fills it becomes
//! the previous one and the older generation is dropped; a hit in the
//! previous generation moves the entry forward. Glyphs drawn every frame
//! therefore stay resident while glyphs no longer drawn age out, without
//! per-entry recency bookkeeping.

use super::GlyphId;
use super::raster::PixelBounds;
use super::text::GlyphWeight;
use std::collections::HashMap;
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

impl GlyphCoverage {
    fn bytes(&self) -> usize {
        self.coverage.len() * size_of::<f64>() + size_of::<Self>()
    }
}

#[derive(Debug, Default)]
pub(super) struct GlyphCache {
    current: HashMap<GlyphKey, GlyphCoverage>,
    previous: HashMap<GlyphKey, GlyphCoverage>,
    current_bytes: usize,
}

impl GlyphCache {
    /// The coverage for `key`, rasterizing it with `render` on a miss.
    ///
    /// `render` returns `None` when the glyph is not needed, such as when it
    /// lies wholly outside the clip; nothing is retained then. A glyph larger
    /// than one generation is rendered but not retained.
    pub(super) fn get_or_render(
        &mut self,
        key: GlyphKey,
        render: impl FnOnce() -> Option<GlyphCoverage>,
    ) -> Option<GlyphLookup<'_>> {
        if self.current.contains_key(&key) {
            return Some(GlyphLookup::Cached(&self.current[&key]));
        }
        let entry = match self.previous.remove(&key) {
            Some(entry) => entry,
            None => render()?,
        };
        let bytes = entry.bytes();
        if bytes > GENERATION_BYTES {
            return Some(GlyphLookup::Uncached(entry));
        }
        if self.current_bytes + bytes > GENERATION_BYTES {
            self.previous = std::mem::take(&mut self.current);
            self.current_bytes = 0;
        }
        self.current_bytes += bytes;
        Some(GlyphLookup::Cached(
            self.current.entry(key).or_insert(entry),
        ))
    }

    /// Coverage bytes retained across both generations.
    #[cfg(test)]
    pub(super) fn retained_bytes(&self) -> usize {
        self.current_bytes
            + self
                .previous
                .values()
                .map(GlyphCoverage::bytes)
                .sum::<usize>()
    }
}

/// A cache lookup result, borrowed when retained.
pub(super) enum GlyphLookup<'cache> {
    Cached(&'cache GlyphCoverage),
    Uncached(GlyphCoverage),
}

impl GlyphLookup<'_> {
    pub(super) fn coverage(&self) -> &GlyphCoverage {
        match self {
            Self::Cached(entry) => entry,
            Self::Uncached(entry) => entry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GENERATION_BYTES, GlyphCache, GlyphCoverage, GlyphKey};
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

    #[test]
    fn generations_bound_memory_and_keep_recent_glyphs() {
        let mut cache = GlyphCache::default();
        let per_entry = GENERATION_BYTES / 64;
        let values = per_entry / 8 - 8;
        for pen in 0..1_000_u32 {
            cache.get_or_render(key(f64::from(pen)), || Some(entry(values)));
            assert!(cache.retained_bytes() <= 2 * GENERATION_BYTES);
        }
        // The most recent glyph is still resident.
        let mut rendered = false;
        cache.get_or_render(key(999.0), || {
            rendered = true;
            Some(entry(values))
        });
        assert!(!rendered);
    }

    #[test]
    fn oversized_glyphs_are_rendered_but_not_retained() {
        let mut cache = GlyphCache::default();
        let values = GENERATION_BYTES / 8 + 1;
        let lookup = cache
            .get_or_render(key(0.0), || Some(entry(values)))
            .expect("rendered");
        assert_eq!(lookup.coverage().coverage.len(), values);
        assert_eq!(cache.retained_bytes(), 0);
        // A glyph its renderer declines is neither drawn nor retained.
        assert!(cache.get_or_render(key(1.0), || None).is_none());
        assert_eq!(cache.retained_bytes(), 0);
    }
}
