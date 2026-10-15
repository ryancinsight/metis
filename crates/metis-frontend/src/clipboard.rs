//! Trusted clipboard command surface: the displayed rate sentence and the
//! copy-result plugin transaction.
//!
//! The sentence builder lives beside the copy methods (rather than in
//! [`crate::presentation`]) so the exact characters the outcome surface
//! shows and the bytes the clipboard authority carries share one module.

use crate::app::{FormState, FrontendApp};
use crate::presentation::{write_result_number, written};
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{
    ClinicalCalcResponsePayload, ClipboardWritePayload, ClipboardWriteResponse,
    PluginInvocationPayload,
};
use metis_ipc::transport::IpcTransport;

/// Builds the exact displayed rate sentence for `response`, so the clipboard
/// copy command carries the same characters the outcome surface shows.
///
/// # Errors
/// Returns a typed markup error when the number renderer fails.
pub(crate) fn result_text(response: &ClinicalCalcResponsePayload) -> Result<String> {
    let mut text = String::new();
    text.push_str("Rate: ");
    written(write_result_number(&mut text, response.rate_ml_hr, 3))?;
    text.push_str(" mL/hr (");
    written(write_result_number(&mut text, response.drug_rate_mg_hr, 2))?;
    text.push_str(" mg/hr)");
    Ok(text)
}

impl<T: IpcTransport> FrontendApp<T> {
    /// Copies the displayed rate sentence through the trusted clipboard
    /// authority. A session without a correlated result only updates the
    /// command status; a rejected or failed copy keeps the session because
    /// the clipboard transaction never invalidates the calculation stream.
    ///
    /// # Errors
    /// Returns rendering failures; copy failures remain in the status line.
    pub(crate) fn copy_result_to_clipboard(&mut self) -> Result<()> {
        let previous_status = self.command_status.clone();
        let outcome = match &self.state {
            FormState::Success(response) => {
                let sentence = result_text(response)?;
                match self.request_clipboard_write(&sentence) {
                    Ok(()) => "Result copied to clipboard".to_owned(),
                    Err(error) => format!("Copy failed ({error})"),
                }
                // Rejections above reach the status line verbatim; no result
                // row changes because the clipboard stream is separate.
            }
            _ => "No result to copy".to_owned(),
        };
        outcome.clone_into(&mut self.command_status);
        if let Err(error) = self.render() {
            self.command_status = previous_status;
            return Err(error);
        }
        Ok(())
    }

    fn request_clipboard_write(&mut self, text: &str) -> Result<()> {
        let client = self.client.as_mut().ok_or_else(Self::closed)?;
        let token = client.active_token().cloned().ok_or_else(|| {
            MetisError::capability(
                ErrorCode::MissingCapability,
                "Establish a session before copying",
            )
        })?;
        let request = PluginInvocationPayload::new(
            token,
            "clipboard",
            "write",
            ClipboardWritePayload::new(text)?.encode()?,
        )?;
        let response = client
            .invoke_plugin(&request)
            .map_err(|error| match error {
                metis_ipc::PluginInvocationError::Local(inner) => inner,
                metis_ipc::PluginInvocationError::Remote(inner) => {
                    // The correlation stream stays authoritative; the peer's
                    // typed code reaches the caller as a typed capability
                    // rejection with its message preserved verbatim.
                    MetisError::capability(
                        ErrorCode::UnexpectedMessageType,
                        format!("Clipboard authority rejected: {}", inner.message),
                    )
                }
                other => {
                    // Remaining plugin envelope states are transport-typed; the
                    // correlation stays closed and the status line carries them.
                    MetisError::capability(
                        ErrorCode::UnexpectedMessageType,
                        format!("Clipboard copy failed: {other}"),
                    )
                }
            })?;
        let carried = ClipboardWriteResponse::decode(response.body())?;
        if carried.bytes() == 0 {
            return Err(MetisError::capability(
                ErrorCode::UnexpectedMessageType,
                "The clipboard authority carried no bytes",
            ));
        }
        Ok(())
    }
}
