//! Form markup and projection of owned application state.

mod cache;
#[cfg(test)]
mod cache_tests;
mod focus_ring;
#[cfg(not(target_arch = "wasm32"))]
mod mark;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod mark_tests;
mod markup;
#[cfg(test)]
mod repaint_tests;
mod theme;
use crate::document::TrackedDocument;
use crate::{FormState, FrontendApp};
use iris::render::RenderBackend;
use metis_core::{ErrorCode, MetisError, Result};
use metis_ipc::{IpcTransport, client::HandshakeError};
use metis_platform::{Damage, Framebuffer};
use metis_ui_lang::{
    Color, DisplayCommand, DisplayList, Edit, LayoutViewport, MAX_SEMANTIC_TEXT_BYTES,
};
use std::borrow::Cow;
use std::fmt::{self, Write};

pub(crate) use cache::RenderCache;
#[cfg(not(target_arch = "wasm32"))]
pub use mark::{APP_MARK_ID, append_app_mark};
pub use markup::CLINICAL_SCREEN_XML;

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
    /// A render of unchanged state keeps the document, its semantic
    /// projection and its layout; the only memory it requests is the new
    /// display list's command storage.
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
        found(
            self.doc
                .restyle("status-badge", |style| style.text_color = badge_color),
            || "Authored form is missing status badge".to_owned(),
        )?;
        self.render_form_text()?;
        // Validate the custom renderer's host-neutral semantics before
        // painting so a malformed identity or action cannot be presented as
        // an accessible control; the same projection decides focus.
        let order = self.cache.focusable(&self.doc)?;
        self.focus.reconcile(&self.doc, order)?;
        let width = i32::try_from(self.framebuffer.width()).map_err(|_| layout_error())?;
        let height = i32::try_from(self.framebuffer.height()).map_err(|_| layout_error())?;
        let mut display = self.cache.frame(
            &self.doc,
            LayoutViewport::with_scale(width, height, self.display_scale),
        )?;
        // The mark fills its authored anchor box; the focus ring stays the
        // last overlay so nothing covers it.
        #[cfg(not(target_arch = "wasm32"))]
        mark::append_cached_app_mark(&mut self.cache, &mut display)?;
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
        let [text, detail] = &mut self.cache.text;
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
        for (id, label, value, decimals, unit) in [
            ("label-weight", "Weight", inputs.weight_kg, 2, "kg"),
            (
                "label-conc",
                "Drug Concentration",
                inputs.concentration_mg_ml,
                2,
                "mg/mL",
            ),
            (
                "label-dose",
                "Target Dose",
                inputs.target_dose_mcg_kg_min,
                3,
                "mcg/kg/min",
            ),
        ] {
            text.clear();
            text.push_str(label);
            text.push_str(": ");
            written(write_input_number(text, value, decimals))?;
            text.push(' ');
            text.push_str(unit);
            set_text(doc, id, text)?;
        }
        text.clear();
        detail.clear();
        let signature = written(write_outcome(&self.state, text, detail))?;
        set_text(doc, "output-rate", text)?;
        set_text(doc, "output-status", detail)?;
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
        found(
            doc.restyle("command-menu", |style| style.display = display),
            || "Authored form is missing command menu".to_owned(),
        )?;
        set_text(doc, "command-status", &self.command_status)
    }
}

