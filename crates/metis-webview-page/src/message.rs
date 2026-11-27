//! Native replies posted to the packaged document.

use serde::{Deserialize, Deserializer, de};

/// The host reply envelope, typed with the exact wire shape the host owns.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum PostedMessage {
    /// One correlated calculation reply.
    Result {
        /// Always `success` on the current host contract.
        status: String,
        /// The audited calculation sequence identifier.
        #[serde(deserialize_with = "audit_sequence")]
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

/// Audit IDs cross the JavaScript object boundary as canonical decimal strings.
fn audit_sequence<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let digits = String::deserialize(deserializer)?;
    if digits.starts_with('+') || (digits.len() > 1 && digits.starts_with('0')) {
        return Err(de::Error::custom("noncanonical audit sequence"));
    }
    digits.parse().map_err(de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::PostedMessage;
    use serde_json::json;

    #[test]
    fn result_uses_the_hosts_full_width_decimal_sequence() {
        for audit in [0, 1, (1_u64 << 53) + 1, u64::MAX] {
            let object = json!({"type":"result", "status":"success",
                "audit_sequence_id":audit.to_string(), "rate_ml_hr":1.5,
                "drug_rate_mg_hr":0.75, "is_pediatric":false});
            assert_eq!(
                serde_json::from_value::<PostedMessage>(object).expect("host reply"),
                PostedMessage::Result {
                    status: "success".into(),
                    audit_sequence_id: audit,
                    rate_ml_hr: 1.5,
                    drug_rate_mg_hr: 0.75
                }
            );
        }
    }

    #[test]
    fn result_rejects_numeric_and_noncanonical_audit_ids() {
        for audit in [
            json!(7),
            json!(null),
            json!(""),
            json!("-1"),
            json!("+1"),
            json!("01"),
            json!(" 1"),
            json!("18446744073709551616"),
        ] {
            let object = json!({"type":"result", "status":"success", "audit_sequence_id":audit,
                "rate_ml_hr":1.5, "drug_rate_mg_hr":0.75});
            assert_eq!(
                serde_json::from_value::<PostedMessage>(object)
                    .expect_err("invalid audit ID")
                    .classify(),
                serde_json::error::Category::Data
            );
        }
    }
}
