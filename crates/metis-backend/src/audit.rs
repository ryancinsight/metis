//! Bounded in-memory request ledger with canonical hash chaining.
//!
//! This ledger retains the last 1024 calls and a predecessor checkpoint. It is
//! diagnostic memory, not durable storage or protection against an attacker who
//! can rewrite both the records and the checkpoint. A deployment must persist
//! authenticated checkpoints outside the process before claiming durable audit.

use metis_core::error::{ErrorCode, MetisError, Result};
use metis_ipc::server::{FailureContext, RequestIdentity};
use moirai_crypto::{constant_time_eq_32, sha256};
use std::collections::VecDeque;

/// Maximum retained calls; fixed-size entries bound resident audit storage.
pub const AUDIT_CAPACITY: usize = 1024;

const AUDIT_HASH_CAPACITY: usize = 99;

/// Application processing and transport failures have distinct audit meanings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditEvent {
    /// A handler processed the request, either accepting or rejecting it.
    Processed(RequestIdentity),
    /// Processing was prevented, aborted, or its response could not be sent.
    Failure(FailureContext),
}

/// Immutable externally visible outcome of processing or transport activity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    /// Monotonically increasing ledger identifier.
    pub sequence_id: u64,
    /// Last trusted UTC milliseconds; zero when no clock reading is available.
    pub timestamp_millis: u64,
    /// Session principal, or zero before a session exists.
    pub actor_id: [u8; 16],
    /// Processing stage and request identity when the server knows it.
    pub event: AuditEvent,
    /// None denotes completed processing; Some names the rejected invariant.
    pub outcome: Option<ErrorCode>,
    /// Digest immediately preceding this record.
    pub prev_hash: [u8; 32],
    /// Digest of the domain tag and every preceding field in canonical order.
    pub record_hash: [u8; 32],
}

impl AuditRecord {
    fn compute_hash(&self) -> [u8; 32] {
        let mut bytes = Vec::with_capacity(AUDIT_HASH_CAPACITY);
        bytes.extend_from_slice(b"METIS-AUDIT-2");
        bytes.extend_from_slice(&self.sequence_id.to_be_bytes());
        bytes.extend_from_slice(&self.timestamp_millis.to_be_bytes());
        bytes.extend_from_slice(&self.actor_id);
        let (tag, identity, event_id) = match self.event {
            AuditEvent::Processed(identity) => (0, Some(identity), None),
            AuditEvent::Failure(FailureContext::Receive) => (1, None, None),
            AuditEvent::Failure(FailureContext::Request(identity)) => (2, Some(identity), None),
            AuditEvent::Failure(FailureContext::Handler(identity)) => (3, Some(identity), None),
            AuditEvent::Failure(FailureContext::Response(identity)) => (4, Some(identity), None),
            AuditEvent::Failure(FailureContext::Event(event_id)) => (5, None, Some(event_id)),
            AuditEvent::Failure(_) => (255, None, None),
        };
        bytes.push(tag);
        if let Some(identity) = identity {
            bytes.extend_from_slice(&(identity.message_type as u16).to_be_bytes());
            bytes.extend_from_slice(&identity.sequence.to_be_bytes());
        }
        if let Some(event_id) = event_id {
            bytes.extend_from_slice(&event_id.get().to_be_bytes());
        }
        bytes.push(u8::from(self.outcome.is_some()));
        bytes.extend_from_slice(&self.outcome.map_or(0, |code| code as u16).to_be_bytes());
        bytes.extend_from_slice(&self.prev_hash);
        sha256(&bytes)
    }
}

/// Ring of recent calls with an eviction checkpoint and continuous sequence.
#[derive(Debug, Clone)]
pub struct AuditLedger {
    records: VecDeque<AuditRecord>,
    checkpoint: [u8; 32],
    checkpoint_sequence: u64,
    last_hash: [u8; 32],
    next_sequence: u64,
}

impl Default for AuditLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditLedger {
    /// Starts an empty ledger under the versioned genesis digest.
    #[must_use]
    pub fn new() -> Self {
        let genesis = sha256(b"METIS-AUDIT-GENESIS-2");
        Self {
            records: VecDeque::with_capacity(AUDIT_CAPACITY),
            checkpoint: genesis,
            checkpoint_sequence: 0,
            last_hash: genesis,
            next_sequence: 1,
        }
    }

