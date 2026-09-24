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
      <button id="command-focus-patient" style="width: 144px; background: linear-gradient(#2c78c4, #2662a8); color: #ffffff; padding: 8px 12px; border-radius: 6px; min-height: 44px; justify-content: center; align-items: center; box-shadow: 0 2px 4px #2c528240;">
        <text style="color: #ffffff; font-size: 14px; font-weight: bold;">Focus patient</text>
      </button>
    </div>
    <div id="command-menu" popover-anchor="command-menu-toggle" role="menu" aria-label="Application commands" aria-hidden="true" style="display: none; flex-direction: column; gap: 6px; background-color: #ffffff; padding: 8px; border-width: 1px; border-color: #e2e8f0; border-radius: 10px; box-shadow: 0 8px 20px #0f172a33;">
      <button id="command-theme-dark" role="menuitem" style="background: linear-gradient(#2c78c4, #2662a8); color: #ffffff; padding: 8px 12px; border-radius: 6px; min-height: 44px; justify-content: center; align-items: center; box-shadow: 0 2px 4px #2c528240;">
        <text style="color: #ffffff; font-size: 14px; font-weight: bold;">Dark theme</text>
      </button>
      <button id="command-theme-system" role="menuitem" style="background: linear-gradient(#2c78c4, #2662a8); color: #ffffff; padding: 8px 12px; border-radius: 6px; min-height: 44px; justify-content: center; align-items: center; box-shadow: 0 2px 4px #2c528240;">
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

mod focus_ring;
#[cfg(test)]
mod repaint_tests;
mod theme;
use crate::{FormState, FrontendApp};
use iris::render::RenderBackend;
use metis_core::{ErrorCode, MetisError, Result};
use metis_ipc::{IpcTransport, client::HandshakeError};
use metis_platform::Framebuffer;
use metis_ui_lang::{Color, Damage, LayoutViewport, MAX_SEMANTIC_TEXT_BYTES, compute_layout};

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

