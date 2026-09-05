//! Unprivileged UI application managing presentation, user input capture, and IPC requests.

use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{
    ClinicalCalcRequestPayload, ClinicalCalcResponsePayload, ErrorResponsePayload, MessageType,
};
use metis_ipc::client::{HandshakeError, IpcClient};
use metis_ipc::transport::IpcTransport;
use metis_platform::PlatformSurface;
use metis_ui_lang::dom::DomDocument;
use metis_ui_lang::layout::compute_layout;
use metis_ui_lang::parser::parse_markup;
use metis_ui_lang::style::Color;

/// Default declarative UI markup template for the medical data entry screen.
pub const CLINICAL_SCREEN_XML: &str = r#"<screen id="main-screen" style="display: flex; flex-direction: column; height: 100%; background-color: #f0f4f8; padding: 20px; gap: 15px;">
  <div id="header" style="display: flex; flex-direction: column; gap: 8px; background-color: #1a365d; padding: 12px;">
    <text style="color: #ffffff; font-size: 16px; font-weight: bold;">METIS FORM DEMONSTRATION</text>
    <text id="status-badge" style="color: #38a169; font-size: 12px;">SYSTEM READY</text>
  </div>

  <card id="patient-card" style="display: flex; flex-direction: column; background-color: #ffffff; padding: 16px; border-radius: 6px; border-width: 1px; border-color: #e2e8f0; gap: 10px;">
    <text style="color: #2d3748; font-size: 14px; font-weight: bold;">Patient Demographics and Drug Prescription</text>
    <div id="row-patient" style="display: flex; flex-direction: row; gap: 10px;">
      <text style="color: #4a5568; font-size: 12px;">Patient ID: PT-9042-ALPHA</text>
    </div>
    <div id="row-weight" style="display: flex; flex-direction: row; gap: 10px;">
      <text id="label-weight" style="color: #4a5568; font-size: 12px;">Weight: 72.50 kg</text>
    </div>
    <div id="row-conc" style="display: flex; flex-direction: row; gap: 10px;">
      <text id="label-conc" style="color: #4a5568; font-size: 12px;">Drug Concentration: 4.00 mg/mL</text>
    </div>
    <div id="row-dose" style="display: flex; flex-direction: row; gap: 10px;">
      <text id="label-dose" style="color: #4a5568; font-size: 12px;">Target Dose: 0.500 mcg/kg/min</text>
    </div>
    <div id="actions" style="display: flex; flex-direction: row; gap: 10px; margin: 10px 0 0 0;">
      <button id="btn-calc" style="background-color: #3182ce; color: #ffffff; padding: 8px 16px; border-radius: 4px; font-weight: bold;">
        <text style="color: #ffffff; font-size: 12px; font-weight: bold;">[ SUBMIT CALCULATION TO BACKEND ]</text>
      </button>
    </div>
  </card>

  <card id="results-card" style="display: flex; flex-direction: column; background-color: #ffffff; padding: 16px; border-radius: 6px; border-width: 1px; border-color: #e2e8f0; gap: 8px;">
    <text style="color: #2d3748; font-size: 14px; font-weight: bold;">Backend Calculation Output</text>
    <text id="output-rate" style="color: #3182ce; font-size: 16px; font-weight: bold;">Rate: Awaiting Backend Calculation...</text>
    <text id="output-status" style="color: #718096; font-size: 12px;">Safety Status: Idle</text>
    <text id="output-signature" style="color: #718096; font-size: 10px;">Backend MAC: None</text>
  </card>
</screen>"#;

/// The unprivileged presentation frontend application.
pub struct FrontendApp<T: IpcTransport> {
    /// Session client; receives no backend secret key.
    pub client: IpcClient<T>,
    /// Presentation document for this form.
    pub doc: DomDocument,
    /// Software rendering surface.
    pub surface: PlatformSurface,
    /// Opaque form identifier sent to the backend.
    pub patient_id: String,
    /// Submitted weight; backend validates domain constraints.
    pub weight_kg: f64,
    /// Submitted concentration; backend validates domain constraints.
    pub concentration_mg_ml: f64,
    /// Submitted dose; backend validates domain constraints.
    pub target_dose_mcg_kg_min: f64,
    /// Most recent correlated backend result, without frontend MAC verification.
    pub last_response: Option<ClinicalCalcResponsePayload>,
    /// Most recent rejected request diagnostic.
    pub last_error: Option<String>,
}

