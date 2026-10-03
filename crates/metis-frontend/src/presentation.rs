//! Form markup and projection of owned application state.

/// Default declarative UI markup template for the medical data entry screen.
pub const CLINICAL_SCREEN_XML: &str = r#"<screen id="main-screen" style="display: flex; flex-direction: column; height: 100%; background-color: #f0f4f8; padding: 20px; gap: 15px;">
  <div id="header" style="display: flex; flex-direction: column; gap: 8px; background: linear-gradient(135deg, #1a365d, #2c5282); padding: 12px; border-radius: 12px; box-shadow: 0 4px 12px #1a365d40;">
    <text style="color: #ffffff; font-size: 22px; font-weight: bold;">METIS FORM DEMONSTRATION</text>
    <text id="status-badge" style="color: #9ae6b4; font-size: 13px;">SYSTEM READY</text>
  </div>

  <nav id="application-navigation" aria-label="Application navigation" style="display: flex; flex-direction: column; gap: 8px; background-color: #e2e8f0; padding: 8px; border-radius: 10px;">
    <div id="application-toolbar" role="toolbar" aria-label="Application commands" style="display: flex; flex-direction: row; gap: 8px; align-items: center;">
      <button id="command-menu-toggle" aria-haspopup="menu" aria-expanded="false" aria-controls="command-menu" style="width: 120px; background: linear-gradient(#2c78c4, #2662a8); color: #ffffff; padding: 8px 12px; border-radius: 6px; min-height: 44px; justify-content: center; align-items: center; box-shadow: 0 2px 4px #2c528240;">
        <text style="color: #ffffff; font-size: 14px; font-weight: bold;">Commands</text>
      </button>
      <button id="command-focus-patient" aria-keyshortcuts="Alt+Shift+P" style="width: 144px; background: linear-gradient(#2c78c4, #2662a8); color: #ffffff; padding: 8px 12px; border-radius: 6px; min-height: 44px; justify-content: center; align-items: center; box-shadow: 0 2px 4px #2c528240;">
        <text style="color: #ffffff; font-size: 14px; font-weight: bold;">Focus patient</text>
      </button>
    </div>
    <div id="command-menu" popover-anchor="command-menu-toggle" role="menu" aria-label="Application commands" aria-hidden="true" style="display: none; flex-direction: column; gap: 6px; background-color: #ffffff; padding: 8px; border-width: 1px; border-color: #e2e8f0; border-radius: 10px; box-shadow: 0 8px 20px #0f172a33;">
      <button id="command-theme-dark" role="menuitem" aria-keyshortcuts="Alt+Shift+D" style="background: linear-gradient(#2c78c4, #2662a8); color: #ffffff; padding: 8px 12px; border-radius: 6px; min-height: 44px; justify-content: center; align-items: center; box-shadow: 0 2px 4px #2c528240;">
        <text style="color: #ffffff; font-size: 14px; font-weight: bold;">Dark theme</text>
      </button>
      <button id="command-theme-system" role="menuitem" aria-keyshortcuts="Alt+Shift+S" style="background: linear-gradient(#2c78c4, #2662a8); color: #ffffff; padding: 8px 12px; border-radius: 6px; min-height: 44px; justify-content: center; align-items: center; box-shadow: 0 2px 4px #2c528240;">
        <text style="color: #ffffff; font-size: 14px; font-weight: bold;">System theme</text>
      </button>
    </div>
    <text id="command-status" role="status" aria-live="polite" style="color: #4a5568; font-size: 13px;">Commands ready</text>
  </nav>

  <card id="patient-card" style="display: flex; flex-direction: column; background-color: #ffffff; padding: 16px; border-width: 1px; border-color: #e2e8f0; border-radius: 12px; gap: 10px; box-shadow: 0 2px 10px #0f172a1f;">
    <text id="patient-heading" style="color: #2d3748; font-size: 16px; font-weight: bold;">Patient Demographics and Drug Prescription</text>
    <div id="row-patient" style="display: flex; flex-direction: row; gap: 10px;">
      <text id="label-patient" role="textbox" aria-label="Patient ID" value="PT-9042-ALPHA" tabindex="0" style="color: #4a5568; font-size: 14px;">Patient ID: PT-9042-ALPHA</text>
    </div>
    <div id="row-weight" style="display: flex; flex-direction: row; gap: 10px;">
      <text id="label-weight" style="color: #4a5568; font-size: 14px;">Weight: 72.50 kg</text>
    </div>
    <div id="row-conc" style="display: flex; flex-direction: row; gap: 10px;">
      <text id="label-conc" style="color: #4a5568; font-size: 14px;">Drug Concentration: 4.00 mg/mL</text>
    </div>
    <div id="row-dose" style="display: flex; flex-direction: row; gap: 10px;">
      <text id="label-dose" style="color: #4a5568; font-size: 14px;">Target Dose: 0.500 mcg/kg/min</text>
    </div>
    <div id="actions" style="display: flex; flex-direction: row; gap: 10px; margin: 10px 0 0 0; justify-content: center;">
      <button id="btn-calc" style="width: 360px; background: linear-gradient(#2c78c4, #2662a8); color: #ffffff; padding: 8px 16px; border-radius: 6px; min-height: 44px; justify-content: center; align-items: center; box-shadow: 0 2px 4px #2c528240;">
        <text style="color: #ffffff; font-size: 14px; font-weight: bold;">Submit calculation</text>
      </button>
    </div>
  </card>

  <card id="results-card" style="display: flex; flex-direction: column; background-color: #ffffff; padding: 16px; border-width: 1px; border-color: #e2e8f0; border-radius: 12px; gap: 8px; box-shadow: 0 2px 10px #0f172a1f;">
    <text id="results-heading" style="color: #2d3748; font-size: 16px; font-weight: bold;">Backend Calculation Output</text>
    <text id="output-rate" style="color: #2b6cb0; font-size: 18px; font-weight: bold;">Rate: Awaiting Backend Calculation...</text>
    <text id="output-status" style="color: #718096; font-size: 13px;">Safety Status: Idle</text>
    <text id="output-signature" style="color: #718096; font-size: 12px;">Backend MAC: None</text>
  </card>
