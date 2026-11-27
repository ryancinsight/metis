//! The calculation submit path and typed host-reply rendering over the
//! exact retired wire lines.

use super::bridge;
use super::form::FormElements;
use super::retain;
use crate::logic::{error_line, number_argument, number_text};
use crate::message::PostedMessage;
use serde::Serialize;
use wasm_bindgen::{JsCast, closure::Closure, prelude::*};
use web_sys::{Event, Window};

/// The outbound submit request, typed with the exact host contract.
#[derive(Serialize)]
struct SubmitRequest<'a> {
    /// The only admitted action.
    action: &'static str,
    /// The adjudicated patient reference.
    patient_id: &'a str,
    /// kg.
    weight_kg: f64,
    /// mg/mL.
    concentration_mg_ml: f64,
    /// mcg/kg/min.
    target_dose_mcg_kg_min: f64,
}

/// Connects the host bridge: typed reply rendering and the calculation
/// submit path over the exact retired wire lines.
pub(super) fn wire_form_bridge(window: &Window, elements: &FormElements) {
    let reply_elements = elements.clone();
    let connection = bridge::connect(move |reply| {
        match reply {
            PostedMessage::Result {
                status,
                audit_sequence_id,
                rate_ml_hr,
                drug_rate_mg_hr,
            } if status == "success" => {
                let line = format!(
                    "Rate {} mL/hour; drug {} mg/hour; audit {audit_sequence_id}",
                    number_text(rate_ml_hr),
                    number_text(drug_rate_mg_hr)
                );
                reply_elements.result.set_text_content(Some(&line));
                reply_elements.submit.set_disabled(false);
            }
            PostedMessage::Result { status, .. } => {
                // The host contract only sends success in this envelope;
                // anything else fails closed instead of rendering blanks.
                let line = format!("{status}: the reply carries no calculation result");
                reply_elements.show_error(&line);
            }
            PostedMessage::Error {
                status,
                error_code,
                message: text,
            } => {
                reply_elements.show_error(&error_line(&status, &text, error_code));
            }
            PostedMessage::PermissionDenied { .. } => {}
        }
    });
    let Ok((bridge, listener)) = connection else {
        elements.show_error("WebView2 bridge is unavailable.");
        elements
            .status
            .set_text_content(Some("Host bridge unavailable"));
        return;
    };
    if bridge::own_session(window, listener, None).is_err() {
        elements.show_error("The host bridge listener could not be installed.");
        return;
    }
    elements.status.set_text_content(Some(
        "Host bridge connected; backend authority remains outside the page.",
    ));
    let submit_elements = elements.clone();
    let on_submit = Closure::<dyn FnMut(JsValue)>::new(move |value: JsValue| {
        (|| {
            let Some(event) = value.dyn_ref::<Event>() else {
                return;
            };
            event.prevent_default();
            let Ok(payload) = serde_json::to_string(&SubmitRequest {
                action: "submit",
                patient_id: &submit_elements.patient.value(),
                weight_kg: number_argument(&submit_elements.weight.value()),
                concentration_mg_ml: number_argument(&submit_elements.concentration.value()),
                target_dose_mcg_kg_min: number_argument(&submit_elements.dose.value()),
            }) else {
                return;
            };
            submit_elements.submit.set_disabled(true);
            submit_elements
                .result
                .set_text_content(Some("Submitting to the supervised backend\u{2026}"));
            if let Err(error) = bridge.post_json(&payload) {
                submit_elements.show_error(&format!("Host submission failed: {error}"));
            }
        })();
    });
    let _ignored = elements
        .form
        .add_event_listener_with_callback("submit", on_submit.as_ref().unchecked_ref());
    retain(on_submit);
}