/// Writes the rate and the safety status for `state` and returns the
/// signature line, which is fixed text.
fn write_outcome(
    state: &FormState,
    rate: &mut String,
    status: &mut String,
) -> std::result::Result<&'static str, fmt::Error> {
    const AWAITING: &str = "Rate: Awaiting Backend Calculation...";
    const NO_RESULT: &str = "Rate: No result";
    const NO_SIGNATURE: &str = "Backend MAC: No result";
    match state {
        FormState::Idle => {
            rate.push_str(AWAITING);
            status.push_str("Safety Status: Idle");
            Ok("Backend MAC: None")
        }
        FormState::Pending => {
            rate.push_str(AWAITING);
            status.push_str("Request in progress");
            Ok("Backend MAC: None")
        }
        FormState::Success(response) => {
            rate.push_str("Rate: ");
            write_result_number(rate, response.rate_ml_hr, 3)?;
            rate.push_str(" mL/hr (");
            write_result_number(rate, response.drug_rate_mg_hr, 2)?;
            rate.push_str(" mg/hr)");
            write!(
                status,
                "Backend response (Audit Seq #{}){}",
                response.audit_sequence_id,
                if response.is_pediatric {
                    " [PEDIATRIC]"
                } else {
                    ""
                }
            )?;
            Ok("Backend MAC: Present (not verified by frontend)")
        }
        FormState::Rejected(error) => {
            rate.push_str(NO_RESULT);
            write!(
                status,
                "Backend rejected request [0x{:04X}]",
                error.error_code
            )?;
            Ok(NO_SIGNATURE)
        }
        FormState::Failed(error) => {
            rate.push_str(NO_RESULT);
            write!(status, "Request not sent [0x{:04X}]", error.code as u16)?;
            Ok(NO_SIGNATURE)
        }
        FormState::Disconnected(error) => {
            rate.push_str(NO_RESULT);
            write!(
                status,
                "Connection failed [0x{:04X}] - reconnect",
                error.code as u16
            )?;
            Ok(NO_SIGNATURE)
        }
        FormState::SessionFailed(error) => {
            rate.push_str(NO_RESULT);
            status.push_str("Session failed [");
            match error {
                HandshakeError::Local(error) => write!(status, "0x{:04X}", error.code as u16)?,
                HandshakeError::Remote(error) => write!(status, "0x{:04X}", error.error_code)?,
                _ => status.push_str("unrecognized"),
            }
            status.push_str("] - reconnect");
            Ok(NO_SIGNATURE)
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

fn set_text(doc: &mut TrackedDocument, id: &str, text: &str) -> Result<()> {
    found(doc.set_text(id, text), || {
        format!("Authored form is missing label {id}")
    })
}

fn set_attribute(doc: &mut TrackedDocument, id: &str, key: &str, value: &str) -> Result<()> {
    found(doc.set_attribute(id, key, value), || {
        format!("Authored form is missing semantic field {id}")
    })
}

fn bool_text(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// Converts a formatting failure into the form's error.
///
/// Writing into a `String` fails only when a `Display` implementation
/// reports failure, which the integers and floats the form writes never do.
fn written<V>(result: std::result::Result<V, fmt::Error>) -> Result<V> {
    result.map_err(|_| {
        MetisError::ui(
            ErrorCode::MalformedMarkup,
            "Authored form text could not be formatted",
        )
    })
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

/// Appends `value` rounded to `decimals` places.
fn write_result_number(out: &mut String, value: f64, decimals: usize) -> fmt::Result {
    let start = out.len();
    write!(out, "{value:.decimals$}")?;
    let rounded = &out[start..];
    // Scientific notation keeps a nonzero mantissa when fixed decimal rounding
    // would erase every significant digit, with the same displayed precision.
    if rounded.len() > NUMBER_GLYPHS
        || value.is_subnormal()
        || (value.is_normal() && rounded.chars().all(|c| matches!(c, '0' | '.' | '-')))
    {
        out.truncate(start);
        write!(out, "{value:.decimals$e}")?;
    }
    Ok(())
}

/// Appends `value` in shortest notation with at least `minimum_decimals`
/// decimal places.
fn write_input_number(out: &mut String, value: f64, minimum_decimals: usize) -> fmt::Result {
    // Shortest scientific f64 notation fits 24 glyphs: sign, 17 significant
    // digits, decimal point, exponent marker/sign and three exponent digits.
    // Preserve those digits instead of rounding accepted small inputs to zero.
    let start = out.len();
    write!(out, "{value}")?;
    if out.len() - start > NUMBER_GLYPHS {
        out.truncate(start);
        return write!(out, "{value:e}");
    }
    if value.is_finite() {
        let decimals = out[start..]
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len());
        if decimals < minimum_decimals {
            if decimals == 0 {
                out.push('.');
            }
            for _ in decimals..minimum_decimals {
                out.push('0');
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod presentation_tests;
