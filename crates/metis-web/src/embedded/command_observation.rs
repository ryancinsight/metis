//! Opt-in command-path observations, separate from readiness and authority.
//!
//! See `command_observation/design.md` for the closed wire schema and reserved
//! terminal budget. These records report actual observations; decoding one
//! grants no command, page, input or backend authority.

mod budget;
mod codec;
mod record;
mod types;

#[cfg(any(target_arch = "wasm32", test))]
pub(super) mod producer;

pub use budget::Budget;
pub use codec::{decode_observation, encode_observation, is_observation};
pub use record::Record;
pub use types::{Admission, Decision, Event, KeyboardSnapshot, Outcome};

/// Document marker that explicitly selects this private diagnostic stream.
pub const MARKER_NAME: &str = "data-metis-command-observation";
/// Exact marker value; other values do not select observation.
pub const MARKER_VALUE: &str = "1";
/// Exact compact Python provider selection for the selected form worker.
pub const PROVIDER_SELECTION: &str = r##"{"start_uri":"https://metis.invalid/index.html","selectors":["#host-status","#result"],"late_input_after_key":68}"##;
/// Inclusive record count, including the final quota or closing notice.
pub const MAX_RECORDS: usize = 128;
/// Inclusive encoded stream bytes, including the reserved final notice.
pub const MAX_BYTES: usize = 32_768;

// Each terminal has only fixed ASCII names, canonical integers, a fixed digest
// and a closed identity. These templates maximize every variable field width.
const QUOTA_TEMPLATE: &str = r#"{"kind":"metis_command_observation","observation_schema":1,"identity":{"schema":1,"role":"permission_probe","mount_id":"18446744073709551615","module_digest":"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"},"sequence":"18446744073709551615","invocation":null,"event":{"type":"quota","prior_records":4294967295,"prior_bytes":4294967295,"omitted_at_notice":"18446744073709551615","failed":false}}"#;
const CLOSING_TEMPLATE: &str = r#"{"kind":"metis_command_observation","observation_schema":1,"identity":{"schema":1,"role":"permission_probe","mount_id":"18446744073709551615","module_digest":"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"},"sequence":"18446744073709551615","invocation":"18446744073709551615","event":{"type":"closing_intent"}}"#;
/// Maximum canonical encoded terminal size derived from both closed schemas.
pub const MAX_TERMINAL_BYTES: usize = if QUOTA_TEMPLATE.len() > CLOSING_TEMPLATE.len() {
    QUOTA_TEMPLATE.len()
} else {
    CLOSING_TEMPLATE.len()
};

const _: () = assert!(super::PAGE_SCHEMA_VERSION == 1);
const _: () = assert!(MAX_RECORDS > 1 && MAX_BYTES > MAX_TERMINAL_BYTES);

#[cfg(test)]
mod tests;
