//! Authenticated, bounded audit snapshots for native backend recovery.

use super::{AUDIT_CAPACITY, AuditEvent, AuditLedger, AuditRecord, EventDescriptor};
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{EventId, MessageType};
use metis_ipc::server::{FailureContext, RequestIdentity};
use moirai_crypto::{constant_time_eq_32, hmac_sha256};
use std::collections::VecDeque;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

mod cursor;
mod permissions;
use cursor::Cursor;
use permissions::{restrict_directory, restrict_file};

const MAGIC: [u8; 16] = *b"METIS-AUDIT-V1\0\0";
const FORMAT_VERSION: u16 = 1;
const HEADER_SIZE: usize = 112;
const RECORD_SIZE: usize = 110;
const MAC_SIZE: usize = 32;
const SLOT_NAMES: [&str; 2] = ["audit-0.bin", "audit-1.bin"];

/// Maximum bytes accepted for one authenticated audit snapshot.
pub const MAX_SNAPSHOT_BYTES: usize = HEADER_SIZE + AUDIT_CAPACITY * RECORD_SIZE + MAC_SIZE;

/// Symmetric key used to authenticate audit snapshots.
///
/// The key stays in the host's secret store or process configuration. It is
/// separate from the ephemeral IPC session key and is never written to a
/// snapshot.
#[derive(Clone)]
pub struct AuditCheckpointKey([u8; 32]);

impl AuditCheckpointKey {
    /// Creates a checkpoint key from caller-owned secret bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for AuditCheckpointKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AuditCheckpointKey(REDACTED)")
    }
}

/// Native store that keeps two authenticated, bounded snapshot slots.
///
/// One slot remains valid while the other is rewritten. A torn or tampered
/// slot fails closed during [`Self::load`]; the service never substitutes an
/// unverified record set.
pub struct FileAuditStore {
    directory: PathBuf,
    key: AuditCheckpointKey,
    generation: u64,
    slot: Option<u8>,
}

impl fmt::Debug for FileAuditStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileAuditStore")
            .field("generation", &self.generation)
            .field("slot", &self.slot)
            .finish_non_exhaustive()
    }
}

impl FileAuditStore {
    /// Opens a bounded native snapshot directory.
    ///
    /// The directory is created when absent. Existing snapshots are validated
    /// by [`Self::load`] so callers can choose when startup recovery occurs.
    ///
    /// # Errors
    /// Returns [`ErrorCode::IoError`] when the directory cannot be created.
    pub fn open(directory: impl AsRef<Path>, key: AuditCheckpointKey) -> Result<Self> {
        let directory = directory.as_ref().to_path_buf();
        fs::create_dir_all(&directory).map_err(|_| storage_error(ErrorCode::IoError))?;
        restrict_directory(&directory)?;
        Ok(Self {
            directory,
            key,
            generation: 0,
            slot: None,
        })
    }

    /// Loads the newest authenticated snapshot, or an empty ledger when none exists.
    ///
    /// Both slots are bounded before their bytes are read. If an existing slot
    /// is malformed, truncated, duplicated, or fails authentication, loading
    /// returns a typed error instead of silently rolling back to older data.
    ///
    /// # Errors
    /// Returns a typed integrity, framing, resource, or IO error when recovery
    /// cannot establish one authenticated ledger.
    pub fn load(&mut self) -> Result<AuditLedger> {
        let mut snapshots = Vec::with_capacity(SLOT_NAMES.len());
        for (index, name) in SLOT_NAMES.iter().enumerate() {
            let path = self.directory.join(name);
            let metadata = match fs::metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => return Err(storage_error(ErrorCode::IoError)),
            };
            if metadata.len() > u64::try_from(MAX_SNAPSHOT_BYTES).unwrap_or(u64::MAX) {
                return Err(storage_error(ErrorCode::PayloadTooLarge));
            }
            let snapshot = read_snapshot(&path, &self.key)?;
            let slot = u8::try_from(index).map_err(|_| storage_error(ErrorCode::IoError))?;
            snapshots.push((slot, snapshot));
        }

