//! Resource bounds for one fragment exchange.
//!
//! Every limit the fragment protocol enforces, in one place, so a reader
//! checking a bound against the specification is not reading it out of the
//! module tree.

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

pub(crate) const ACTION_PREFIX_BYTES: usize = 16;
pub(crate) const PATCH_SET_PREFIX_BYTES: usize = 12;
