//! Exact zlib termination and scanline extent, before PNG sample conversion.

use super::{AssetError, AssetErrorKind, take_chunk};
use flate2::{Decompress, FlushDecompress, Status};

pub(super) fn validate(bytes: &[u8], header: &png::Info<'_>) -> Result<(), AssetError> {
    let expected = scanline_bytes(header);
    let mut inflater = Decompress::new(true);
    let mut ended = false;
    let mut remaining = bytes.get(8..).ok_or_else(malformed)?;
    while !remaining.is_empty() {
        let (kind, payload) = take_chunk(&mut remaining)?;
        if kind == b"IDAT" {
            ended = inflate(&mut inflater, payload, expected, ended)?;
        }
    }
    // Empty input drains buffered output but cannot manufacture a missing
    // checksum. Success requires the inflater's explicit StreamEnd verdict.
    ended = inflate(&mut inflater, &[], expected, ended)?;
    if !ended || inflater.total_out() != expected {
        return Err(malformed());
    }
    Ok(())
}

fn inflate(
    inflater: &mut Decompress,
    mut input: &[u8],
    expected: u64,
    ended: bool,
) -> Result<bool, AssetError> {
    if ended {
        return if input.is_empty() {
            Ok(true)
        } else {
            Err(malformed())
        };
    }
    // Fixed scratch storage bounds validation independently of input size.
    let mut scratch = [0_u8; 8192];
    loop {
        let consumed = inflater.total_in();
        let produced = inflater.total_out();
        let status = inflater
            .decompress(input, &mut scratch, FlushDecompress::None)
            .map_err(|_| malformed())?;
        let used = usize::try_from(inflater.total_in() - consumed).map_err(|_| malformed())?;
        input = input.get(used..).ok_or_else(malformed)?;
        if inflater.total_out() > expected {
            return Err(malformed());
        }
        if status == Status::StreamEnd {
            return if input.is_empty() {
                Ok(true)
            } else {
                Err(malformed())
            };
        }
        if used == 0 && inflater.total_out() == produced {
            return if input.is_empty() {
                Ok(false)
            } else {
                Err(malformed())
            };
        }
        // A call with no input also drains an inflater whose last call filled
        // scratch exactly; total output is checked before every repetition.
    }
}

fn scanline_bytes(header: &png::Info<'_>) -> u64 {
    let depth = match header.bit_depth {
        png::BitDepth::One => 1,
        png::BitDepth::Two => 2,
        png::BitDepth::Four => 4,
        png::BitDepth::Eight => 8,
        png::BitDepth::Sixteen => 16,
    };
    let channels = match header.color_type {
        png::ColorType::Grayscale | png::ColorType::Indexed => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
    };
    // PNG 3 section 8.1: Adam7 start columns/rows and strides. Every nonempty
    // pass row contributes a filter byte plus its byte-rounded sample width.
    let passes: &[(u32, u32, u32, u32)] = if header.interlaced {
        &[
            (0, 0, 8, 8),
            (4, 0, 8, 8),
            (0, 4, 4, 8),
            (2, 0, 4, 4),
            (0, 2, 2, 4),
            (1, 0, 2, 2),
            (0, 1, 1, 2),
        ]
    } else {
        &[(0, 0, 1, 1)]
    };
    passes
        .iter()
        .map(|&(x, y, dx, dy)| {
            let width = header.width.saturating_sub(x).div_ceil(dx);
            let height = header.height.saturating_sub(y).div_ceil(dy);
            if width == 0 {
                return 0;
            }
            (1 + (u64::from(width) * depth * channels).div_ceil(8)) * u64::from(height)
        })
        .sum()
}

fn malformed() -> AssetError {
    AssetError::new(AssetErrorKind::Malformed)
}