        if snapshots.is_empty() {
            self.generation = 0;
            self.slot = None;
            return Ok(AuditLedger::new());
        }
        snapshots.sort_by_key(|(_, snapshot)| snapshot.generation);
        if snapshots.len() == 2 {
            let first = snapshots[0].1.generation;
            let second = snapshots[1].1.generation;
            if first == second || second != first.saturating_add(1) {
                return Err(storage_error(ErrorCode::ChecksumMismatch));
            }
        }
        let Some((slot, snapshot)) = snapshots.pop() else {
            return Err(storage_error(ErrorCode::MalformedPayload));
        };
        self.generation = snapshot.generation;
        self.slot = Some(slot);
        Ok(snapshot.ledger)
    }

    /// Persists a verified ledger into the inactive snapshot slot.
    ///
    /// The candidate ledger is checked before any write. The generation and
    /// active slot advance only after the complete snapshot has been flushed
    /// and synchronized, so a failed write leaves the caller's ledger usable.
    ///
    /// # Errors
    /// Returns a typed integrity, resource, sequence, or IO error.
    pub fn persist(&mut self, ledger: &AuditLedger) -> Result<()> {
        ledger.verify_chain()?;
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| storage_error(ErrorCode::ClinicalInterlockBlocked))?;
        let bytes = encode_snapshot(ledger, generation, &self.key)?;
        let slot = self.slot.map_or(0, |active| 1_u8.saturating_sub(active));
        let path = self.directory.join(SLOT_NAMES[usize::from(slot)]);
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .map_err(|_| storage_error(ErrorCode::IoError))?;
        file.write_all(&bytes)
            .map_err(|_| storage_error(ErrorCode::IoError))?;
        file.sync_all()
            .map_err(|_| storage_error(ErrorCode::IoError))?;
        restrict_file(&file)?;
        self.generation = generation;
        self.slot = Some(slot);
        Ok(())
    }

    /// Returns the last committed snapshot generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

struct Snapshot {
    generation: u64,
    ledger: AuditLedger,
}

fn storage_error(code: ErrorCode) -> MetisError {
    let message = match code {
        ErrorCode::ChecksumMismatch => "Audit snapshot failed integrity verification",
        ErrorCode::FrameTruncated => "Audit snapshot ended before its declared fields",
        ErrorCode::MalformedPayload => "Audit snapshot contains an invalid field",
        ErrorCode::PayloadTooLarge => "Audit snapshot exceeds its fixed resource bound",
        ErrorCode::IoError => "Audit snapshot IO operation failed",
        ErrorCode::ClinicalInterlockBlocked => "Audit snapshot generation exhausted",
        _ => "Audit snapshot recovery failed",
    };
    MetisError::new(code, message, "REQ-METIS-AUDIT-001")
}

fn read_snapshot(path: &Path, key: &AuditCheckpointKey) -> Result<Snapshot> {
    let file = File::open(path).map_err(|_| storage_error(ErrorCode::IoError))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve(MAX_SNAPSHOT_BYTES.saturating_add(1))
        .map_err(|_| storage_error(ErrorCode::IoError))?;
    file.take(
        u64::try_from(MAX_SNAPSHOT_BYTES)
            .unwrap_or(u64::MAX)
            .saturating_add(1),
    )
    .read_to_end(&mut bytes)
    .map_err(|_| storage_error(ErrorCode::IoError))?;
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err(storage_error(ErrorCode::PayloadTooLarge));
    }
    decode_snapshot(&bytes, key)
}

fn encode_snapshot(
    ledger: &AuditLedger,
    generation: u64,
    key: &AuditCheckpointKey,
) -> Result<Vec<u8>> {
    let count = u16::try_from(ledger.records.len())
        .map_err(|_| storage_error(ErrorCode::PayloadTooLarge))?;
    if ledger.records.len() > AUDIT_CAPACITY {
        return Err(storage_error(ErrorCode::PayloadTooLarge));
    }
    let body_len = ledger
        .records
        .len()
        .checked_mul(RECORD_SIZE)
        .ok_or_else(|| storage_error(ErrorCode::PayloadTooLarge))?;
    let total_len = HEADER_SIZE
        .checked_add(body_len)
        .and_then(|length| length.checked_add(MAC_SIZE))
        .ok_or_else(|| storage_error(ErrorCode::PayloadTooLarge))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve(total_len)
        .map_err(|_| storage_error(ErrorCode::IoError))?;
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
    bytes.extend_from_slice(&generation.to_be_bytes());
    bytes.extend_from_slice(&count.to_be_bytes());
    bytes.extend_from_slice(&ledger.checkpoint_sequence.to_be_bytes());
    bytes.extend_from_slice(&ledger.checkpoint);
    bytes.extend_from_slice(&ledger.last_hash);
    bytes.extend_from_slice(&ledger.next_sequence.to_be_bytes());
    bytes.extend_from_slice(
        &u32::try_from(body_len)
            .map_err(|_| storage_error(ErrorCode::PayloadTooLarge))?
            .to_be_bytes(),
    );
    for record in &ledger.records {
        encode_record(record, &mut bytes)?;
    }
    let mac = hmac_sha256(&key.0, &bytes);
    bytes.extend_from_slice(&mac);
    debug_assert_eq!(bytes.len(), total_len);
    Ok(bytes)
}

