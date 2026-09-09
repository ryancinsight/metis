//! Form markup and projection of owned application state.

/// Default declarative UI markup template for the medical data entry screen.
pub const CLINICAL_SCREEN_XML: &str = r#"<screen id="main-screen" style="display: flex; flex-direction: column; height: 100%; background-color: #f0f4f8; padding: 20px; gap: 15px;">
  <div id="header" style="display: flex; flex-direction: column; gap: 8px; background-color: #1a365d; padding: 12px;">
    <text style="color: #ffffff; font-size: 16px;">METIS FORM DEMONSTRATION</text>
    <text id="status-badge" style="color: #38a169; font-size: 12px;">SYSTEM READY</text>
  </div>

  <card id="patient-card" style="display: flex; flex-direction: column; background-color: #ffffff; padding: 16px; border-width: 1px; border-color: #e2e8f0; gap: 10px;">
    <text style="color: #2d3748; font-size: 14px;">Patient Demographics and Drug Prescription</text>
    <div id="row-patient" style="display: flex; flex-direction: row; gap: 10px;">
      <text id="label-patient" style="color: #4a5568; font-size: 12px;">Patient ID: PT-9042-ALPHA</text>
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
      <button id="btn-calc" style="background-color: #3182ce; color: #ffffff; padding: 8px 16px;">
        <text style="color: #ffffff; font-size: 12px;">[ SUBMIT CALCULATION TO BACKEND ]</text>
      </button>
    </div>
  </card>

  <card id="results-card" style="display: flex; flex-direction: column; background-color: #ffffff; padding: 16px; border-width: 1px; border-color: #e2e8f0; gap: 8px;">
    <text style="color: #2d3748; font-size: 14px;">Backend Calculation Output</text>
    <text id="output-rate" style="color: #3182ce; font-size: 16px;">Rate: Awaiting Backend Calculation...</text>
    <text id="output-status" style="color: #718096; font-size: 12px;">Safety Status: Idle</text>
    <text id="output-signature" style="color: #718096; font-size: 10px;">Backend MAC: None</text>
  </card>
</screen>"#;

use crate::{FormState, FrontendApp};
use iris::render::RenderBackend;
use metis_core::{ErrorCode, MetisError, Result};
use metis_ipc::{IpcTransport, client::HandshakeError};
use metis_ui_lang::{Color, compute_layout};

impl<T: IpcTransport> FrontendApp<T> {
    /// Projects the owned state and renders the complete form.
    /// # Errors
    /// Rejects invalid layout or a missing authored label before changing pixels.
    pub fn render(&mut self) -> Result<()> {
        let badge = if self.client.is_none() {
            "SESSION CLOSED"
        } else if self
            .client
            .as_ref()
            .and_then(metis_ipc::IpcClient::active_token)
            .is_some()
        {
            "SESSION ACTIVE"
        } else {
            "SYSTEM READY"
        };
        self.text("status-badge", badge)?;
        let status = self
            .doc
            .find_element_by_id_mut("status-badge")
            .ok_or_else(|| {
                MetisError::ui(
                    ErrorCode::MalformedMarkup,
                    "Authored form is missing status badge",
                )
            })?;
        status.computed_style.text_color = if self.client.is_none() {
            Color::RED
        } else {
            Color::GREEN
        };
        // Forty glyphs plus the label fit the 800-pixel form. The ellipsis states
        // omission explicitly; the wire request uses the full captured identifier.
        let mut preview: String = self.inputs.patient_id.chars().take(40).collect();
        if self.inputs.patient_id.chars().nth(40).is_some() {
            preview.push_str("...");
        }
        if let Some(composition) = self.composition.as_deref() {
            preview.push_str(" [");
            let mut composition_preview: String = composition.chars().take(24).collect();
            if composition.chars().nth(24).is_some() {
                composition_preview.push_str("...");
            }
            preview.push_str(&composition_preview);
            preview.push(']');
        }
        self.text("label-patient", format!("Patient ID: {preview}"))?;
        self.text(
            "label-weight",
            format!("Weight: {} kg", input_number(self.inputs.weight_kg, 2)),
        )?;
        self.text(
            "label-conc",
            format!(
                "Drug Concentration: {} mg/mL",
                input_number(self.inputs.concentration_mg_ml, 2)
            ),
        )?;
        self.text(
            "label-dose",
            format!(
                "Target Dose: {} mcg/kg/min",
                input_number(self.inputs.target_dose_mcg_kg_min, 3)
            ),
        )?;
        let (rate, status, signature) = self.outcome_text();
        self.text("output-rate", rate)?;
        self.text("output-status", status)?;
        self.text("output-signature", signature)?;
        let width = i32::try_from(self.framebuffer.width()).map_err(|_| layout_error())?;
        let height = i32::try_from(self.framebuffer.height()).map_err(|_| layout_error())?;
        let display = compute_layout(&self.doc, width, height)?;
        self.framebuffer.clear(Color::rgb(240, 244, 248));
        self.framebuffer
            .render(&display)
            .unwrap_or_else(|never| match never {});
        Ok(())
    }

