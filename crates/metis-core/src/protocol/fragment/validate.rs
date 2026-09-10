//! Field validation and the length arithmetic that precedes encoding.

use super::errors::{malformed, too_large};
use super::limits::{ACTION_PREFIX_BYTES, MAX_FRAGMENT_ATTRIBUTE_BYTES, MAX_FRAGMENT_BODY_BYTES};
use crate::error::Result;
use crate::protocol::wire::MAX_PAYLOAD_SIZE;

pub(crate) fn action_length(action: &str, target: &str, input_len: usize) -> Result<usize> {
    let total = ACTION_PREFIX_BYTES
        .checked_add(action.len())
        .and_then(|size| size.checked_add(target.len()))
        .and_then(|size| size.checked_add(input_len))
        .ok_or_else(|| too_large("Fragment action size overflows its resource bound"))?;
    check_fragment_length(total)?;
    if total > MAX_FRAGMENT_BODY_BYTES {
        return Err(too_large("Fragment action exceeds its resource bound"));
    }
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
    if valid {
        Ok(())
    } else {
        Err(malformed(match field {
            "action" => "Fragment action identifier is invalid",
            "target" => "Fragment target identifier is invalid",
            _ => "Fragment identifier is invalid",
        }))
    }
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
