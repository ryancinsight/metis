//! Unprivileged form transitions and correlated backend requests.
use crate::presentation::CLINICAL_SCREEN_XML;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{
    ClinicalCalcRequestPayload, ClinicalCalcResponsePayload, ErrorResponsePayload, MessageType,
    RemoteEventPayload,
};
use metis_ipc::client::{HandshakeError, IpcClient};
use metis_ipc::transport::IpcTransport;
use metis_platform::Framebuffer;
use metis_ui_lang::{DomDocument, parse_markup};

/// Outcome for the current inputs. Only success carries a result.
/// Pending is painted before synchronous IPC; it does not imply an async host.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FormState {
    /// No calculation belongs to the current inputs.
    Idle,
    /// A handshake or calculation is awaiting the backend.
    Pending,
    /// Correlated response; its MAC is not verified by this frontend.
    Success(ClinicalCalcResponsePayload),
    /// Correlated peer rejection, preserving the exact wire code and diagnostic.
    Rejected(ErrorResponsePayload),
    /// Failure before dispatch; the transport remains usable.
    Failed(MetisError),
    /// Dispatch or reply failed; recovery requires a new transport and app.
    Disconnected(MetisError),
    /// Handshake failed; the session is closed, preserving the exact cause.
    SessionFailed(HandshakeError),
}

/// Maximum UTF-8 bytes retained for one uncommitted text composition.
pub const MAX_COMPOSITION_BYTES: usize = 128;

/// Raw captured inputs; domain validation remains exclusively in the backend.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FormInputs {
    /// Exact patient reference sent on submission, including preview-omitted text.
    pub patient_id: String,
    /// Captured mass, in kilograms.
    pub weight_kg: f64,
    /// Captured concentration, in milligrams per milliliter.
    pub concentration_mg_ml: f64,
    /// Captured target dose, in micrograms per kilogram per minute.
    pub target_dose_mcg_kg_min: f64,
}

impl FormInputs {
    /// Creates a captured input set owned by a frontend host.
    #[must_use]
    pub fn new(
        patient_id: impl Into<String>,
        weight_kg: f64,
        concentration_mg_ml: f64,
        target_dose_mcg_kg_min: f64,
    ) -> Self {
        Self {
            patient_id: patient_id.into(),
            weight_kg,
            concentration_mg_ml,
            target_dose_mcg_kg_min,
        }
    }
}

/// Owns inputs and presentation so callers cannot bypass result invalidation.
pub struct FrontendApp<T> {
    pub(crate) client: Option<IpcClient<T>>,
    pub(crate) doc: DomDocument,
    pub(crate) framebuffer: Framebuffer,
    pub(crate) inputs: FormInputs,
    pub(crate) composition: Option<String>,
    pub(crate) state: FormState,
}

impl<T: IpcTransport> FrontendApp<T> {
    /// Constructs and renders an idle form without a session capability.
    /// # Errors
    /// Rejects invalid markup, surface dimensions or layout.
    pub fn new(transport: T, width: u32, height: u32) -> Result<Self> {
        let mut app = Self {
            client: Some(IpcClient::new(transport)),
            doc: parse_markup(CLINICAL_SCREEN_XML)?,
            framebuffer: Framebuffer::new(width, height)?,
            inputs: FormInputs::new("PT-9042-ALPHA", 72.5, 4.0, 0.5),
            composition: None,
            state: FormState::Idle,
        };
        app.render()?;
        Ok(app)
    }

    /// Outcome associated with current inputs; edits invalidate it.
    #[must_use]
    pub const fn state(&self) -> &FormState {
        &self.state
    }
    /// Exact captured inputs, including values the backend may reject.
    #[must_use]
    pub const fn inputs(&self) -> &FormInputs {
        &self.inputs
    }

    /// Returns the current uncommitted native text composition, if any.
    #[must_use]
    pub fn composition(&self) -> Option<&str> {
        self.composition.as_deref()
    }
    /// Presentation document; mutations go through form transitions.
    #[must_use]
    pub const fn document(&self) -> &DomDocument {
        &self.doc
    }
    /// Actual rendered pixels for host presentation or capture.
    #[must_use]
    pub const fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    /// Acquires a capability and clears any earlier calculation.
    /// # Errors
    /// Preserves peer/local handshake failures and closes the transport on failure.
    /// If rendering fails, the operation outcome remains in `state()`.
    pub fn init(&mut self, process_id: u32, principal: [u8; 16]) -> Result<(), HandshakeError> {
        self.state = FormState::Pending;
        self.render()?;
        let outcome = self
            .client
            .as_mut()
            .ok_or_else(Self::closed)
            .map_err(HandshakeError::from)
            .and_then(|client| client.handshake(process_id, principal));
        match outcome {
            Ok(_) => {
                self.state = FormState::Idle;
                self.render()?;
                Ok(())
            }
            Err(error) => {
                self.client = None;
                self.state = FormState::SessionFailed(error.clone());
                self.render()?;
                Err(error)
            }
        }
    }