</screen>"#;

mod cache;
mod focus_ring;
#[cfg(test)]
mod repaint_tests;
mod theme;
use crate::{FormState, FrontendApp};
use iris::render::RenderBackend;
use metis_core::{ErrorCode, MetisError, Result};
use metis_ipc::{IpcTransport, client::HandshakeError};
use metis_platform::{Damage, Framebuffer};
use metis_ui_lang::{
    Color, DisplayCommand, DisplayList, DomDocument, Edit, LayoutViewport, MAX_SEMANTIC_TEXT_BYTES,
    compute_layout,
};
use std::borrow::Cow;

pub(crate) use cache::RenderCache;

/// Status badge color while a backend session is open.
///
/// Legible at 4.5:1 or better over either theme's header gradient (WCAG 2.2
/// criterion 1.4.3), as the theme contrast tests assert.
pub const BADGE_READY: Color = Color::rgb(154, 230, 180);
/// Status badge color once the backend session has closed, legible over
/// either theme's header like [`BADGE_READY`].
pub const BADGE_CLOSED: Color = Color::rgb(254, 178, 178);
/// Surface color behind the authored form.
const BACKDROP: Color = Color::rgb(240, 244, 248);
/// Glyphs of the patient reference a label shows before it states the
/// omission with an ellipsis; forty glyphs plus the label fit the 800-pixel
/// form.
const PATIENT_PREVIEW_GLYPHS: usize = 40;
/// Glyphs of the uncommitted composition a label shows before the ellipsis.
const COMPOSITION_PREVIEW_GLYPHS: usize = 24;