impl<T: IpcTransport> FrontendApp<T> {
    /// Initializes a form document and a bounded software surface.
    ///
    /// # Errors
    /// Rejects invalid markup or surface dimensions.
    pub fn new(transport: T, width: u32, height: u32) -> Result<Self> {
        let doc = parse_markup(CLINICAL_SCREEN_XML)?;
        let surface = PlatformSurface::new(width, height)?;
        Ok(Self {
            client: IpcClient::new(transport),
            doc,
            surface,
            patient_id: "PT-9042-ALPHA".to_string(),
            weight_kg: 72.5,
            concentration_mg_ml: 4.0,
            target_dose_mcg_kg_min: 0.5,
            last_response: None,
            last_error: None,
        })
    }

    /// Performs the initial handshake with the backend to acquire capability token.
    ///
    /// # Errors
    /// Preserves decoded peer rejections in [`HandshakeError::Remote`]; local
    /// handshake, correlation and presentation failures use [`HandshakeError::Local`].
    pub fn init(&mut self, process_id: u32, principal_id: [u8; 16]) -> Result<(), HandshakeError> {
        let token = self.client.handshake(process_id, principal_id)?;
        self.doc.set_text_content(
            "status-badge",
            format!(
                "TOKEN #{} ACTIVE (SCOPE 0x{:02X})",
                token.token_id, token.scope.0
            ),
        );
        self.render()?;
        Ok(())
    }

    /// Sets form parameter inputs and updates DOM text labels.
    pub fn set_inputs(&mut self, patient_id: &str, weight: f64, conc: f64, dose: f64) {
        self.patient_id = patient_id.to_string();
        self.weight_kg = weight;
        self.concentration_mg_ml = conc;
        self.target_dose_mcg_kg_min = dose;

        self.doc
            .set_text_content("row-patient", format!("Patient ID: {}", self.patient_id));
        self.doc
            .set_text_content("label-weight", format!("Weight: {:.2} kg", self.weight_kg));
        self.doc.set_text_content(
            "label-conc",
            format!("Drug Concentration: {:.2} mg/mL", self.concentration_mg_ml),
        );
        self.doc.set_text_content(
            "label-dose",
            format!("Target Dose: {:.3} mcg/kg/min", self.target_dose_mcg_kg_min),
        );
    }

    /// Submits the current form data over IPC to the backend for safety verification and calculation.
    ///
    /// # Errors
    /// Rejects missing capabilities, wire failures and unrenderable responses.
    pub fn submit_calculation(&mut self) -> Result<()> {
        let token = self.client.active_token().cloned().ok_or_else(|| {
            MetisError::capability(
                ErrorCode::MissingCapability,
                "No active capability token held by frontend",
            )
        })?;

        let req = ClinicalCalcRequestPayload {
            token,
            patient_id: self.patient_id.clone(),
            weight_kg: self.weight_kg,
            concentration_mg_ml: self.concentration_mg_ml,
            target_dose_mcg_kg_min: self.target_dose_mcg_kg_min,
        };

        let (resp_type, resp_payload) = self
            .client
            .send_and_recv(MessageType::ClinicalCalcReq, &req.encode()?)?;

        match resp_type {
            MessageType::ClinicalCalcResp => {
                let resp = ClinicalCalcResponsePayload::decode(&resp_payload)?;
                self.doc.set_text_content(
                    "output-rate",
                    format!(
                        "Rate: {:.3} mL/hr ({:.2} mg/hr){}",
                        resp.rate_ml_hr,
                        resp.drug_rate_mg_hr,
                        if resp.is_pediatric {
                            " [PEDIATRIC ENVELOPE]"
                        } else {
                            ""
                        }
                    ),
                );
                self.doc.set_text_content(
                    "output-status",
                    format!("Backend response (Audit Seq #{})", resp.audit_sequence_id),
                );
                self.doc.set_text_content(
                    "output-signature",
                    format!(
                        "Backend MAC: {:02X}{:02X}{:02X}{:02X}... (not verified by frontend)",
                        resp.result_signature[0],
                        resp.result_signature[1],
                        resp.result_signature[2],
                        resp.result_signature[3]
                    ),
                );
                self.last_response = Some(resp);
                self.last_error = None;
            }
            MessageType::ErrorResp => {
                let err = ErrorResponsePayload::decode(&resp_payload)?;
                self.doc
                    .set_text_content("output-rate", "Rate: BLOCKED BY SAFETY INTERLOCK");
                self.doc.set_text_content(
                    "output-status",
                    format!(
                        "SAFETY INTERLOCK [0x{:04X}]: {}",
                        err.error_code, err.message
                    ),
                );
                self.doc
                    .set_text_content("output-signature", "Backend MAC: No result");
                self.last_error = Some(err.message);
                self.last_response = None;
            }
            other => {
                return Err(MetisError::protocol(
                    ErrorCode::UnexpectedMessageType,
                    format!("Unexpected response type: {other:?}"),
                ));
            }
        }

        self.render()?;
        Ok(())
    }

