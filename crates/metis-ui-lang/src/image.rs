//! Validated raster images and clipped display-list placement.

use metis_core::error::{ErrorCode, MetisError, Result};
use metis_platform::framebuffer::{Color, Framebuffer, MAX_PIXELS, Rect};
use std::sync::Arc;

/// An immutable raster image with bounded dimensions and storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterImage {
    width: u32,
    height: u32,
    pixels: Arc<[Color]>,
}

impl RasterImage {
    /// Creates an image from row-major straight RGBA pixels.
    ///
    /// The dimensions and pixel count use the same limits as the software
    /// framebuffer. The pixel storage is retained through an atomic reference
    /// count so placements can share decoded image data without copying it.
    ///
    /// # Errors
    /// Returns [`ErrorCode::SurfaceAllocationError`] for zero, oversized or
    /// unrepresentable dimensions, and [`ErrorCode::RenderFailure`] when the
    /// pixel count does not match the dimensions.
    pub fn new(width: u32, height: u32, pixels: impl Into<Arc<[Color]>>) -> Result<Self> {
        let pixels = pixels.into();
        let count = u64::from(width) * u64::from(height);
        if width == 0
            || height == 0
            || width > i32::MAX as u32
            || height > i32::MAX as u32
            || count > MAX_PIXELS as u64
        {
            return Err(MetisError::ui(
                ErrorCode::SurfaceAllocationError,
                "Raster image dimensions exceed storage or coordinate limits",
            ));
        }
        let expected = usize::try_from(count).map_err(|_| {
            MetisError::ui(
                ErrorCode::SurfaceAllocationError,
                "Raster image pixel count exceeds addressable storage",
            )
        })?;
        if pixels.len() != expected {
            return Err(MetisError::ui(
                ErrorCode::RenderFailure,
                format!(
                    "Raster image pixel count {} does not match dimensions {}x{}",
                    pixels.len(),
                    width,
                    height
                ),
            ));
        }
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

    /// Row-major straight RGBA pixels borrowed from the shared image storage.
    #[must_use]
    pub fn pixels(&self) -> &[Color] {
        &self.pixels
    }
}

/// Sampling policy for a raster image placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImageSampling {
    /// Select the source texel containing each destination pixel center.
    Nearest,
}

/// A validated source crop and destination rectangle for one image command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePlacement {
    image: Arc<RasterImage>,
    source: Rect,
    destination: Rect,
    sampling: ImageSampling,
}

impl ImagePlacement {
    /// Creates a nearest-neighbor image placement.
    ///
    /// The source rectangle must be a positive, in-bounds crop. Destination
    /// coordinates may be off-screen for clipping, but its dimensions must be
    /// positive.
    ///
    /// # Errors
    /// Returns [`ErrorCode::RenderFailure`] when either rectangle violates its
    /// geometry contract.
    pub fn new(
        image: impl Into<Arc<RasterImage>>,
        source: Rect,
        destination: Rect,
        sampling: ImageSampling,
    ) -> Result<Self> {
        let image = image.into();
        let image_width = i64::from(image.width());
        let image_height = i64::from(image.height());
        let source_right = i64::from(source.x) + i64::from(source.width);
        let source_bottom = i64::from(source.y) + i64::from(source.height);
        if source.x < 0
            || source.y < 0
            || source.width <= 0
            || source.height <= 0
            || source_right > image_width
            || source_bottom > image_height
        {
            return Err(image_error(
                "Image source rectangle is empty or outside image bounds",
            ));
        }
        if destination.width <= 0 || destination.height <= 0 {
            return Err(image_error(
                "Image destination rectangle must have positive dimensions",
            ));
        }
        Ok(Self {
            image,
            source,
            destination,
            sampling,
        })
    }

    /// Returns the shared immutable source image.
    #[must_use]
    pub fn image(&self) -> &RasterImage {
        &self.image
    }

    /// Returns the validated source crop.
    #[must_use]
    pub const fn source(&self) -> Rect {
        self.source
    }

    /// Returns the destination rectangle, which may extend beyond a surface.
    #[must_use]
    pub const fn destination(&self) -> Rect {
        self.destination
    }

    /// Returns the sampling policy.
    #[must_use]
    pub const fn sampling(&self) -> ImageSampling {
        self.sampling
    }

