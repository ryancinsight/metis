use super::{AssetError, AssetErrorKind, Cause};
use crate::RasterImage;
use consus_raster::exif::Orientation;
use metis_platform::Color;

pub(super) fn apply(
    width: u32,
    height: u32,
    pixels: Vec<Color>,
    orientation: Orientation,
) -> Result<RasterImage, AssetError> {
    if orientation == Orientation::Normal {
        return RasterImage::new(width, height, pixels).map_err(raster_error);
    }
    let (output_width, output_height) = if orientation.swaps_axes() {
        (height, width)
    } else {
        (width, height)
    };
    let count = pixels.len();
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| AssetError::new(AssetErrorKind::Allocation))?;
    for y in 0..output_height {
        for x in 0..output_width {
            let (source_x, source_y) = orientation.source_coordinate(width, height, x, y)?;
            let index =
                usize::try_from(u64::from(source_y) * u64::from(width) + u64::from(source_x))
                    .map_err(|_| AssetError::new(AssetErrorKind::TooLarge))?;
            output.push(
                *pixels
                    .get(index)
                    .ok_or_else(|| AssetError::new(AssetErrorKind::Malformed))?,
            );
        }
    }
    drop(pixels);
    RasterImage::new(output_width, output_height, output).map_err(raster_error)
}

fn raster_error(error: metis_core::error::MetisError) -> AssetError {
    let kind = if error.code == metis_core::error::ErrorCode::SurfaceAllocationError {
        AssetErrorKind::Allocation
    } else {
        AssetErrorKind::Malformed
    };
    AssetError {
        kind,
        cause: Some(Cause::Raster(error)),
    }
}
