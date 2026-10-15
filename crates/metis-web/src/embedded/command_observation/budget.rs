//! One accounting law for canonical producer bytes and original consumer bytes.

use super::{MAX_BYTES, MAX_RECORDS, MAX_TERMINAL_BYTES};
use std::io;

/// Atomic encoded-byte/record accounting with one reserved terminal notice.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Budget {
    records: usize,
    bytes: usize,
    terminal: bool,
}

impl Budget {
    /// Returns the number of records actually admitted by this owner.
    #[must_use]
    pub const fn records(&self) -> usize {
        self.records
    }
    /// Returns this owner's actual admitted encoded byte count.
    #[must_use]
    pub const fn bytes(&self) -> usize {
        self.bytes
    }
    /// Reports whether the one terminal record has already been admitted.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        self.terminal
    }

    /// Admits an entire record while preserving the reserved terminal space.
    ///
    /// `false` leaves all state unchanged when a resource cap rejects the
    /// record. Canonical producer bytes and original consumer bytes use separate
    /// instances; their counters need not agree after JSON object reserialization.
    /// The canonical terminal bound reserves space for a locally encoded notice;
    /// a received terminal may use any remaining bytes within the total cap.
    /// # Errors
    /// Rejects an empty record, use after terminal admission or arithmetic
    /// overflow without changing any counter or the terminal flag.
    pub fn admit(&mut self, encoded_bytes: usize, terminal: bool) -> io::Result<bool> {
        if self.terminal {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Command observation budget is already terminal",
            ));
        }
        if encoded_bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Command observation record must contain encoded bytes",
            ));
        }
        let records = self.records.checked_add(1).ok_or_else(overflow)?;
        let bytes = self.bytes.checked_add(encoded_bytes).ok_or_else(overflow)?;
        let (record_cap, byte_cap) = if terminal {
            (MAX_RECORDS, MAX_BYTES)
        } else {
            (MAX_RECORDS - 1, MAX_BYTES - MAX_TERMINAL_BYTES)
        };
        if records > record_cap || bytes > byte_cap {
            return Ok(false);
        }
        self.records = records;
        self.bytes = bytes;
        self.terminal = terminal;
        Ok(true)
    }
}

fn overflow() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Command observation accounting arithmetic is exhausted",
    )
}
