//! Bounded edits of the authored patient identifier.
//!
//! Every edit copies the current value, validates the candidate and commits it
//! through [`FrontendApp::set_inputs`], so a rejected or unrenderable edit
//! leaves the document, the form inputs and the painted frame unchanged.

use metis_core::error::{ErrorCode, MetisError, Result};
use metis_frontend::FrontendApp;
use metis_ipc::IpcTransport;

/// Maximum UTF-8 bytes a native patient identifier may hold.
pub(super) const MAX_PATIENT_ID_BYTES: usize = 128;

pub(super) fn append_patient_character<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    patient_id: &mut String,
    character: char,
) -> Result<bool> {
    let mut encoded = [0; 4];
    let text = character.encode_utf8(&mut encoded);
    append_patient_text(app, patient_id, text)
}

pub(super) fn append_patient_text<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    patient_id: &mut String,
    text: &str,
) -> Result<bool> {
    if text.chars().any(char::is_control) {
        return Ok(false);
    }
    // The bounded copy lets set_inputs commit the edit only after validation
    // and rendering succeed; native text input is a cold control-plane path.
    let mut updated = patient_id.clone();
    let next_bytes = updated
        .len()
        .checked_add(text.len())
        .ok_or_else(input_limit_error)?;
    if next_bytes > MAX_PATIENT_ID_BYTES {
        return Err(input_limit_error());
    }
    updated.try_reserve(text.len()).map_err(|_| {
        MetisError::ui(
            ErrorCode::SurfaceAllocationError,
            "Patient identifier storage reservation failed",
        )
    })?;
    updated.push_str(text);
    replace_patient_id(app, patient_id, updated)
}

pub(super) fn remove_patient_character<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    patient_id: &mut String,
) -> Result<bool> {
    let mut updated = patient_id.clone();
    if updated.pop().is_none() {
        return Ok(false);
    }
    replace_patient_id(app, patient_id, updated)
}

pub(super) fn set_patient_value<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    patient_id: &mut String,
    value: &str,
) -> Result<bool> {
    validate_patient_value(value)?;
    replace_patient_id(app, patient_id, value.to_owned())
}

fn replace_patient_id<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    patient_id: &mut String,
    updated: String,
) -> Result<bool> {
    validate_patient_value(&updated)?;
    if *patient_id == updated {
        return Ok(false);
    }
    let weight = app.inputs().weight_kg;
    let concentration = app.inputs().concentration_mg_ml;
    let dose = app.inputs().target_dose_mcg_kg_min;
    app.set_inputs(&updated, weight, concentration, dose)?;
    *patient_id = updated;
    Ok(true)
}

fn validate_patient_value(value: &str) -> Result<()> {
    if value.len() > MAX_PATIENT_ID_BYTES {
        return Err(input_limit_error());
    }
    if value.chars().any(char::is_control) {
        return Err(MetisError::protocol(
            ErrorCode::MalformedPayload,
            "Patient identifier contains a control character",
        ));
    }
    Ok(())
}

pub(super) fn input_limit_error() -> MetisError {
    MetisError::protocol(
        ErrorCode::PayloadTooLarge,
        "Patient identifier exceeds the bounded native input limit",
    )
}
