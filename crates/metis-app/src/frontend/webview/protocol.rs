//! Private JSON contract shared by the Windows host and portable protocol tests.
//!
//! The native host uses these DTOs directly; host tests compile this same leaf
//! without importing Windows UI bindings. Backend binary payloads keep their
//! existing integer representation. Only the page's JSON text boundary changes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct WebViewRequest {
    pub(super) action: WebViewAction,
    pub(super) patient_id: String,
    pub(super) weight_kg: f64,
    pub(super) concentration_mg_ml: f64,
    pub(super) target_dose_mcg_kg_min: f64,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum WebViewAction {
    Submit,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PermissionProbeComplete {
    action: PermissionProbeAction,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
enum PermissionProbeAction {
    #[serde(rename = "permission_probe_complete")]
    Complete,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum PageMessage {
    Result {
        status: &'static str,
        #[serde(serialize_with = "serialize_audit_sequence")]
        audit_sequence_id: u64,
        rate_ml_hr: f64,
        drug_rate_mg_hr: f64,
        is_pediatric: bool,
    },
    Error {
        status: &'static str,
        error_code: u16,
        message: String,
    },
    PermissionDenied {
        status: &'static str,
        error_code: u16,
        permission: String,
        message: String,
        user_initiated: bool,
    },
}

/// Emits the unique base-ten digit sequence D(n) as a JSON string.
///
/// At the retained `u64` field, for every n in `0..=u64::MAX`, parsing D(n)
/// as `u64` returns n. Thus D is
/// injective, has no leading zeros except D(0) = "0", and needs at most 20
/// digits. `WebView2` parses JSON numbers through binary64, which cannot preserve
/// every integer above 2^53 - 1; a JSON string never enters that numeric path.
/// `collect_str` delegates integer formatting and string storage to the selected
/// serializer. The audit ledger's positive-sequence admission is separate from
/// this full-range representation contract.
#[inline]
fn serialize_audit_sequence<S: serde::Serializer>(
    audit: impl std::fmt::Display,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_str(&audit)
}

pub(super) fn is_permission_probe_complete(json: &str) -> bool {
    matches!(
        serde_json::from_str::<PermissionProbeComplete>(json),
        Ok(PermissionProbeComplete {
            action: PermissionProbeAction::Complete
        })
    )
}

#[cfg(test)]
#[path = "protocol/tests.rs"]
mod tests;
