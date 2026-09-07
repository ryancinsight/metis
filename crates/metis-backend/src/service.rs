//! Session authorization and audited dispatch for one connected frontend.
//!
//! A service belongs to one supervisor-created private transport. A supplied
//! principal is a session label, not OS identity proof. The supervisor owns peer
//! authentication through private pipe transfer. The launcher supplies a fresh
//! OS-generated key which never crosses that transport.

use crate::audit::{AuditEvent, AuditLedger};
use crate::clinical::{
    DrugConcentrationMgMl, PatientWeightKg, SafetyEnvelope, TargetDoseRate, calculate_infusion_rate,
};
use metis_core::capability::{CapabilityGrantSpec, CapabilityScope, CapabilityToken};
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::host::{HostContext, HostPolicy, HostSessionId};
use metis_core::protocol::{
    CapabilityCatalogPayload, ClinicalCalcRequestPayload, ClinicalCalcResponsePayload,
    ErrorResponsePayload, FrameHeader, HandshakeRequestPayload, HandshakeResponsePayload,
    MessageType, PROTOCOL_VERSION, RemoteEventPayload, SUPPORTED_COMMANDS,
};
use metis_ipc::server::{FailureContext, IpcHandler, RequestIdentity};
use moirai_crypto::hmac_sha256;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Session lifetime policy: one hour; changes require expiry-policy review.
pub const SESSION_LIFETIME: Duration = Duration::from_hours(1);

/// Paired wall and monotonic observations from one clock boundary.
#[derive(Debug, Clone, Copy)]
pub struct ClockReading {
    /// UTC duration since the Unix epoch, used for audit and token claims.
    pub unix_time: Duration,
    /// Elapsed duration from a fixed origin, used for session expiry.
    pub monotonic: Duration,
}

/// Clock boundary with a fixed monotonic origin for each service.
pub trait Clock {
    /// Samples UTC and monotonic time.
    ///
    /// # Errors
    /// Reports unavailable or unrepresentable clock observations.
    fn now(&self) -> Result<ClockReading>;
}

/// Production clock using `SystemTime` for UTC and `Instant` for elapsed time.
pub struct SystemClock {
    origin: Instant,
}

impl Default for SystemClock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Result<ClockReading> {
        let unix_time = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
            MetisError::capability(
                ErrorCode::CapabilityExpired,
                "System clock precedes Unix epoch",
            )
        })?;
        Ok(ClockReading {
            unix_time,
            monotonic: self.origin.elapsed(),
        })
    }
}

struct Session {
    token: CapabilityToken,
    context: HostContext,
    started: Duration,
}

/// Backend state belonging exclusively to one private transport session.
pub struct BackendService<C = SystemClock> {
    master_key: [u8; 32],
    envelope: SafetyEnvelope,
    host_policy: HostPolicy,
    trusted_context: Option<HostContext>,
    ledger: AuditLedger,
    clock: C,
    session: Option<Session>,
    last_sequence: u64,
    last_reading: Option<ClockReading>,
    clock_failed: bool,
    pending_event: Option<RemoteEventPayload>,
}

impl BackendService {
    /// Creates a service with real UTC and monotonic clocks.
    ///
    /// The launcher must supply an unpredictable key unique to this session.
    /// Example envelope limits do not constitute clinical validation.
    #[must_use]
    pub fn new(master_key: [u8; 32], envelope: SafetyEnvelope) -> Self {
        Self::with_clock_and_policy(
            master_key,
            envelope,
            SystemClock::default(),
            HostPolicy::native(),
        )
    }
}

impl<C> BackendService<C> {
    /// Selects a clock implementation for deterministic temporal verification.
    #[must_use]
    pub fn with_clock(master_key: [u8; 32], envelope: SafetyEnvelope, clock: C) -> Self {
        Self::with_clock_and_policy(master_key, envelope, clock, HostPolicy::native())
    }