fn encode_record(record: &AuditRecord, bytes: &mut Vec<u8>) -> Result<()> {
    bytes.extend_from_slice(&record.sequence_id.to_be_bytes());
    bytes.extend_from_slice(&record.timestamp_millis.to_be_bytes());
    bytes.extend_from_slice(&record.actor_id);
    let descriptor = super::event_descriptor(record.event);
    let (tag, payload) = match descriptor {
        EventDescriptor::Empty(tag) if tag == 1 => (tag, [0; 10]),
        EventDescriptor::Identity(tag, identity) => {
            let mut payload = [0; 10];
            payload[..2].copy_from_slice(&(identity.message_type as u16).to_be_bytes());
            payload[2..].copy_from_slice(&identity.sequence.to_be_bytes());
            (tag, payload)
        }
        EventDescriptor::Event(tag, event_id) => {
            let mut payload = [0; 10];
            payload[..8].copy_from_slice(&event_id.get().to_be_bytes());
            (tag, payload)
        }
        EventDescriptor::Empty(_) => return Err(storage_error(ErrorCode::MalformedPayload)),
    };
    bytes.push(tag);
    bytes.extend_from_slice(&payload);
    let (present, code) = record
        .outcome
        .map_or((0_u8, 0_u16), |code| (1, code as u16));
    bytes.push(present);
    bytes.extend_from_slice(&code.to_be_bytes());
    bytes.extend_from_slice(&record.prev_hash);
    bytes.extend_from_slice(&record.record_hash);
    Ok(())
}

fn decode_snapshot(bytes: &[u8], key: &AuditCheckpointKey) -> Result<Snapshot> {
    if bytes.len() < HEADER_SIZE + MAC_SIZE {
        return Err(storage_error(ErrorCode::FrameTruncated));
    }
    let mut cursor = Cursor::new(bytes);
    if cursor.take(MAGIC.len())? != MAGIC {
        return Err(storage_error(ErrorCode::MagicMismatch));
    }
    if cursor.u16()? != FORMAT_VERSION {
        return Err(storage_error(ErrorCode::VersionMismatch));
    }
    let generation = cursor.u64()?;
    if generation == 0 {
        return Err(storage_error(ErrorCode::MalformedPayload));
    }
    let count = usize::from(cursor.u16()?);
    if count > AUDIT_CAPACITY {
        return Err(storage_error(ErrorCode::PayloadTooLarge));
    }
    let checkpoint_sequence = cursor.u64()?;
    let checkpoint = cursor.array::<32>()?;
    let last_hash = cursor.array::<32>()?;
    let next_sequence = cursor.u64()?;
    let body_len =
        usize::try_from(cursor.u32()?).map_err(|_| storage_error(ErrorCode::PayloadTooLarge))?;
    let expected_len = HEADER_SIZE
        .checked_add(body_len)
        .and_then(|length| length.checked_add(MAC_SIZE))
        .ok_or_else(|| storage_error(ErrorCode::PayloadTooLarge))?;
    if body_len != count.saturating_mul(RECORD_SIZE) || expected_len != bytes.len() {
        return Err(storage_error(ErrorCode::MalformedPayload));
    }
    let mac_offset = bytes
        .len()
        .checked_sub(MAC_SIZE)
        .ok_or_else(|| storage_error(ErrorCode::FrameTruncated))?;
    let expected_mac = hmac_sha256(&key.0, &bytes[..mac_offset]);
    let supplied_mac = bytes
        .get(mac_offset..)
        .ok_or_else(|| storage_error(ErrorCode::FrameTruncated))?;
    let supplied_mac: [u8; 32] = supplied_mac
        .try_into()
        .map_err(|_| storage_error(ErrorCode::FrameTruncated))?;
    if !constant_time_eq_32(&expected_mac, &supplied_mac) {
        return Err(storage_error(ErrorCode::ChecksumMismatch));
    }
    let mut ledger = AuditLedger {
        records: VecDeque::with_capacity(AUDIT_CAPACITY),
        checkpoint,
        checkpoint_sequence,
        last_hash,
        next_sequence,
    };
    for _ in 0..count {
        ledger.records.push_back(decode_record(&mut cursor)?);
    }
    if cursor.offset != mac_offset {
        return Err(storage_error(ErrorCode::MalformedPayload));
    }
    validate_ledger_header(&ledger)?;
    ledger.verify_chain()?;
    Ok(Snapshot { generation, ledger })
}

