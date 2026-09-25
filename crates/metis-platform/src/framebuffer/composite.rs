//! Straight-alpha source-over compositing onto packed ARGB pixels.

use super::{Color, pack_color, unpack_color};

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

    /// The source `color` with its opacity scaled by an antialiasing
    /// `coverage`, clamped to `[0, 1]` and rounded half away from zero.
    ///
    /// The rounding is written out rather than calling `f64::round`, which on
    /// the baseline x86-64 target is a library call per pixel; the product
    /// lies in `[0, 255]`, where truncating and comparing the exact fraction
    /// with one half is the same rounding. A NaN coverage yields a
    /// transparent source, as the saturating cast of `f64::round` does.
    #[inline]
    pub(crate) fn covering(color: Color, coverage: f64) -> Self {
        let scaled = f64::from(color.a) * coverage.clamp(0.0, 1.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a clamped coverage times a byte channel lies in [0, 255]"
        )]
        let whole = scaled as u8;
        let alpha = whole + u8::from(scaled - f64::from(whole) >= 0.5);
        Self::new(Color::rgba(color.r, color.g, color.b, alpha))
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
    /// Both arms evaluate the same source-over expression; the reference
    /// test holds them to its definition for every source alpha.
    #[inline]
    pub(crate) fn apply(self, dst: u32) -> u32 {
        if dst >> 24 == 0xFF {
            return self.over_opaque(dst);
        }
        let dst = unpack_color(dst);
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

    /// Composites over a destination known to be opaque.
    ///
    /// The alpha denominator is then the constant `255 * 255`, which the
    /// compiler reduces to a multiply and shift, and the channels stay in
    /// the packed word: with `sa` the source alpha, a channel numerator is at
    /// most `255 * sa * 255 + 255 * (255 - sa) * 255 = 255 * OPAQUE_ALPHA`,
    /// so each rounded quotient is at most 255 and fits its byte without a
    /// checked conversion. That keeps the loop free of panicking branches, so
    /// a span over an opaque surface compiles to straight-line code.
    /// Composites this source over every pixel of `span`, each of which must
    /// be opaque, giving each exactly [`Self::over_opaque`]'s result.
    ///
    /// With `t = s·α + d·(255 − α)`, `over_opaque` computes
    /// `(255·t + 32512) / 65025`. Writing `t = 255·q + k`, both that and
    /// `(t + 127) / 255` equal `q + [k ≥ 128]`. The second form keeps every
    /// intermediate within `u16`, so the loop vectorizes to 16-bit lanes. The
    /// alpha lane uses `t = 255 · 255`, which gives 255.
    #[inline]
    pub(crate) fn over_opaque_span(self, span: &mut [u32]) {
        debug_assert!(
            span.iter().all(|pixel| pixel >> 24 == 0xFF),
            "invariant: the destination span is opaque"
        );
        let src = unpack_color(self.packed);
        let alpha = u16::from(src.a);
        let weight = 255 - alpha;
        // Little-endian lanes of `0xAARRGGBB`: blue, green, red, alpha.
        let add = [
            u16::from(src.b) * alpha,
            u16::from(src.g) * alpha,
            u16::from(src.r) * alpha,
            255 * 255,
        ];
        let scale = [weight, weight, weight, 0];
        for pixel in span {
            let lanes = pixel.to_le_bytes();
            *pixel = u32::from_le_bytes([
                opaque_lane(add[0], scale[0], lanes[0]),
                opaque_lane(add[1], scale[1], lanes[1]),
                opaque_lane(add[2], scale[2], lanes[2]),
                opaque_lane(add[3], scale[3], lanes[3]),
            ]);
        }
    }

    #[inline]
    pub(crate) fn over_opaque(self, dst: u32) -> u32 {
        let dest_weight = self.inverse_alpha * 255;
        let channel = |numerator: u32, shift: u32| {
            let destination = (dst >> shift) & 0xFF;
            ((numerator + destination * dest_weight + OPAQUE_ALPHA / 2) / OPAQUE_ALPHA) << shift
        };
        0xFF00_0000
            | channel(self.numerators[0], 16)
            | channel(self.numerators[1], 8)
            | channel(self.numerators[2], 0)
    }
}

/// One byte lane of [`SourceOver::over_opaque_span`]: `(add + d·scale + 127) / 255`.
///
/// `add + d·scale` is at most `255 · 255`, so the sum plus 127 is at most
/// 65152 and never wraps, and the quotient is a byte. The operations are
/// written wrapping because this crate's release profile keeps overflow
/// checks, whose per-lane branches would stop the loop vectorizing.
#[inline]
fn opaque_lane(add: u16, scale: u16, destination: u8) -> u8 {
    let sum = add
        .wrapping_add(u16::from(destination).wrapping_mul(scale))
        .wrapping_add(127);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the quotient of at most 65152 by 255 is at most 255"
    )]
    let value = (sum / 255) as u8;
    value
}

