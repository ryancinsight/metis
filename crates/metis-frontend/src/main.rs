//! One headless form submission over inherited pipes; stdout is wire bytes only.
use metis_frontend::FrontendApp;
use metis_ipc::transport::StreamTransport;
use std::io::{stdin, stdout};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Binary error reporter is the non-hot type-erasure boundary.
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [weight, concentration, dose] = arguments.as_slice() else {
        return Err("usage: metis-backend WEIGHT_KG CONCENTRATION_MG_ML DOSE_MCG_KG_MIN".into());
    };
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
    );
    app.submit_calculation()?;
    match app.last_response.as_ref() {
        Some(response) => eprintln!(
            "frontend_pid={pid} rate_ml_hr={} drug_rate_mg_hr={} audit_sequence={}",
            response.rate_ml_hr, response.drug_rate_mg_hr, response.audit_sequence_id
        ),
        None => {
            return Err(app
                .last_error
                .as_deref()
                .unwrap_or("Backend returned no result")
                .into());
        }
    }
    Ok(())
}
