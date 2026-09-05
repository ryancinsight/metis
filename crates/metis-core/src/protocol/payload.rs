//! Exact-length payload codecs with validated UTF-8 and bounded strings.
use super::wire::{MAX_PAYLOAD_SIZE, check_length, finish, malformed, take};
use crate::capability::{CapabilityScope, CapabilityToken};
use crate::error::{ErrorCode, MetisError, Result};
const TOKEN_SIZE: usize = 84;
const CLINICAL_PREFIX: usize = TOKEN_SIZE + 3 * 8 + 2;

/// Initial client identity declaration. It is not proof of identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeRequestPayload {
    /// Requested wire version.
    pub client_version: u16,
    /// Claimed operating-system process identifier.
    pub client_process_id: u32,
    /// Principal requested for the session.
    pub principal_id: [u8; 16],
}
impl HandshakeRequestPayload {
    /// Encodes the fixed 22-byte request.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(22);
        buf.extend_from_slice(&self.client_version.to_be_bytes());
        buf.extend_from_slice(&self.client_process_id.to_be_bytes());
        buf.extend_from_slice(&self.principal_id);
        buf
    }
    /// Decodes an exact 22-byte request.
    /// # Errors
    /// Rejects truncated or trailing bytes.
    pub fn decode(mut buf: &[u8]) -> Result<Self> {
        let value = Self {
            client_version: u16::from_be_bytes(take(&mut buf)?),
            client_process_id: u32::from_be_bytes(take(&mut buf)?),
            principal_id: take(&mut buf)?,
        };
        finish(buf)?;
        Ok(value)
    }
}
/// Initial restricted capability returned by the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeResponsePayload {
    /// Server wire version.
    pub server_version: u16,
    /// Server-issued session token.
    pub initial_token: CapabilityToken,
}
impl HandshakeResponsePayload {
    /// Encodes the fixed 86-byte response.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(2 + TOKEN_SIZE);
        buf.extend_from_slice(&self.server_version.to_be_bytes());
        encode_token(&self.initial_token, &mut buf);
        buf
    }
    /// Decodes an exact 86-byte response without authenticating its token.
    /// # Errors
    /// Rejects truncated or trailing bytes.
    pub fn decode(mut buf: &[u8]) -> Result<Self> {
        let value = Self {
            server_version: u16::from_be_bytes(take(&mut buf)?),
            initial_token: decode_token(&mut buf)?,
        };
        finish(buf)?;
        Ok(value)
    }
}
/// Clinical calculation inputs and their authorization token.
#[derive(Debug, Clone, PartialEq)]
pub struct ClinicalCalcRequestPayload {
    /// Authorization checked by the backend before calculation.
    pub token: CapabilityToken,
    /// UTF-8 patient reference, bounded by the frame payload capacity.
    pub patient_id: String,
    /// Patient mass in kilograms; domain validation belongs to the backend.
    pub weight_kg: f64,
    /// Drug concentration in milligrams per milliliter.
    pub concentration_mg_ml: f64,
    /// Target dose in micrograms per kilogram per minute.
    pub target_dose_mcg_kg_min: f64,
}
impl ClinicalCalcRequestPayload {
    /// Encodes the request without truncating its patient reference.
    /// # Errors
    /// Returns `PayloadTooLarge` if the reference exceeds the frame capacity.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let length = string_length(&self.patient_id, CLINICAL_PREFIX)?;
        let mut buf = Vec::with_capacity(CLINICAL_PREFIX + self.patient_id.len());
        encode_token(&self.token, &mut buf);
        buf.extend_from_slice(&self.weight_kg.to_be_bytes());
        buf.extend_from_slice(&self.concentration_mg_ml.to_be_bytes());
        buf.extend_from_slice(&self.target_dose_mcg_kg_min.to_be_bytes());
        buf.extend_from_slice(&length.to_be_bytes());
        buf.extend_from_slice(self.patient_id.as_bytes());
        Ok(buf)
    }
    /// Decodes exact field lengths and validates UTF-8 before allocation.
    /// # Errors
    /// Rejects oversized, truncated, trailing, or invalid UTF-8 payloads.
    pub fn decode(mut buf: &[u8]) -> Result<Self> {
        check_length(buf.len())?;
        let token = decode_token(&mut buf)?;
        let weight_kg = f64::from_be_bytes(take(&mut buf)?);
        let concentration_mg_ml = f64::from_be_bytes(take(&mut buf)?);
        let target_dose_mcg_kg_min = f64::from_be_bytes(take(&mut buf)?);
        let patient_id = decode_string(&mut buf)?;
        Ok(Self {
            token,
            patient_id,
            weight_kg,
            concentration_mg_ml,
            target_dose_mcg_kg_min,
        })
    }
}
/// Signed clinical result produced by the backend.
#[derive(Debug, Clone, PartialEq)]
pub struct ClinicalCalcResponsePayload {
    /// Audit record identifying this calculation.
    pub audit_sequence_id: u64,
    /// Infusion rate in milliliters per hour.
    pub rate_ml_hr: f64,
    /// Drug delivery rate in milligrams per hour.
    pub drug_rate_mg_hr: f64,
    /// Whether the pediatric interlock applies.
    pub is_pediatric: bool,
    /// Backend signature, verified by the receiving trust boundary.
    pub result_signature: [u8; 32],
}
impl ClinicalCalcResponsePayload {
    /// Encodes the fixed 57-byte response.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(57);
        buf.extend_from_slice(&self.audit_sequence_id.to_be_bytes());
        buf.extend_from_slice(&self.rate_ml_hr.to_be_bytes());
        buf.extend_from_slice(&self.drug_rate_mg_hr.to_be_bytes());
        buf.push(u8::from(self.is_pediatric));
        buf.extend_from_slice(&self.result_signature);
        buf
    }
    /// Decodes an exact 57-byte response and canonical boolean field.
    /// # Errors
    /// Rejects truncated/trailing bytes or a boolean other than zero or one.
    pub fn decode(mut buf: &[u8]) -> Result<Self> {
        let audit_sequence_id = u64::from_be_bytes(take(&mut buf)?);
        let rate_ml_hr = f64::from_be_bytes(take(&mut buf)?);
        let drug_rate_mg_hr = f64::from_be_bytes(take(&mut buf)?);
        let is_pediatric = match take::<1>(&mut buf)? {
            [0] => false,
            [1] => true,
            _ => return Err(malformed("Invalid boolean encoding")),
        };
        let result_signature = take(&mut buf)?;
        finish(buf)?;
        Ok(Self {
            audit_sequence_id,
            rate_ml_hr,
            drug_rate_mg_hr,
            is_pediatric,
            result_signature,
        })
    }
}
/// Structured remote failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorResponsePayload {
    /// Remote error classification.
    pub error_code: u16,
    /// UTF-8 diagnostic text.
    pub message: String,
}
impl ErrorResponsePayload {
    /// Encodes a bounded diagnostic without truncation.
    /// # Errors
    /// Returns `PayloadTooLarge` if the diagnostic exceeds frame capacity.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let length = string_length(&self.message, 4)?;
        let mut buf = Vec::with_capacity(4 + self.message.len());
        buf.extend_from_slice(&self.error_code.to_be_bytes());
        buf.extend_from_slice(&length.to_be_bytes());
        buf.extend_from_slice(self.message.as_bytes());
        Ok(buf)
    }
    /// Decodes exact field lengths and validates UTF-8 before allocation.
    /// # Errors
    /// Rejects oversized, truncated, trailing, or invalid UTF-8 payloads.
    pub fn decode(mut buf: &[u8]) -> Result<Self> {
        check_length(buf.len())?;
        Ok(Self {
            error_code: u16::from_be_bytes(take(&mut buf)?),
            message: decode_string(&mut buf)?,
        })
    }
}
fn string_length(value: &str, prefix: usize) -> Result<u16> {
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
fn decode_string(buf: &mut &[u8]) -> Result<String> {
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
fn encode_token(token: &CapabilityToken, buf: &mut Vec<u8>) {
    buf.extend_from_slice(&token.token_id.to_be_bytes());
    buf.extend_from_slice(&token.principal_id);
    buf.extend_from_slice(&token.scope.0.to_be_bytes());
    buf.extend_from_slice(&token.issued_at_secs.to_be_bytes());
    buf.extend_from_slice(&token.expires_at_secs.to_be_bytes());
    buf.extend_from_slice(&token.nonce.to_be_bytes());
    buf.extend_from_slice(&token.signature);
}
fn decode_token(buf: &mut &[u8]) -> Result<CapabilityToken> {
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
