//! Bounded, typed browser actions and DOM patch responses.

pub(super) use super::wire::{finish, take};

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
mod support;
#[cfg(test)]
mod tests;

pub use action::FragmentAction;
pub use patch::{FragmentPatch, FragmentPatchSet};
pub(super) use support::{
    action_length, check_fragment_length, encode_string, malformed, take_string, take_string_bytes,
    too_large, validate_attribute_name, validate_identifier, validate_text,
};
