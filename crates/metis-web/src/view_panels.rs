//! Cached-write renderers for the result, drop, text, and clipboard panels.

use super::BrowserState;
use super::view::{set_attribute, set_text};
use super::view_explorer::render_explorer;
use crate::controls::{DisplayUnit, ResultDetail};
use crate::text_policy::CompositionState;
use metis_frontend::FormState;
use moirai_pal::wasm::WebDocument;
use std::io;

pub(super) fn render_result(
    document: &WebDocument,
    state: &mut BrowserState,
    message: &str,
) -> io::Result<()> {
    let BrowserState {
        state: form_state,
        controls,
        dom_cache,
        ..
    } = &mut *state;
    let metrics = match form_state {
        FormState::Success(response) => match controls.display_unit() {
            DisplayUnit::Volume => format!("Volume rate: {:.6} mL/hr", response.rate_ml_hr),
            DisplayUnit::DrugMass => {
                format!("Drug mass rate: {:.6} mg/hr", response.drug_rate_mg_hr)
            }
        },
        _ => match controls.display_unit() {
            DisplayUnit::Volume => "Volume rate: unavailable",
            DisplayUnit::DrugMass => "Drug mass rate: unavailable",
        }
        .to_owned(),
    };
    set_text(dom_cache, document, "result-metrics", &metrics)?;
    let detail = match (form_state, controls.result_detail()) {
        (FormState::Success(response), ResultDetail::Summary) => {
            format!(
                "Clinical summary for response {}",
                response.audit_sequence_id
            )
        }
        (FormState::Success(response), ResultDetail::Audit) => {
            format!("Audit detail: sequence {}", response.audit_sequence_id)
        }
        (_, ResultDetail::Summary) => "Clinical summary awaiting backend response".to_owned(),
        (_, ResultDetail::Audit) => "Audit detail unavailable until backend response".to_owned(),
    };
    set_text(dom_cache, document, "result-detail", &detail)?;
    set_attribute(
        dom_cache,
        document,
        "view-options",
        "data-result-scale-percent",
        &controls.scale().value().to_string(),
    )?;
    set_text(dom_cache, document, "result-state", message)?;
    render_explorer(document, state)
}

pub(super) fn render_drop(document: &WebDocument, state: &mut BrowserState) -> io::Result<()> {
    let BrowserState {
        drop_state,
        drop_read_state,
        dom_cache,
        ..
    } = &mut *state;
    let drop_status = drop_state.status_message();
    let drop_byte_status = drop_read_state.status_message();
    let drop_zone_state = drop_state.state_name();
    let drop_count = drop_state.file_count();
    let drop_byte_state = drop_read_state.state_name();
    set_text(dom_cache, document, "drop-status", &drop_status)?;
    set_text(dom_cache, document, "drop-byte-status", &drop_byte_status)?;
    set_attribute(
        dom_cache,
        document,
        "drop-zone",
        "data-drop-state",
        drop_zone_state,
    )?;
    set_attribute(
        dom_cache,
        document,
        "drop-zone",
        "data-drop-count",
        &drop_count.to_string(),
    )?;
    set_attribute(
        dom_cache,
        document,
        "drop-zone",
        "data-byte-state",
        drop_byte_state,
    )
}

pub(super) fn render_text(document: &WebDocument, state: &mut BrowserState) -> io::Result<()> {
    let BrowserState {
        text_state: text,
        dom_cache,
        ..
    } = &mut *state;
    let text_status = text.text_status();
    let preview = format!("Text value preview: {}", text.value_display());
    let composition_status = text.composition_status();
    let selection_status = text.selection_status();
    let selection = text.selection();
    let text_state_name = text.state_name();
    let selection_start = selection.start();
    let selection_end = selection.end();
    let selection_direction = selection.direction().label();
    let composing = if matches!(text.composition(), CompositionState::Active) {
        "true"
    } else {
        "false"
    };
    let input_type = text.input_type().to_owned();
    set_text(dom_cache, document, "text-status", &text_status)?;
    set_text(dom_cache, document, "text-preview", &preview)?;
    set_text(
        dom_cache,
        document,
        "composition-status",
        &composition_status,
    )?;
    set_text(dom_cache, document, "selection-status", &selection_status)?;
    set_attribute(
        dom_cache,
        document,
        "text-specimen",
        "data-text-state",
        text_state_name,
    )?;
    set_attribute(
        dom_cache,
        document,
        "text-specimen",
        "data-selection-start",
        &selection_start.to_string(),
    )?;
    set_attribute(
        dom_cache,
        document,
        "text-specimen",
        "data-selection-end",
        &selection_end.to_string(),
    )?;
    set_attribute(
        dom_cache,
        document,
        "text-specimen",
        "data-selection-direction",
        selection_direction,
    )?;
    set_attribute(
        dom_cache,
        document,
        "text-specimen",
        "data-composing",
        composing,
    )?;
    set_attribute(
        dom_cache,
        document,
        "text-specimen",
        "data-input-type",
        &input_type,
    )
}

pub(super) fn render_clipboard(document: &WebDocument, state: &mut BrowserState) -> io::Result<()> {
    let BrowserState {
        clipboard_status: status,
        dom_cache,
        ..
    } = &mut *state;
    let message = status.message();
    let clipboard_state = status.state_name();
    set_text(dom_cache, document, "clipboard-status", &message)?;
    set_attribute(
        dom_cache,
        document,
        "clipboard-status",
        "data-clipboard-state",
        clipboard_state,
    )
}
