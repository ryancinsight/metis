//! Exact-length payload codecs with validated UTF-8 and bounded strings.
use super::event::EventCodec;
use super::plugin::{MAX_PLUGIN_NAME_BYTES, MAX_PLUGIN_OPERATION_NAME_BYTES, valid_identifier};
use super::wire::{MAX_PAYLOAD_SIZE, check_length, finish, malformed, take};
use crate::capability::{CapabilityScope, CapabilityToken};
use crate::error::{ErrorCode, MetisError, Result};
const TOKEN_SIZE: usize = 84;
const CLINICAL_PREFIX: usize = TOKEN_SIZE + 3 * 8 + 2;
const PLUGIN_INVOCATION_PREFIX: usize = TOKEN_SIZE + 2 + 2 + 4;
const PLUGIN_RESPONSE_PREFIX: usize = 4;

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

impl EventCodec for ClinicalCalcResponsePayload {
    const NAME: &'static str = "clinical.result";

    fn encode(&self) -> Result<Vec<u8>> {
        Ok(ClinicalCalcResponsePayload::encode(self))
    }

    fn decode(payload: &[u8]) -> Result<Self> {
        ClinicalCalcResponsePayload::decode(payload)
    }
}

/// Authenticated request for one registered plugin operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInvocationPayload {
    token: CapabilityToken,
    plugin_name: String,
    operation_name: String,
    body: Vec<u8>,
}

impl PluginInvocationPayload {
    /// Builds a bounded invocation with validated plugin and operation names.
    ///
    /// The body is opaque to Metis and is decoded by the registered plugin
    /// executor at its extension boundary.
    ///
    /// # Errors
    /// Returns a malformed-payload error for invalid identifiers or a body
    /// that exceeds the frame resource bound.
    pub fn new(
        token: CapabilityToken,
        plugin_name: impl AsRef<str>,
        operation_name: impl AsRef<str>,
        body: impl AsRef<[u8]>,
    ) -> Result<Self> {
        let plugin_name = plugin_name.as_ref();
        let operation_name = operation_name.as_ref();
        let body = body.as_ref();
        validate_plugin_invocation_parts(plugin_name, operation_name, body.len())?;
        Ok(Self {
            token,
            plugin_name: plugin_name.to_owned(),
            operation_name: operation_name.to_owned(),
            body: body.to_vec(),
        })
    }

    /// Returns the capability token presented for authorization.
    #[must_use]
    pub const fn token(&self) -> &CapabilityToken {
        &self.token
    }

    /// Returns the exact registered plugin identifier.
    #[must_use]
    pub fn plugin_name(&self) -> &str {
        &self.plugin_name
    }

    /// Returns the exact registered operation identifier.
    #[must_use]
    pub fn operation_name(&self) -> &str {
        &self.operation_name
    }

    /// Returns the opaque plugin body without copying it.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Encodes the invocation with exact field lengths.
    ///
    /// # Errors
    /// Returns a malformed-payload or resource-bound error when the validated
    /// invocation no longer satisfies its wire contract.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let (plugin_len, operation_len, body_len, total) =
            plugin_invocation_lengths(&self.plugin_name, &self.operation_name, self.body.len())?;
        let mut encoded = Vec::with_capacity(total);
        encode_token(&self.token, &mut encoded);
        encoded.extend_from_slice(&plugin_len.to_be_bytes());
        encoded.extend_from_slice(&operation_len.to_be_bytes());
        encoded.extend_from_slice(&body_len.to_be_bytes());
        encoded.extend_from_slice(self.plugin_name.as_bytes());
        encoded.extend_from_slice(self.operation_name.as_bytes());
        encoded.extend_from_slice(&self.body);
        Ok(encoded)
    }

    /// Decodes an invocation from untrusted wire bytes.
    ///
    /// # Errors
    /// Rejects truncation, invalid identifiers or UTF-8, trailing bytes and
    /// envelopes over the frame resource bound.
    pub fn decode(mut payload: &[u8]) -> Result<Self> {
        check_length(payload.len())?;
        let token = decode_token(&mut payload)?;
        let plugin_len = usize::from(u16::from_be_bytes(take(&mut payload)?));
        let operation_len = usize::from(u16::from_be_bytes(take(&mut payload)?));
        let body_len = usize::try_from(u32::from_be_bytes(take(&mut payload)?)).map_err(|_| {
            MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Plugin invocation body length cannot fit this target",
            )
        })?;
        let (plugin_bytes, remaining) = payload
            .split_at_checked(plugin_len)
            .ok_or_else(|| malformed("Plugin invocation plugin name is truncated"))?;
        let (operation_bytes, body) = remaining
            .split_at_checked(operation_len)
            .ok_or_else(|| malformed("Plugin invocation operation name is truncated"))?;
        let (body, trailing) = body
            .split_at_checked(body_len)
            .ok_or_else(|| malformed("Plugin invocation body is truncated"))?;
        finish(trailing)?;
        let plugin_name = std::str::from_utf8(plugin_bytes)
            .map_err(|_| malformed("Plugin invocation plugin name is not valid UTF-8"))?
            .to_owned();
        let operation_name = std::str::from_utf8(operation_bytes)
            .map_err(|_| malformed("Plugin invocation operation name is not valid UTF-8"))?
            .to_owned();
        validate_plugin_invocation_parts(&plugin_name, &operation_name, body.len())?;
        Ok(Self {
            token,
            plugin_name,
            operation_name,
            body: body.to_vec(),
        })
    }
}

