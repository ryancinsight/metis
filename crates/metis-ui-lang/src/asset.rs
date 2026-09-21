//! Bounded native PNG and JPEG admission before raster presentation.
//!
//! Static PNG and JPEG decode to straight row-major
//! RGBA. EXIF orientation is normalized into the returned pixel grid. Metadata
//! without defined presentation semantics, including color profiles, fails
//! closed rather than being silently ignored.

use crate::RasterImage;
use metis_core::capability::CapabilityScope;
use metis_core::host::VerifiedHostCapability;
use metis_platform::{Color, ScopedFileProvider};
use std::{fmt, io, path::Path};

mod compression;
mod jpeg;
mod orientation;

/// Maximum encoded image bytes, matching the scoped file-read budget.
pub const MAX_ENCODED_IMAGE_BYTES: usize = 64 * 1024 * 1024;
/// Maximum width or height, matching the Windows presentation provider.
pub const MAX_IMAGE_DIMENSION: u32 = 16_384;
/// Maximum decoded pixels, matching the framebuffer allocation budget.
pub const MAX_IMAGE_PIXELS: usize = metis_platform::framebuffer::MAX_PIXELS;
const MAX_DECODE_BYTES: usize = MAX_IMAGE_PIXELS * 4;
const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// Classification of a rejected native image asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AssetErrorKind {
    /// The input is malformed, truncated or fails its checksum.
    Malformed,
    /// A format or metadata feature has no admitted presentation semantics.
    Unsupported,
    /// An encoded, dimension, pixel or decoder budget is exceeded.
    TooLarge,
    /// The scoped file cannot be opened or read.
    Access,
    /// Bounded storage could not be reserved.
    Allocation,
}

/// An asset failure retaining its underlying decoder or filesystem cause.
#[derive(Debug)]
pub struct AssetError {
    kind: AssetErrorKind,
    cause: Option<Cause>,
}

#[derive(Debug)]
enum Cause {
    File(io::Error),
    Png(png::DecodingError),
    Codec(consus_raster::DecodeError),
    Raster(metis_core::error::MetisError),
}

impl AssetError {
    /// Returns the stable failure classification.
    #[must_use]
    pub const fn kind(&self) -> AssetErrorKind {
        self.kind
    }

    const fn new(kind: AssetErrorKind) -> Self {
        Self { kind, cause: None }
    }
}

impl fmt::Display for AssetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "native image admission failed: {:?}", self.kind)
    }
}

impl std::error::Error for AssetError {
    // Error-chain type erasure is a non-hot diagnostic boundary.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.cause.as_ref()? {
            Cause::File(error) => Some(error),
            Cause::Png(error) => Some(error),
            Cause::Codec(error) => Some(error),
            Cause::Raster(error) => Some(error),
        }
    }
}

impl From<png::DecodingError> for AssetError {
    fn from(error: png::DecodingError) -> Self {
        let kind = if matches!(error, png::DecodingError::LimitsExceeded) {
            AssetErrorKind::TooLarge
        } else {
            AssetErrorKind::Malformed
        };
        Self {
            kind,
            cause: Some(Cause::Png(error)),
        }
    }
}

impl From<consus_raster::DecodeError> for AssetError {
    fn from(error: consus_raster::DecodeError) -> Self {
        use consus_raster::DecodeErrorKind;
        let kind = match error.kind() {
            DecodeErrorKind::Malformed => AssetErrorKind::Malformed,
            DecodeErrorKind::TooLarge => AssetErrorKind::TooLarge,
            DecodeErrorKind::Allocation => AssetErrorKind::Allocation,
            _ => AssetErrorKind::Unsupported,
        };
        Self {
            kind,
            cause: Some(Cause::Codec(error)),
        }
    }
}