    fn outcome_text(&self) -> (String, String, &'static str) {
        match &self.state {
            FormState::Idle => (
                "Rate: Awaiting Backend Calculation...".into(),
                "Safety Status: Idle".into(),
                "Backend MAC: None",
            ),
            FormState::Pending => (
                "Rate: Awaiting Backend Calculation...".into(),
                "Request in progress".into(),
                "Backend MAC: None",
            ),
            FormState::Success(response) => (
                format!(
                    "Rate: {} mL/hr ({} mg/hr)",
                    result_number(response.rate_ml_hr, 3),
                    result_number(response.drug_rate_mg_hr, 2)
                ),
                format!(
                    "Backend response (Audit Seq #{}){}",
                    response.audit_sequence_id,
                    if response.is_pediatric {
                        " [PEDIATRIC]"
                    } else {
                        ""
                    }
                ),
                "Backend MAC: Present (not verified by frontend)",
            ),
            FormState::Rejected(error) => (
                "Rate: No result".into(),
                format!("Backend rejected request [0x{:04X}]", error.error_code),
                "Backend MAC: No result",
            ),
            FormState::Failed(error) => (
                "Rate: No result".into(),
                format!("Request not sent [0x{:04X}]", error.code as u16),
                "Backend MAC: No result",
            ),
            FormState::Disconnected(error) => (
                "Rate: No result".into(),
                format!(
                    "Connection failed [0x{:04X}] - reconnect",
                    error.code as u16
                ),
                "Backend MAC: No result",
            ),
            FormState::SessionFailed(error) => {
                let code = match error {
                    HandshakeError::Local(error) => format!("0x{:04X}", error.code as u16),
                    HandshakeError::Remote(error) => format!("0x{:04X}", error.error_code),
                    _ => "unrecognized".into(),
                };
                (
                    "Rate: No result".into(),
                    format!("Session failed [{code}] - reconnect"),
                    "Backend MAC: No result",
                )
            }
        }
    }

    fn text(&mut self, id: &str, value: impl Into<String>) -> Result<()> {
        if self.doc.set_text_content(id, value) {
            Ok(())
        } else {
            Err(MetisError::ui(
                ErrorCode::MalformedMarkup,
                format!("Authored form is missing label {id}"),
            ))
        }
    }
}

fn layout_error() -> MetisError {
    MetisError::ui(
        ErrorCode::LayoutOverflow,
        "Surface exceeds layout coordinates",
    )
}

// Shortest scientific f64 notation fits 24 glyphs: sign, 17 significant digits,
// decimal point, exponent marker/sign and three exponent digits.
const NUMBER_GLYPHS: usize = 24;

fn result_number(value: f64, decimals: usize) -> String {
    let rounded = format!("{value:.decimals$}");
    // Scientific notation keeps a nonzero mantissa when fixed decimal rounding
    // would erase every significant digit, with the same displayed precision.
    if rounded.len() > NUMBER_GLYPHS
        || value.is_subnormal()
        || (value.is_normal() && rounded.chars().all(|c| matches!(c, '0' | '.' | '-')))
    {
        format!("{value:.decimals$e}")
    } else {
        rounded
    }
}

fn input_number(value: f64, minimum_decimals: usize) -> String {
    // Shortest scientific f64 notation fits 24 glyphs: sign, 17 significant
    // digits, decimal point, exponent marker/sign and three exponent digits.
    // Preserve those digits instead of rounding accepted small inputs to zero.
    let mut text = value.to_string();
    if text.len() > NUMBER_GLYPHS {
        return format!("{value:e}");
    }
    if value.is_finite() {
        let decimals = text
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len());
        if decimals < minimum_decimals {
            if decimals == 0 {
                text.push('.');
            }
            for _ in decimals..minimum_decimals {
                text.push('0');
            }
        }
    }
    text
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
