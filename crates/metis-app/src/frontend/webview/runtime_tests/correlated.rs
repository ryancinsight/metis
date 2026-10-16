//! Genuine OS submission, real authorized backend transaction and hostile replies.

use super::{
    harness::{Failure, Runtime},
    tracing,
};
use crate::frontend::webview::requests;
use metis_backend::{BackendService, audit::AuditEvent, clinical::SafetyEnvelope};
use metis_core::{
    error::{ErrorCode, MetisError},
    protocol::MessageType,
};
use metis_frontend::{FormInputs, FrontendApp};
use metis_ipc::{
    server::{IpcServer, RequestIdentity},
    transport::MemoryTransport,
};
use metis_platform::native::MAX_WEBVIEW_WAIT_MILLISECONDS;
use metis_web::embedded::{PageCommand, PageIdentity, SequenceId};
use std::{io, time::Duration};

#[test]
#[ignore = "explicit Windows gate supplies genuine OS input to installed WebView2"]
fn rust_page_real_correlated_terminal_rejections() -> Result<(), Failure> {
    let _trace = tracing();
    let _test = tracing::info_span!("real_rust_page_correlated_terminal").entered();
    let mut runtime = tracing::info_span!("arrange").in_scope(|| Runtime::new(true))?;
    let identity = runtime.page.session.identity();
    runtime.ready(identity)?;
    let (request, inputs) = tracing::info_span!("act", phase = "actual_submit")
        .in_scope(|| runtime.genuine_submission())?;
    let command = tracing::info_span!("act", phase = "real_backend_transaction")
        .in_scope(|| backend_result(identity, request, &inputs))?;
    let PageCommand::Result { response, .. } = &command else {
        return Err(
            io::Error::other(format!("Actual backend did not calculate: {command:?}")).into(),
        );
    };
    tracing::info_span!("assert", phase = "dyadic_backend_values").in_scope(|| {
        // .5 × 62.5 × 60 / (1.875 × 1000) = 1 exactly in binary;
        // the real two-request service ledger issues calculation audit 2.
        assert_eq!(response.rate_ml_hr.to_bits(), 1.0_f64.to_bits());
        assert_eq!(response.drug_rate_mg_hr.to_bits(), 1.875_f64.to_bits());
        assert!(!response.is_pediatric);
        assert_eq!(response.audit_sequence_id, 2);
    });
    tracing::info_span!("act", phase = "hostile_correlation").in_scope(
        || -> Result<(), Failure> {
            runtime.post(&PageCommand::Result {
                identity,
                request: request.next()?,
                response: response.clone(),
            })?;
            runtime.failed("matching pending request")?;
            let stale = PageIdentity::new(
                identity.role(),
                identity.mount().next()?,
                identity.module_digest(),
            );
            runtime.post(&PageCommand::Result {
                identity: stale,
                request,
                response: response.clone(),
            })?;
            runtime.failed("identity or lifecycle stage")?;
            runtime.post(&command)?;
            runtime.page.session.complete_request(identity, request)?;
            runtime.post(&command)?;
            runtime.failed("matching pending request")?;
            Ok(())
        },
    )?;
    tracing::info_span!("assert", phase = "actual_cleanup").in_scope(|| {
        assert_eq!(runtime.page.session.pending(), None);
        runtime.finish()
    })?;
    tracing::info!(
        event = "fixture_completed",
        fixture = "correlated_terminal_rejections",
        rejections = 3,
        actual_backend_transactions = 1
    );
    Ok(())
}

pub(super) fn backend_result(
    identity: PageIdentity,
    request: SequenceId,
    inputs: &FormInputs,
) -> Result<PageCommand, Failure> {
    let master_key = crate::entropy::session_key()?;
    let mut principal = [0; 16];
    principal[..4].copy_from_slice(&std::process::id().to_be_bytes());
    let (frontend, backend) = MemoryTransport::pair_with_timeout(Duration::from_millis(u64::from(
        MAX_WEBVIEW_WAIT_MILLISECONDS,
    )));
    let dispatcher = tracing::dispatcher::get_default(Clone::clone);
    let worker = std::thread::spawn(move || {
        tracing::dispatcher::with_default(&dispatcher, || {
            let _worker = tracing::info_span!("actual_backend_wire_actor").entered();
            let mut service = BackendService::new(master_key, SafetyEnvelope::default());
            let mut server = IpcServer::new(backend);
            for _ in 0..2 {
                if !server.step(&mut service)? {
                    return Err(MetisError::transport(
                        ErrorCode::ConnectionClosed,
                        "Real backend fixture peer closes before handshake and calculation",
                    ));
                }
            }
            Ok::<_, MetisError>(service)
        })
    });
    // MemoryTransport is the real bounded wire implementation; IpcServer and
    // BackendService authorize, compute, MAC-sign and audit the actual payload.
    let result = (|| {
        let mut app = FrontendApp::new(frontend, 1024, 768)?;
        app.init(std::process::id(), principal)
            .map_err(|error| io::Error::other(error.to_string()))?;
        Ok::<_, Failure>(requests::submit(&mut app, identity, request, inputs))
    })();
    let service = worker
        .join()
        .map_err(|_| io::Error::other("Actual backend wire actor panics"))??;
    service.ledger().verify_chain()?;
    let records = service.ledger().records();
    assert_eq!(records.len(), 2);
    for (record, (sequence, message_type)) in records.iter().zip([
        (1, MessageType::HandshakeReq),
        (2, MessageType::ClinicalCalcReq),
    ]) {
        assert_eq!(record.sequence_id, sequence);
        assert_eq!(
            record.event,
            AuditEvent::Processed(RequestIdentity {
                message_type,
                sequence
            })
        );
        assert_eq!(record.actor_id, principal);
        assert_eq!(record.outcome, None);
    }
    result
}
