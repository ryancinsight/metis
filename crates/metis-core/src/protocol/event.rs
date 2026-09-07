//! Bounded remote event envelopes and typed event codecs.

use super::wire::{MAX_PAYLOAD_SIZE, finish, malformed, take};
use crate::error::{ErrorCode, MetisError, Result};
use std::num::NonZeroU64;

const ENVELOPE_PREFIX_SIZE: usize = 2 + 8 + 2 + 4;

/// Maximum UTF-8 name length for one remote event.
pub const MAX_EVENT_NAME_BYTES: usize = 128;

/// Identifies one unsolicited event without permitting the zero sentinel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct EventId(NonZeroU64);

impl EventId {
    /// Returns the event's wire sequence value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl TryFrom<u64> for EventId {
    type Error = MetisError;

    fn try_from(value: u64) -> Result<Self> {
        NonZeroU64::new(value).map(Self).ok_or_else(|| {
            MetisError::protocol(
                ErrorCode::SequenceMismatch,
                "Remote event identifier must be nonzero",
            )
        })
    }
}

/// Encodes and decodes one typed event body without dynamic dispatch.
pub trait EventCodec: Sized {
    /// Stable event name carried by the remote envelope.
    const NAME: &'static str;

    /// Encodes the event body into bounded wire bytes.
    ///
    /// # Errors
    /// Returns a typed protocol error when the event body cannot be encoded.
    fn encode(&self) -> Result<Vec<u8>>;

    /// Decodes the event body from bounded wire bytes.
    ///
    /// # Errors
    /// Returns a typed protocol error when the body is malformed.
    fn decode(payload: &[u8]) -> Result<Self>;
}

/// Versioned, bounded envelope for one unsolicited remote event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteEventPayload {
    protocol_version: u16,
    event_id: EventId,
    name: String,
    payload: Vec<u8>,
}

impl RemoteEventPayload {
    /// Builds an envelope from a stable name and encoded event body.
    ///
    /// # Errors
    /// Returns a protocol error when the identifier is zero, the name is empty
    /// or over its bound, or the complete envelope exceeds the frame bound.
    pub fn new(event_id: u64, name: impl Into<String>, payload: impl AsRef<[u8]>) -> Result<Self> {
        let name = name.into();
        let payload = payload.as_ref();
        validate_parts(name.as_bytes(), payload.len())?;
        Self::with_version(
            super::wire::PROTOCOL_VERSION,
            EventId::try_from(event_id)?,
            name,
            payload.to_vec(),
        )
    }

    /// Encodes an [`EventCodec`] into a versioned envelope.
    ///
    /// # Errors
    /// Propagates the event codec or envelope bound error.
    pub fn from_event<E: EventCodec>(event_id: u64, event: &E) -> Result<Self> {
        Self::with_version(
            super::wire::PROTOCOL_VERSION,
            EventId::try_from(event_id)?,
            E::NAME.to_owned(),
            event.encode()?,
        )
    }

    /// Decodes an envelope from untrusted wire bytes.
    ///
    /// # Errors
    /// Rejects truncation, invalid UTF-8, empty or oversized names, trailing
    /// bytes, a zero identifier, and envelopes over the frame bound.
    pub fn decode(mut payload: &[u8]) -> Result<Self> {
        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Remote event exceeds the payload resource bound",
            ));
        }
        let protocol_version = u16::from_be_bytes(take(&mut payload)?);
        let event_id = EventId::try_from(u64::from_be_bytes(take(&mut payload)?))?;
        let name_len = usize::from(u16::from_be_bytes(take(&mut payload)?));
        let body_len = usize::try_from(u32::from_be_bytes(take(&mut payload)?)).map_err(|_| {
            MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Remote event body length cannot fit this target",
            )
        })?;
        let (name_bytes, remaining) = payload
            .split_at_checked(name_len)
            .ok_or_else(|| malformed("Remote event name is truncated"))?;
        let (body, trailing) = remaining
            .split_at_checked(body_len)
            .ok_or_else(|| malformed("Remote event body is truncated"))?;
        finish(trailing)?;
        let name = std::str::from_utf8(name_bytes)
            .map_err(|_| malformed("Remote event name is not valid UTF-8"))?
            .to_owned();
        Self::with_version(protocol_version, event_id, name, body.to_vec())
    }

    /// Returns the envelope's wire protocol version.
    #[must_use]
    pub const fn protocol_version(&self) -> u16 {
        self.protocol_version
    }

    /// Returns the nonzero event identifier.
    #[must_use]
    pub const fn event_id(&self) -> EventId {
        self.event_id
    }

    /// Returns the stable event name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the encoded event body without copying it.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Decodes this envelope with the named typed event codec.
    ///
    /// # Errors
    /// Returns a protocol error when the codec name differs from the envelope,
    /// or propagates the codec's body decoding error.
    pub fn decode_as<E: EventCodec>(&self) -> Result<E> {
        if self.name != E::NAME {
            return Err(malformed("Remote event name does not match its codec"));
        }
        E::decode(&self.payload)
    }

    fn with_version(
        protocol_version: u16,
        event_id: EventId,
        name: String,
        payload: Vec<u8>,
    ) -> Result<Self> {
        validate_parts(name.as_bytes(), payload.len())?;
        Ok(Self {
            protocol_version,
            event_id,
            name,
            payload,
        })
    }

    /// Encodes this envelope with exact field lengths.
    ///
    /// # Errors
    /// Returns a protocol error when the envelope's resource bounds are
    /// violated.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let (name_len, body_len, total) = validate_parts(self.name.as_bytes(), self.payload.len())?;
        let mut encoded = Vec::with_capacity(total);
        encoded.extend_from_slice(&self.protocol_version.to_be_bytes());
        encoded.extend_from_slice(&self.event_id.get().to_be_bytes());
        encoded.extend_from_slice(&name_len.to_be_bytes());
        encoded.extend_from_slice(&body_len.to_be_bytes());
        encoded.extend_from_slice(self.name.as_bytes());
        encoded.extend_from_slice(&self.payload);
        Ok(encoded)
    }
}

