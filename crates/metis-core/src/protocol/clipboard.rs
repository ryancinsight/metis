//! Encoding and validation for the trusted clipboard plugin's wire bodies.
//!
//! The clipboard write is a trusted backend operation requested by an
//! authorized session: the request carries bounded UTF-8 text and the
//! response carries the carried byte count. The wire codec keeps the request
//! inside [`MAX_CLIPBOARD_WRITE_BYTES`] so every clipboard journey has a
//! finite, admission-checked size before the provider touch.

use super::wire::{check_length, finish, malformed, take};
use crate::error::Result;

/// Maximum UTF-8 bytes one clipboard write request may carry. The value must
/// stay equal to `metis_platform::MAX_CLIPBOARD_TEXT_BYTES`; the equality is
/// pinned by a test there.
pub const MAX_CLIPBOARD_WRITE_BYTES: usize = 1 << 20;

/// One bounded clipboard text-write request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardWritePayload {
    text: String,
}

impl ClipboardWritePayload {
    /// Builds a bounded request.
    ///
    /// # Errors
    /// Returns a malformed-payload error for empty or oversized text.
    pub fn new(text: impl AsRef<str>) -> Result<Self> {
        let text = text.as_ref();
        if text.is_empty() {
            return Err(malformed("Clipboard write text must not be empty"));
        }
        if text.len() > MAX_CLIPBOARD_WRITE_BYTES {
            return Err(malformed(
                "Clipboard write text exceeds the request byte bound",
            ));
        }
        Ok(Self {
            text: text.to_owned(),
        })
    }

    /// Returns the requested clipboard text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Encodes the request as a 4-byte length and the UTF-8 text.
    ///
    /// # Errors
    /// Returns a typed resource-bound error if the validated text cannot fit
    /// the wire length field, which a bounded request never can.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut encoded = Vec::with_capacity(4 + self.text.len());
        encoded.extend_from_slice(
            &u32::try_from(self.text.len())
                .map_err(|_| malformed("Clipboard write length cannot fit its wire field"))?
                .to_be_bytes(),
        );
        encoded.extend_from_slice(self.text.as_bytes());
        Ok(encoded)
    }

    /// Decodes an untrusted request.
    ///
    /// # Errors
    /// Rejects over-size envelopes, oversized text, invalid UTF-8 and
    /// trailing bytes.
    pub fn decode(mut payload: &[u8]) -> Result<Self> {
        check_length(payload.len())?;
        let length = usize::try_from(u32::from_be_bytes(take(&mut payload)?))
            .map_err(|_| malformed("Clipboard write length cannot fit this target"))?;
        if length > MAX_CLIPBOARD_WRITE_BYTES {
            return Err(malformed(
                "Clipboard write text exceeds the request byte bound",
            ));
        }
        if payload.len() < length {
            return Err(malformed("Clipboard write text is truncated"));
        }
        let Some(text) = payload.get(..length) else {
            return Err(malformed("Clipboard write text is truncated"));
        };
        let text = std::str::from_utf8(text)
            .map_err(|_| malformed("Clipboard write text is not UTF-8"))?;
        let request = Self::new(text)?;
        finish(&payload[length..])?;
        Ok(request)
    }
}

/// The byte count one accepted clipboard write carried onto the clipboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipboardWriteResponse {
    bytes: u32,
}

impl ClipboardWriteResponse {
    /// Builds and bounds-checks a response.
    ///
    /// # Errors
    /// Returns a malformed-payload error when the count exceeds the request
    /// bound, which no admitted write can produce.
    pub fn new(bytes: u32) -> Result<Self> {
        let bound = u64::try_from(MAX_CLIPBOARD_WRITE_BYTES)
            .map_err(|_| malformed("Clipboard request byte bound exceeds u64"))?;
        if u64::from(bytes) > bound {
            return Err(malformed(
                "Clipboard write response exceeds the request byte bound",
            ));
        }
        Ok(Self { bytes })
    }

    /// Returns the carried byte count.
    #[must_use]
    pub const fn bytes(&self) -> u32 {
        self.bytes
    }

    /// Encodes the fixed 4-byte response.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.bytes.to_be_bytes().to_vec()
    }

    /// Decodes an exact 4-byte response.
    ///
    /// # Errors
    /// Rejects truncated or trailing bytes and over-bound counts.
    pub fn decode(mut payload: &[u8]) -> Result<Self> {
        let bytes = u32::from_be_bytes(take(&mut payload)?);
        finish(payload)?;
        Self::new(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::{ClipboardWritePayload, ClipboardWriteResponse, MAX_CLIPBOARD_WRITE_BYTES};

    #[test]
    fn requests_round_trip_with_bounds() {
        let request =
            ClipboardWritePayload::new("M\u{e9}tis clipboard write").expect("bounded request");
        let encoded = request.encode().expect("bounded encode");
        assert_eq!(
            ClipboardWritePayload::decode(&encoded)
                .expect("decode")
                .text(),
            "M\u{e9}tis clipboard write"
        );
        assert!(ClipboardWritePayload::new("").is_err());
        let overbound = "x".repeat(MAX_CLIPBOARD_WRITE_BYTES + 1);
        assert!(ClipboardWritePayload::new(overbound).is_err());
        let oversized = ClipboardWritePayload::new("y".repeat(MAX_CLIPBOARD_WRITE_BYTES))
            .expect("hue bound text")
            .encode()
            .expect("encode");
        // One byte over the provider bound is refused at decode.
        assert!(
            ClipboardWritePayload::decode(&{
                let mut stretched = oversized;
                stretched[0..4].copy_from_slice(
                    &u32::try_from(MAX_CLIPBOARD_WRITE_BYTES + 1)
                        .expect("provider bound fits u32")
                        .to_be_bytes(),
                );
                stretched.extend_from_slice(b"z");
                stretched
            })
            .is_err()
        );
        assert!(ClipboardWritePayload::decode(b"").is_err());
        assert!(ClipboardWritePayload::decode(b"\x00\x00\x00\x02ab\x00").is_err());
    }

    #[test]
    fn responses_round_trip_with_bounds() {
        let response = ClipboardWriteResponse::new(1_024).expect("bounded response");
        assert_eq!(
            ClipboardWriteResponse::decode(&response.encode()).expect("decode"),
            response
        );
        assert!(
            ClipboardWriteResponse::new(
                u32::try_from(MAX_CLIPBOARD_WRITE_BYTES + 1).expect("provider bound fits u32")
            )
            .is_err()
        );
        assert!(ClipboardWriteResponse::decode(b"").is_err());
        assert!(ClipboardWriteResponse::decode(b"\x00\x00\x00\x01\x00").is_err());
    }
}
