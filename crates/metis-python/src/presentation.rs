//! `PyO3` access to the bounded Rust software presentation contract.

use crate::error::map_error;
use metis_platform::framebuffer::{Color, Framebuffer, Rect as RustRect};
use metis_ui_lang::{ImagePlacement, ImageSampling, RasterImage};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

/// Integer rectangle used for image crops and canvas destinations.
#[pyclass(frozen, name = "Rect")]
pub(crate) struct PyRect {
    inner: RustRect,
}

#[pymethods]
impl PyRect {
    /// Creates a half-open rectangle.
    #[new]
    fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            inner: RustRect::new(x, y, width, height),
        }
    }

    /// Returns the left coordinate.
    #[getter]
    fn x(&self) -> i32 {
        self.inner.x
    }

    /// Returns the top coordinate.
    #[getter]
    fn y(&self) -> i32 {
        self.inner.y
    }

    /// Returns the horizontal extent.
    #[getter]
    fn width(&self) -> i32 {
        self.inner.width
    }

    /// Returns the vertical extent.
    #[getter]
    fn height(&self) -> i32 {
        self.inner.height
    }
}

/// Immutable row-major RGBA image owned by Rust.
#[pyclass(frozen, name = "RasterImage")]
pub(crate) struct PyRasterImage {
    inner: RasterImage,
}

#[pymethods]
impl PyRasterImage {
    /// Creates an image from exact row-major RGBA bytes.
    #[new]
    fn new(width: u32, height: u32, rgba: Vec<u8>) -> PyResult<Self> {
        RasterImage::from_rgba_bytes(width, height, rgba)
            .map(|inner| Self { inner })
            .map_err(|error| map_error(&error))
    }

    /// Returns the horizontal pixel count.
    #[getter]
    fn width(&self) -> u32 {
        self.inner.width()
    }

    /// Returns the vertical pixel count.
    #[getter]
    fn height(&self) -> u32 {
        self.inner.height()
    }
}

/// Bounded Rust software canvas for Python composition.
#[pyclass(name = "Canvas")]
pub(crate) struct Canvas {
    framebuffer: Framebuffer,
}

#[pymethods]
impl Canvas {
    /// Allocates a transparent canvas under the shared framebuffer limit.
    #[new]
    fn new(width: u32, height: u32) -> PyResult<Self> {
        Framebuffer::new(width, height)
            .map(|framebuffer| Self { framebuffer })
            .map_err(|error| map_error(&error))
    }

    /// Returns the horizontal pixel count.
    #[getter]
    fn width(&self) -> u32 {
        self.framebuffer.width()
    }

    /// Returns the vertical pixel count.
    #[getter]
    fn height(&self) -> u32 {
        self.framebuffer.height()
    }

    /// Replaces every pixel with a straight RGBA color.
    fn clear(&mut self, red: u8, green: u8, blue: u8, alpha: u8) {
        self.framebuffer.clear(Color::rgba(red, green, blue, alpha));
    }

    /// Composites a validated image crop into the canvas with nearest sampling.
    ///
    /// # Errors
    /// Returns a `ValueError` carrying the stable Metis error code when the
    /// source crop or destination rectangle is invalid.
    fn draw_image(
        &mut self,
        image: &PyRasterImage,
        source: &PyRect,
        destination: &PyRect,
    ) -> PyResult<()> {
        let placement = ImagePlacement::new(
            image.inner.clone(),
            source.inner,
            destination.inner,
            ImageSampling::Nearest,
        )
        .map_err(|error| map_error(&error))?;
        placement.render_to(&mut self.framebuffer);
        Ok(())
    }

    /// Returns a cold-boundary copy of row-major straight RGBA bytes.
    ///
    /// # Errors
    /// Returns a `ValueError` if the result buffer cannot be reserved.
    fn to_rgba<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let pixel_count = usize::try_from(
            u64::from(self.framebuffer.width()) * u64::from(self.framebuffer.height()),
        )
        .map_err(|_| map_error(&allocation_error()))?;
        let byte_count = pixel_count
            .checked_mul(4)
            .ok_or_else(|| map_error(&allocation_error()))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(byte_count)
            .map_err(|_| map_error(&allocation_error()))?;
        for y in 0..self.framebuffer.height() {
            for x in 0..self.framebuffer.width() {
                let color = self.framebuffer.get_pixel(
                    i32::try_from(x).expect("invariant: framebuffer width fits i32"),
                    i32::try_from(y).expect("invariant: framebuffer height fits i32"),
                );
                bytes.extend([color.r, color.g, color.b, color.a]);
            }
        }
        Ok(PyBytes::new(py, &bytes))
    }
}

fn allocation_error() -> metis_core::MetisError {
    metis_core::MetisError::ui(
        metis_core::error::ErrorCode::SurfaceAllocationError,
        "Unable to reserve Python presentation byte storage",
    )
}
