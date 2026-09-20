//! Aspect-preserving placement inside bounded presentation geometry.

use super::{ImagePlacement, ImageSampling, ImageTransform, RasterImage, image_error};
use metis_core::error::Result;
use metis_platform::framebuffer::Rect;
use std::sync::Arc;

impl ImagePlacement {
    /// Fits an entire image inside `bounds` without changing its aspect ratio.
    ///
    /// Quarter-turn rotations exchange the source width and height before the
    /// fit is calculated. The limiting extent fills its bound and the other is
    /// the integer floor of the exact scaled extent, so its rounding error is
    /// strictly less than one destination pixel. Any spare pixels are divided
    /// around the image, with an odd pixel retained on the right or bottom.
    ///
    /// # Examples
    ///
    /// ```
    /// use metis_platform::framebuffer::{Color, Rect};
    /// use metis_ui_lang::{ImagePlacement, ImageTransform, RasterImage};
    ///
    /// let image = RasterImage::new(2, 1, vec![Color::RED, Color::BLUE])
    ///     .expect("valid image");
    /// let placement = ImagePlacement::contain(
    ///     image,
    ///     Rect::new(10, 20, 8, 8),
    ///     ImageTransform::Identity,
    /// )
    /// .expect("image fits");
    /// assert_eq!(placement.source(), Rect::new(0, 0, 2, 1));
    /// assert_eq!(placement.destination(), Rect::new(10, 22, 8, 4));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`metis_core::error::ErrorCode::RenderFailure`] when `bounds`
    /// has a nonpositive extent, an extent would round to zero, centering would
    /// exceed the coordinate range, or `transform` is arbitrary affine.
    pub fn contain(
        image: impl Into<Arc<RasterImage>>,
        bounds: Rect,
        transform: ImageTransform,
    ) -> Result<Self> {
        if bounds.width <= 0 || bounds.height <= 0 {
            return Err(image_error(
                "Image fit bounds must have positive dimensions",
            ));
        }
        let swaps_extents = match transform {
            ImageTransform::RotateClockwise | ImageTransform::RotateCounterClockwise => true,
            ImageTransform::Identity
            | ImageTransform::FlipHorizontal
            | ImageTransform::FlipVertical => false,
            ImageTransform::Affine(_) => {
                return Err(image_error(
                    "Arbitrary affine transforms cannot guarantee bounded image containment",
                ));
            }
        };
        let image = image.into();
        let (oriented_width, oriented_height) = if swaps_extents {
            (i64::from(image.height()), i64::from(image.width()))
        } else {
            (i64::from(image.width()), i64::from(image.height()))
        };
        let bound_width = i64::from(bounds.width);
        let bound_height = i64::from(bounds.height);
        let (destination_width, destination_height) =
            if bound_width * oriented_height <= bound_height * oriented_width {
                (bound_width, bound_width * oriented_height / oriented_width)
            } else {
                (
                    bound_height * oriented_width / oriented_height,
                    bound_height,
                )
            };
        if destination_width == 0 || destination_height == 0 {
            return Err(image_error(
                "Image aspect ratio cannot fit a positive pixel extent inside bounds",
            ));
        }

        let destination_width = i32::try_from(destination_width)
            .map_err(|_| image_error("Fitted image width exceeds coordinate range"))?;
        let destination_height = i32::try_from(destination_height)
            .map_err(|_| image_error("Fitted image height exceeds coordinate range"))?;
        let destination = Rect::new(
            centered_origin(bounds.x, bounds.width, destination_width, "horizontal")?,
            centered_origin(bounds.y, bounds.height, destination_height, "vertical")?,
            destination_width,
            destination_height,
        );
        let source = Rect::new(
            0,
            0,
            i32::try_from(image.width())
                .map_err(|_| image_error("Image width exceeds source coordinate range"))?,
            i32::try_from(image.height())
                .map_err(|_| image_error("Image height exceeds source coordinate range"))?,
        );
        ImagePlacement::new(image, source, destination, ImageSampling::Nearest)
            .map(|placement| placement.with_transform(transform))
    }
}

