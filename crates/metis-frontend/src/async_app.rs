//! Event-driven browser form state and bounded asynchronous requests.

use crate::{FormInputs, FormState};
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{
    CapabilityCatalogPayload, ClinicalCalcRequestPayload, ClinicalCalcResponsePayload,
    ErrorResponsePayload, MessageType, RemoteEventPayload, TargetCapabilityPayload,
};
use metis_ipc::async_client::AsyncIpcClient;
use metis_ipc::client::{CapabilityError, HandshakeError, TargetCapabilityError};
use metis_ipc::transport::AsyncIpcTransport;
use std::time::Duration;

/// Owns browser form state while requests run on the event loop.
pub struct AsyncFrontendApp<T> {
    client: Option<AsyncIpcClient<T>>,
    capabilities: Option<CapabilityCatalogPayload>,
    target_capabilities: Option<TargetCapabilityPayload>,
    inputs: FormInputs,
    state: FormState,
}

impl<T: AsyncIpcTransport> AsyncFrontendApp<T> {
    /// Creates an idle browser application with a finite request deadline.
    ///
    /// # Errors
    /// Returns [`ErrorCode::Timeout`] when `request_timeout` is zero.
    pub fn new(transport: T, request_timeout: Duration) -> Result<Self> {
        Ok(Self {
            client: Some(AsyncIpcClient::new(transport, request_timeout)?),
            capabilities: None,
            target_capabilities: None,
            inputs: FormInputs::new("PT-9042-ALPHA", 72.5, 4.0, 0.5),
            state: FormState::Idle,
        })
    }

    /// Returns the outcome associated with the current inputs.
    #[must_use]
    pub const fn state(&self) -> &FormState {
        &self.state
    }

    /// Returns the exact values captured from browser controls.
    #[must_use]
    pub const fn inputs(&self) -> &FormInputs {
        &self.inputs
    }

    /// Returns the capability catalog acquired during initialization.
    #[must_use]
    pub const fn capabilities(&self) -> Option<&CapabilityCatalogPayload> {
        self.capabilities.as_ref()
    }

    /// Returns the host target descriptor acquired during initialization.
    #[must_use]
    pub const fn target_capabilities(&self) -> Option<&TargetCapabilityPayload> {
        self.target_capabilities.as_ref()
    }

    /// Acquires a session capability without blocking the browser event loop.
    ///
    /// # Errors
    /// Preserves local transport/validation errors and the peer's exact
    /// session-initialization rejection. A failed initialization closes this
    /// application.
    pub async fn init(
        &mut self,
        process_id: u32,
        principal: [u8; 16],
    ) -> std::result::Result<(), HandshakeError> {
        self.state = FormState::Pending;
        let outcome =
            match self.client.as_mut() {
                Some(client) => {
                    async {
                        let token = client.handshake(process_id, principal).await?;
                        let catalog = client.discover_capabilities().await.map_err(|error| {
                            match error {
                                CapabilityError::Local(error) => HandshakeError::Local(error),
                                CapabilityError::Remote(error) => HandshakeError::Remote(error),
                                _ => HandshakeError::Local(MetisError::protocol(
                                    ErrorCode::UnexpectedMessageType,
                                    "Capability discovery returned an unsupported error variant",
                                )),
                            }
                        })?;
                        let target = client
                        .discover_target_capabilities()
                        .await
                        .map_err(|error| match error {
                            TargetCapabilityError::Local(error) => HandshakeError::Local(error),
                            TargetCapabilityError::Remote(error) => HandshakeError::Remote(error),
                            _ => HandshakeError::Local(MetisError::protocol(
                                ErrorCode::UnexpectedMessageType,
                                "Target capability discovery returned an unsupported error variant",
                            )),
                        })?;
                        Ok::<_, HandshakeError>((token, catalog, target))
                    }
                    .await
                }
                None => Err(HandshakeError::from(Self::closed())),
            };
        match outcome {
            Ok((_, catalog, target)) => {
                self.capabilities = Some(catalog);
                self.target_capabilities = Some(target);
                self.state = FormState::Idle;
                Ok(())
            }
            Err(error) => {
                self.client = None;
                self.capabilities = None;
                self.target_capabilities = None;
                self.state = FormState::SessionFailed(error.clone());
                Err(error)
            }
        }
    }

    /// Replaces browser inputs and invalidates any earlier result.
    pub fn set_inputs(&mut self, patient_id: &str, weight: f64, concentration: f64, dose: f64) {
        self.inputs = FormInputs::new(patient_id, weight, concentration, dose);
        self.state = FormState::Idle;
    }

    /// Cancels all requests left outstanding by a dropped browser task.
    ///
    /// The session capability remains valid. When at least one request was
    /// removed, the form returns to idle without presenting a stale result.
    #[must_use]
    pub fn cancel_pending_requests(&mut self) -> usize {
        let cancelled = self
            .client
            .as_mut()
            .map_or(0, AsyncIpcClient::cancel_all_requests);
        if cancelled != 0 {
            self.state = FormState::Idle;
        }
        cancelled
    }

    /// Sends one bounded request and awaits its correlated response.
    ///
    /// # Errors
    /// Preparation failures retain the session. A dispatched request closes
    /// the application when its outcome is uncertain; the exact cause remains
    /// in [`Self::state`].
    pub async fn submit_calculation(&mut self) -> Result<()> {
        self.state = FormState::Pending;
        let payload = match self.request_payload() {
            Ok(payload) => payload,
            Err(error) => {
                self.state = if self.client.is_none() {
                    FormState::Disconnected(error.clone())
                } else {
                    FormState::Failed(error.clone())
                };
                return Err(error);
            }
        };
        match self.exchange(&payload).await {
            Ok(state) => {
                self.state = state;
                Ok(())
            }
            Err(error) => {
                self.client = None;
                self.state = FormState::Disconnected(error.clone());
                Err(error)
            }
        }
    }

