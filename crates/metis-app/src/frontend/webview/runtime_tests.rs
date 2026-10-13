//! Explicit installed-runtime acceptance fixtures invoked by the Windows gate.

mod correlated;
mod harness;

use harness::{Failure, Runtime};
use metis_core::error::ErrorCode;
use metis_web::embedded::{PageCommand, PageIdentity, PageStage, SequenceId};

fn tracing() -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .json()
            .without_time()
            .with_ansi(false)
            .with_writer(std::io::stderr)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .finish(),
    )
}

#[test]
#[ignore = "explicit Windows gate supplies genuine OS input to installed WebView2"]
fn rust_page_real_stop_remount_destruction() -> Result<(), Failure> {
    let _trace = tracing();
    let _test = tracing::info_span!("real_rust_page_remount").entered();
    let mut runtime = tracing::info_span!("arrange").in_scope(|| Runtime::new(true))?;
    let original = runtime.page.session.identity();
    tracing::info_span!("act").in_scope(|| -> Result<(), Failure> {
        runtime.ready(original)?;
        for mount in 1..=3 {
            let identity = runtime.page.session.identity();
            assert_eq!(identity.mount().get(), mount);
            let (request, inputs) = runtime.genuine_submission()?;
            let command = correlated::backend_result(identity, request, &inputs)?;
            let PageCommand::Result { response, .. } = &command else {
                return Err(std::io::Error::other(format!(
                    "Actual remount backend does not calculate: {command:?}"
                ))
                .into());
            };
            assert_eq!(response.rate_ml_hr.to_bits(), 1.0_f64.to_bits());
            assert_eq!(response.drug_rate_mg_hr.to_bits(), 1.875_f64.to_bits());
            assert_eq!(response.audit_sequence_id, 2);
            runtime.post(&command)?;
            runtime.page.session.complete_request(identity, request)?;
            // Physical Escape follows UIA inspection of the computed result.
            // Its existing production event admits the stop/remount transition.
            runtime.physical_close_request()?;
            if mount == 3 {
                break;
            }
            runtime.stop()?;
            let next = PageIdentity::new(
                original.role(),
                SequenceId::new(mount + 1)?,
                original.module_digest(),
            );
            runtime.page.session.remount(next)?;
            runtime.post(&PageCommand::Mount { identity: next })?;
            runtime.ready(next)?;
        }
        Ok(())
    })?;
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(runtime.page.session.identity().mount().get(), 3);
        assert_eq!(runtime.page.session.stage(), PageStage::Ready);
        runtime.finish()
    })?;
    tracing::info!(
        event = "fixture_completed",
        fixture = "stop_remount_destruction",
        mounts = 3,
        actual_backend_transactions = 3
    );
    Ok(())
}

#[test]
#[ignore = "explicit Windows acceptance gate requires actual installed WebView2"]
fn rust_page_real_malformed_duplicate_stale_commands() -> Result<(), Failure> {
    let _trace = tracing();
    let _test = tracing::info_span!("real_rust_page_hostile_commands").entered();
    let mut runtime = tracing::info_span!("arrange").in_scope(|| Runtime::new(false))?;
    let identity = runtime.page.session.identity();
    tracing::info_span!("act").in_scope(|| -> Result<(), Failure> {
        runtime.ready(identity)?;
        runtime.malformed(r#"{"type":"stop"}"#)?;
        runtime.failed("identity")?;
        runtime.post(&PageCommand::Mount { identity })?;
        runtime.failed("remount")?;
        let stale = PageIdentity::new(
            identity.role(),
            identity.mount().next()?,
            identity.module_digest(),
        );
        runtime.post(&PageCommand::Error {
            identity: stale,
            request: Some(SequenceId::new(1)?),
            status: "adversarial_stale_reply".into(),
            error_code: ErrorCode::SequenceMismatch.to_wire(),
            message: "Deliberately stale native correlation input".into(),
        })?;
        runtime.failed("current ready page")?;
        Ok(())
    })?;
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(runtime.page.session.stage(), PageStage::Ready);
        assert_eq!(runtime.page.session.pending(), None);
        runtime.finish()
    })?;
    tracing::info!(
        event = "fixture_completed",
        fixture = "malformed_duplicate_stale_commands",
        rejections = 3
    );
    Ok(())
}
