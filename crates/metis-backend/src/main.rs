//! Launches the demonstration frontend and owns its backend service.
mod entropy;
use metis_backend::{BackendService, clinical::SafetyEnvelope, supervisor::run_session};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Binary reporter boundary erases errors; no hot-path dispatch.
    let frontend = std::env::current_exe()?
        .with_file_name(format!("metis-frontend{}", std::env::consts::EXE_SUFFIX));
    let mut service = BackendService::new(entropy::session_key()?, SafetyEnvelope::default());
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    eprintln!("backend_pid={}", std::process::id());
    run_session(&frontend, &arguments, &mut service)?;
    service.ledger().verify_chain()?;
    eprintln!(
        "Metis session completed; {} audit records verified",
        service.ledger().records().len()
    );
    Ok(())
}
