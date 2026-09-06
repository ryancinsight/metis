//! Parent-process application state and supervised presentation launch.
use crate::{entropy, invocation::FRONTEND_ROLE};
use metis_backend::{BackendService, clinical::SafetyEnvelope, supervisor::run_session};

pub(crate) fn run(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    // Binary reporter boundary erases errors; no hot-path dispatch.
    let executable = std::env::current_exe()?;
    let mut service = BackendService::new(entropy::session_key()?, SafetyEnvelope::default());
    let [weight, concentration, dose] = inputs;
    let arguments = [FRONTEND_ROLE.to_owned(), weight, concentration, dose];
    eprintln!("backend_pid={}", std::process::id());
    run_session(&executable, &arguments, &mut service)?;
    service.ledger().verify_chain()?;
    eprintln!(
        "Metis session completed; {} audit records verified",
        service.ledger().records().len()
    );
    Ok(())
}