fn centered_origin(origin: i32, outer: i32, inner: i32, axis: &'static str) -> Result<i32> {
    let offset = (outer - inner) / 2;
    origin
        .checked_add(offset)
        .ok_or_else(|| image_error(format!("Image fit {axis} center exceeds coordinate range")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use metis_core::error::ErrorCode;
    use metis_platform::framebuffer::{Color, Framebuffer};

    const BACKGROUND: Color = Color::rgb(11, 13, 17);
    const A: Color = Color::rgb(10, 0, 0);
    const B: Color = Color::rgb(20, 0, 0);
    const C: Color = Color::rgb(30, 0, 0);
    const D: Color = Color::rgb(40, 0, 0);
    const E: Color = Color::rgb(50, 0, 0);
    const F: Color = Color::rgba(200, 80, 40, 128);
    const BLENDED_F: Color = Color::rgba(106, 47, 29, 255);

    fn fixture() -> RasterImage {
        RasterImage::new(2, 3, vec![A, B, C, D, E, F]).expect("orientation fixture")
    }

    fn render(transform: ImageTransform) -> (ImagePlacement, Vec<Color>) {
        let placement = ImagePlacement::contain(fixture(), Rect::new(0, 0, 4, 5), transform)
            .expect("contained placement");
        let mut framebuffer = Framebuffer::new(4, 5).expect("fixture surface");
        framebuffer.clear(BACKGROUND);
        placement.render_to(&mut framebuffer);
        let mut pixels = Vec::with_capacity(20);
        for y in 0..5 {
            for x in 0..4 {
                pixels.push(framebuffer.get_pixel(x, y));
            }
        }
        (placement, pixels)
    }

    #[test]
    fn contain_renders_each_discrete_orientation_with_alpha_and_letterboxing() {
        let cases = [
            (
                ImageTransform::Identity,
                Rect::new(0, 0, 3, 5),
                [
                    A, A, B, BACKGROUND, A, A, B, BACKGROUND, C, C, D, BACKGROUND, C, C, D,
                    BACKGROUND, E, E, BLENDED_F, BACKGROUND,
                ],
            ),
            (
                ImageTransform::FlipHorizontal,
                Rect::new(0, 0, 3, 5),
                [
                    B, B, A, BACKGROUND, B, B, A, BACKGROUND, D, D, C, BACKGROUND, D, D, C,
                    BACKGROUND, BLENDED_F, BLENDED_F, E, BACKGROUND,
                ],
            ),
            (
                ImageTransform::FlipVertical,
                Rect::new(0, 0, 3, 5),
                [
                    E, E, BLENDED_F, BACKGROUND, E, E, BLENDED_F, BACKGROUND, C, C, D, BACKGROUND,
                    C, C, D, BACKGROUND, A, A, B, BACKGROUND,
                ],
            ),
            (
                ImageTransform::RotateClockwise,
                Rect::new(0, 1, 4, 2),
                [
                    BACKGROUND, BACKGROUND, BACKGROUND, BACKGROUND, E, E, C, A, BLENDED_F,
                    BLENDED_F, D, B, BACKGROUND, BACKGROUND, BACKGROUND, BACKGROUND, BACKGROUND,
                    BACKGROUND, BACKGROUND, BACKGROUND,
                ],
            ),
            (
                ImageTransform::RotateCounterClockwise,
                Rect::new(0, 1, 4, 2),
                [
                    BACKGROUND, BACKGROUND, BACKGROUND, BACKGROUND, B, B, D, BLENDED_F, A, A, C, E,
                    BACKGROUND, BACKGROUND, BACKGROUND, BACKGROUND, BACKGROUND, BACKGROUND,
                    BACKGROUND, BACKGROUND,
                ],
            ),
        ];

        for (transform, expected_destination, expected_pixels) in cases {
            let (placement, pixels) = render(transform);
            assert_eq!(placement.source(), Rect::new(0, 0, 2, 3));
            assert_eq!(placement.destination(), expected_destination);
            assert_eq!(placement.sampling(), ImageSampling::Nearest);
            assert_eq!(placement.transform(), transform);
            assert_eq!(pixels, expected_pixels);
        }
    }

    #[test]
    fn contain_uses_maximal_integer_ratio_with_subpixel_rounding_error() {
        let width_limited =
            ImagePlacement::contain(fixture(), Rect::new(5, 7, 8, 20), ImageTransform::Identity)
                .expect("width-limited fit");
        assert_eq!(width_limited.destination(), Rect::new(5, 11, 8, 12));
        let height_limited =
            ImagePlacement::contain(fixture(), Rect::new(5, 7, 20, 8), ImageTransform::Identity)
                .expect("height-limited fit");
        assert_eq!(height_limited.destination(), Rect::new(12, 7, 5, 8));

        // floor(8 * 2 / 3) = 5 and the discarded numerator 1 is below the
        // divisor 3, which proves the fitted width is less than one pixel
        // below the exact aspect-preserving width.
        let destination = height_limited.destination();
        let discarded_numerator =
            i64::from(destination.height) * 2 - i64::from(destination.width) * 3;
        assert_eq!(discarded_numerator, 1);
        assert!(discarded_numerator < 3);
    }

    #[test]
    fn contain_rejects_invalid_bounds_zero_rounding_affine_and_coordinate_overflow() {
        for bounds in [
            Rect::new(0, 0, 0, 1),
            Rect::new(0, 0, 1, 0),
            Rect::new(0, 0, -1, 1),
            Rect::new(0, 0, 1, -1),
        ] {
            let error = ImagePlacement::contain(fixture(), bounds, ImageTransform::Identity)
                .expect_err("invalid bounds");
            assert_eq!(error.code, ErrorCode::RenderFailure);
        }

        let zero_rounded = RasterImage::new(1, 3, vec![A, B, C]).expect("narrow image");
        let error = ImagePlacement::contain(
            zero_rounded,
            Rect::new(0, 0, 1, 1),
            ImageTransform::Identity,
        )
        .expect_err("rounded-to-zero width");
        assert_eq!(error.code, ErrorCode::RenderFailure);

        let affine = ImageTransform::Affine(
            super::super::AffineTransform::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
                .expect("identity affine"),
        );
        let error = ImagePlacement::contain(fixture(), Rect::new(0, 0, 4, 5), affine)
            .expect_err("affine containment");
        assert_eq!(error.code, ErrorCode::RenderFailure);

        let horizontal = ImagePlacement::contain(
            fixture(),
            Rect::new(i32::MAX, 0, 5, 5),
            ImageTransform::Identity,
        )
        .expect_err("horizontal centering overflow");
        assert_eq!(horizontal.code, ErrorCode::RenderFailure);
        assert!(horizontal.message.contains("center"));

        let vertical = ImagePlacement::contain(
            fixture(),
            Rect::new(0, i32::MAX, 4, 5),
            ImageTransform::RotateClockwise,
        )
        .expect_err("vertical centering overflow");
        assert_eq!(vertical.code, ErrorCode::RenderFailure);
        assert!(vertical.message.contains("center"));
    }
}
