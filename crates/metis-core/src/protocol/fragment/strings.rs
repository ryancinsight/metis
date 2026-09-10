//! Length-prefixed strings on the fragment wire.

use super::errors::{malformed, too_large};
use super::take;
use crate::error::Result;

/// The big-endian integer that prefixes a string on the fragment wire.
///
/// The protocol uses two widths -- identifiers and attribute names carry a
/// `u16` prefix, free text a `u32` -- and the width is the only thing that
/// differed between the encode and decode pairs. It enters as a type
/// parameter so one implementation serves both, rather than as a second copy
/// of each function named after its integer.
///
/// The methods are named apart from the inherent `to_be_bytes`/`try_from` so
/// an implementation can call those without recursing into itself.
pub(crate) trait WireLength: Copy {
    /// Big-endian byte form; its length is the prefix width on the wire.
    type Bytes: AsRef<[u8]>;

    /// The prefix for a string of `length` bytes, or `None` when the length
    /// does not fit this width.
    fn from_len(length: usize) -> Option<Self>;

    /// This prefix as the bytes that precede the string.
    fn be_bytes(self) -> Self::Bytes;

    /// Read a prefix from `buf` and answer the string length it names.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::PayloadTooLarge`] when the buffer is short, or
    /// when the decoded length exceeds this target's `usize`.
    fn read_len(buf: &mut &[u8]) -> Result<usize>;
}

impl WireLength for u16 {
    type Bytes = [u8; 2];

    fn from_len(length: usize) -> Option<Self> {
        Self::try_from(length).ok()
    }

    fn be_bytes(self) -> Self::Bytes {
        self.to_be_bytes()
    }

    fn read_len(buf: &mut &[u8]) -> Result<usize> {
        // A `u16` always fits `usize` on every target this builds for, so the
        // conversion that `u32` needs has no counterpart here.
        Ok(usize::from(Self::from_be_bytes(take(buf)?)))
    }
}

impl WireLength for u32 {
    type Bytes = [u8; 4];

    fn from_len(length: usize) -> Option<Self> {
        Self::try_from(length).ok()
    }

    fn be_bytes(self) -> Self::Bytes {
        self.to_be_bytes()
    }

    fn read_len(buf: &mut &[u8]) -> Result<usize> {
        usize::try_from(Self::from_be_bytes(take(buf)?))
            .map_err(|_| too_large("Fragment string length cannot fit this target"))
    }
}

/// Append `value` prefixed by its length in `L`.
///
/// The per-field messages are keyed on `field` rather than on the width: the
/// two sets were already disjoint, since a field carries one prefix width for
/// the life of the protocol version.
pub(crate) fn encode_string<L: WireLength>(
    encoded: &mut Vec<u8>,
    value: &str,
    field: &str,
) -> Result<()> {
    let length = L::from_len(value.len()).ok_or_else(|| {
        too_large(match field {
            "Fragment target identifier" => "Fragment target identifier exceeds its wire length",
            "Fragment attribute name" => "Fragment attribute name exceeds its wire length",
            "Fragment action input" => "Fragment action input exceeds its wire length",
            _ => "Fragment string exceeds its wire length",
        })
    })?;
    encoded.extend_from_slice(length.be_bytes().as_ref());
    encoded.extend_from_slice(value.as_bytes());
    Ok(())
}

/// Read a string whose length is carried by an `L`-wide prefix.
pub(crate) fn take_prefixed_string<L: WireLength>(
    buf: &mut &[u8],
    max_bytes: usize,
    field: &str,
) -> Result<String> {
    let length = L::read_len(buf)?;
    take_string(buf, length, max_bytes, field)
}

pub(crate) fn take_string(
    buf: &mut &[u8],
    length: usize,
    max_bytes: usize,
    field: &str,
) -> Result<String> {
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
