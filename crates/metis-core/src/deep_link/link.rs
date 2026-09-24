//! Parsing the link text the operating system passes to the application.

use super::{DeepLinkError, DeepLinkScheme};

/// Longest admitted link, in bytes.
pub const MAX_DEEP_LINK_BYTES: usize = 2048;
/// Most path segments one link may carry.
pub const MAX_DEEP_LINK_SEGMENTS: usize = 32;
/// Most query pairs one link may carry.
pub const MAX_DEEP_LINK_QUERY_PAIRS: usize = 32;

/// A parsed link: `scheme:[//]segment/segment?key=value&…`.
///
/// Segments and query values are percent-decoded and must decode to UTF-8
/// without control characters. An authority after `//` is read as the first
/// path segment, as `myapp://open/study` and `myapp:open/study` name the
/// same route. A fragment is ignored. Empty segments are dropped, and `.` or
/// `..` segments are rejected rather than resolved, so a route cannot be
/// written to look like a different one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepLink {
    scheme: DeepLinkScheme,
    segments: Vec<String>,
    query: Vec<(String, String)>,
}

impl DeepLink {
    /// Parses `text` if its scheme is one of `schemes`.
    ///
    /// # Errors
    /// Returns [`DeepLinkError::UnknownScheme`] for an undeclared scheme,
    /// [`DeepLinkError::TooLarge`] beyond a bound, and
    /// [`DeepLinkError::Malformed`] for anything else that does not parse.
    pub fn parse(text: &str, schemes: &[DeepLinkScheme]) -> Result<Self, DeepLinkError> {
        if text.len() > MAX_DEEP_LINK_BYTES {
            return Err(DeepLinkError::TooLarge);
        }
        if !text.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(DeepLinkError::Malformed);
        }
        let (scheme, rest) = text.split_once(':').ok_or(DeepLinkError::Malformed)?;
        let scheme = scheme.to_ascii_lowercase();
        let scheme = schemes
            .iter()
            .find(|declared| declared.as_str() == scheme)
            .ok_or(DeepLinkError::UnknownScheme)?
            .clone();
        let rest = rest.split_once('#').map_or(rest, |(before, _)| before);
        let rest = rest.strip_prefix("//").unwrap_or(rest);
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));

        let mut segments = Vec::new();
        for raw in path.split('/').filter(|segment| !segment.is_empty()) {
            if segments.len() == MAX_DEEP_LINK_SEGMENTS {
                return Err(DeepLinkError::TooLarge);
            }
            let segment = decode(raw)?;
            if segment == "." || segment == ".." || segment.contains('/') {
                return Err(DeepLinkError::Malformed);
            }
            segments.push(segment);
        }
        let mut pairs = Vec::new();
        for raw in query.split('&').filter(|pair| !pair.is_empty()) {
            if pairs.len() == MAX_DEEP_LINK_QUERY_PAIRS {
                return Err(DeepLinkError::TooLarge);
            }
            let (key, value) = raw.split_once('=').unwrap_or((raw, ""));
            let key = decode(key)?;
            if key.is_empty() {
                return Err(DeepLinkError::Malformed);
            }
            pairs.push((key, decode(value)?));
        }
        Ok(Self {
            scheme,
            segments,
            query: pairs,
        })
    }

    /// Finds the first argument that parses as a link for `schemes`, the
    /// form in which platforms hand a clicked link to the new process.
    #[must_use]
    pub fn from_arguments<I, A>(arguments: I, schemes: &[DeepLinkScheme]) -> Option<Self>
    where
        I: IntoIterator<Item = A>,
        A: AsRef<str>,
    {
        arguments
            .into_iter()
            .find_map(|argument| Self::parse(argument.as_ref(), schemes).ok())
    }

    /// The link's scheme.
    #[must_use]
    pub const fn scheme(&self) -> &DeepLinkScheme {
        &self.scheme
    }

    /// The decoded path segments, the route.
    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// The decoded query pairs in order.
    #[must_use]
    pub fn query(&self) -> &[(String, String)] {
        &self.query
    }

    /// The first value of query key `key`.
    #[must_use]
    pub fn query_value(&self, key: &str) -> Option<&str> {
        self.query
            .iter()
            .find_map(|(name, value)| (name == key).then_some(value.as_str()))
    }
}

/// Percent-decodes one component into UTF-8 without control characters.
/// `+` stays a literal plus: this is a URI component, not a form body.
fn decode(component: &str) -> Result<String, DeepLinkError> {
    let bytes = component.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        if byte == b'%' {
            let digits = bytes
                .get(index + 1..index + 3)
                .ok_or(DeepLinkError::Malformed)?;
            let text = std::str::from_utf8(digits).map_err(|_| DeepLinkError::Malformed)?;
            decoded.push(u8::from_str_radix(text, 16).map_err(|_| DeepLinkError::Malformed)?);
            index += 3;
        } else {
            decoded.push(byte);
            index += 1;
        }
    }
    let text = String::from_utf8(decoded).map_err(|_| DeepLinkError::Malformed)?;
    if text.chars().any(char::is_control) {
        return Err(DeepLinkError::Malformed);
    }
    Ok(text)
}