    /// Receives the next unsolicited event from the authenticated backend.
    ///
    /// Event delivery is separate from the correlated calculation response;
    /// a successful calculation may therefore be displayed before its event
    /// is consumed. A receive or decoding failure closes this application
    /// because the message stream can no longer be trusted.
    ///
    /// # Errors
    /// Returns transport, event-envelope, sequence, or typed protocol errors.
    pub async fn recv_event(&mut self) -> Result<RemoteEventPayload> {
        let result = match self.client.as_mut() {
            Some(client) => client.recv_event().await,
            None => Err(Self::closed()),
        };
        if result.is_err() {
            self.client = None;
        }
        result
    }

    fn closed() -> MetisError {
        MetisError::transport(
            ErrorCode::ConnectionClosed,
            "Create a new browser app with a new transport to reconnect",
        )
    }

    fn request_payload(&self) -> Result<Vec<u8>> {
        let client = self.client.as_ref().ok_or_else(Self::closed)?;
        let token = client.active_token().cloned().ok_or_else(|| {
            MetisError::capability(
                ErrorCode::MissingCapability,
                "Establish a session before submitting",
            )
        })?;
        ClinicalCalcRequestPayload {
            token,
            patient_id: self.inputs.patient_id.clone(),
            weight_kg: self.inputs.weight_kg,
            concentration_mg_ml: self.inputs.concentration_mg_ml,
            target_dose_mcg_kg_min: self.inputs.target_dose_mcg_kg_min,
        }
        .encode()
    }

    async fn exchange(&mut self, payload: &[u8]) -> Result<FormState> {
        let client = self.client.as_mut().ok_or_else(Self::closed)?;
        let (kind, response) = client
            .send_and_recv(MessageType::ClinicalCalcReq, payload)
            .await?;
        match kind {
            MessageType::ClinicalCalcResp => {
                ClinicalCalcResponsePayload::decode(&response).map(FormState::Success)
            }
            MessageType::ErrorResp => {
                ErrorResponsePayload::decode(&response).map(FormState::Rejected)
            }
            other => Err(MetisError::protocol(
                ErrorCode::UnexpectedMessageType,
                format!("Unexpected response type: {other:?}"),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use metis_core::protocol::FrameHeader;
    use std::future::{Future, ready};
    use std::pin::Pin;
    use std::task::{Context, Poll, Waker};

    struct ClosedTransport;

    impl AsyncIpcTransport for ClosedTransport {
        fn send_frame(&mut self, _frame: &[u8]) -> Result<()> {
            Err(MetisError::transport(
                ErrorCode::ConnectionClosed,
                "test transport is closed",
            ))
        }

        fn recv_message(
            &mut self,
            _timeout: Duration,
        ) -> impl Future<Output = Result<(FrameHeader, Vec<u8>)>> + '_ {
            ready(Err(MetisError::transport(
                ErrorCode::ConnectionClosed,
                "test transport is closed",
            )))
        }
    }

    struct OpenTransport;

    impl AsyncIpcTransport for OpenTransport {
        fn send_frame(&mut self, _frame: &[u8]) -> Result<()> {
            Ok(())
        }

        fn recv_message(
            &mut self,
            _timeout: Duration,
        ) -> impl Future<Output = Result<(FrameHeader, Vec<u8>)>> + '_ {
            ready(Err(MetisError::transport(
                ErrorCode::ConnectionClosed,
                "test transport has no response",
            )))
        }
    }

    fn poll_ready<F: Future>(future: F) -> F::Output {
        let mut future = Box::pin(future);
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        match Pin::new(&mut future).poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("closed transport future must be ready"),
        }
    }

    #[test]
    fn browser_submission_without_a_session_is_a_typed_local_failure() {
        let mut app = AsyncFrontendApp::new(ClosedTransport, Duration::from_secs(1))
            .expect("positive timeout");
        let error = poll_ready(app.submit_calculation()).expect_err("session is required");
        assert_eq!(error.code, ErrorCode::MissingCapability);
        assert!(
            matches!(app.state(), FormState::Failed(error) if error.code == ErrorCode::MissingCapability)
        );
    }

    #[test]
    fn input_edits_clear_a_prior_outcome() {
        let mut app = AsyncFrontendApp::new(ClosedTransport, Duration::from_secs(1))
            .expect("positive timeout");
        app.set_inputs("patient", 60.0, 2.0, 0.2);
        assert_eq!(app.state(), &FormState::Idle);
        assert_eq!(app.inputs().weight_kg.to_bits(), 60.0_f64.to_bits());
    }

    #[test]
    fn cancelling_a_dropped_request_returns_the_form_to_idle() {
        let mut app =
            AsyncFrontendApp::new(OpenTransport, Duration::from_secs(1)).expect("positive timeout");
        app.state = FormState::Pending;
        app.client
            .as_mut()
            .expect("client")
            .send_request(MessageType::HeartbeatReq, b"request")
            .expect("request");

        assert_eq!(app.cancel_pending_requests(), 1);
        assert_eq!(app.state(), &FormState::Idle);
        assert_eq!(app.cancel_pending_requests(), 0);
    }
}