    pub(crate) const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub(crate) fn record(
        &mut self,
        timestamp_millis: u64,
        actor_id: [u8; 16],
        event: AuditEvent,
        outcome: Option<ErrorCode>,
    ) -> Result<()> {
        let sequence_id = self.next_sequence;
        let next = sequence_id.checked_add(1).ok_or_else(|| {
            MetisError::new(
                ErrorCode::ClinicalInterlockBlocked,
                "Audit sequence exhausted",
                "REQ-METIS-AUDIT-001",
            )
        })?;
        let mut entry = AuditRecord {
            sequence_id,
            timestamp_millis,
            actor_id,
            event,
            outcome,
            prev_hash: self.last_hash,
            record_hash: [0; 32],
        };
        entry.record_hash = entry.compute_hash();
        if self.records.len() == AUDIT_CAPACITY {
            let evicted = self.records.pop_front().expect("invariant: ring is full");
            self.checkpoint = evicted.record_hash;
            self.checkpoint_sequence = evicted.sequence_id;
        }
        self.last_hash = entry.record_hash;
        self.records.push_back(entry);
        self.next_sequence = next;
        Ok(())
    }

    /// Verifies retained ordering, field digests, and the final chain head.
    ///
    /// # Errors
    /// Returns a checksum error if a retained field, sequence, or head diverges.
    pub fn verify_chain(&self) -> Result<()> {
        let mut previous = self.checkpoint;
        let mut sequence = self.checkpoint_sequence;
        for entry in &self.records {
            let next = sequence.checked_add(1);
            if next != Some(entry.sequence_id)
                || !constant_time_eq_32(&previous, &entry.prev_hash)
                || !constant_time_eq_32(&entry.compute_hash(), &entry.record_hash)
            {
                return Err(MetisError::new(
                    ErrorCode::ChecksumMismatch,
                    "Retained audit record failed integrity verification",
                    "REQ-METIS-AUDIT-001",
                ));
            }
            sequence = entry.sequence_id;
            previous = entry.record_hash;
        }
        if !constant_time_eq_32(&previous, &self.last_hash) {
            return Err(MetisError::new(
                ErrorCode::ChecksumMismatch,
                "Audit head differs from retained chain",
                "REQ-METIS-AUDIT-001",
            ));
        }
        Ok(())
    }

    /// Retained calls in chronological ledger order; mutation stays private.
    #[must_use]
    pub const fn records(&self) -> &VecDeque<AuditRecord> {
        &self.records
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use metis_core::protocol::{EventId, MessageType};

    #[test]
    fn eviction_preserves_sequence_and_detects_field_changes() {
        let mut ledger = AuditLedger::new();
        for sequence in 1..=1025 {
            ledger
                .record(
                    1000,
                    [1; 16],
                    AuditEvent::Processed(RequestIdentity {
                        message_type: MessageType::HeartbeatReq,
                        sequence,
                    }),
                    None,
                )
                .expect("record");
        }
        assert_eq!(ledger.records.len(), AUDIT_CAPACITY);
        assert_eq!(ledger.records.front().expect("front").sequence_id, 2);
        assert_eq!(ledger.records.back().expect("back").sequence_id, 1025);
        assert_eq!(ledger.verify_chain(), Ok(()));
        let identity = RequestIdentity {
            message_type: MessageType::HeartbeatReq,
            sequence: 2,
        };
        for context in [
            FailureContext::Receive,
            FailureContext::Request(identity),
            FailureContext::Handler(identity),
            FailureContext::Response(identity),
            FailureContext::Event(EventId::try_from(7).expect("nonzero event id")),
        ] {
            let mut altered = ledger.clone();
            altered.records.front_mut().expect("front").event = AuditEvent::Failure(context);
            assert_eq!(
                altered.verify_chain().expect_err("stage tamper").code,
                ErrorCode::ChecksumMismatch
            );
        }
        ledger.records.front_mut().expect("front").outcome = Some(ErrorCode::Timeout);
        assert_eq!(
            ledger.verify_chain().expect_err("tamper").code,
            ErrorCode::ChecksumMismatch
        );
    }
}
