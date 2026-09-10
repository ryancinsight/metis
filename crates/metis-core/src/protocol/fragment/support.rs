use super::super::wire::MAX_PAYLOAD_SIZE;
use super::{ACTION_PREFIX_BYTES, MAX_FRAGMENT_ATTRIBUTE_BYTES, MAX_FRAGMENT_BODY_BYTES};
use crate::error::{ErrorCode, MetisError, Result};

pub(crate) fn action_length(action: &str, target: &str, input_len: usize) -> Result<usize> {
    let total = ACTION_PREFIX_BYTES
        .checked_add(action.len())
        .and_then(|size| size.checked_add(target.len()))
        .and_then(|size| size.checked_add(input_len))
        .ok_or_else(|| too_large("Fragment action size overflows its resource bound"))?;
    check_fragment_length(total)?;
    Ok(total)
}

pub(crate) fn check_fragment_length(length: usize) -> Result<()> {
    if length > MAX_FRAGMENT_BODY_BYTES || length > MAX_PAYLOAD_SIZE {
        return Err(too_large("Fragment body exceeds its resource bound"));
    }
    Ok(())
}

pub(crate) fn validate_identifier(value: &str, max_bytes: usize, field: &str) -> Result<()> {
    let valid = value.len() <= max_bytes
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        });
    valid.then_some(()).ok_or_else(|| {
        malformed(match field {
            "action" => "Fragment action identifier is invalid",
            "target" => "Fragment target identifier is invalid",
            _ => "Fragment identifier is invalid",
        })
    })
}

pub(crate) fn validate_attribute_name(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_FRAGMENT_ATTRIBUTE_BYTES
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b':' | b'_')
        })
    {
        return Err(malformed("Fragment attribute name is invalid"));
    }
    Ok(())
}

pub(crate) fn validate_text(value: &str, max_bytes: usize, field: &str) -> Result<()> {
    if value.len() > max_bytes {
        return Err(too_large(match field {
            "input" => "Fragment action input exceeds its resource bound",
            "text value" => "Fragment text value exceeds its resource bound",
            "attribute value" => "Fragment attribute value exceeds its resource bound",
            "child text" => "Fragment child text exceeds its resource bound",
            _ => "Fragment text exceeds its resource bound",
        }));
    }
    Ok(())
}

pub(crate) fn encode_string<const WIDTH: usize>(
    encoded: &mut Vec<u8>,
    value: &str,
    field: &str,
) -> Result<()> {
    let limit = match WIDTH {
        2 => usize::from(u16::MAX),
        4 => usize::try_from(u32::MAX)
            .map_err(|_| too_large("Fragment string length cannot fit this target"))?,
        _ => return Err(malformed("Fragment wire length width is invalid")),
    };
    let length = u64::try_from(value.len())
        .map_err(|_| too_large("Fragment string length cannot fit this target"))?;
    if value.len() > limit {
        return Err(too_large(match field {
            "Fragment target identifier" => "Fragment target identifier exceeds its wire length",
            "Fragment attribute name" => "Fragment attribute name exceeds its wire length",
            "Fragment action input" => "Fragment action input exceeds its wire length",
            _ => "Fragment string exceeds its wire length",
        }));
    }
    let bytes = length.to_be_bytes();
    encoded.extend_from_slice(&bytes[8 - WIDTH..]);
    encoded.extend_from_slice(value.as_bytes());
    Ok(())
}

pub(crate) fn take_string<const WIDTH: usize>(
    buf: &mut &[u8],
    max_bytes: usize,
    field: &str,
) -> Result<String> {
    if !matches!(WIDTH, 2 | 4) {
        return Err(malformed("Fragment wire length width is invalid"));
    }
    let prefix = take(buf, WIDTH)?;
    let length = prefix
        .iter()
        .try_fold(0usize, |value, byte| {
            value
                .checked_mul(256)
                .and_then(|value| value.checked_add(usize::from(*byte)))
        })
        .ok_or_else(|| too_large("Fragment string length cannot fit this target"))?;
    take_string_bytes(buf, length, max_bytes, field)
}

pub(crate) fn take_string_bytes(
    buf: &mut &[u8],
    length: usize,
    max_bytes: usize,
    field: &str,
) -> Result<String> {
    if length > max_bytes {
        return Err(too_large(match field {
            "Fragment action identifier" => "Fragment action identifier exceeds its resource bound",
            "Fragment target identifier" => "Fragment target identifier exceeds its resource bound",
            "Fragment attribute name" => "Fragment attribute name exceeds its resource bound",
            _ => "Fragment string exceeds its resource bound",
        }));
    }
    let (bytes, remaining) = buf
        .split_at_checked(length)
        .ok_or_else(|| malformed("Fragment string is truncated"))?;
    let value = std::str::from_utf8(bytes)
        .map_err(|_| malformed("Fragment string is not valid UTF-8"))?
        .to_owned();
    *buf = remaining;
    Ok(value)
}

fn take<'a>(buf: &mut &'a [u8], length: usize) -> Result<&'a [u8]> {
    let (head, tail) = buf
        .split_at_checked(length)
        .ok_or_else(|| malformed("Fragment string is truncated"))?;
    *buf = tail;
    Ok(head)
}

pub(crate) fn malformed(message: &'static str) -> MetisError {
    MetisError::protocol(ErrorCode::MalformedPayload, message)
}
pub(crate) fn too_large(message: &'static str) -> MetisError {
    MetisError::protocol(ErrorCode::PayloadTooLarge, message)
}
