use super::{BackendService, Clock, ClockReading, SESSION_LIFETIME, Session};
use crate::audit::AuditEvent;
use crate::clinical::{
    DrugConcentrationMgMl, PatientWeightKg, TargetDoseRate, calculate_infusion_rate,
};
use metis_core::capability::{CapabilityGrantSpec, CapabilityScope, CapabilityToken};
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::host::HostSessionId;
use metis_core::protocol::{
    CapabilityCatalogPayload, ClinicalCalcRequestPayload, ClinicalCalcResponsePayload,
    ErrorResponsePayload, FrameHeader, HandshakeRequestPayload, HandshakeResponsePayload,
    MessageType, PROTOCOL_VERSION, PluginInvocationPayload, RemoteEventPayload, SUPPORTED_COMMANDS,
};
use metis_ipc::server::{FailureContext, IpcHandler, RequestIdentity};
use moirai_crypto::hmac_sha256;

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
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(store) = self.audit_store.as_mut() {
            let mut candidate = self.ledger.clone();
            candidate.record(timestamp, actor, event, outcome)?;
            store.persist(&candidate)?;
            self.ledger = candidate;
            return Ok(());
        }
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
                scope: CapabilityScope::SUBMIT_CALCULATION.union(CapabilityScope::UI_RENDER),
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

    fn authorize_scope(
        &self,
        token: &CapabilityToken,
        reading: ClockReading,
        required_scope: CapabilityScope,
    ) -> Result<()> {
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
        self.host_policy.check_context(&session.context)?;
        token.verify_for_host(
            required_scope,
            reading.unix_time.as_secs(),
            &self.master_key,
            &session.context,
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

    fn authorize(&self, token: &CapabilityToken, reading: ClockReading) -> Result<()> {
        self.authorize_scope(token, reading, CapabilityScope::SUBMIT_CALCULATION)
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

    fn target_capability_payload(&self, payload: &[u8]) -> Result<Vec<u8>> {
        if !payload.is_empty() {
            return Err(MetisError::protocol(
                ErrorCode::MalformedPayload,
                "Target capability discovery request must have an empty payload",
            ));
        }
        if self.session.is_none() {
            return Err(MetisError::capability(
                ErrorCode::MissingCapability,
                "Handshake required before target capability discovery",
            ));
        }
        self.target_capabilities.encode()
    }

    fn invoke_plugin(&mut self, payload: &[u8], reading: ClockReading) -> Result<Vec<u8>> {
        let request = PluginInvocationPayload::decode(payload)?;
        if !self.plugins.contains(request.plugin_name()) {
            return Err(MetisError::capability(
                ErrorCode::PluginNotFound,
                "Plugin invocation names an unregistered plugin",
            ));
        }
        let operation = self
            .plugins
            .command(request.plugin_name(), request.operation_name())
            .ok_or_else(|| {
                MetisError::capability(
                    ErrorCode::PluginOperationNotFound,
                    "Plugin invocation names an undeclared operation",
                )
            })?;
        self.authorize_scope(request.token(), reading, operation.required_scope())?;
        self.plugins.invoke(&request)?.encode()
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
            MessageType::TargetCapabilityReq => Ok((
                MessageType::TargetCapabilityResp,
                self.target_capability_payload(payload)?,
            )),
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
            MessageType::PluginInvokeReq => Ok((
                MessageType::PluginInvokeResp,
                self.invoke_plugin(payload, reading)?,
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
