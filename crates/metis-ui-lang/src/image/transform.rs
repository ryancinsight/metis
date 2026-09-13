//! Pixel-grid orientation strategies for raster placement.

use metis_core::error::{ErrorCode, MetisError, Result};

/// A finite, invertible affine mapping in normalized image coordinates.
///
/// The six coefficients represent the source-to-destination mapping
/// `x' = a*x + c*y + tx` and `y' = b*x + d*y + ty`. Coordinates are normalized
/// to the source and destination rectangles, with the origin at their top-left
/// corner. Construction rejects non-finite coefficients, singular matrices and
/// matrices whose inverse cannot be represented by finite values. The
/// renderer samples the inverse mapping with nearest-neighbor semantics.
#[derive(Debug, Clone, Copy)]
pub struct AffineTransform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    tx: f64,
    ty: f64,
}

impl AffineTransform {
    /// Creates a finite, invertible source-to-destination mapping.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::RenderFailure`] when a coefficient is non-finite,
    /// the linear part is singular, or its inverse overflows.
    pub fn new(a: f64, b: f64, c: f64, d: f64, tx: f64, ty: f64) -> Result<Self> {
        let values = [a, b, c, d, tx, ty].map(canonical_zero);
        if values.iter().any(|value| !value.is_finite()) {
            return Err(transform_error(
                "Affine transform coefficients must be finite",
            ));
        }
        let [a, b, c, d, tx, ty] = values;
        let determinant = a.mul_add(d, -(b * c));
        if determinant == 0.0 || !determinant.is_finite() {
            return Err(transform_error("Affine transform linear part is singular"));
        }
        let inverse = inverse_coefficients(a, b, c, d, tx, ty, determinant);
        if inverse.iter().any(|value| !value.is_finite()) {
            return Err(transform_error("Affine transform inverse is not finite"));
        }
        Ok(Self { a, b, c, d, tx, ty })
    }

    /// Returns the identity mapping.
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// Returns coefficients in row-major affine order `[a, b, c, d, tx, ty]`.
    #[must_use]
    pub const fn coefficients(self) -> [f64; 6] {
        [self.a, self.b, self.c, self.d, self.tx, self.ty]
    }

    pub(super) fn inverse(self) -> Self {
        let determinant = self.a.mul_add(self.d, -(self.b * self.c));
        let [a, b, c, d, tx, ty] = inverse_coefficients(
            self.a,
            self.b,
            self.c,
            self.d,
            self.tx,
            self.ty,
            determinant,
        );
        Self { a, b, c, d, tx, ty }
    }

    pub(super) fn map(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a.mul_add(x, self.c.mul_add(y, self.tx)),
            self.b.mul_add(x, self.d.mul_add(y, self.ty)),
        )
    }
}

impl PartialEq for AffineTransform {
    fn eq(&self, other: &Self) -> bool {
        self.coefficients()
            .iter()
            .zip(other.coefficients())
            .all(|(left, right)| left.to_bits() == right.to_bits())
    }
}

impl Eq for AffineTransform {}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn inverse_coefficients(
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    tx: f64,
    ty: f64,
    determinant: f64,
) -> [f64; 6] {
    let inverse_a = d / determinant;
    let inverse_b = -b / determinant;
    let inverse_c = -c / determinant;
    let inverse_d = a / determinant;
    [
        inverse_a,
        inverse_b,
        inverse_c,
        inverse_d,
        -inverse_a.mul_add(tx, inverse_c * ty),
        -inverse_b.mul_add(tx, inverse_d * ty),
    ]
}

fn transform_error(message: &'static str) -> MetisError {
    MetisError::ui(ErrorCode::RenderFailure, message)
}

/// Discrete transform applied while sampling a raster image placement.
///
/// The transform operates on the validated source crop before it is scaled
/// into the destination rectangle. It is limited to pixel-grid orientation
/// operations, so browser, native and deterministic software hosts share the
/// same mapping without floating-point rounding.
///
/// # Examples
///
/// ```
/// use metis_ui_lang::ImageTransform;
///
/// let transform = ImageTransform::RotateClockwise;
/// assert_eq!(transform, ImageTransform::RotateClockwise);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImageTransform {
    /// Preserve source row and column order.
    Identity,
    /// Reverse each source row.
    FlipHorizontal,
    /// Reverse source row order.
    FlipVertical,
    /// Rotate the source crop clockwise by a quarter turn.
    RotateClockwise,
    /// Rotate the source crop counter-clockwise by a quarter turn.
    RotateCounterClockwise,
    /// Apply a validated arbitrary affine mapping in normalized coordinates.
    Affine(AffineTransform),
}

pub(super) trait ImageMapper {
    fn map(
        relative_x: i64,
        relative_y: i64,
        source_width: i64,
        source_height: i64,
        destination_width: i64,
        destination_height: i64,
    ) -> (i64, i64);
}

pub(super) struct IdentityMapper;

impl ImageMapper for IdentityMapper {
    fn map(
        relative_x: i64,
        relative_y: i64,
        source_width: i64,
        source_height: i64,
        destination_width: i64,
        destination_height: i64,
    ) -> (i64, i64) {
        (
            relative_x * source_width / destination_width,
            relative_y * source_height / destination_height,
        )
    }
}

pub(super) struct FlipHorizontalMapper;

impl ImageMapper for FlipHorizontalMapper {
    fn map(
        relative_x: i64,
        relative_y: i64,
        source_width: i64,
        source_height: i64,
        destination_width: i64,
        destination_height: i64,
    ) -> (i64, i64) {
        (
            source_width - 1 - relative_x * source_width / destination_width,
            relative_y * source_height / destination_height,
        )
    }
}

pub(super) struct FlipVerticalMapper;

impl ImageMapper for FlipVerticalMapper {
    fn map(
        relative_x: i64,
        relative_y: i64,
        source_width: i64,
        source_height: i64,
        destination_width: i64,
        destination_height: i64,
    ) -> (i64, i64) {
        (
            relative_x * source_width / destination_width,
            source_height - 1 - relative_y * source_height / destination_height,
        )
    }
}

pub(super) struct RotateClockwiseMapper;

impl ImageMapper for RotateClockwiseMapper {
    fn map(
        relative_x: i64,
        relative_y: i64,
        source_width: i64,
        source_height: i64,
        destination_width: i64,
        destination_height: i64,
    ) -> (i64, i64) {
        let output_x = relative_x * source_height / destination_width;
        let output_y = relative_y * source_width / destination_height;
        (output_y, source_height - 1 - output_x)
    }
}

pub(super) struct RotateCounterClockwiseMapper;

impl ImageMapper for RotateCounterClockwiseMapper {
    fn map(
        relative_x: i64,
        relative_y: i64,
        source_width: i64,
        source_height: i64,
        destination_width: i64,
        destination_height: i64,
    ) -> (i64, i64) {
        let output_x = relative_x * source_height / destination_width;
        let output_y = relative_y * source_width / destination_height;
        (source_width - 1 - output_y, output_x)
    }
}
