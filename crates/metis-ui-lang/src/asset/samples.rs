//! In-place expansion of decoded PNG samples to straight RGBA8 pixels.

use super::{AssetError, AssetErrorKind};

/// Expands `samples` eight-bit channels per pixel into four-channel pixels.
///
/// `bytes` holds `count` output pixels of four bytes each. On entry the decoded
/// samples occupy its last `samples * count` bytes; on return it holds the
/// expanded pixels. Pixel `i` is read from `[(4 - samples) * count + samples * i, ..)`
/// and written to `[4 * i, 4 * i + 4)`. The write ends at `4 * i + 4`, and the
/// first unread sample, that of pixel `i + 1`, starts at
/// `4 * i + 4 + (4 - samples) * (count - i - 1)`, so a forward pass never
/// overwrites a sample it has yet to read. Expanding in the buffer that
/// receives the decoded samples avoids a second image-sized allocation.
///
/// # Errors
/// Returns [`AssetErrorKind::Unsupported`] for a channel count other than one
/// through four and [`AssetErrorKind::Malformed`] when `bytes` is not a whole
/// number of pixels.
pub(super) fn expand_to_rgba(bytes: &mut [u8], samples: usize) -> Result<(), AssetError> {
    if !(1..=4).contains(&samples) {
        return Err(AssetError::new(AssetErrorKind::Unsupported));
    }
    let (count, remainder) = (bytes.len() / 4, bytes.len() % 4);
    if remainder != 0 {
        return Err(AssetError::new(AssetErrorKind::Malformed));
    }
    if samples == 4 {
        return Ok(());
    }
    let mut source = (4 - samples) * count;
    for write in (0..count).map(|pixel| pixel * 4) {
        let next = source + samples;
        let sample = bytes
            .get(source..next)
            .ok_or_else(|| AssetError::new(AssetErrorKind::Malformed))?;
        let pixel = match *sample {
            [gray] => [gray, gray, gray, 255],
            [gray, alpha] => [gray, gray, gray, alpha],
            [red, green, blue] => [red, green, blue, 255],
            [red, green, blue, alpha] => [red, green, blue, alpha],
            _ => return Err(AssetError::new(AssetErrorKind::Unsupported)),
        };
        bytes
            .get_mut(write..write + 4)
            .ok_or_else(|| AssetError::new(AssetErrorKind::Malformed))?
            .copy_from_slice(&pixel);
        source = next;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn separate_expansion(samples: &[u8], channels: usize) -> Vec<u8> {
        samples
            .chunks_exact(channels)
            .flat_map(|sample| match *sample {
                [gray] => [gray, gray, gray, 255],
                [gray, alpha] => [gray, gray, gray, alpha],
                [red, green, blue] => [red, green, blue, 255],
                [red, green, blue, alpha] => [red, green, blue, alpha],
                _ => unreachable!("channel counts are one through four"),
            })
            .collect()
    }

    #[test]
    fn in_tail_expansion_matches_expansion_into_separate_storage() {
        for channels in 1..=4 {
            for count in [1_usize, 2, 3, 5, 8, 31] {
                let samples: Vec<u8> = (0..count * channels)
                    .map(|index| u8::try_from(index * 37 % 251).expect("residue is below 251"))
                    .collect();
                let mut bytes = vec![0_u8; count * 4];
                let tail = bytes.len() - samples.len();
                bytes[tail..].copy_from_slice(&samples);
                expand_to_rgba(&mut bytes, channels).expect("whole pixels");
                assert_eq!(
                    bytes,
                    separate_expansion(&samples, channels),
                    "{channels} channel(s), {count} pixel(s)"
                );
            }
        }
    }

    #[test]
    fn unsupported_channel_counts_and_partial_pixels_are_rejected() {
        for channels in [0, 5] {
            let error = expand_to_rgba(&mut [0; 8], channels).expect_err("channel count");
            assert_eq!(error.kind(), AssetErrorKind::Unsupported);
        }
        let error = expand_to_rgba(&mut [0; 7], 3).expect_err("partial pixel");
        assert_eq!(error.kind(), AssetErrorKind::Malformed);
    }
}
