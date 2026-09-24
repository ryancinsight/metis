//! The rule for an application's custom URL scheme.

use super::DeepLinkError;
use std::fmt;

/// Longest admitted scheme, in bytes.
pub const MAX_DEEP_LINK_SCHEME_BYTES: usize = 32;

/// Schemes an application must not claim: they belong to browsers, the
/// operating system or other standard handlers, and registering one would
/// take links away from them.
const RESERVED: &[&str] = &[
    "about",
    "blob",
    "chrome",
    "data",
    "file",
    "ftp",
    "http",
    "https",
    "javascript",
    "mailto",
    "ms-settings",
    "news",
    "sms",
    "ssh",
    "tel",
    "urn",
    "view-source",
    "ws",
    "wss",
];

/// A validated custom URL scheme: a lowercase ASCII letter, then lowercase
/// letters, digits, `+`, `-` or `.`, at most [`MAX_DEEP_LINK_SCHEME_BYTES`]
/// long, and not a reserved scheme. A dot-separated reverse-domain form such
/// as `org.example.viewer` avoids collisions with other applications.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeepLinkScheme(Box<str>);

impl DeepLinkScheme {
    /// Validates `scheme`.
    ///
    /// # Errors
    /// Returns [`DeepLinkError::InvalidScheme`] for any other text.
    pub fn new(scheme: &str) -> Result<Self, DeepLinkError> {
        let bytes = scheme.as_bytes();
        let valid = !bytes.is_empty()
            && bytes.len() <= MAX_DEEP_LINK_SCHEME_BYTES
            && bytes[0].is_ascii_lowercase()
            && bytes.iter().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"+-.".contains(byte)
            })
            && !RESERVED.contains(&scheme);
        if valid {
            Ok(Self(scheme.into()))
        } else {
            Err(DeepLinkError::InvalidScheme)
        }
    }

    /// The scheme text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeepLinkScheme {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Debug for DeepLinkScheme {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, formatter)
    }
}
