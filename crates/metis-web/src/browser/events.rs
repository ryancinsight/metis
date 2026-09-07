use metis_core::error::{ErrorCode, MetisError};
use metis_core::protocol::ClinicalCalcResponsePayload;
use metis_frontend::AsyncFrontendApp;
use metis_ipc::BrowserWebSocketTransport;

pub(super) async fn receive_result_event(
    app: &mut AsyncFrontendApp<BrowserWebSocketTransport>,
    expected: ClinicalCalcResponsePayload,
) -> metis_core::error::Result<Option<String>> {
    let event = app.recv_event().await?;
    let received = event.decode_as::<ClinicalCalcResponsePayload>()?;
    if event.event_id().get() != expected.audit_sequence_id {
        return Err(MetisError::protocol(
            ErrorCode::SequenceMismatch,
            "Remote event identifier differs from its correlated response",
        ));
    }
    if received != expected {
        return Err(MetisError::protocol(
            ErrorCode::SequenceMismatch,
            "Remote event result differs from its correlated response",
        ));
    }
    Ok(Some(format!(
        "Remote event: {} #{} (audit={} rate={:.6} ml/hr drug={:.6} mg/hr)",
        event.name(),
        event.event_id().get(),
        received.audit_sequence_id,
        received.rate_ml_hr,
        received.drug_rate_mg_hr,
    )))
}
