use serde::Deserialize;

/// The host reply envelope, typed with the exact wire shape the host owns.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum PostedMessage {
    /// One correlated calculation reply.
    Result {
        /// Always `success` on the current host contract.
        status: String,
        /// The audited calculation sequence identifier.
        audit_sequence_id: u64,
        /// mL per hour.
        rate_ml_hr: f64,
        /// mg per hour.
        drug_rate_mg_hr: f64,
    },
    /// One rejected, failed or host-generated reply.
    Error {
        /// The typed status token.
        status: String,
        /// The Metis error code.
        error_code: u16,
        /// The human-readable diagnostic.
        message: String,
    },
    /// One bounded capability denial, delivered on the probe page.
    PermissionDenied {
        /// The typed status token.
        status: String,
        /// The Metis error code.
        error_code: u16,
        /// The denied capability name.
        permission: String,
        /// The human-readable diagnostic.
        message: String,
        /// Whether the browser recorded a user-initiated denial.
        user_initiated: bool,
    },
}