    pub(crate) fn render_to(&self, framebuffer: &mut Framebuffer) {
        match self.sampling {
            ImageSampling::Nearest => {}
        }
        let destination_left = i64::from(self.destination.x);
        let destination_top = i64::from(self.destination.y);
        let destination_right = destination_left + i64::from(self.destination.width);
        let destination_bottom = destination_top + i64::from(self.destination.height);
        let clip_left = destination_left.clamp(0, i64::from(framebuffer.width()));
        let clip_top = destination_top.clamp(0, i64::from(framebuffer.height()));
        let clip_right = destination_right.clamp(0, i64::from(framebuffer.width()));
        let clip_bottom = destination_bottom.clamp(0, i64::from(framebuffer.height()));
        if clip_left >= clip_right || clip_top >= clip_bottom {
            return;
        }

        let source_x = i64::from(self.source.x);
        let source_y = i64::from(self.source.y);
        let source_width = i64::from(self.source.width);
        let source_height = i64::from(self.source.height);
        let destination_width = i64::from(self.destination.width);
        let destination_height = i64::from(self.destination.height);
        let image_width = u64::from(self.image.width());
        for y in clip_top..clip_bottom {
            let relative_y = y - destination_top;
            let selected_y = source_y + relative_y * source_height / destination_height;
            let row_offset = u64::try_from(selected_y)
                .expect("invariant: validated image source coordinate is nonnegative")
                * image_width;
            for x in clip_left..clip_right {
                let relative_x = x - destination_left;
                let selected_x = source_x + relative_x * source_width / destination_width;
                let source_index = usize::try_from(
                    row_offset
                        + u64::try_from(selected_x)
                            .expect("invariant: validated image source coordinate is nonnegative"),
                )
                .expect("invariant: validated image storage fits addressable memory");
                let color = *self
                    .image
                    .pixels()
                    .get(source_index)
                    .expect("invariant: validated crop maps inside image storage");
                framebuffer.blend_pixel(
                    i32::try_from(x).expect("invariant: clipped framebuffer x fits i32"),
                    i32::try_from(y).expect("invariant: clipped framebuffer y fits i32"),
                    color,
                );
            }
        }
    }
}

fn image_error(message: impl Into<String>) -> MetisError {
    MetisError::ui(ErrorCode::RenderFailure, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{DisplayCommand, DisplayList};

    fn image_2x1() -> RasterImage {
        RasterImage::new(2, 1, vec![Color::RED, Color::BLUE]).expect("image")
    }

    #[test]
    fn image_validation_rejects_empty_oversized_and_mismatched_storage() {
        for (width, height) in [(0, 1), (1, 0), (4097, 4096), (u32::MAX, 1)] {
            let error = RasterImage::new(width, height, Vec::<Color>::new())
                .expect_err("invalid image dimensions");
            assert_eq!(error.code, ErrorCode::SurfaceAllocationError);
        }
        let error = RasterImage::new(2, 1, vec![Color::RED]).expect_err("mismatched pixels");
        assert_eq!(error.code, ErrorCode::RenderFailure);
        assert!(error.message.contains("2x1"));
    }

    #[test]
    fn placement_validation_rejects_invalid_source_and_destination() {
        let image = image_2x1();
        for source in [
            Rect::new(-1, 0, 1, 1),
            Rect::new(0, 0, 0, 1),
            Rect::new(1, 0, 2, 1),
            Rect::new(0, 1, 1, 1),
        ] {
            let error = ImagePlacement::new(
                image.clone(),
                source,
                Rect::new(0, 0, 1, 1),
                ImageSampling::Nearest,
            )
            .expect_err("invalid image source");
            assert_eq!(error.code, ErrorCode::RenderFailure);
        }
        for destination in [Rect::new(0, 0, 0, 1), Rect::new(0, 0, 1, -1)] {
            let error = ImagePlacement::new(
                image.clone(),
                Rect::new(0, 0, 2, 1),
                destination,
                ImageSampling::Nearest,
            )
            .expect_err("invalid image destination");
            assert_eq!(error.code, ErrorCode::RenderFailure);
        }
    }

    #[test]
    fn nearest_scaling_clips_and_preserves_painter_order() {
        let image = image_2x1();
        let placement = ImagePlacement::new(
            image,
            Rect::new(0, 0, 2, 1),
            Rect::new(-1, 0, 4, 2),
            ImageSampling::Nearest,
        )
        .expect("placement");
        let mut list = DisplayList {
            commands: vec![DisplayCommand::FillRect {
                rect: Rect::new(0, 0, 3, 2),
                color: Color::WHITE,
            }],
        };
        list.append_image(placement).expect("append image");
        let mut framebuffer = Framebuffer::new(3, 2).expect("surface");
        list.render_to(&mut framebuffer);
        for y in 0..2 {
            assert_eq!(framebuffer.get_pixel(0, y), Color::RED);
            assert_eq!(framebuffer.get_pixel(1, y), Color::BLUE);
            assert_eq!(framebuffer.get_pixel(2, y), Color::BLUE);
        }
    }

    #[test]
    fn image_alpha_composites_over_existing_surface() {
        let image = RasterImage::new(1, 1, vec![Color::rgba(0, 0, 0, 128)]).expect("image");
        let placement = ImagePlacement::new(
            image,
            Rect::new(0, 0, 1, 1),
            Rect::new(0, 0, 1, 1),
            ImageSampling::Nearest,
        )
        .expect("placement");
        let mut list = DisplayList::default();
        list.append_image(placement).expect("append image");
        let mut framebuffer = Framebuffer::new(1, 1).expect("surface");
        framebuffer.clear(Color::WHITE);
        list.render_to(&mut framebuffer);
        assert_eq!(framebuffer.get_pixel(0, 0), Color::rgba(127, 127, 127, 255));
    }
}