    /// Re-evaluates layout and renders to the platform surface.
    ///
    /// # Errors
    /// Rejects coordinates or document layout exceeding presentation limits.
    pub fn render(&mut self) -> Result<()> {
        let w = i32::try_from(self.surface.framebuffer.width()).map_err(|_| {
            MetisError::ui(
                ErrorCode::LayoutOverflow,
                "Surface width exceeds layout coordinates",
            )
        })?;
        let h = i32::try_from(self.surface.framebuffer.height()).map_err(|_| {
            MetisError::ui(
                ErrorCode::LayoutOverflow,
                "Surface height exceeds layout coordinates",
            )
        })?;
        self.surface.framebuffer.clear(Color::rgb(240, 244, 248));
        let display_list = compute_layout(&self.doc, w, h)?;
        display_list.render_to(&mut self.surface.framebuffer);
        Ok(())
    }
}

#[cfg(test)]
mod presentation_tests {
    use super::CLINICAL_SCREEN_XML;
    use metis_platform::{Color, FONT_HEIGHT, FONT_WIDTH, Framebuffer};
    use metis_ui_lang::{DisplayCommand, compute_layout, parse_markup};

    #[test]
    fn authored_form_text_and_status_fit_the_viewport() {
        let document = parse_markup(CLINICAL_SCREEN_XML).expect("authored markup");
        let display = compute_layout(&document, 800, 600).expect("authored layout");
        let mut text_runs = Vec::new();
        for command in &display.commands {
            if let DisplayCommand::DrawText {
                text, x, y, scale, ..
            } = command
            {
                let width = i64::try_from(text.chars().count()).expect("bounded text")
                    * i64::from(FONT_WIDTH)
                    * i64::from(*scale);
                let height = i64::from(FONT_HEIGHT) * i64::from(*scale);
                assert!(
                    *x >= 0 && i64::from(*x) + width <= 800,
                    "horizontal clipping: {text}"
                );
                assert!(
                    *y >= 0 && i64::from(*y) + height <= 600,
                    "vertical clipping: {text}"
                );
                text_runs.push((text.as_str(), *x, *y));
            }
        }
        assert_eq!(text_runs[0], ("METIS FORM DEMONSTRATION", 32, 32));
        assert_eq!(text_runs[1], ("SYSTEM READY", 32, 56));
        assert_eq!(text_runs[2].0, "Patient Demographics and Drug Prescription");
        let mut framebuffer = Framebuffer::new(800, 600).expect("presentation surface");
        display.render_to(&mut framebuffer);
        // 'S' has an ink pixel at cell (2,2); the complete status line is in bounds.
        assert_eq!(framebuffer.get_pixel(34, 58), Color::GREEN);
        assert_eq!(framebuffer.get_pixel(799, 599), Color::rgb(240, 244, 248));
    }
}