fn normalized_channel(value: u32) -> u8 {
    u8::try_from(value).expect("invariant: normalized composite channel is at most 255")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Source-over straight RGBA written out from its definition: output
    /// alpha `sa * 255 + da * (255 - sa)` over `255 * 255`, and each channel
    /// the alpha-weighted sum over that numerator, all rounded to nearest.
    fn reference(src: Color, dst: Color) -> Color {
        let (sa, da) = (u64::from(src.a), u64::from(dst.a));
        let alpha = sa * 255 + da * (255 - sa);
        if alpha == 0 {
            return dst;
        }
        let channel = |s: u8, d: u8| {
            let value =
                (u64::from(s) * sa * 255 + u64::from(d) * da * (255 - sa) + alpha / 2) / alpha;
            u8::try_from(value).expect("a weighted mean of bytes is a byte")
        };
        Color::rgba(
            channel(src.r, dst.r),
            channel(src.g, dst.g),
            channel(src.b, dst.b),
            u8::try_from((alpha + 127) / 255).expect("an alpha numerator over 255 is a byte"),
        )
    }

    #[test]
    fn covering_rounds_exactly_as_the_library_does() {
        let reference = |alpha: u8, coverage: f64| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the saturating cast is the behavior under comparison"
            )]
            let rounded = (f64::from(alpha) * coverage.clamp(0.0, 1.0)).round() as u8;
            rounded
        };
        let mut coverages = vec![0.0, 1.0, -0.25, 1.75, f64::NAN, f64::INFINITY];
        coverages.extend((0..=4096).map(|step| f64::from(step) / 4096.0));
        for alpha in 0..=255_u8 {
            // Coverages whose product lands on, just below and just above a
            // half: the boundary the fraction comparison must resolve.
            for whole in 0..u16::from(alpha) {
                let half = (f64::from(whole) + 0.5) / f64::from(alpha);
                coverages.extend([half, half.next_down(), half.next_up()]);
            }
            for &coverage in &coverages {
                let color = Color::rgba(9, 99, 199, alpha);
                assert_eq!(
                    unpack_color(SourceOver::covering(color, coverage).packed()).a,
                    reference(alpha, coverage),
                    "alpha {alpha} coverage {coverage:e}"
                );
            }
            coverages.truncate(4103);
        }
    }

    /// Every source alpha, source channels at the rounding extremes, and every
    /// destination byte in each channel, over spans long enough to take the
    /// vector body and short enough to take every tail length.
    #[test]
    fn an_opaque_span_matches_the_definition_for_every_byte() {
        let channels = [0, 1, 17, 127, 128, 200, 254, 255];
        let destination = |d: u8| Color::rgba(d, 255 - d, d / 2, 255);
        for source_alpha in 1..=255_u8 {
            for &value in &channels {
                let src = Color::rgba(value, 255 - value, value / 3, source_alpha);
                let source = SourceOver::new(src);
                let mut span: Vec<u32> = (0..=255).map(|d| pack_color(destination(d))).collect();
                source.over_opaque_span(&mut span);
                for (d, &pixel) in (0..=255_u8).zip(&span) {
                    assert_eq!(
                        unpack_color(pixel),
                        reference(src, destination(d)),
                        "{src:?} over {:?}",
                        destination(d)
                    );
                }
            }
        }
        let source = SourceOver::new(Color::rgba(49, 130, 206, 128));
        for length in 0..=33_u8 {
            let mut span: Vec<u32> = (0..length)
                .map(|d| pack_color(destination(d * 7)))
                .collect();
            source.over_opaque_span(&mut span);
            let expected: Vec<u32> = (0..length)
                .map(|d| source.over_opaque(pack_color(destination(d * 7))))
                .collect();
            assert_eq!(span, expected, "span of {length}");
        }
    }

    #[test]
    fn source_over_matches_its_definition_for_every_source_alpha() {
        let channels = [0, 1, 17, 127, 128, 200, 254, 255];
        let destination_alphas = [0, 1, 64, 127, 128, 203, 254, 255];
        for source_alpha in 1..=255_u8 {
            for &value in &channels {
                let src = Color::rgba(value, 255 - value, value / 2, source_alpha);
                let source = SourceOver::new(src);
                for &dest_alpha in &destination_alphas {
                    for &dest in &channels {
                        let dst = Color::rgba(dest, dest / 3, 255 - dest, dest_alpha);
                        assert_eq!(
                            unpack_color(source.apply(pack_color(dst))),
                            reference(src, dst),
                            "{src:?} over {dst:?}"
                        );
                    }
                }
            }
        }
    }
}