    /// Selects a clock and exact host policy for deterministic verification.
    #[must_use]
    pub fn with_clock_and_policy(
        master_key: [u8; 32],
        envelope: SafetyEnvelope,
        clock: C,
        host_policy: HostPolicy,
    ) -> Self {
        Self {
            master_key,
            envelope,
            host_policy,
            trusted_context: None,
            ledger: AuditLedger::new(),
            clock,
            session: None,
            last_sequence: 0,
            last_reading: None,
            clock_failed: false,
            pending_event: None,
        }
    }

    /// Selects a clock, exact host policy, and already-observed session context.
    ///
    /// A native or service acceptor uses this constructor after validating the
    /// browser `Origin` header and generating the session principal. The
    /// handshake may repeat that principal as a routing label, but it cannot
    /// replace the trusted context with values supplied by the browser.
    ///
    /// # Errors
    /// Returns [`ErrorCode::NavigationDenied`] or [`ErrorCode::InvalidWindow`]
    /// when `context` does not satisfy `host_policy`.
    pub fn with_trusted_context(
        master_key: [u8; 32],
        envelope: SafetyEnvelope,
        clock: C,
        host_policy: HostPolicy,
        context: HostContext,
    ) -> Result<Self> {
        host_policy.check_context(&context)?;
        Ok(Self {
            master_key,
            envelope,
            host_policy,
            trusted_context: Some(context),
            ledger: AuditLedger::new(),
            clock,
            session: None,
            last_sequence: 0,
            last_reading: None,
            clock_failed: false,
            pending_event: None,
        })
    }

    /// Retained request outcomes, including rejected requests.
    #[must_use]
    pub const fn ledger(&self) -> &AuditLedger {
        &self.ledger
    }

    pub(crate) fn browser_policy(&self) -> &HostPolicy {
        &self.host_policy
    }

    pub(crate) const fn has_trusted_context(&self) -> bool {
        self.trusted_context.is_some()
    }
}

impl<C: Clock> BackendService<C> {
    fn record_event(&mut self, event: AuditEvent, outcome: Option<ErrorCode>) -> Result<()> {
        let actor = self
            .session
            .as_ref()
            .map_or([0; 16], |session| session.token.principal_id);
        let timestamp = self
            .last_reading
            .map_or(0, |reading| reading.unix_time.as_millis());
        let timestamp = u64::try_from(timestamp).map_err(|_| {
            MetisError::new(
                ErrorCode::ClinicalInterlockBlocked,
                "Audit timestamp exceeds wire range",
                "REQ-METIS-AUDIT-001",
            )
        })?;
        self.ledger.record(timestamp, actor, event, outcome)
    }

    fn observe_clock(&mut self) -> Result<ClockReading> {
        if self.clock_failed {
            return Err(MetisError::capability(
                ErrorCode::CapabilityExpired,
                "Clock fault requires a new service session",
            ));
        }
        let reading = self.clock.now().inspect_err(|_| {
            self.clock_failed = true;
        })?;
        if u64::try_from(reading.unix_time.as_millis()).is_err() {
            self.clock_failed = true;
            return Err(MetisError::capability(
                ErrorCode::CapabilityExpired,
                "UTC time exceeds the audit timestamp representation",
            ));
        }
        if self.last_reading.is_some_and(|last| {
            reading.monotonic < last.monotonic || reading.unix_time < last.unix_time
        }) {
            self.clock_failed = true;
            return Err(MetisError::capability(
                ErrorCode::CapabilityExpired,
                "Clock moved backward; session must be restarted",
            ));
        }
        self.last_reading = Some(reading);
        Ok(reading)
    }

