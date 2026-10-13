//! One closed, byte-bounded JSON codec with private serialized types.
//!
//! The JSON dependency enables `float_roundtrip`: a finite backend binary64
//! value must decode to its original bits, including subnormal boundaries.

mod commands;
mod events;
mod number;
mod permission;
mod resources;
mod shape;

use super::identity::invalid;
use super::{PageCommand, PageEvent};
use serde::{Serialize, de::DeserializeOwned};
use std::io::{self, Write};

/// Encodes a page event within the caller's actual bridge byte budget.
/// # Errors
/// Rejects nonfinite values or a serialized message exceeding the supplied budget.
pub fn encode_page_event(event: &PageEvent, byte_budget: usize) -> io::Result<String> {
    encode(&events::WireEvent::try_from(event)?, byte_budget)
}
/// Decodes one complete page event using the same actual bridge budget.
/// # Errors
/// Rejects oversize input, unknown fields, malformed identities or trailing JSON.
pub fn decode_page_event(json: &str, byte_budget: usize) -> io::Result<PageEvent> {
    PageEvent::try_from(decode::<events::WireEvent>(json, byte_budget)?)
}
/// Encodes a native command without sending exact integers through binary64.
/// # Errors
/// Rejects nonfinite results or serialized output exceeding the supplied budget.
pub fn encode_page_command(command: &PageCommand, byte_budget: usize) -> io::Result<String> {
    encode(&commands::WireCommand::try_from(command)?, byte_budget)
}
/// Decodes one complete closed native command.
/// # Errors
/// Rejects oversize input, ambiguous integer text, unknown fields or trailing JSON.
pub fn decode_page_command(json: &str, byte_budget: usize) -> io::Result<PageCommand> {
    PageCommand::try_from(decode::<commands::WireCommand>(json, byte_budget)?)
}

fn decode<T: DeserializeOwned>(json: &str, budget: usize) -> io::Result<T> {
    if json.len() > budget {
        return Err(invalid("Page JSON exceeds the bridge byte budget"));
    }
    serde_json::from_str(json)
        .map_err(|error| invalid(format!("Page JSON decoding failed: {error}")))
}

fn encode(value: &impl Serialize, budget: usize) -> io::Result<String> {
    let mut writer = BoundedWriter {
        bytes: Vec::new(),
        budget,
    };
    serde_json::to_writer(&mut writer, value).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Page JSON encoding failed: {error}"),
        )
    })?;
    String::from_utf8(writer.bytes).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Page JSON is not UTF-8: {error}"),
        )
    })
}

struct BoundedWriter {
    bytes: Vec<u8>,
    budget: usize,
}

impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let needed = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|length| *length <= self.budget)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Page JSON exceeds the bridge byte budget",
                )
            })?;
        if needed > self.bytes.capacity() {
            // Geometric growth preserves amortized linear copying; clipping to
            // the admitted budget bounds retained capacity before reservation.
            let capacity = needed
                .checked_next_power_of_two()
                .map_or(self.budget, |size| size.min(self.budget));
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(|error| {
                    io::Error::other(format!("Page JSON reservation failed: {error}"))
                })?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn finite(values: &[f64]) -> io::Result<()> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(invalid(
            "Page numeric values must be finite backend-precision numbers",
        ))
    }
}
