//! Typed accelerator and shortcut-binding failures.

use std::{error::Error, fmt};

/// Why accelerator text was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AcceleratorError {
    /// The text was empty.
    Empty,
    /// The text exceeded [`MAX_ACCELERATOR_BYTES`](super::MAX_ACCELERATOR_BYTES).
    TooLong,
    /// Two separators were adjacent, or the text began or ended with one.
    EmptyToken,
    /// A token named neither a modifier nor a supported key.
    UnknownToken(String),
    /// A modifier appeared twice, including `CmdOrCtrl` beside its resolution.
    DuplicateModifier,
    /// Only modifiers were named.
    MissingKey,
    /// More than one non-modifier key was named, or a key preceded a modifier.
    MultipleKeys,
    /// The primary modifier supplied for `CmdOrCtrl` was not exactly one flag.
    InvalidPrimary,
}

impl fmt::Display for AcceleratorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("accelerator is empty"),
            Self::TooLong => formatter.write_str("accelerator exceeds its byte bound"),
            Self::EmptyToken => formatter.write_str("accelerator contains an empty token"),
            Self::UnknownToken(token) => write!(formatter, "unknown accelerator token `{token}`"),
            Self::DuplicateModifier => formatter.write_str("accelerator repeats a modifier"),
            Self::MissingKey => formatter.write_str("accelerator names no key"),
            Self::MultipleKeys => formatter.write_str("accelerator must end with exactly one key"),
            Self::InvalidPrimary => formatter.write_str("primary modifier must be one flag"),
        }
    }
}

impl Error for AcceleratorError {}

/// Why a shortcut binding was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ShortcutError {
    /// The accelerator is already bound; the existing binding is kept.
    Conflict,
    /// The map holds [`MAX_SHORTCUTS`](super::MAX_SHORTCUTS) bindings.
    Full,
}

impl fmt::Display for ShortcutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => formatter.write_str("accelerator is already bound"),
            Self::Full => formatter.write_str("shortcut map is full"),
        }
    }
}

impl Error for ShortcutError {}
