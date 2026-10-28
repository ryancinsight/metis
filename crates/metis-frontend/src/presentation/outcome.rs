//! Outcome writing for the form's result display.

use crate::FormState;
use crate::presentation::format::write_result_number;
use metis_ipc::client::HandshakeError;
use std::fmt;
use std::fmt::Write as FmtWrite;

/// Writes the rate and the safety status for `state` and returns the
/// signature line, which is fixed text.
pub(crate) fn write_outcome(
    state: &FormState,
    rate: &mut String,
    status: &mut String,
) -> Result<&'static str, fmt::Error> {
    const AWAITING: &str = "Rate: Awaiting Backend Calculation...";
    const NO_RESULT: &str = "Rate: No result";
    const NO_SIGNATURE: &str = "Backend MAC: No result";
    match state {
        FormState::Idle => {
            rate.push_str(AWAITING);
            status.push_str("Safety Status: Idle");
            Ok("Backend MAC: None")
        }
        FormState::Pending => {
            rate.push_str(AWAITING);
            status.push_str("Request in progress");
            Ok("Backend MAC: None")
        }
        FormState::Success(response) => {
            rate.push_str("Rate: ");
            write_result_number(rate, response.rate_ml_hr, 3)?;
            rate.push_str(" mL/hr (");
            write_result_number(rate, response.drug_rate_mg_hr, 2)?;
            rate.push_str(" mg/hr)");
            write!(
                status,
                "Backend response (Audit Seq #{}){}",
                response.audit_sequence_id,
                if response.is_pediatric {
                    " [PEDIATRIC]"
                } else {
                    ""
                }
            )?;
            Ok("Backend MAC: Present (not verified by frontend)")
        }
        FormState::Rejected(error) => {
            rate.push_str(NO_RESULT);
            write!(
                status,
                "Backend rejected request [0x{:04X}]",
                error.error_code
            )?;
            Ok(NO_SIGNATURE)
        }
        FormState::Failed(error) => {
            rate.push_str(NO_RESULT);
            write!(status, "Request not sent [0x{:04X}]", error.code as u16)?;
            Ok(NO_SIGNATURE)
        }
        FormState::Disconnected(error) => {
            rate.push_str(NO_RESULT);
            write!(
                status,
                "Connection failed [0x{:04X}] - reconnect",
                error.code as u16
            )?;
            Ok(NO_SIGNATURE)
        }
        FormState::SessionFailed(error) => {
            rate.push_str(NO_RESULT);
            status.push_str("Session failed [");
            match error {
                HandshakeError::Local(error) => write!(status, "0x{:04X}", error.code as u16)?,
                HandshakeError::Remote(error) => write!(status, "0x{:04X}", error.error_code)?,
                _ => status.push_str("unrecognized"),
            }
            status.push_str("] - reconnect");
            Ok(NO_SIGNATURE)
        }
    }
}
