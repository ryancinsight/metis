use super::BrowserState;
use metis_core::error::{ErrorCode, MetisError};
use metis_frontend::FormState;

pub(super) fn result_state_update(
    state: &mut BrowserState,
    patient_id: &str,
    result: &metis_core::error::Result<()>,
    event_error: Option<&MetisError>,
) -> metis_core::error::Result<()> {
    if let Err(error) = result {
        state.result_explorer.fail(error.clone());
        return Ok(());
    }
    if let Some(error) = event_error {
        state.result_explorer.fail(error.clone());
        return Ok(());
    }
    match &state.state {
        FormState::Success(response) => state
            .result_explorer
            .record_response(patient_id, response)
            .map(|_| ()),
        FormState::Rejected(error) => {
            state.result_explorer.fail(MetisError::clinical(
                ErrorCode::ClinicalInterlockBlocked,
                format!(
                    "Backend rejected explorer response [0x{:04X}]",
                    error.error_code
                ),
            ));
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) fn update_result_explorer(
    state: &mut BrowserState,
    patient_id: &str,
    result: &metis_core::error::Result<()>,
    event_error: Option<&MetisError>,
) {
    if let Err(error) = result_state_update(state, patient_id, result, event_error) {
        state.result_explorer.fail(error);
    }
}