impl<T: IpcTransport> FrontendApp<T> {
    /// Projects the owned state and renders the complete form.
    ///
    /// The labels are written through reused buffers and replace the held
    /// text only when it differs.
    /// # Errors
    /// Rejects invalid layout or a missing authored label before changing pixels.
    pub fn render(&mut self) -> Result<()> {
        self.apply_theme()?;
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
        set_text(&mut self.doc, "status-badge", badge)?;
        self.render_command_surface()?;
        let badge_color = if self.client.is_none() {
            BADGE_CLOSED
        } else {
            BADGE_READY
        };
        let status = self
            .doc
            .find_element_by_id_mut("status-badge")
            .ok_or_else(|| {
                MetisError::ui(
                    ErrorCode::MalformedMarkup,
                    "Authored form is missing status badge",
                )
            })?;
        status.computed_style.text_color = badge_color;
        self.render_form_text()?;
        // Validate the custom renderer's host-neutral semantics before
        // painting so a malformed identity or action cannot be presented as
        // an accessible control; the same projection decides focus.
        let semantics = self.semantic_tree()?;
        self.reconcile_focus(&semantics)?;
        let width = i32::try_from(self.framebuffer.width()).map_err(|_| layout_error())?;
        let height = i32::try_from(self.framebuffer.height()).map_err(|_| layout_error())?;
        let mut display = compute_layout(
            &self.doc,
            LayoutViewport::with_scale(width, height, self.display_scale),
        )?;
        self.append_focus_ring(&mut display)?;
        // Repaint only what changed since the painted frame: a keystroke
        // changes one field, not the form.
        let surface = metis_ui_lang::Rect::new(0, 0, width, height);
        let damage = self.painted.as_ref().map_or(Damage::Full, |painted| {
            display.damage_since(painted, surface)
        });
        let repaint = |framebuffer: &mut Framebuffer| paint(framebuffer, &display);
        match damage {
            Damage::Unchanged => {}
            Damage::Region(region) => self.framebuffer.render_clipped(region, repaint),
            Damage::Full => repaint(&mut self.framebuffer),
        }
        self.unpresented = self.unpresented.merge(damage);
        self.painted = Some(display);
        Ok(())
    }

    /// Writes the labels that show the inputs and the outcome, each through
    /// a reused buffer so text that did not change requests no memory.
    fn render_form_text(&mut self) -> Result<()> {
        let text = &mut self.cache.text;
        let doc = &mut self.doc;
        let inputs = &self.inputs;
        text.clear();
        text.push_str("Patient ID: ");
        push_preview(text, &inputs.patient_id, PATIENT_PREVIEW_GLYPHS);
        // The composition is uncommitted native input; the wire request
        // uses the committed identifier alone.
        if let Some(composition) = self.composition.as_deref() {
            text.push_str(" [");
            push_preview(text, composition, COMPOSITION_PREVIEW_GLYPHS);
            text.push(']');
        }
        set_text(doc, "label-patient", text)?;
        set_attribute(
            doc,
            "label-patient",
            "value",
            &bounded_accessible_value(&inputs.patient_id),
        )?;
        set_text(
            doc,
            "label-weight",
            &format!("Weight: {} kg", input_number(inputs.weight_kg, 2)),
        )?;
        set_text(
            doc,
            "label-conc",
            &format!(
                "Drug Concentration: {} mg/mL",
                input_number(inputs.concentration_mg_ml, 2)
            ),
        )?;
        set_text(
            doc,
            "label-dose",
            &format!(
                "Target Dose: {} mcg/kg/min",
                input_number(inputs.target_dose_mcg_kg_min, 3)
            ),
        )?;
        let (rate, status, signature) = outcome_text(&self.state);
        set_text(doc, "output-rate", &rate)?;
        set_text(doc, "output-status", &status)?;
        set_text(doc, "output-signature", signature)
    }

    fn render_command_surface(&mut self) -> Result<()> {
        let menu_open = self.command_menu_open();
        let doc = &mut self.doc;
        set_attribute(
            doc,
            "command-menu-toggle",
            "aria-expanded",
            bool_text(menu_open),
        )?;
        set_attribute(doc, "command-menu", "aria-hidden", bool_text(!menu_open))?;
        let display = if menu_open {
            metis_ui_lang::Display::Flex
        } else {
            metis_ui_lang::Display::None
        };
        let menu = doc.find_element_by_id_mut("command-menu").ok_or_else(|| {
            MetisError::ui(
                ErrorCode::MalformedMarkup,
                "Authored form is missing command menu",
            )
        })?;
        menu.computed_style.display = display;
        set_text(doc, "command-status", &self.command_status)
    }
}

