//! Pixel-grid orientation strategies for raster placement.

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