    fn handshake(&mut self, payload: &[u8], reading: ClockReading) -> Result<Vec<u8>> {
        let request = HandshakeRequestPayload::decode(payload)?;
        if request.client_version != PROTOCOL_VERSION {
            return Err(MetisError::protocol(
                ErrorCode::VersionMismatch,
                "Handshake payload version differs from the wire contract",
            ));
        }
        if self.session.is_some() {
            return Err(MetisError::capability(
                ErrorCode::PrivilegeEscalationAttempt,
                "A session can perform only one handshake",
            ));
        }
        if request.principal_id == [0; 16] || request.client_process_id == 0 {
            return Err(MetisError::capability(
                ErrorCode::InvalidPrincipal,
                "Session requires a nonzero principal label and process identifier",
            ));
        }
        // The session key is unique; the issuance sequence is a public identifier,
        // never a substitute for the key's unpredictability.
        let token_id = self.ledger.next_sequence();
        let session_id = HostSessionId::new(request.principal_id)?;
        let context = match self.trusted_context.as_ref() {
            Some(expected) if expected.session_id() != session_id => {
                return Err(MetisError::capability(
                    ErrorCode::InvalidPrincipal,
                    "Handshake principal is not bound to the trusted host session",
                ));
            }
            Some(expected) => expected.clone(),
            None => self.host_policy.context_for(session_id),
        };
        let token = context.issue_capability(
            CapabilityGrantSpec {
                token_id,
                principal_id: request.principal_id,
                scope: CapabilityScope::SUBMIT_CALCULATION,
                issued_at_secs: reading.unix_time.as_secs(),
                duration_secs: SESSION_LIFETIME.as_secs(),
                nonce: token_id,
            },
            &self.master_key,
        )?;
        let response = HandshakeResponsePayload {
            server_version: PROTOCOL_VERSION,
            initial_token: token.clone(),
        }
        .encode();
        self.session = Some(Session {
            token,
            context,
            started: reading.monotonic,
        });
        Ok(response)
    }

    fn authorize(&self, token: &CapabilityToken, reading: ClockReading) -> Result<()> {
        let session = self.session.as_ref().ok_or_else(|| {
            MetisError::capability(
                ErrorCode::MissingCapability,
                "Handshake required before calculation",
            )
        })?;
        if token != &session.token {
            return Err(MetisError::capability(
                ErrorCode::InvalidPrincipal,
                "Capability was not issued to this transport session",
            ));
        }
        let _verified = self
            .host_policy
            .authorize::<{ CapabilityScope::SUBMIT_CALCULATION.0 }>(
                token,
                &session.context,
                reading.unix_time.as_secs(),
                &self.master_key,
            )?;
        let elapsed = reading
            .monotonic
            .checked_sub(session.started)
            .ok_or_else(|| {
                MetisError::capability(
                    ErrorCode::CapabilityExpired,
                    "Monotonic clock precedes session start",
                )
            })?;
        if elapsed >= SESSION_LIFETIME {
            return Err(MetisError::capability(
                ErrorCode::CapabilityExpired,
                "Session lifetime elapsed",
            ));
        }
        Ok(())
    }

    fn calculate(
        &mut self,
        header: &FrameHeader,
        payload: &[u8],
        reading: ClockReading,
    ) -> Result<Vec<u8>> {
        let request = ClinicalCalcRequestPayload::decode(payload)?;
        self.authorize(&request.token, reading)?;
        let result = calculate_infusion_rate(
            PatientWeightKg::new(request.weight_kg)?,
            DrugConcentrationMgMl::new(request.concentration_mg_ml)?,
            TargetDoseRate::new(request.target_dose_mcg_kg_min)?,
            &self.envelope,
        )?;
        let mut response = ClinicalCalcResponsePayload {
            audit_sequence_id: self.ledger.next_sequence(),
            rate_ml_hr: result.rate_ml_hr,
            drug_rate_mg_hr: result.drug_rate_mg_hr,
            is_pediatric: result.is_pediatric,
            result_signature: [0; 32],
        };
        response.result_signature = self.result_signature(header, &request, &response)?;
        self.pending_event = Some(RemoteEventPayload::from_event(
            response.audit_sequence_id,
            &response,
        )?);
        Ok(response.encode())
    }

    fn capabilities(&self, payload: &[u8]) -> Result<Vec<u8>> {
        if !payload.is_empty() {
            return Err(MetisError::protocol(
                ErrorCode::MalformedPayload,
                "Capability discovery request must have an empty payload",
            ));
        }
        if self.session.is_none() {
            return Err(MetisError::capability(
                ErrorCode::MissingCapability,
                "Handshake required before capability discovery",
            ));
        }
        CapabilityCatalogPayload::new(SUPPORTED_COMMANDS.iter().copied())?.encode()
    }