fn outcome_text(state: &FormState) -> (String, String, &'static str) {
    match state {
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

/// Reports an edit that found no element as a malformed authored form.
fn found(edit: Edit, missing: impl FnOnce() -> String) -> Result<()> {
    if edit == Edit::Missing {
        Err(MetisError::ui(ErrorCode::MalformedMarkup, missing()))
    } else {
        Ok(())
    }
}

fn set_text(doc: &mut DomDocument, id: &str, text: &str) -> Result<()> {
    found(doc.set_text_content(id, text), || {
        format!("Authored form is missing label {id}")
    })
}

fn set_attribute(doc: &mut DomDocument, id: &str, key: &str, value: &str) -> Result<()> {
    found(doc.set_attribute(id, key, value), || {
        format!("Authored form is missing semantic field {id}")
    })
}

fn bool_text(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// Appends the first `glyphs` characters of `value`, and an ellipsis when
/// more follow, so the omission is stated explicitly.
fn push_preview(out: &mut String, value: &str, glyphs: usize) {
    match value.char_indices().nth(glyphs) {
        Some((end, _)) => {
            out.push_str(&value[..end]);
            out.push_str("...");
        }
        None => out.push_str(value),
    }
}

/// Paints `display` over the writable region, whatever the region held.
///
/// The backdrop clear is skipped when [`clear_is_redundant`] proves the
/// display list overwrites every pixel the clear would write.
fn paint(framebuffer: &mut Framebuffer, display: &DisplayList) {
    if !clear_is_redundant(framebuffer, display) {
        framebuffer.clear(BACKDROP);
    }
    framebuffer
        .render(display)
        .unwrap_or_else(|never| match never {});
}

/// Reports whether `display` holds an opaque, square-cornered rectangle fill
/// containing the whole writable region.
///
/// Such a fill replaces every writable pixel outright, so what those pixels
/// held before it, the backdrop clear included, cannot reach the result.
///
/// Commands are read in painter order. `DisplayCommand` is non-exhaustive
/// across crates, so a variant this function has not been taught ends the
/// scan with `false`: it may change how a later fill lands, and the clear then
/// runs. Every known variant is named, so a change to the list of them is a
/// deliberate edit here.
fn clear_is_redundant(framebuffer: &Framebuffer, display: &DisplayList) -> bool {
    for command in &display.commands {
        match command {
            DisplayCommand::FillRect {
                rect,
                radius,
                color,
            } if color.is_opaque() && radius.is_square() && framebuffer.clip_within(*rect) => {
                return true;
            }
            DisplayCommand::FillRect { .. }
            | DisplayCommand::ElementRect { .. }
            | DisplayCommand::DrawShadow { .. }
            | DisplayCommand::FillGradient { .. }
            | DisplayCommand::DrawBorder { .. }
            | DisplayCommand::DrawLine { .. }
            | DisplayCommand::DrawPolyline { .. }
            | DisplayCommand::DrawText { .. }
            | DisplayCommand::DrawImage { .. } => {}
            _ => return false,
        }
    }
    false
}

fn layout_error() -> MetisError {
    MetisError::ui(
        ErrorCode::LayoutOverflow,
        "Surface exceeds layout coordinates",
    )
}

fn bounded_accessible_value(value: &str) -> Cow<'_, str> {
    const ELLIPSIS: &str = "...";
    if value.len() <= MAX_SEMANTIC_TEXT_BYTES {
        return Cow::Borrowed(value);
    }
    let limit = MAX_SEMANTIC_TEXT_BYTES - ELLIPSIS.len();
    let mut end = 0;
    for (index, character) in value.char_indices() {
        let next = index + character.len_utf8();
        if next > limit {
            break;
        }
        end = next;
    }
    let mut bounded = String::with_capacity(MAX_SEMANTIC_TEXT_BYTES);
    bounded.push_str(&value[..end]);
    bounded.push_str(ELLIPSIS);
    Cow::Owned(bounded)
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
mod presentation_tests;
