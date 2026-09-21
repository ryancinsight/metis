//! Convert provider-owned JPEG samples into the host raster contract.

use super::{
    AssetError, AssetErrorKind, MAX_DECODE_BYTES, MAX_ENCODED_IMAGE_BYTES, MAX_IMAGE_DIMENSION,
    MAX_IMAGE_PIXELS, orientation,
};
use crate::RasterImage;
use consus_raster::{DecodeLimits, PixelFormat};
use metis_platform::Color;

pub(super) fn decode(bytes: &[u8]) -> Result<RasterImage, AssetError> {
    let decoded = consus_raster::jpeg::decode(
        bytes,
        DecodeLimits {
            max_encoded_bytes: MAX_ENCODED_IMAGE_BYTES,
            max_dimension: MAX_IMAGE_DIMENSION,
            max_pixels: MAX_IMAGE_PIXELS,
            max_working_bytes: MAX_DECODE_BYTES,
        },
    )?;
    let width = decoded.width();
    let height = decoded.height();
    let orientation = decoded.orientation();
    if !matches!(decoded.format(), PixelFormat::Gray | PixelFormat::Rgb) {
        return Err(AssetError::new(AssetErrorKind::Unsupported));
    }
    let count = usize::try_from(u64::from(width) * u64::from(height))
        .map_err(|_| AssetError::new(AssetErrorKind::TooLarge))?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(count)
        .map_err(|_| AssetError::new(AssetErrorKind::Allocation))?;
    match decoded.format() {
        PixelFormat::Gray => pixels.extend(
            decoded
                .pixels()
                .iter()
                .map(|&gray| Color::rgb(gray, gray, gray)),
        ),
        PixelFormat::Rgb => {
            for sample in decoded.pixels().chunks_exact(3) {
                let &[red, green, blue] = sample else {
                    return Err(AssetError::new(AssetErrorKind::Malformed));
                };
                pixels.push(Color::rgb(red, green, blue));
            }
        }
        _ => return Err(AssetError::new(AssetErrorKind::Unsupported)),
    }
    drop(decoded);
    orientation::apply(width, height, pixels, orientation)
}
