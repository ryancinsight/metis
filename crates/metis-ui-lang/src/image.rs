//! Validated raster images and clipped display-list placement.

use metis_core::error::{ErrorCode, MetisError, Result};
use metis_platform::framebuffer::{Color, Framebuffer, MAX_PIXELS, Rect};
use std::sync::Arc;

mod transform;
pub use transform::ImageTransform;
use transform::{
    FlipHorizontalMapper, FlipVerticalMapper, IdentityMapper, ImageMapper, RotateClockwiseMapper,
    RotateCounterClockwiseMapper,
};

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
        let expected = pixel_count(width, height)?;
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

    /// Creates an image from row-major RGBA bytes.
    ///
    /// The byte length must equal four channels for every pixel. Conversion is
    /// performed once at the boundary, after the shared image dimensions and
    /// storage limits have been validated.
    ///
    /// # Errors
    /// Returns [`ErrorCode::SurfaceAllocationError`] for invalid dimensions or
    /// allocation failure, and [`ErrorCode::RenderFailure`] for a byte-length
    /// mismatch.
    pub fn from_rgba_bytes(width: u32, height: u32, rgba: impl AsRef<[u8]>) -> Result<Self> {
        let rgba = rgba.as_ref();
        let expected_pixels = pixel_count(width, height)?;
        let expected_bytes = expected_pixels.checked_mul(4).ok_or_else(|| {
            MetisError::ui(
                ErrorCode::SurfaceAllocationError,
                "Raster image byte count exceeds addressable storage",
            )
        })?;
        if rgba.len() != expected_bytes {
            return Err(MetisError::ui(
                ErrorCode::RenderFailure,
                format!(
                    "Raster image byte count {} does not match dimensions {}x{}",
                    rgba.len(),
                    width,
                    height
                ),
            ));
        }
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(expected_pixels).map_err(|_| {
            MetisError::ui(
                ErrorCode::SurfaceAllocationError,
                "Unable to reserve raster image pixel storage",
            )
        })?;
        for channels in rgba.chunks_exact(4) {
            let &[red, green, blue, alpha] = channels else {
                return Err(MetisError::ui(
                    ErrorCode::RenderFailure,
                    "RGBA byte chunks do not contain four channels",
                ));
            };
            pixels.push(Color::rgba(red, green, blue, alpha));
        }
        Self::new(width, height, pixels)
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

fn pixel_count(width: u32, height: u32) -> Result<usize> {
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
    usize::try_from(count).map_err(|_| {
        MetisError::ui(
            ErrorCode::SurfaceAllocationError,
            "Raster image pixel count exceeds addressable storage",
        )
    })
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
    transform: ImageTransform,
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
            transform: ImageTransform::Identity,
        })
    }

    /// Applies a discrete source orientation to this placement.
    #[must_use]
    pub const fn with_transform(mut self, transform: ImageTransform) -> Self {
        self.transform = transform;
        self
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

    /// Returns the discrete source orientation.
    #[must_use]
    pub const fn transform(&self) -> ImageTransform {
        self.transform
    }

    /// Renders the validated placement into a framebuffer.
    ///
    /// The destination is clipped to the framebuffer, and source-over alpha
    /// uses the framebuffer's existing compositor. Rendering performs no
    /// allocation.
    ///
    /// # Panics
    /// Panics only if an invariant established by [`Self::new`] or the
    /// framebuffer's validated dimensions is broken internally.
    pub fn render_to(&self, framebuffer: &mut Framebuffer) {
        match (self.sampling, self.transform) {
            (ImageSampling::Nearest, ImageTransform::Identity) => {
                self.render_with::<IdentityMapper>(framebuffer);
            }
            (ImageSampling::Nearest, ImageTransform::FlipHorizontal) => {
                self.render_with::<FlipHorizontalMapper>(framebuffer);
            }
            (ImageSampling::Nearest, ImageTransform::FlipVertical) => {
                self.render_with::<FlipVerticalMapper>(framebuffer);
            }
            (ImageSampling::Nearest, ImageTransform::RotateClockwise) => {
                self.render_with::<RotateClockwiseMapper>(framebuffer);
            }
            (ImageSampling::Nearest, ImageTransform::RotateCounterClockwise) => {
                self.render_with::<RotateCounterClockwiseMapper>(framebuffer);
            }
        }
    }

    fn render_with<M: ImageMapper>(&self, framebuffer: &mut Framebuffer) {
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
            for x in clip_left..clip_right {
                let relative_x = x - destination_left;
                let (local_x, local_y) = M::map(
                    relative_x,
                    relative_y,
                    source_width,
                    source_height,
                    destination_width,
                    destination_height,
                );
                let selected_x = source_x + local_x;
                let selected_y = source_y + local_y;
                let source_index = usize::try_from(
                    u64::try_from(selected_y)
                        .expect("invariant: validated image source coordinate is nonnegative")
                        * image_width
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
mod tests;
