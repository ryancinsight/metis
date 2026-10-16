//! Original JSON is strictly decoded; representation-equivalent objects agree.

mod event;
mod words;

use super::Record;
use crate::embedded::{
    SequenceId,
    identity::invalid,
    wire::{decode, encode, shape::WireIdentity},
};
use event::WireEvent;
use serde::{Deserialize, Deserializer, Serialize};
use std::io;

const KIND: &str = "metis_command_observation";
const SCHEMA: u16 = 1;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRecord {
    kind: String,
    observation_schema: u16,
    identity: WireIdentity,
    sequence: String,
    #[serde(deserialize_with = "required_option")]
    invocation: Option<String>,
    event: WireEvent,
}

/// Encodes a validated observation within the supplied encoded-byte budget.
/// # Errors
/// Rejects output exceeding that budget, including a partial encoded record.
pub fn encode_observation(record: &Record, byte_budget: usize) -> io::Result<String> {
    encode(
        &WireRecord {
            kind: KIND.to_owned(),
            observation_schema: SCHEMA,
            identity: record.identity().into(),
            sequence: record.sequence().to_string(),
            invocation: record.invocation().map(|value| value.to_string()),
            event: record.event().into(),
        },
        byte_budget,
    )
}

/// Decodes the original complete JSON without requiring a canonical spelling.
///
/// Whitespace, field order and equivalent JSON escapes may change when the
/// bridge parses and serializes an object. Unknown and duplicate fields remain
/// errors because the original input goes directly to the closed private DTO.
/// # Errors
/// Rejects oversize JSON, unknown/duplicate fields, malformed identities,
/// noncanonical decimal fields, unsupported schema or incoherent event shapes.
pub fn decode_observation(json: &str, byte_budget: usize) -> io::Result<Record> {
    let wire = decode::<WireRecord>(json, byte_budget)?;
    if wire.kind != KIND || wire.observation_schema != SCHEMA {
        return Err(invalid("Command observation kind or schema does not match"));
    }
    Record::new(
        wire.identity.try_into()?,
        SequenceId::parse(&wire.sequence)?,
        wire.invocation
            .as_deref()
            .map(SequenceId::parse)
            .transpose()?,
        wire.event.try_into()?,
    )
}

/// Recognizes the semantic top-level discriminator without admitting a record.
///
/// Call `decode_observation` on the original JSON after this routing decision;
/// this predicate neither validates payload fields nor grants page authority.
#[must_use]
pub fn is_observation(json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .is_some_and(|value| value.get("kind").and_then(serde_json::Value::as_str) == Some(KIND))
}

fn required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