impl<T: IpcTransport> FrontendApp<T> {
    /// Projects the owned state and renders the complete form.
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
        self.text("status-badge", badge)?;
        self.render_command_surface()?;
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
            BADGE_CLOSED
        } else {
            BADGE_READY
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
        self.attribute(
            "label-patient",
            "value",
            bounded_accessible_value(&self.inputs.patient_id),
        )?;
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
        let repaint = |framebuffer: &mut Framebuffer| {
            framebuffer.clear(BACKDROP);
            framebuffer
                .render(&display)
                .unwrap_or_else(|never| match never {});
        };
        match damage {
            Damage::Unchanged => {}
            Damage::Region(region) => self.framebuffer.render_clipped(region, repaint),
            Damage::Full => repaint(&mut self.framebuffer),
        }
        self.painted = Some(display);
        Ok(())
    }

    fn render_command_surface(&mut self) -> Result<()> {
        let menu_open = self.command_menu_open();
        self.attribute(
            "command-menu-toggle",
            "aria-expanded",
            menu_open.to_string(),
        )?;
        self.attribute("command-menu", "aria-hidden", (!menu_open).to_string())?;
        let menu = self
            .doc
            .find_element_by_id_mut("command-menu")
            .ok_or_else(|| {
                MetisError::ui(
                    ErrorCode::MalformedMarkup,
                    "Authored form is missing command menu",
                )
            })?;
        menu.computed_style.display = if menu_open {
            metis_ui_lang::Display::Flex
        } else {
            metis_ui_lang::Display::None
        };
        self.text("command-status", self.command_status.clone())
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

    fn attribute(&mut self, id: &str, key: &str, value: impl Into<String>) -> Result<()> {
        if self.doc.set_attribute(id, key, value) {
            Ok(())
        } else {
            Err(MetisError::ui(
                ErrorCode::MalformedMarkup,
                format!("Authored form is missing semantic field {id}"),
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

fn bounded_accessible_value(value: &str) -> String {
    const ELLIPSIS: &str = "...";
    if value.len() <= MAX_SEMANTIC_TEXT_BYTES {
        return value.to_owned();
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
    bounded
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
    use super::{BADGE_CLOSED, BADGE_READY, CLINICAL_SCREEN_XML, bounded_accessible_value};
    use metis_platform::{Color, Framebuffer};
    use metis_ui_lang::{
        DisplayCommand, LayoutViewport, MAX_SEMANTIC_TEXT_BYTES, compute_layout, parse_markup,
    };

    #[test]
    fn accessible_patient_value_is_bounded_without_splitting_utf8() {
        let short = "PT-9042-ALPHA";
        assert_eq!(bounded_accessible_value(short), short);
        let exact = "x".repeat(MAX_SEMANTIC_TEXT_BYTES);
        assert_eq!(bounded_accessible_value(&exact), exact);

        let oversized = "é".repeat(MAX_SEMANTIC_TEXT_BYTES);
        let bounded = bounded_accessible_value(&oversized);
        assert!(bounded.len() <= MAX_SEMANTIC_TEXT_BYTES);
        assert!(bounded.ends_with("..."));
        assert!(bounded.is_char_boundary(bounded.len() - 3));
    }

    /// Inked pixels inside `bounds` counted by the candidate ink that best
    /// explains each.
    ///
    /// Antialiased strokes at small sizes may never reach full coverage, so a
    /// single probe pixel is not an oracle, and a nearest-color vote misreads
    /// half-covered edges, whose blend with the background can lie nearer
    /// another candidate than the ink that produced them. Each pixel is instead
    /// fitted as a blend of the background toward each ink: the run's own ink
    /// fits its strokes exactly and collects them, and the others collect
    /// nothing.
    fn ink_counts<const N: usize>(
        framebuffer: &Framebuffer,
        bounds: (i32, i32, i32, i32),
        inks: [Color; N],
    ) -> [usize; N] {
        let (x, y, width, height) = bounds;
        // The header just left of the run is the background the glyphs blend
        // over; sampling it keeps the model true on a gradient header.
        let background = framebuffer.get_pixel(x - 4, y + height / 2);
        let channels = |color: Color| [color.r, color.g, color.b].map(f64::from);
        let base = channels(background);
        let mut counts = [0; N];
        for row in y..y + height {
            for column in x..x + width {
                let pixel = channels(framebuffer.get_pixel(column, row));
                // An antialiased pixel is `background + a * (ink - background)`:
                // project onto each ink's blend line and keep the closest.
                let fits = inks.map(|ink| {
                    let ink = channels(ink);
                    let (mut along, mut length) = (0.0, 0.0);
                    for channel in 0..3 {
                        along += (pixel[channel] - base[channel]) * (ink[channel] - base[channel]);
                        length += (ink[channel] - base[channel]).powi(2);
                    }
                    let coverage = (along / length).clamp(0.0, 1.0);
                    let residual: f64 = (0..3)
                        .map(|channel| {
                            let blend =
                                coverage.mul_add(ink[channel] - base[channel], base[channel]);
                            (pixel[channel] - blend).powi(2)
                        })
                        .sum();
                    (coverage, residual)
                });
                let (index, (coverage, _)) = fits
                    .iter()
                    .enumerate()
                    .min_by(|left, right| left.1.1.total_cmp(&right.1.1))
                    .expect("invariant: at least one ink");
                // Pixels at least half inked count; fainter edges carry too little
                // of the ink to tell candidates apart.
                if *coverage >= 0.5 {
                    counts[index] += 1;
                }
            }
        }
        counts
    }

    /// Rounds a small nonnegative extent up to a whole pixel count.
    fn whole(extent: f64) -> i32 {
        let rounded = extent.ceil();
        assert!((0.0..4096.0).contains(&rounded), "extent {extent}");
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a whole value checked to lie in 0..4096"
        )]
        let pixels = rounded as i32;
        pixels
    }

    #[test]
    fn authored_form_text_and_status_fit_the_viewport() {
        let document = parse_markup(CLINICAL_SCREEN_XML).expect("authored markup");
        let display =
            compute_layout(&document, LayoutViewport::new(800, 600)).expect("authored layout");
        let mut text_runs = Vec::new();
        for command in &display.commands {
            if let DisplayCommand::DrawText { text, x, y, style } = command {
                let width = style.advance(text);
                let height = style.line_height();
                assert!(
                    *x >= 0 && f64::from(*x) + width <= 800.0,
                    "horizontal clipping: {text} x={x} width={width}"
                );
                assert!(
                    *y >= 0 && f64::from(*y) + height <= 600.0,
                    "vertical clipping: {text}"
                );
                text_runs.push((text.as_str(), *x, *y));
            }
        }
        assert!(
            text_runs
                .iter()
                .any(|run| run.0 == "METIS FORM DEMONSTRATION")
        );
        assert!(text_runs.iter().any(|run| run.0 == "SYSTEM READY"));
        assert!(
            text_runs
                .iter()
                .any(|run| run.0 == "Patient Demographics and Drug Prescription")
        );
        let mut framebuffer = Framebuffer::new(800, 600).expect("presentation surface");
        display.render_to(&mut framebuffer);
        // The status run paints in the ready color over the header.
        let status = display
            .commands
            .iter()
            .find_map(|command| match command {
                DisplayCommand::DrawText { text, x, y, style } if text == "SYSTEM READY" => {
                    Some((*x, *y, style.advance(text), style.line_height()))
                }
                _ => None,
            })
            .expect("status run");
        let bounds = (status.0, status.1, whole(status.2), whole(status.3));
        let [ready, closed] = ink_counts(&framebuffer, bounds, [BADGE_READY, BADGE_CLOSED]);
        assert!(ready > 20 && closed == 0, "ready {ready}, closed {closed}");
        assert_eq!(framebuffer.get_pixel(799, 599), Color::rgb(240, 244, 248));
    }
}