impl RasterImage {
    /// Decodes an admitted PNG or JPEG into straight, row-major RGBA samples.
    ///
    /// PNG palette and sub-byte grayscale samples expand losslessly. JPEG
    /// sequential and progressive DCT scans admit eight- and twelve-bit gray and
    /// RGB samples; single-component lossless scans admit two- through sixteen-bit
    /// gray samples. The provider maps each sample once to eight-bit display
    /// channels with nearest full-range integer rounding; this is not clinical
    /// windowing, rescaling or precision-preserving storage.
    /// A valid EXIF orientation is applied once so the returned dimensions and pixels
    /// already have top-left display orientation. JPEG has opaque alpha.
    /// Container checks require an exact terminal marker and no trailing bytes.
    /// Encoded input is at most 64 MiB, decoded pixels at most 16 Mi, and each
    /// edge at most 16,384. The decoder has a separate 64 MiB workspace limit;
    /// output and immutable raster storage each require at most 64 MiB.
    ///
    /// # Errors
    /// Returns a classified [`AssetError`] for malformed/truncated input,
    /// unsupported metadata or depth, exhausted budgets, or allocation failure.
    ///
    /// # Examples
    /// ```
    /// use metis_ui_lang::{asset::AssetErrorKind, RasterImage};
    ///
    /// let error = RasterImage::decode(b"not an image").expect_err("unsupported bytes");
    /// assert_eq!(error.kind(), AssetErrorKind::Unsupported);
    /// ```
    pub fn decode(bytes: &[u8]) -> Result<Self, AssetError> {
        if bytes.len() > MAX_ENCODED_IMAGE_BYTES {
            return Err(AssetError::new(AssetErrorKind::TooLarge));
        }
        if bytes.starts_with(SIGNATURE) {
            return decode_png(bytes);
        }
        if bytes.starts_with(&[0xff, 0xd8]) {
            return jpeg::decode(bytes);
        }
        if SIGNATURE.starts_with(bytes) || [0xff, 0xd8].starts_with(bytes) {
            return Err(AssetError::new(AssetErrorKind::Malformed));
        }
        Err(AssetError::new(AssetErrorKind::Unsupported))
    }

    /// Reads and decodes an image through a capability-witnessed scoped root.
    ///
    /// Absolute paths, parent traversal, links and non-regular files are
    /// rejected by the handle-based platform opener before bytes are decoded.
    ///
    /// # Errors
    /// Returns [`AssetErrorKind::Access`] with the I/O cause for a failed
    /// scoped read, or the same classifications as [`Self::decode`].
    pub fn load(
        provider: &ScopedFileProvider,
        capability: &VerifiedHostCapability<{ CapabilityScope::READ_FILE.0 }>,
        path: impl AsRef<Path>,
    ) -> Result<Self, AssetError> {
        let path = path.as_ref();
        if path.as_os_str().is_empty()
            || path
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
            || path.as_os_str().to_string_lossy().contains(':')
        {
            return Err(AssetError {
                kind: AssetErrorKind::Access,
                cause: Some(Cause::File(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "image asset path must contain only relative normal components",
                ))),
            });
        }
        let bytes = provider
            .read(capability, path)
            .map_err(|error| AssetError {
                kind: AssetErrorKind::Access,
                cause: Some(Cause::File(error)),
            })?;
        Self::decode(&bytes)
    }
}

fn decode_png(bytes: &[u8]) -> Result<RasterImage, AssetError> {
    let orientation = envelope(bytes)?;
    let mut options = png::DecodeOptions::default();
    options.set_ignore_checksums(false);
    options.set_skip_ancillary_crc_failures(false);
    let mut decoder = png::Decoder::new_with_options(io::Cursor::new(bytes), options);
    decoder.set_limits(png::Limits {
        bytes: MAX_DECODE_BYTES,
    });
    decoder.set_transformations(png::Transformations::EXPAND);
    let header = decoder.read_header_info()?;
    let count = u64::from(header.width) * u64::from(header.height);
    if header.width > MAX_IMAGE_DIMENSION
        || header.height > MAX_IMAGE_DIMENSION
        || count > MAX_IMAGE_PIXELS as u64
    {
        return Err(AssetError::new(AssetErrorKind::TooLarge));
    }
    if header.bit_depth == png::BitDepth::Sixteen {
        return Err(AssetError::new(AssetErrorKind::Unsupported));
    }
    compression::validate(bytes, header)?;
    let mut reader = decoder.read_info()?;
    let size = reader
        .output_buffer_size()
        .filter(|size| *size <= MAX_DECODE_BYTES)
        .ok_or_else(|| AssetError::new(AssetErrorKind::TooLarge))?;
    let mut channels = Vec::new();
    channels
        .try_reserve_exact(size)
        .map_err(|_| AssetError::new(AssetErrorKind::Allocation))?;
    channels.resize(size, 0);
    let output = reader.next_frame(&mut channels)?;
    reader.finish()?;
    drop(reader);
    let count = usize::try_from(count).map_err(|_| AssetError::new(AssetErrorKind::TooLarge))?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(count)
        .map_err(|_| AssetError::new(AssetErrorKind::Allocation))?;
    let samples = channels
        .get(..output.buffer_size())
        .ok_or_else(|| AssetError::new(AssetErrorKind::Malformed))?;
    for sample in samples.chunks_exact(output.color_type.samples()) {
        let pixel = match *sample {
            [gray] => Color::rgb(gray, gray, gray),
            [gray, alpha] => Color::rgba(gray, gray, gray, alpha),
            [red, green, blue] => Color::rgb(red, green, blue),
            [red, green, blue, alpha] => Color::rgba(red, green, blue, alpha),
            _ => return Err(AssetError::new(AssetErrorKind::Unsupported)),
        };
        pixels.push(pixel);
    }
    drop(channels);
    orientation::apply(output.width, output.height, pixels, orientation)
}

