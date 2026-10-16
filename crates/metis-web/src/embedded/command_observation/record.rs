//! Validated identity, invocation shape and observation coherence.

use super::{Admission, Decision, Event};
use crate::embedded::{PageIdentity, SequenceId, identity::invalid};
use std::io;

/// A validated command observation bound to one exact page identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    identity: PageIdentity,
    sequence: SequenceId,
    invocation: Option<SequenceId>,
    event: Event,
}

impl Record {
    /// Admits one event without changing page or command state.
    /// # Errors
    /// Rejects an invocation shape or metadata/admission/decision mismatch.
    pub fn new(
        identity: PageIdentity,
        sequence: SequenceId,
        invocation: Option<SequenceId>,
        event: Event,
    ) -> io::Result<Self> {
        let independent = matches!(event, Event::Selected | Event::Quota { .. });
        if independent == invocation.is_some() {
            return Err(invalid(
                "Command observation invocation does not match its event",
            ));
        }
        if let Event::MetadataDecision {
            keyboard,
            admission,
            decision,
        } = &event
        {
            let coherent = match admission {
                Admission::Ready => keyboard.is_some() && *decision != Decision::NotEvaluated,
                Admission::NotReady | Admission::NotKeyboard | Admission::MetadataError => {
                    keyboard.is_none() && *decision == Decision::NotEvaluated
                }
            };
            if !coherent {
                return Err(invalid(
                    "Command observation metadata admission is incoherent",
                ));
            }
        }
        Ok(Self {
            identity,
            sequence,
            invocation,
            event,
        })
    }

    /// Returns the exact admitted page identity.
    #[must_use]
    pub const fn identity(&self) -> PageIdentity {
        self.identity
    }
    /// Returns the stream's exact positive sequence.
    #[must_use]
    pub const fn sequence(&self) -> SequenceId {
        self.sequence
    }
    /// Returns the actual listener invocation, if this event owns one.
    #[must_use]
    pub const fn invocation(&self) -> Option<SequenceId> {
        self.invocation
    }
    /// Borrows the admitted event without duplicating metadata strings.
    #[must_use]
    pub const fn event(&self) -> &Event {
        &self.event
    }
    /// Reports whether this event consumes the stream's one terminal slot.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self.event, Event::ClosingIntent | Event::Quota { .. })
    }
}