fn validate_ledger_header(ledger: &AuditLedger) -> Result<()> {
    let genesis = super::genesis_hash();
    if ledger.records.is_empty() {
        if ledger.checkpoint_sequence != 0
            || ledger.checkpoint != genesis
            || ledger.last_hash != genesis
            || ledger.next_sequence != 1
        {
            return Err(storage_error(ErrorCode::MalformedPayload));
        }
        return Ok(());
    }
    if ledger
        .records
        .back()
        .and_then(|record| record.sequence_id.checked_add(1))
        != Some(ledger.next_sequence)
    {
        return Err(storage_error(ErrorCode::MalformedPayload));
    }
    if ledger.records.len() < AUDIT_CAPACITY
        && (ledger.checkpoint_sequence != 0 || ledger.checkpoint != genesis)
    {
        return Err(storage_error(ErrorCode::MalformedPayload));
    }
    Ok(())
}

fn decode_record(cursor: &mut Cursor<'_>) -> Result<AuditRecord> {
    let sequence_id = cursor.u64()?;
    let timestamp_millis = cursor.u64()?;
    let actor_id = cursor.array::<16>()?;
    let tag = cursor.byte()?;
    let payload = cursor.take(10)?;
    let event = decode_event(tag, payload)?;
    let present = cursor.byte()?;
    let outcome_code = cursor.u16()?;
    let outcome = match present {
        0 if outcome_code == 0 => None,
        1 => Some(
            ErrorCode::from_wire(outcome_code)
                .ok_or_else(|| storage_error(ErrorCode::MalformedPayload))?,
        ),
        _ => return Err(storage_error(ErrorCode::MalformedPayload)),
    };
    let prev_hash = cursor.array::<32>()?;
    let record_hash = cursor.array::<32>()?;
    Ok(AuditRecord {
        sequence_id,
        timestamp_millis,
        actor_id,
        event,
        outcome,
        prev_hash,
        record_hash,
    })
}

fn decode_event(tag: u8, payload: &[u8]) -> Result<AuditEvent> {
    let identity = || -> Result<RequestIdentity> {
        let message_type = MessageType::from_wire(u16::from_be_bytes([payload[0], payload[1]]))
            .ok_or_else(|| storage_error(ErrorCode::MalformedPayload))?;
        Ok(RequestIdentity {
            message_type,
            sequence: u64::from_be_bytes(
                payload[2..]
                    .try_into()
                    .map_err(|_| storage_error(ErrorCode::MalformedPayload))?,
            ),
        })
    };
    match tag {
        0 => Ok(AuditEvent::Processed(identity()?)),
        1 if payload.iter().all(|byte| *byte == 0) => {
            Ok(AuditEvent::Failure(FailureContext::Receive))
        }
        2 => Ok(AuditEvent::Failure(FailureContext::Request(identity()?))),
        3 => Ok(AuditEvent::Failure(FailureContext::Handler(identity()?))),
        4 => Ok(AuditEvent::Failure(FailureContext::Response(identity()?))),
        5 if payload[8..].iter().all(|byte| *byte == 0) => {
            let value = u64::from_be_bytes(
                payload[..8]
                    .try_into()
                    .map_err(|_| storage_error(ErrorCode::MalformedPayload))?,
            );
            let event_id =
                EventId::try_from(value).map_err(|_| storage_error(ErrorCode::MalformedPayload))?;
            Ok(AuditEvent::Failure(FailureContext::Event(event_id)))
        }
        _ => Err(storage_error(ErrorCode::MalformedPayload)),
    }
}

#[cfg(test)]
#[path = "persistence/tests.rs"]
mod tests;
