//! Decoding the path and query of a URI into bounded, unambiguous parts.
//!
//! Deep links and routes read paths the same way: segments are
//! percent-decoded UTF-8 without control characters, empty segments are
//! dropped, and `.`, `..` or an encoded `/` are rejected rather than
//! resolved, so one path can never be written to look like another.

/// Why a path or query was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PathError {
    /// The text does not decode under the rules above.
    Malformed,
    /// The text exceeds a segment or pair bound.
    TooLarge,
}

/// Decodes the segments of `path`, at most `limit` of them.
pub(crate) fn segments(path: &str, limit: usize) -> Result<Vec<String>, PathError> {
    let mut segments = Vec::new();
    for raw in path.split('/').filter(|segment| !segment.is_empty()) {
        if segments.len() == limit {
            return Err(PathError::TooLarge);
        }
        let segment = decode(raw)?;
        if segment == "." || segment == ".." || segment.contains('/') {
            return Err(PathError::Malformed);
        }
        segments.push(segment);
    }
    Ok(segments)
}

/// Decodes the `key=value` pairs of `query`, at most `limit` of them.
pub(crate) fn query_pairs(query: &str, limit: usize) -> Result<Vec<(String, String)>, PathError> {
    let mut pairs = Vec::new();
    for raw in query.split('&').filter(|pair| !pair.is_empty()) {
        if pairs.len() == limit {
            return Err(PathError::TooLarge);
        }
        let (key, value) = raw.split_once('=').unwrap_or((raw, ""));
        let key = decode(key)?;
        if key.is_empty() {
            return Err(PathError::Malformed);
        }
        pairs.push((key, decode(value)?));
    }
    Ok(pairs)
}

/// Percent-encodes one segment so [`segments`] decodes it back exactly:
/// everything but RFC 3986 unreserved characters is escaped.
pub(crate) fn encode_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0F)]));
        }
    }
    encoded
}

/// Percent-decodes one component into UTF-8 without control characters.
/// `+` stays a literal plus: this is a URI component, not a form body.
fn decode(component: &str) -> Result<String, PathError> {
    let bytes = component.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        if byte == b'%' {
            let digits = bytes
                .get(index + 1..index + 3)
                .ok_or(PathError::Malformed)?;
            let text = std::str::from_utf8(digits).map_err(|_| PathError::Malformed)?;
            decoded.push(u8::from_str_radix(text, 16).map_err(|_| PathError::Malformed)?);
            index += 3;
        } else {
            decoded.push(byte);
            index += 1;
        }
    }
    let text = String::from_utf8(decoded).map_err(|_| PathError::Malformed)?;
    if text.chars().any(char::is_control) {
        return Err(PathError::Malformed);
    }
    Ok(text)
}