    /// Replaces inputs, invalidates any result and immediately renders.
    /// # Errors
    /// Reports presentation failure; new inputs and idle state remain owned.
    /// Numeric acceptance and calculation are backend operations.
    pub fn set_inputs(
        &mut self,
        patient_id: &str,
        weight: f64,
        conc: f64,
        dose: f64,
    ) -> Result<()> {
        self.inputs = FormInputs {
            patient_id: patient_id.into(),
            weight_kg: weight,
            concentration_mg_ml: conc,
            target_dose_mcg_kg_min: dose,
        };
        self.composition = None;
        self.state = FormState::Idle;
        self.render()
    }

    /// Replaces the uncommitted text composition and repaints the form.
    ///
    /// Composition text is transient and is never included in a backend
    /// request until the host commits it through [`Self::set_inputs`].
    ///
    /// # Errors
    /// Returns a bounded-input or presentation error and restores the previous
    /// composition when repainting fails.
    pub fn set_composition(&mut self, composition: Option<String>) -> Result<()> {
        if let Some(value) = composition.as_ref()
            && (value.len() > MAX_COMPOSITION_BYTES || value.chars().any(char::is_control))
        {
            return Err(MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Native text composition exceeds the bounded patient-field limit",
            ));
        }
        let previous = std::mem::replace(&mut self.composition, composition);
        if let Err(error) = self.render() {
            self.composition = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Replaces the presentation surface at a native host's reported client size.
    ///
    /// The previous surface remains intact when allocation or the first render
    /// of the replacement fails, so a resize cannot leave the application with
    /// a partially initialized framebuffer.
    ///
    /// # Errors
    /// Returns a bounded surface-allocation or layout error and preserves the
    /// previous framebuffer when the replacement cannot be rendered.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        let replacement = Framebuffer::new(width, height)?;
        let previous = std::mem::replace(&mut self.framebuffer, replacement);
        if let Err(error) = self.render() {
            self.framebuffer = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Submits inputs; peer rejection is a completed exchange in `state()`.
    /// # Errors
    /// Preparation failures retain the session. Dispatch, correlation and decoding
    /// failures close it because the transaction outcome is uncertain. The cause
    /// remains in `state()` even if subsequent rendering fails.
    pub fn submit_calculation(&mut self) -> Result<()> {
        self.state = FormState::Pending;
        self.render()?;
        let payload = match self.request_payload() {
            Ok(payload) => payload,
            Err(error) => {
                self.state = if self.client.is_none() {
                    FormState::Disconnected(error.clone())
                } else {
                    FormState::Failed(error.clone())
                };
                self.render()?;
                return Err(error);
            }
        };
        match self.exchange(&payload) {
            Ok(state) => {
                self.state = state;
                self.render()
            }
            Err(error) => {
                self.client = None;
                self.state = FormState::Disconnected(error.clone());
                self.render()?;
                Err(error)
            }
        }
    }

    /// Receives the next unsolicited event from the authenticated backend.
    ///
    /// The correlated response and its event are separate messages. Call this
    /// after a successful operation when the host owns the receive stream.
    /// A receive or decoding failure closes the application because the stream
    /// can no longer be trusted.
    ///
    /// # Errors
    /// Returns transport, event-envelope, sequence, or typed protocol errors.
    pub fn recv_event(&mut self) -> Result<RemoteEventPayload> {
        let result = match self.client.as_mut() {
            Some(client) => client.recv_event(),
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
            "Create a new app with a new transport to reconnect",
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
    fn exchange(&mut self, payload: &[u8]) -> Result<FormState> {
        let client = self.client.as_mut().ok_or_else(Self::closed)?;
        let (kind, response) = client.send_and_recv(MessageType::ClinicalCalcReq, payload)?;
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
    use super::{FrontendApp, MAX_COMPOSITION_BYTES};
    use metis_core::ErrorCode;
    use metis_ipc::MemoryTransport;

    #[test]
    fn resize_replaces_surface_and_preserves_it_on_invalid_dimensions() {
        let (transport, _peer) = MemoryTransport::pair();
        let mut app = FrontendApp::new(transport, 800, 600).expect("initial form");
        app.resize(640, 480).expect("valid resize");
        assert_eq!(app.framebuffer().width(), 640);
        assert_eq!(app.framebuffer().height(), 480);

        let error = app.resize(0, 480).expect_err("zero width");
        assert_eq!(error.code, ErrorCode::SurfaceAllocationError);
        assert_eq!(app.framebuffer().width(), 640);
        assert_eq!(app.framebuffer().height(), 480);
    }

    #[test]
    fn composition_is_transient_and_bounded() {
        let (transport, _peer) = MemoryTransport::pair();
        let mut app = FrontendApp::new(transport, 800, 600).expect("initial form");
        app.set_composition(Some("東京".to_owned()))
            .expect("bounded composition");
        assert_eq!(app.composition(), Some("東京"));
        assert_eq!(
            app.document()
                .find_element_by_id("label-patient")
                .expect("patient label")
                .text_content(),
            "Patient ID: PT-9042-ALPHA [東京]"
        );

        let error = app
            .set_composition(Some("x".repeat(MAX_COMPOSITION_BYTES + 1)))
            .expect_err("oversized composition");
        assert_eq!(error.code, ErrorCode::PayloadTooLarge);
        assert_eq!(app.composition(), Some("東京"));

        app.set_inputs("PT-2", 70.0, 4.0, 0.5)
            .expect("committed input edit");
        assert_eq!(app.composition(), None);
    }
}
