//! Admitted shared page submissions reach the actual supervised frontend.

use super::MAX_PATIENT_ID_BYTES;
use metis_core::error::{ErrorCode, MetisError};
use metis_frontend::{FormInputs, FormState, FrontendApp};
use metis_ipc::IpcTransport;
use metis_web::embedded::{PageCommand, PageIdentity, SequenceId};

pub(super) fn error(
    identity: PageIdentity,
    request: Option<SequenceId>,
    status: &str,
    code: ErrorCode,
    message: impl Into<String>,
) -> PageCommand {
    PageCommand::Error {
        identity,
        request,
        status: status.into(),
        error_code: code.to_wire(),
        message: message.into(),
    }
}

fn failure(
    identity: PageIdentity,
    request: SequenceId,
    status: &str,
    failure: MetisError,
) -> PageCommand {
    error(
        identity,
        Some(request),
        status,
        failure.code,
        failure.message,
    )
}

pub(super) fn submit<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    identity: PageIdentity,
    request: SequenceId,
    inputs: &FormInputs,
) -> PageCommand {
    if inputs.patient_id.is_empty()
        || inputs.patient_id.len() > MAX_PATIENT_ID_BYTES
        || inputs.patient_id.chars().any(char::is_control)
    {
        return error(
            identity,
            Some(request),
            "invalid_request",
            ErrorCode::PayloadTooLarge,
            "Patient reference is empty, oversized or contains controls",
        );
    }
    if ![
        inputs.weight_kg,
        inputs.concentration_mg_ml,
        inputs.target_dose_mcg_kg_min,
    ]
    .into_iter()
    .all(f64::is_finite)
    {
        return error(
            identity,
            Some(request),
            "invalid_request",
            ErrorCode::NumericInstability,
            "Numeric inputs must be finite",
        );
    }
    if let Err(cause) = app.set_inputs(
        &inputs.patient_id,
        inputs.weight_kg,
        inputs.concentration_mg_ml,
        inputs.target_dose_mcg_kg_min,
    ) {
        return failure(identity, request, "failed", cause);
    }
    // Preserve the existing synchronous supervised IPC execution regime.
    if let Err(cause) = app.submit_calculation() {
        return failure(identity, request, "transport_failed", cause);
    }
    match app.state() {
        FormState::Success(response) => PageCommand::Result {
            identity,
            request,
            response: response.clone(),
        },
        FormState::Rejected(cause) => PageCommand::Error {
            identity,
            request: Some(request),
            status: "rejected".into(),
            error_code: cause.error_code,
            message: cause.message.clone(),
        },
        FormState::Failed(cause) | FormState::Disconnected(cause) => {
            failure(identity, request, "failed", cause.clone())
        }
        _ => error(
            identity,
            Some(request),
            "failed",
            ErrorCode::TransportBroken,
            "The calculation did not reach a terminal state",
        ),
    }
}
