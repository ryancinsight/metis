//! Custom URL schemes and the links an application receives through them.
//!
//! This is the counterpart of Tauri's deep-link plugin. An application
//! declares its schemes once; the build tool registers them with each
//! platform's installer, and the operating system starts the application
//! with the clicked link as an argument. [`DeepLinkScheme`] is the one rule
//! for what a scheme may be, shared by the manifest check and the parser, so
//! a scheme the installer registered is exactly a scheme the application
//! accepts. [`DeepLink::parse`] turns untrusted link text into a bounded,
//! percent-decoded value; it grants nothing, and a host still decides what
//! each route may do.

mod link;
mod scheme;

pub use link::{DeepLink, MAX_DEEP_LINK_BYTES, MAX_DEEP_LINK_QUERY_PAIRS, MAX_DEEP_LINK_SEGMENTS};
pub use scheme::{DeepLinkScheme, MAX_DEEP_LINK_SCHEME_BYTES};

use std::{error::Error, fmt};

/// Why a scheme or link was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DeepLinkError {
    /// The scheme is malformed or reserved for another handler.
    InvalidScheme,
    /// The link is not `scheme:` followed by an admitted path and query.
    Malformed,
    /// The link exceeds a byte, segment or query bound.
    TooLarge,
    /// The link's scheme is not one the application declared.
    UnknownScheme,
}

impl fmt::Display for DeepLinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidScheme => "deep-link scheme is malformed or reserved",
            Self::Malformed => "deep link is malformed",
            Self::TooLarge => "deep link exceeds its bounds",
            Self::UnknownScheme => "deep link uses an undeclared scheme",
        })
    }
}

impl Error for DeepLinkError {}

#[cfg(test)]
mod tests;
