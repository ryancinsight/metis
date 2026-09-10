//! Bounded, typed browser actions and DOM patch responses.

use super::wire::MAX_PAYLOAD_SIZE;
pub(super) use super::wire::{finish, take};
use crate::error::{ErrorCode, MetisError, Result};

/// Maximum UTF-8 byte length of an action identifier.
pub const MAX_FRAGMENT_ACTION_BYTES: usize = 64;
/// Maximum UTF-8 byte length of a DOM target identifier.
pub const MAX_FRAGMENT_TARGET_BYTES: usize = 64;
/// Maximum UTF-8 byte length of an attribute name.
pub const MAX_FRAGMENT_ATTRIBUTE_BYTES: usize = 64;
/// Maximum UTF-8 byte length of a text or attribute value.
pub const MAX_FRAGMENT_VALUE_BYTES: usize = 4096;
/// Maximum number of DOM mutations in one response.
pub const MAX_FRAGMENT_PATCHES: usize = 32;
/// Maximum encoded control-plane body for one fragment request or response.
///
/// The 16 KiB bound keeps DOM mutation traffic below the 65,536-byte IPC frame
/// limit while leaving room for the plugin invocation envelope.
pub const MAX_FRAGMENT_BODY_BYTES: usize = 16 * 1024;

const ACTION_PREFIX_BYTES: usize = 16;
const PATCH_SET_PREFIX_BYTES: usize = 12;

mod action;
mod patch;
#[cfg(test)]
mod tests;

pub use action::FragmentAction;
pub use patch::{FragmentPatch, FragmentPatchSet};

fn action_length(action: &str, target: &str, input_len: usize) -> Result<usize> {
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

fn check_fragment_length(length: usize) -> Result<()> {
    if length > MAX_FRAGMENT_BODY_BYTES || length > MAX_PAYLOAD_SIZE {
        return Err(too_large("Fragment body exceeds its resource bound"));
    }
    Ok(())
}

fn validate_identifier(value: &str, max_bytes: usize, field: &str) -> Result<()> {
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

fn validate_attribute_name(value: &str) -> Result<()> {
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

fn validate_text(value: &str, max_bytes: usize, field: &str) -> Result<()> {
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

fn encode_string_u16(encoded: &mut Vec<u8>, value: &str, field: &str) -> Result<()> {
    let length = u16::try_from(value.len()).map_err(|_| {
        too_large(match field {
            "Fragment target identifier" => "Fragment target identifier exceeds its wire length",
            "Fragment attribute name" => "Fragment attribute name exceeds its wire length",
            _ => "Fragment string exceeds its wire length",
        })
    })?;
    encoded.extend_from_slice(&length.to_be_bytes());
    encoded.extend_from_slice(value.as_bytes());
    Ok(())
}

fn encode_string_u32(encoded: &mut Vec<u8>, value: &str, field: &str) -> Result<()> {
    let length = u32::try_from(value.len()).map_err(|_| {
        too_large(match field {
            "Fragment action input" => "Fragment action input exceeds its wire length",
            _ => "Fragment string exceeds its wire length",
        })
    })?;
    encoded.extend_from_slice(&length.to_be_bytes());
    encoded.extend_from_slice(value.as_bytes());
    Ok(())
}

fn take_string_u16(buf: &mut &[u8], max_bytes: usize, field: &str) -> Result<String> {
    let length = usize::from(u16::from_be_bytes(take(buf)?));
    take_string(buf, length, max_bytes, field)
}

fn take_string_u32(buf: &mut &[u8], max_bytes: usize, field: &str) -> Result<String> {
    let length = usize::try_from(u32::from_be_bytes(take(buf)?))
        .map_err(|_| too_large("Fragment string length cannot fit this target"))?;
    take_string(buf, length, max_bytes, field)
}

fn take_string(buf: &mut &[u8], length: usize, max_bytes: usize, field: &str) -> Result<String> {
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

fn malformed(message: &'static str) -> MetisError {
    MetisError::protocol(ErrorCode::MalformedPayload, message)
}

fn too_large(message: &'static str) -> MetisError {
    MetisError::protocol(ErrorCode::PayloadTooLarge, message)
}
