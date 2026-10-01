//! Bounded, deadline-aware capture of child output pipes.
//!
//! Each pipe is drained by a detached reader thread that holds a claim on one of
//! [`MAX_SCOPED_PROCESS_OUTPUT_READERS`] process-wide slots, so output volume and
//! reader lifetime stay finite even when a descendant keeps a pipe open.

use super::{
    MAX_SCOPED_PROCESS_OUTPUT_BYTES, MAX_SCOPED_PROCESS_OUTPUT_READERS, ScopedProcessError,
};
use std::{
    fs::File,
    io::{ErrorKind, Read},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    time::{Duration, Instant},
};

const READ_CHUNK_BYTES: usize = 8 * 1024;
pub(super) static LIVE_OUTPUT_READERS: AtomicUsize = AtomicUsize::new(0);

/// Reader thread for one output pipe whose result is awaited under a deadline.
///
/// The thread is detached rather than scoped: a scope joins readers that a
/// descendant still feeds, so the caller could not return at its deadline. The
/// [`LIVE_OUTPUT_READERS`] bound keeps stalled readers finite.
pub(super) struct OutputReader<T> {
    pub(super) result: Receiver<Result<T, ScopedProcessError>>,
}

impl<T: Send + 'static> OutputReader<T> {
    pub(super) fn spawn(
        pipe: File,
        slot: ReaderSlot,
        read: impl FnOnce(File) -> Result<T, ScopedProcessError> + Send + 'static,
    ) -> Result<Self, ScopedProcessError> {
        let (sender, result) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("metis-scoped-process-output".to_owned())
            .spawn(move || {
                let _slot = slot;
                match sender.send(read(pipe)) {
                    Ok(()) => {}
                    // The caller drops the receiver when it abandons the run at its deadline.
                    Err(mpsc::SendError(_abandoned)) => {}
                }
            })?;
        Ok(Self { result })
    }

    pub(super) fn collect(
        self,
        started: Instant,
        deadline: Duration,
    ) -> Result<T, ScopedProcessError> {
        match self
            .result
            .recv_timeout(deadline.saturating_sub(started.elapsed()))
        {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => Err(ScopedProcessError::DeadlineExceeded),
            Err(RecvTimeoutError::Disconnected) => Err(ScopedProcessError::ReaderPanicked),
        }
    }
}

/// Claim on one of [`MAX_SCOPED_PROCESS_OUTPUT_READERS`] live reader threads.
pub(super) struct ReaderSlot;

impl ReaderSlot {
    pub(super) fn acquire() -> Result<Self, ScopedProcessError> {
        // The counter publishes no other data, so the updates need no ordering.
        LIVE_OUTPUT_READERS
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |live| {
                (live < MAX_SCOPED_PROCESS_OUTPUT_READERS).then_some(live + 1)
            })
            .map(|_| Self)
            .map_err(|_| ScopedProcessError::OutputReadersExhausted)
    }
}

impl Drop for ReaderSlot {
    fn drop(&mut self) {
        LIVE_OUTPUT_READERS.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Feeds every chunk of `reader` to `accept` and returns the byte total.
///
/// Fails with [`ScopedProcessError::OutputTooLarge`] as soon as the total would
/// exceed [`MAX_SCOPED_PROCESS_OUTPUT_BYTES`], without reading further.
pub(super) fn drain_bounded(
    mut reader: impl Read,
    mut accept: impl FnMut(&[u8]) -> Result<(), ScopedProcessError>,
) -> Result<usize, ScopedProcessError> {
    let mut chunk = [0u8; READ_CHUNK_BYTES];
    let mut total = 0usize;
    loop {
        let read = match reader.read(&mut chunk) {
            Ok(0) => return Ok(total),
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        };
        total = total
            .checked_add(read)
            .filter(|total| *total <= MAX_SCOPED_PROCESS_OUTPUT_BYTES)
            .ok_or(ScopedProcessError::OutputTooLarge)?;
        accept(&chunk[..read])?;
    }
}
