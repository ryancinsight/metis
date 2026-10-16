//! Selected document observations carry evidence, never page authority.

use metis_web::embedded::{
    PageRole, PageSession, PageStage,
    command_observation::{
        Budget, MARKER_NAME, MARKER_VALUE, MAX_BYTES, MAX_RECORDS, MAX_TERMINAL_BYTES,
        PROVIDER_SELECTION, decode_observation, is_observation,
    },
};
use std::{borrow::Cow, ffi::OsStr, io};

#[cfg(windows)]
const SELECTION_ENV: &str = "MOIRAI_WEBVIEW_STARTUP_OBSERVATION";
pub(super) const TARGET: &str = "metis_app::webview::command_observation";
pub(super) const EVENT: &str = "page_command_observation";

// Cartesian bound: maximal native counters, decimal width, longest reason,
// and the longer Boolean. This is a native rejection, never a renderer event.
const NATIVE_TERMINAL_BYTES: usize = r#"{"kind":"native_command_observation_terminal","reason":"receiver_quota_exhausted","prior_records":128,"prior_bytes":32768,"expected_sequence":"18446744073709551615","has_error":false,"failed":false}"#.len();
const _: () = assert!(MAX_RECORDS == 128 && MAX_BYTES == 32_768);
const _: () = assert!(NATIVE_TERMINAL_BYTES <= MAX_TERMINAL_BYTES);

#[derive(Debug)]
pub(super) enum Receipt<'a> {
    Functional,
    Record(Cow<'a, str>),
    Censored,
}

#[derive(Debug)]
pub(super) struct Receiver {
    next: u64,
    budget: Budget,
    error: Option<io::Error>,
}

impl Receiver {
    pub(super) fn new() -> Self {
        Self {
            next: 1,
            budget: Budget::default(),
            error: None,
        }
    }

    /// The caller must retain the existing source admission before this call.
    /// A private observation changes only this independent evidence owner.
    pub(super) fn receive<'a>(&mut self, json: &'a str, session: &PageSession) -> Receipt<'a> {
        if !is_observation(json) {
            return Receipt::Functional;
        }
        if self.budget.is_terminal() {
            return Receipt::Censored;
        }
        if session.identity().role() != PageRole::Form || session.stage() != PageStage::Ready {
            return self.reject("not_current_ready_form", None);
        }
        let record = match decode_observation(json, MAX_BYTES) {
            Ok(record) => record,
            Err(error) => return self.reject("invalid_record", Some(error)),
        };
        if record.identity() != session.identity() {
            return self.reject("identity_mismatch", None);
        }
        if record.sequence().get() != self.next {
            return self.reject("unexpected_sequence", None);
        }
        let Some(next) = self.next.checked_add(1) else {
            return self.reject("unexpected_sequence", None);
        };
        match self.budget.admit(json.len(), record.is_terminal()) {
            Ok(true) => {}
            Ok(false) => return self.reject("receiver_quota_exhausted", None),
            Err(error) => return self.reject("receiver_quota_exhausted", Some(error)),
        }
        self.next = next;
        Receipt::Record(Cow::Borrowed(json))
    }

    fn reject(&mut self, reason: &'static str, error: Option<io::Error>) -> Receipt<'static> {
        self.error = error;
        let encoded = serde_json::json!({
            "kind": "native_command_observation_terminal", "reason": reason,
            "prior_records": self.budget.records(), "prior_bytes": self.budget.bytes(),
            "expected_sequence": self.next.to_string(), "has_error": self.error.is_some(),
            "failed": true,
        })
        .to_string();
        assert!(
            encoded.len() <= NATIVE_TERMINAL_BYTES,
            "native finite terminal bound"
        );
        assert!(
            self.budget
                .admit(encoded.len(), true)
                .expect("finite native terminal spends its reserved capacity")
        );
        Receipt::Record(Cow::Owned(encoded))
    }
}

/// The existing fixture environment crosses both boundaries without rewriting.
/// Only its exact owned selection enables this additional observation channel.
#[cfg(windows)]
pub(super) fn selected(role: PageRole) -> bool {
    selected_configuration(role, std::env::var_os(SELECTION_ENV).as_deref())
}

fn selected_configuration(role: PageRole, encoded: Option<&OsStr>) -> bool {
    role == PageRole::Form && encoded == Some(OsStr::new(PROVIDER_SELECTION))
}

pub(super) fn mark_document(index: String, enabled: bool) -> io::Result<String> {
    if !enabled {
        return Ok(index);
    }
    if index.matches("<body ").count() != 1 || index.contains(MARKER_NAME) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Selected command observation requires one unmarked packaged body",
        ));
    }
    Ok(index.replacen(
        "<body ",
        &format!("<body {MARKER_NAME}=\"{MARKER_VALUE}\" "),
        1,
    ))
}

#[cfg(test)]
#[path = "command_observation/tests.rs"]
mod tests;