    fn result_signature(
        &self,
        header: &FrameHeader,
        request: &ClinicalCalcRequestPayload,
        response: &ClinicalCalcResponsePayload,
    ) -> Result<[u8; 32]> {
        let request_bytes = request.encode()?;
        let mut bytes = Vec::with_capacity(request_bytes.len() + 80);
        bytes.extend_from_slice(b"METIS-CALCULATION-RESULT-1");
        bytes.extend_from_slice(&(header.msg_type as u16).to_be_bytes());
        bytes.extend_from_slice(&header.sequence_id.to_be_bytes());
        let length = u32::try_from(request_bytes.len()).map_err(|_| {
            MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Canonical request exceeds signing length field",
            )
        })?;
        bytes.extend_from_slice(&length.to_be_bytes());
        bytes.extend_from_slice(&request_bytes);
        bytes.extend_from_slice(&response.audit_sequence_id.to_be_bytes());
        bytes.extend_from_slice(&response.rate_ml_hr.to_be_bytes());
        bytes.extend_from_slice(&response.drug_rate_mg_hr.to_be_bytes());
        bytes.push(u8::from(response.is_pediatric));
        Ok(hmac_sha256(&self.master_key, &bytes))
    }

    fn dispatch(&mut self, header: &FrameHeader, payload: &[u8]) -> Result<(MessageType, Vec<u8>)> {
        if header.sequence_id <= self.last_sequence {
            return Err(MetisError::protocol(
                ErrorCode::ReplayDetected,
                "Request sequence must increase strictly within the session",
            ));
        }
        self.last_sequence = header.sequence_id;
        let reading = self.observe_clock()?;
        match header.msg_type {
            MessageType::HandshakeReq => Ok((
                MessageType::HandshakeResp,
                self.handshake(payload, reading)?,
            )),
            MessageType::CapabilityReq => {
                Ok((MessageType::CapabilityResp, self.capabilities(payload)?))
            }
            MessageType::HeartbeatReq if payload.is_empty() => {
                Ok((MessageType::HeartbeatResp, Vec::new()))
            }
            MessageType::HeartbeatReq => Err(MetisError::protocol(
                ErrorCode::MalformedPayload,
                "Heartbeat request must have an empty payload",
            )),
            MessageType::ClinicalCalcReq => Ok((
                MessageType::ClinicalCalcResp,
                self.calculate(header, payload, reading)?,
            )),
            _ => Err(MetisError::protocol(
                ErrorCode::UnexpectedMessageType,
                "Message type is not a supported request operation",
            )),
        }
    }
}

impl<C: Clock> IpcHandler for BackendService<C> {
    fn handle_request(
        &mut self,
        header: &FrameHeader,
        payload: &[u8],
    ) -> Result<(MessageType, Vec<u8>)> {
        self.pending_event = None;
        let result = self.dispatch(header, payload);
        if let Err(error) = self.record_event(
            AuditEvent::Processed(RequestIdentity::from(header)),
            result.as_ref().err().map(|error| error.code),
        ) {
            self.pending_event = None;
            return Err(error);
        }
        match result {
            Ok(response) => Ok(response),
            Err(error) => Ok((
                MessageType::ErrorResp,
                ErrorResponsePayload {
                    error_code: error.code as u16,
                    message: error.code.as_str().to_owned(),
                }
                .encode()?,
            )),
        }
    }

    fn handle_failure(&mut self, context: FailureContext, error: ErrorCode) -> Result<()> {
        self.pending_event = None;
        // Preserve the transport event even if the clock fails. In that case
        // the record uses the last trusted timestamp and the clock fault stops
        // the server after the event is appended.
        let clock_result = self.observe_clock();
        self.record_event(AuditEvent::Failure(context), Some(error))?;
        clock_result.map(|_| ())
    }

    fn take_event(&mut self) -> Option<RemoteEventPayload> {
        self.pending_event.take()
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod service_tests;
