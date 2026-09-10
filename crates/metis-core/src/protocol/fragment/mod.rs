//! Bounded, typed browser actions and DOM patch responses.

mod action;
mod errors;
mod limits;
mod patch;
mod strings;
#[cfg(test)]
mod tests;

pub(super) use super::wire::{finish, take};
pub use action::FragmentAction;
pub(crate) use errors::{malformed, too_large};
pub(crate) use limits::PATCH_SET_PREFIX_BYTES;
pub use limits::{
    MAX_FRAGMENT_ACTION_BYTES, MAX_FRAGMENT_ATTRIBUTE_BYTES, MAX_FRAGMENT_BODY_BYTES,
    MAX_FRAGMENT_PATCHES, MAX_FRAGMENT_TARGET_BYTES, MAX_FRAGMENT_VALUE_BYTES,
};
pub use patch::{FragmentPatch, FragmentPatchSet};
pub(crate) use strings::{encode_string, take_prefixed_string, take_string};
pub(crate) use validate::{
    action_length, check_fragment_length, validate_attribute_name, validate_identifier,
    validate_text,
};
mod validate;
