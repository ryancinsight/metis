use super::super::wire::{MAX_PAYLOAD_SIZE, malformed, take};
use crate::capability::{CapabilityScope, CapabilityToken};
use crate::error::{ErrorCode, MetisError, Result};

pub(super) fn string_length(value: &str, prefix: usize) -> Result<u16> {
    if value.len() > MAX_PAYLOAD_SIZE - prefix {
        return Err(MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "String exceeds frame resource bound",
        ));
    }
    u16::try_from(value.len()).map_err(|_| {
        MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "String exceeds wire length field",
        )
    })
}
pub(super) fn decode_string(buf: &mut &[u8]) -> Result<String> {
    let length = usize::from(u16::from_be_bytes(take(buf)?));
    if buf.len() != length {
        return Err(malformed("String length does not match remaining payload"));
    }
    let value = std::str::from_utf8(buf)
        .map_err(|_| malformed("Invalid UTF-8 string"))?
        .to_owned();
    *buf = &[];
    Ok(value)
}
pub(super) fn encode_token(token: &CapabilityToken, buf: &mut Vec<u8>) {
    buf.extend_from_slice(&token.token_id.to_be_bytes());
    buf.extend_from_slice(&token.principal_id);
    buf.extend_from_slice(&token.scope.0.to_be_bytes());
    buf.extend_from_slice(&token.issued_at_secs.to_be_bytes());
    buf.extend_from_slice(&token.expires_at_secs.to_be_bytes());
    buf.extend_from_slice(&token.nonce.to_be_bytes());
    buf.extend_from_slice(&token.signature);
}
pub(super) fn decode_token(buf: &mut &[u8]) -> Result<CapabilityToken> {
    Ok(CapabilityToken {
        token_id: u64::from_be_bytes(take(buf)?),
        principal_id: take(buf)?,
        scope: CapabilityScope(u32::from_be_bytes(take(buf)?)),
        issued_at_secs: u64::from_be_bytes(take(buf)?),
        expires_at_secs: u64::from_be_bytes(take(buf)?),
        nonce: u64::from_be_bytes(take(buf)?),
        signature: take(buf)?,
    })
}
