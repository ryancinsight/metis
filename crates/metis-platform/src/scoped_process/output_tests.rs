//! Unit tests for the output pipeline: deadline waits, the reader-slot
//! pool and the retained-output bound. No child process runs here; the
//! slot guard serializes these against the process tests that claim slots.

use super::output_reader::LIVE_OUTPUT_READERS;
use super::tests::{SLOT_TEST_GUARD, wait_for_drained_slots};
use super::*;
use std::io::{Read, Write};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
fn collect_waits_only_for_the_time_left_of_the_deadline() {
    let (sender, result) = mpsc::sync_channel::<Result<(), ScopedProcessError>>(1);
    let reader = OutputReader { result };
    let deadline = Duration::from_secs(5);
    // The run began four seconds ago, so one second of the deadline remains.
    let started = Instant::now()
        .checked_sub(Duration::from_secs(4))
        .expect("a clock more than four seconds past its origin");
    let waiting = Instant::now();
    let error = reader
        .collect(started, deadline)
        .expect_err("no output arrives");
    let waited = waiting.elapsed();
    drop(sender);
    assert!(matches!(error, ScopedProcessError::DeadlineExceeded));
    // `recv_timeout` never returns early, and just under one second remains.
    assert!(
        (Duration::from_millis(500)..Duration::from_secs(3)).contains(&waited),
        "waited {waited:?} of a deadline with one second left"
    );
}

#[test]
fn reader_slots_are_bounded_and_released() {
    let _slots = SLOT_TEST_GUARD.lock().expect("slot-test guard");
    wait_for_drained_slots();
    let mut held = Vec::new();
    for _ in 0..MAX_SCOPED_PROCESS_OUTPUT_READERS {
        held.push(ReaderSlot::acquire().expect("slot below the bound"));
    }
    assert!(matches!(
        ReaderSlot::acquire(),
        Err(ScopedProcessError::OutputReadersExhausted)
    ));
    held.pop();
    held.push(ReaderSlot::acquire().expect("a released slot is reusable"));
    held.clear();
    assert_eq!(LIVE_OUTPUT_READERS.load(Ordering::Relaxed), 0);
}

#[test]
fn output_exactly_at_the_bound_is_accepted_and_one_byte_more_is_not() {
    let bound = u64::try_from(MAX_SCOPED_PROCESS_OUTPUT_BYTES).expect("bound fits u64");
    let at_bound = std::io::repeat(b'x').take(bound);
    assert_eq!(
        drain_bounded(at_bound, |_| Ok(())).expect("output at the bound"),
        MAX_SCOPED_PROCESS_OUTPUT_BYTES
    );
    let over = std::io::repeat(b'x').take(bound + 1);
    assert!(matches!(
        drain_bounded(over, |_| Ok(())),
        Err(ScopedProcessError::OutputTooLarge)
    ));
}

#[test]
fn output_beyond_the_bound_fails_without_retaining_it() {
    let (reader, mut writer) = std::io::pipe().expect("pipe");
    let writer_thread = std::thread::spawn(move || {
        let chunk = [b'x'; 4096];
        for _ in 0..=MAX_SCOPED_PROCESS_OUTPUT_BYTES / chunk.len() {
            if writer.write_all(&chunk).is_err() {
                break;
            }
        }
    });
    let mut retained = 0usize;
    let error = drain_bounded(reader, |chunk| {
        retained += chunk.len();
        Ok(())
    })
    .expect_err("bound");
    writer_thread.join().expect("writer");
    assert!(matches!(error, ScopedProcessError::OutputTooLarge));
    assert!(retained <= MAX_SCOPED_PROCESS_OUTPUT_BYTES);
}