/// Bounded response body returned by one plugin operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInvocationResponsePayload {
    body: Vec<u8>,
}

impl PluginInvocationResponsePayload {
    /// Builds a response body within the frame resource bound.
    ///
    /// # Errors
    /// Returns `PayloadTooLarge` when the body cannot fit its length field and
    /// the bounded frame.
    pub fn new(body: impl AsRef<[u8]>) -> Result<Self> {
        let body = body.as_ref();
        response_body_length(body.len())?;
        Ok(Self {
            body: body.to_vec(),
        })
    }

    /// Returns the plugin response body without copying it.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Encodes the response with its exact body length.
    ///
    /// # Errors
    /// Returns `PayloadTooLarge` when the response no longer satisfies its
    /// resource bound.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let body_len = response_body_length(self.body.len())?;
        let mut encoded = Vec::with_capacity(PLUGIN_RESPONSE_PREFIX + self.body.len());
        encoded.extend_from_slice(&body_len.to_be_bytes());
        encoded.extend_from_slice(&self.body);
        Ok(encoded)
    }

    /// Decodes a response body from untrusted wire bytes.
    ///
    /// # Errors
    /// Rejects truncation, trailing bytes and bodies over the frame bound.
    pub fn decode(mut payload: &[u8]) -> Result<Self> {
        check_length(payload.len())?;
        let body_len = usize::try_from(u32::from_be_bytes(take(&mut payload)?)).map_err(|_| {
            MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Plugin response body length cannot fit this target",
            )
        })?;
        let (body, trailing) = payload
            .split_at_checked(body_len)
            .ok_or_else(|| malformed("Plugin response body is truncated"))?;
        finish(trailing)?;
        response_body_length(body.len())?;
        Ok(Self {
            body: body.to_vec(),
        })
    }
}

fn validate_plugin_invocation_parts(
    plugin_name: &str,
    operation_name: &str,
    body_len: usize,
) -> Result<()> {
    if !valid_identifier(plugin_name, MAX_PLUGIN_NAME_BYTES)
        || !valid_identifier(operation_name, MAX_PLUGIN_OPERATION_NAME_BYTES)
    {
        return Err(malformed(
            "Plugin invocation identifiers are invalid or exceed their byte bounds",
        ));
    }
    plugin_invocation_lengths(plugin_name, operation_name, body_len).map(|_| ())
}

fn plugin_invocation_lengths(
    plugin_name: &str,
    operation_name: &str,
    body_len: usize,
) -> Result<(u16, u16, u32, usize)> {
    let plugin_len = u16::try_from(plugin_name.len()).map_err(|_| {
        MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Plugin invocation plugin name exceeds its wire length field",
        )
    })?;
    let operation_len = u16::try_from(operation_name.len()).map_err(|_| {
        MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Plugin invocation operation name exceeds its wire length field",
        )
    })?;
    let body_len_u32 = u32::try_from(body_len).map_err(|_| {
        MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Plugin invocation body exceeds its wire length field",
        )
    })?;
    let total = PLUGIN_INVOCATION_PREFIX
        .checked_add(plugin_name.len())
        .and_then(|length| length.checked_add(operation_name.len()))
        .and_then(|length| length.checked_add(body_len))
        .ok_or_else(|| {
            MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Plugin invocation size overflows its resource bound",
            )
        })?;
    if total > MAX_PAYLOAD_SIZE {
        return Err(MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Plugin invocation exceeds the payload resource bound",
        ));
    }
    Ok((plugin_len, operation_len, body_len_u32, total))
}

fn response_body_length(body_len: usize) -> Result<u32> {
    let encoded_len = PLUGIN_RESPONSE_PREFIX
        .checked_add(body_len)
        .ok_or_else(|| {
            MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Plugin response size overflows its resource bound",
            )
        })?;
    if encoded_len > MAX_PAYLOAD_SIZE {
        return Err(MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Plugin response exceeds the payload resource bound",
        ));
    }
    u32::try_from(body_len).map_err(|_| {
        MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Plugin response body exceeds its wire length field",
        )
    })
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
