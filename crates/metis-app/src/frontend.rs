//! One headless form submission over inherited pipes; stdout is wire bytes only.
use metis_frontend::{FormState, FrontendApp};
use metis_ipc::transport::StreamTransport;
use std::io::{stdin, stdout};

pub(crate) fn run(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    // Binary error reporter is the non-hot type-erasure boundary.
    let [weight, concentration, dose] = inputs;
    let transport = StreamTransport::new(stdin(), stdout());
    let mut app = FrontendApp::new(transport, 800, 600)?;
    let pid = std::process::id();
    let mut principal_id = [0; 16];
    principal_id[..4].copy_from_slice(&pid.to_be_bytes());
    app.init(pid, principal_id)?;
    app.set_inputs(
        "demo",
        weight.parse()?,
        concentration.parse()?,
        dose.parse()?,
    )?;
    app.submit_calculation()?;
    match app.state() {
        FormState::Success(response) => eprintln!(
            "frontend_pid={pid} rate_ml_hr={} drug_rate_mg_hr={} audit_sequence={}",
            response.rate_ml_hr, response.drug_rate_mg_hr, response.audit_sequence_id
        ),
        FormState::Rejected(error) => {
            return Err(format!(
                "Backend rejected request [0x{:04X}]: {}",
                error.error_code, error.message
            )
            .into());
        }
        state => return Err(format!("Submission completed without a result: {state:?}").into()),
    }
    Ok(())
}
