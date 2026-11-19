//! Borrowed frame contract shared by browser consumers.

use std::io;

use super::aspect;

/// Physical row and column spacing for a borrowed display frame.
///
/// The values are format-neutral sample distances. RITK derives them from its
/// validated volume geometry; Metis only validates the positive finite
/// contract and uses the values to reject a malformed upload before it reaches
/// the browser provider.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplaySpacing {
    row: f64,
    column: f64,
}

impl DisplaySpacing {
    /// Creates validated row and column sample distances.
    ///
    /// # Errors
    /// Returns [`io::ErrorKind::InvalidInput`] when either distance is zero,
    /// negative or non-finite.
    pub fn try_new(row: f64, column: f64) -> io::Result<Self> {
        if !row.is_finite() || row <= 0.0 || !column.is_finite() || column <= 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "display spacing must be finite and positive",
            ));
        }
        Ok(Self { row, column })
    }

    /// Returns the row and column distances in display order.
    #[must_use]
    pub const fn values(self) -> [f64; 2] {
        [self.row, self.column]
    }

    /// Computes the physical width-to-height ratio for a frame.
    ///
    /// # Errors
    /// Returns [`io::ErrorKind::InvalidInput`] for zero frame dimensions or a
    /// ratio that cannot be represented as a positive finite value.
    pub fn aspect(self, width: u32, height: u32) -> io::Result<f64> {
        if width == 0 || height == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "display frame dimensions must be positive",
            ));
        }
        aspect::physical_ratio(width, height, self.row, self.column).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "display frame aspect is not representable",
            )
        })
    }
}

/// A borrowed row-major straight-alpha RGBA8 frame.
///
/// Implement this seam on a consumer-owned presentation value. The browser
/// host validates the dimensions and byte count before handing the borrowed
/// view to the Moirai canvas provider, so the frame does not acquire a second
/// pixel allocation or retain format-specific metadata.
pub trait CanvasFrame {
    /// Returns the frame width in device pixels.
    fn width(&self) -> u32;

    /// Returns the frame height in device pixels.
    fn height(&self) -> u32;

    /// Returns contiguous row-major RGBA8 bytes.
    fn rgba(&self) -> &[u8];

    /// Returns validated physical row and column spacing when the producer
    /// has display geometry to preserve.
    ///
    /// The default keeps purely pixel-space frames source-compatible. A
    /// consumer that supplies spacing must construct it through
    /// [`DisplaySpacing::try_new`], so the host never receives an invalid
    /// physical extent.
    fn display_spacing(&self) -> Option<DisplaySpacing> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{CanvasFrame, DisplaySpacing};

    struct Fixture {
        pixels: [u8; 8],
    }

    impl CanvasFrame for Fixture {
        fn width(&self) -> u32 {
            2
        }

        fn height(&self) -> u32 {
            1
        }

        fn rgba(&self) -> &[u8] {
            &self.pixels
        }
    }

    #[test]
    fn frame_seam_preserves_borrowed_dimensions_and_storage() {
        let fixture = Fixture { pixels: [1; 8] };
        assert_eq!(fixture.width(), 2);
        assert_eq!(fixture.height(), 1);
        assert_eq!(fixture.rgba(), &[1; 8]);
        assert_eq!(fixture.display_spacing(), None);
    }

    #[test]
    fn display_spacing_validates_and_preserves_physical_aspect() {
        let spacing = DisplaySpacing::try_new(2.5, 0.5).expect("positive spacing");
        for (actual, expected) in spacing.values().into_iter().zip([2.5_f64, 0.5_f64]) {
            let bound = 2.0 * f64::EPSILON * expected.abs().max(1.0);
            assert!((actual - expected).abs() <= bound);
        }
        let aspect = spacing.aspect(512, 94).expect("representable aspect");
        let expected: f64 = (512.0 * 0.5) / (94.0 * 2.5);
        let bound = 8.0 * f64::EPSILON * expected.abs().max(1.0);
        assert!((aspect - expected).abs() <= bound);
    }

    #[test]
    fn display_spacing_rejects_invalid_values_and_dimensions() {
        for (row, column) in [
            (0.0, 1.0),
            (-1.0, 1.0),
            (f64::NAN, 1.0),
            (1.0, 0.0),
            (1.0, f64::INFINITY),
        ] {
            assert_eq!(
                DisplaySpacing::try_new(row, column)
                    .expect_err("invalid display spacing")
                    .kind(),
                std::io::ErrorKind::InvalidInput
            );
        }
        let spacing = DisplaySpacing::try_new(1.0, 1.0).expect("spacing");
        for dimensions in [(0, 1), (1, 0)] {
            assert_eq!(
                spacing
                    .aspect(dimensions.0, dimensions.1)
                    .expect_err("invalid frame dimensions")
                    .kind(),
                std::io::ErrorKind::InvalidInput
            );
        }
        for unrepresentable in [
            DisplaySpacing::try_new(f64::from_bits(1), f64::MAX)
                .expect("valid overflow-boundary spacing"),
            DisplaySpacing::try_new(f64::MAX, f64::from_bits(1))
                .expect("valid underflow-boundary spacing"),
        ] {
            assert_eq!(
                unrepresentable
                    .aspect(1, 1)
                    .expect_err("unrepresentable final aspect")
                    .kind(),
                std::io::ErrorKind::InvalidInput
            );
        }
    }

    #[test]
    fn display_spacing_preserves_extreme_representable_aspects() {
        for value in [f64::MAX, f64::from_bits(1)] {
            let spacing = DisplaySpacing::try_new(value, value).expect("valid equal spacing");
            assert_eq!(
                spacing
                    .aspect(2, 2)
                    .expect("equal physical extents")
                    .to_bits(),
                1.0_f64.to_bits()
            );
        }

        let spacing = DisplaySpacing::try_new(2.0, f64::from_bits(1))
            .expect("valid dimension-compensated spacing");
        assert_eq!(
            spacing
                .aspect(2, 1)
                .expect("minimum subnormal aspect")
                .to_bits(),
            1
        );

        let spacing = DisplaySpacing::try_new(7.0, 7.0).expect("valid equal spacing");
        assert_eq!(
            spacing
                .aspect(u32::MAX, u32::MAX)
                .expect("maximum equal dimensions")
                .to_bits(),
            1.0_f64.to_bits()
        );
    }

    #[test]
    fn display_spacing_rounds_boundary_counterexamples_once() {
        let maximum = DisplaySpacing::try_new(
            f64::from_bits(0x3efb_ea7f_22df_9fae),
            f64::from_bits(0x7f07_fdd5_0854_7ce0),
        )
        .expect("valid maximum-boundary spacing");
        assert_eq!(
            maximum
                .aspect(4_300, 7_391)
                .expect("ratio rounds to maximum finite")
                .to_bits(),
            f64::MAX.to_bits()
        );

        let minimum =
            DisplaySpacing::try_new(f64::from_bits(0x4004_2a8d_b1ae_f7a0), f64::from_bits(1))
                .expect("valid minimum-boundary spacing");
        assert_eq!(
            minimum
                .aspect(8_553, 6_786)
                .expect("ratio rounds to minimum subnormal")
                .to_bits(),
            1
        );
    }
}
