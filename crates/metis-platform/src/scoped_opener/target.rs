//! Validation of the URL handed to the system opener.

use super::OpenError;
use metis_core::host::HostOrigin;

/// Longest URL the opener accepts, in bytes.
pub const MAX_OPEN_URL_BYTES: usize = 2048;

/// An absolute http(s) URL that is safe to pass as one launcher argument.
///
/// Only printable ASCII from the RFC 3986 URI character set is admitted, so
/// the value carries no whitespace, quote, angle bracket or backslash that a
/// launcher or the handler it starts could split or reinterpret, and it
/// cannot begin with `-` and be read as an option. Non-ASCII text must
/// already be percent-encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTarget {
    url: Box<str>,
    origin: HostOrigin,
}

impl OpenTarget {
    /// Validates `url`.
    ///
    /// # Errors
    /// Returns [`OpenError::InvalidUrl`] for an empty, oversized or
    /// non-http(s) URL, or one with a character outside the admitted set,
    /// user information or a malformed percent escape.
    pub fn parse(url: &str) -> Result<Self, OpenError> {
        if url.is_empty()
            || url.len() > MAX_OPEN_URL_BYTES
            || !url.bytes().all(is_uri_byte)
            || !percent_escapes_are_complete(url)
        {
            return Err(OpenError::InvalidUrl);
        }
        let origin = HostOrigin::from_url(url).map_err(|_| OpenError::InvalidUrl)?;
        if !is_web_origin(&origin) {
            return Err(OpenError::InvalidUrl);
        }
        Ok(Self {
            url: url.into(),
            origin,
        })
    }

    /// The validated URL.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.url
    }

    /// The URL's canonical origin.
    #[must_use]
    pub const fn origin(&self) -> &HostOrigin {
        &self.origin
    }
}

/// Whether an origin is one a browser should be asked to open.
pub(super) fn is_web_origin(origin: &HostOrigin) -> bool {
    let origin = origin.as_str();
    origin.starts_with("https://") || origin.starts_with("http://")
}

/// RFC 3986 unreserved, reserved and `%` characters.
fn is_uri_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&'()*+,;=%".contains(&byte)
}

fn percent_escapes_are_complete(url: &str) -> bool {
    let bytes = url.as_bytes();
    bytes.iter().enumerate().all(|(index, byte)| {
        *byte != b'%'
            || bytes
                .get(index + 1..index + 3)
                .is_some_and(|digits| digits.iter().all(u8::is_ascii_hexdigit))
    })
}
