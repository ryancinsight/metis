//! Portable ownership and admission for the selected document producer.

#[cfg(test)]
mod tests;

use super::{Budget, Event, MAX_BYTES, MAX_TERMINAL_BYTES, Record, encode_observation};
use crate::embedded::{PageIdentity, SequenceId};
use std::io;

/// One mount owns one finite stream; posting occurs after this owner is released.
pub(in crate::embedded) struct Producer {
    identity: PageIdentity,
    budget: Budget,
    invocation: Option<SequenceId>,
    armed: bool,
    failure: Option<io::Error>,
    terminal_failure: Option<io::Error>,
}

impl Producer {
    pub(in crate::embedded) fn new(identity: PageIdentity) -> Self {
        Self {
            identity,
            budget: Budget::default(),
            invocation: None,
            armed: false,
            failure: None,
            terminal_failure: None,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(in crate::embedded) fn identity(&self) -> PageIdentity {
        self.identity
    }

    pub(in crate::embedded) fn active(&self) -> bool {
        self.failure.is_none() && self.terminal_failure.is_none() && !self.budget.is_terminal()
    }

    pub(in crate::embedded) fn armed(&self) -> bool {
        self.active() && self.armed
    }

    pub(in crate::embedded) fn selected(&mut self) -> Option<String> {
        if self.budget.records() != 0 {
            return None;
        }
        self.prepare(None, Event::Selected)
    }

    pub(in crate::embedded) fn arm(&mut self) -> Option<SequenceId> {
        if !self.active() || self.armed {
            return None;
        }
        self.armed = true;
        self.next_invocation()
    }

    pub(in crate::embedded) fn next_invocation(&mut self) -> Option<SequenceId> {
        if !self.armed() {
            return None;
        }
        let next = self
            .invocation
            .map_or_else(|| SequenceId::new(1), SequenceId::next);
        match next {
            Ok(next) => {
                self.invocation = Some(next);
                Some(next)
            }
            Err(error) => {
                self.disable(error);
                None
            }
        }
    }

    /// The first typed publication failure stays owned; domain results are untouched.
    pub(in crate::embedded) fn disable(&mut self, error: io::Error) {
        self.failure.get_or_insert(error);
    }

    pub(in crate::embedded) fn prepare(
        &mut self,
        invocation: Option<SequenceId>,
        event: Event,
    ) -> Option<String> {
        if !self.active() {
            return None;
        }
        let terminal = matches!(event, Event::ClosingIntent);
        let encoding_cap = if terminal {
            MAX_TERMINAL_BYTES
        } else {
            MAX_BYTES
        };
        let encoded = self
            .record(invocation, event)
            .and_then(|record| encode_observation(&record, encoding_cap));
        let encoded = match encoded {
            Ok(encoded) => encoded,
            Err(error) => {
                self.disable(error);
                return self.quota(true);
            }
        };
        match self.budget.admit(encoded.len(), terminal) {
            Ok(true) => Some(encoded),
            Ok(false) => self.quota(false),
            Err(error) => {
                self.disable(error);
                self.quota(true)
            }
        }
    }

    fn record(&self, invocation: Option<SequenceId>, event: Event) -> io::Result<Record> {
        let sequence = self
            .budget
            .records()
            .checked_add(1)
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| io::Error::other("Command observation sequence exceeds its domain"))?;
        Record::new(self.identity, SequenceId::new(sequence)?, invocation, event)
    }

    fn quota(&mut self, failed: bool) -> Option<String> {
        let result = (|| {
            let event = Event::Quota {
                prior_records: u32::try_from(self.budget.records()).map_err(io::Error::other)?,
                prior_bytes: u32::try_from(self.budget.bytes()).map_err(io::Error::other)?,
                // This notice follows the one actual rejected record. The
                // terminal stops production; later events are not sampled.
                omitted_at_notice: 1,
                failed,
            };
            let encoded = encode_observation(&self.record(None, event)?, MAX_TERMINAL_BYTES)?;
            if !self.budget.admit(encoded.len(), true)? {
                return Err(io::Error::other(
                    "Reserved command observation terminal does not fit",
                ));
            }
            Ok(encoded)
        })();
        match result {
            Ok(encoded) => Some(encoded),
            Err(error) => {
                self.terminal_failure.get_or_insert(error);
                None
            }
        }
    }
}