fn validate_parts(name: &[u8], payload_len: usize) -> Result<(u16, u32, usize)> {
    if name.is_empty() || name.len() > MAX_EVENT_NAME_BYTES {
        return Err(malformed(
            "Remote event name is empty or exceeds its byte bound",
        ));
    }
    let name_len = u16::try_from(name.len()).map_err(|_| {
        MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Remote event name exceeds its wire length field",
        )
    })?;
    let body_len = u32::try_from(payload_len).map_err(|_| {
        MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Remote event body exceeds its wire length field",
        )
    })?;
    let total = ENVELOPE_PREFIX_SIZE
        .checked_add(name.len())
        .and_then(|length| length.checked_add(payload_len))
        .ok_or_else(|| {
            MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Remote event envelope size overflows its resource bound",
            )
        })?;
    if total > MAX_PAYLOAD_SIZE {
        return Err(MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Remote event envelope exceeds the payload resource bound",
        ));
    }
    Ok((name_len, body_len, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct TestEvent(u16);

    impl EventCodec for TestEvent {
        const NAME: &'static str = "test.value";

        fn encode(&self) -> Result<Vec<u8>> {
            Ok(self.0.to_be_bytes().to_vec())
        }

        fn decode(mut payload: &[u8]) -> Result<Self> {
            let value = u16::from_be_bytes(take(&mut payload)?);
            finish(payload)?;
            Ok(Self(value))
        }
    }

    #[test]
    fn typed_event_round_trip_preserves_name_id_and_body() {
        let original = RemoteEventPayload::from_event(7, &TestEvent(0x1234)).expect("event");
        let encoded = original.encode().expect("encoded event");
        let decoded = RemoteEventPayload::decode(&encoded).expect("decoded event");
        assert_eq!(decoded, original);
        assert_eq!(decoded.event_id().get(), 7);
        assert_eq!(
            decoded.decode_as::<TestEvent>().expect("typed body"),
            TestEvent(0x1234)
        );
    }

    #[test]
    fn envelope_rejects_zero_empty_oversized_and_truncated_values() {
        assert_eq!(
            RemoteEventPayload::new(0, "test.value", [])
                .expect_err("zero id")
                .code,
            ErrorCode::SequenceMismatch
        );
        assert_eq!(
            RemoteEventPayload::new(1, "", [])
                .expect_err("empty name")
                .code,
            ErrorCode::MalformedPayload
        );
        assert_eq!(
            RemoteEventPayload::new(1, "test.value", vec![0; MAX_PAYLOAD_SIZE])
                .expect_err("oversized body")
                .code,
            ErrorCode::PayloadTooLarge
        );
        let mut truncated = vec![0; ENVELOPE_PREFIX_SIZE - 1];
        truncated[9] = 1;
        let error = RemoteEventPayload::decode(&truncated).expect_err("truncated envelope");
        assert_eq!(error.code, ErrorCode::MalformedPayload);
    }

    #[test]
    fn codec_name_mismatch_is_rejected_before_body_decode() {
        let envelope = RemoteEventPayload::new(1, "other.value", [0, 1]).expect("event");
        let error = envelope
            .decode_as::<TestEvent>()
            .expect_err("wrong event codec");
        assert_eq!(error.code, ErrorCode::MalformedPayload);
    }
}
