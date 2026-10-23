//! Form text rendering for input labels and outcome display.

use crate::document::TrackedDocument;
use crate::{FrontendApp, FormInputs};
use metis_core::{ErrorCode, MetisError, Result};
use metis_ui_lang::MAX_SEMANTIC_TEXT_BYTES;
use std::borrow::Cow;
use std::fmt;

use crate::presentation::format::write_input_number;
use crate::presentation::outcome::write_outcome;

const PATIENT_PREVIEW_GLYPHS: usize = 40;
const COMPOSITION_PREVIEW_GLYPHS: usize = 24;

/// Sets the text content of an element in the authored form.
/// Returns an error if the element is missing.
pub(crate) fn set_text(doc: &mut TrackedDocument, id: &str, text: &str) -> Result<()> {
    found(doc.set_text(id, text), || {
        format!("Authored form is missing label {id}")
    })
}

/// Sets an attribute on an element in the authored form.
/// Returns an error if the element is missing.
pub(crate) fn set_attribute(doc: &mut TrackedDocument, id: &str, key: &str, value: &str) -> Result<()> {
    found(doc.set_attribute(id, key, value), || {
        format!("Authored form is missing semantic field {id}")
    })
}

/// Reports an edit that found no element as a malformed authored form.
pub(crate) fn found(edit: metis_ui_lang::Edit, missing: impl FnOnce() -> String) -> Result<()> {
    if edit == metis_ui_lang::Edit::Missing {
        Err(MetisError::ui(ErrorCode::MalformedMarkup, missing()))
    } else {
        Ok(())
    }
}

/// Converts a formatting failure into the form's error.
pub(crate) fn written(result: fmt::Result) -> Result<()> {
    result.map_err(|_| MetisError::ui(
        ErrorCode::MalformedMarkup,
        "Authored form text could not be formatted",
    ))
}

/// Converts a write_outcome result into the form's error.
pub(crate) fn written_outcome(result: Result<&'static str, fmt::Error>) -> Result<&'static str> {
    result.map_err(|_| MetisError::ui(
        ErrorCode::MalformedMarkup,
        "Authored form text could not be formatted",
    ))
}

/// Appends the first `glyphs` characters of `value`, and an ellipsis when
/// more follow, so the omission is stated explicitly.
pub(crate) fn push_preview(out: &mut String, value: &str, glyphs: usize) {
    match value.char_indices().nth(glyphs) {
        Some((end, _)) => {
            out.push_str(&value[..end]);
            out.push_str("...");
        }
        None => out.push_str(value),
    }
}

/// Returns a bounded accessible value for ARIA attributes.
pub(crate) fn bounded_accessible_value(value: &str) -> Cow<'_, str> {
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

impl<T: metis_ipc::IpcTransport> FrontendApp<T> {
    /// Writes the labels that show the inputs and the outcome, each through
    /// a reused buffer so text that did not change requests no memory.
    pub(crate) fn render_form_text(&mut self) -> Result<()> {
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
        let signature = written_outcome(write_outcome(&self.state, text, detail))?;
        set_text(doc, "output-rate", text)?;
        set_text(doc, "output-status", detail)?;
        set_text(doc, "output-signature", signature)
    }
}