// PNG 3 sections 5.3 and 5.6 define length/type/data/CRC framing and IEND.
// The codec owns CRC, compression and color interpretation; this scan owns
// the host's admitted chunk set and exact end-of-input contract.
fn envelope(bytes: &[u8]) -> Result<consus_raster::exif::Orientation, AssetError> {
    if bytes.len() > MAX_ENCODED_IMAGE_BYTES {
        return Err(AssetError::new(AssetErrorKind::TooLarge));
    }
    let mut remaining = bytes
        .strip_prefix(SIGNATURE)
        .ok_or_else(|| AssetError::new(AssetErrorKind::Malformed))?;
    let mut image_header = None;
    let mut palette = None;
    let mut transparency = false;
    let mut data = false;
    let mut orientation = None;
    while !remaining.is_empty() {
        let (kind, payload) = take_chunk(&mut remaining)?;
        let length = payload.len();
        match kind {
            b"IHDR" if image_header.is_none() && length == 13 => {
                image_header = Some((payload[8], payload[9]));
            }
            b"PLTE" if image_header.is_some() && palette.is_none() && !transparency && !data => {
                let (depth, color) =
                    image_header.expect("invariant: header guard establishes presence");
                if length == 0 || length > 768 || length % 3 != 0 || matches!(color, 0 | 4) {
                    return Err(AssetError::new(AssetErrorKind::Malformed));
                }
                if color == 3 && (depth > 8 || length / 3 != 1_usize << depth) {
                    return Err(AssetError::new(AssetErrorKind::Unsupported));
                }
                palette = Some(payload.len() / 3);
            }
            b"tRNS" if image_header.is_some() && !transparency && !data => {
                let (depth, color) =
                    image_header.expect("invariant: header guard establishes presence");
                let valid = match color {
                    0 => {
                        payload.len() == 2
                            && depth <= 8
                            && payload[0] == 0
                            && u32::from(payload[1]) < 1_u32 << depth
                    }
                    2 => {
                        payload.len() == 6
                            && depth == 8
                            && payload.chunks_exact(2).all(|value| value[0] == 0)
                    }
                    3 => palette
                        .is_some_and(|entries| !payload.is_empty() && payload.len() <= entries),
                    _ => false,
                };
                if !valid {
                    return Err(AssetError::new(AssetErrorKind::Malformed));
                }
                transparency = true;
            }
            b"eXIf" if image_header.is_some() && orientation.is_none() && !data => {
                orientation = Some(consus_raster::exif::parse(payload)?);
            }
            b"IDAT" if image_header.is_some_and(|(_, color)| color != 3 || palette.is_some()) => {
                data = true;
            }
            b"IEND" if data && length == 0 && remaining.is_empty() => {
                return Ok(orientation.unwrap_or_default());
            }
            b"IHDR" | b"PLTE" | b"tRNS" | b"eXIf" | b"IDAT" | b"IEND" => {
                return Err(AssetError::new(AssetErrorKind::Malformed));
            }
            _ => return Err(AssetError::new(AssetErrorKind::Unsupported)),
        }
    }
    Err(AssetError::new(AssetErrorKind::Malformed))
}

fn take_chunk<'input>(
    remaining: &mut &'input [u8],
) -> Result<(&'input [u8], &'input [u8]), AssetError> {
    let header = remaining
        .get(..8)
        .ok_or_else(|| AssetError::new(AssetErrorKind::Malformed))?;
    let length = u32::from_be_bytes(
        header[..4]
            .try_into()
            .map_err(|_| AssetError::new(AssetErrorKind::Malformed))?,
    );
    let size = usize::try_from(length)
        .ok()
        .and_then(|size| size.checked_add(12))
        .ok_or_else(|| AssetError::new(AssetErrorKind::TooLarge))?;
    let chunk = remaining
        .get(..size)
        .ok_or_else(|| AssetError::new(AssetErrorKind::Malformed))?;
    *remaining = &remaining[size..];
    Ok((&header[4..8], &chunk[8..size - 4]))
}

#[cfg(test)]
mod tests;
