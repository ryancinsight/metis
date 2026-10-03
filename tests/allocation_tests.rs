//! Allocation counts of frame transport.
//!
//! A counting global allocator tallies the allocations of the measuring thread
//! only, so the test runner and other threads cannot perturb a count. Each
//! test asserts the exact count its contract allows next to the value it
//! produced, so a path that allocated less by doing less would fail the
//! value assertions.

use metis_core::error::ErrorCode;
use metis_core::protocol::{FrameHeader, HEADER_SIZE, MAX_PAYLOAD_SIZE, MessageType, build_frame};
use metis_ipc::{IpcTransport, MemoryTransport, StreamTransport, read_frame, write_frame};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    static BYTES: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

fn record(bytes: usize) {
    ALLOCATIONS.with(|count| count.set(count.get() + 1));
    BYTES.with(|total| total.set(total.get() + bytes));
}

#[expect(
    unsafe_code,
    reason = "the GlobalAlloc contract is unsafe; every method forwards unchanged to System"
)]
// SAFETY: each method forwards its arguments, unchanged, to `System`, which
// upholds the `GlobalAlloc` contract; the only addition is a thread-local
// counter that never allocates.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        // SAFETY: the caller's layout is forwarded.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        // SAFETY: the caller's layout is forwarded.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record(new_size);
        // SAFETY: the caller's pointer, layout and size are forwarded.
        unsafe { System.realloc(pointer, layout, new_size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the caller's pointer and layout are forwarded.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Allocator calls and requested bytes of one measured closure.
struct Usage {
    allocations: usize,
    bytes: usize,
}

/// Runs `work` and returns its result with what this thread allocated.
fn usage_during<R>(work: impl FnOnce() -> R) -> (R, Usage) {
    let before = (ALLOCATIONS.with(Cell::get), BYTES.with(Cell::get));
    let result = work();
    let usage = Usage {
        allocations: ALLOCATIONS.with(Cell::get) - before.0,
        bytes: BYTES.with(Cell::get) - before.1,
    };
    (result, usage)
}

/// Runs `work` and returns its result with the allocator calls this thread made.
fn allocations_during<R>(work: impl FnOnce() -> R) -> (R, usize) {
    let (result, usage) = usage_during(work);
    (result, usage.allocations)
}

/// A payload whose bytes depend on its length and position, so a wrong slice
/// or a stale buffer tail changes the value.
fn payload(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| u8::try_from((index * 7 + length) % 251).expect("remainder fits a byte"))
        .collect()
}

#[test]
fn a_reused_buffer_builds_frames_without_allocating() {
    let largest = payload(MAX_PAYLOAD_SIZE);
    let mut frame = Vec::new();
    build_frame(MessageType::HeartbeatReq, 1, &largest, &mut frame).expect("largest frame");
    assert_eq!(frame.len(), HEADER_SIZE + MAX_PAYLOAD_SIZE);

    let lengths = [0, 1, 17, 4096, MAX_PAYLOAD_SIZE, 3];
    let (sizes, allocations) = allocations_during(|| {
        let mut sizes = [0; 6];
        for ((sequence, length), size) in (2..).zip(lengths).zip(&mut sizes) {
            build_frame(
                MessageType::HeartbeatReq,
                sequence,
                &largest[..length],
                &mut frame,
            )
            .expect("frame");
            *size = frame.len();
        }
        sizes
    });
    assert_eq!(allocations, 0);
    assert_eq!(sizes, lengths.map(|length| HEADER_SIZE + length));

    // The final frame is the 3-byte payload: the previous, longer frames left
    // no tail behind it.
    let (header, decoded) = read_frame(&mut frame.as_slice()).expect("decodes");
    assert_eq!(header.sequence_id, 7);
    assert_eq!(decoded, &largest[..3]);

    // A refused frame leaves the buffer empty rather than holding the previous
    // frame, which a caller could otherwise resend.
    let oversized = vec![0; MAX_PAYLOAD_SIZE + 1];
    let error = build_frame(MessageType::HeartbeatReq, 9, &oversized, &mut frame)
        .expect_err("oversized payload");
    assert_eq!(error.code, ErrorCode::PayloadTooLarge);
    assert!(frame.is_empty());
}

#[test]
fn a_stream_transport_sends_without_allocating_and_preserves_every_frame() {
    let payloads = [MAX_PAYLOAD_SIZE, 0, 5, 1024, 65_000, 2].map(payload);
    let mut sink = Vec::with_capacity(payloads.iter().map(|body| HEADER_SIZE + body.len()).sum());
    {
        let mut transport = StreamTransport::new(std::io::empty(), &mut sink);
        // The first frame is the largest, so it sizes the encode buffer.
        transport
            .send_message(MessageType::HeartbeatReq, 1, &payloads[0])
            .expect("first frame");
        let ((), allocations) = allocations_during(|| {
            for (sequence, body) in (2..).zip(&payloads[1..]) {
                transport
                    .send_message(MessageType::HeartbeatReq, sequence, body)
                    .expect("frame");
            }
        });
        assert_eq!(allocations, 0);
    }
    let mut wire = sink.as_slice();
    for (sequence, body) in (1..).zip(&payloads) {
        let (header, decoded) = read_frame(&mut wire).expect("frame");
        assert_eq!(header.sequence_id, sequence);
        assert_eq!(header.msg_type, MessageType::HeartbeatReq);
        assert_eq!(&decoded, body);
    }
    assert!(wire.is_empty());
}

#[test]
fn write_frame_reuses_the_callers_buffer() {
    let largest = payload(MAX_PAYLOAD_SIZE);
    let mut output = Vec::with_capacity(2 * (HEADER_SIZE + MAX_PAYLOAD_SIZE));
    let mut frame = Vec::new();
    write_frame(
        &mut output,
        MessageType::HeartbeatResp,
        1,
        &largest,
        &mut frame,
    )
    .expect("warm frame");
    let (result, allocations) = allocations_during(|| {
        write_frame(
            &mut output,
            MessageType::HeartbeatResp,
            2,
            b"second",
            &mut frame,
        )
    });
    result.expect("second frame");
    assert_eq!(allocations, 0);
    let mut wire = output.as_slice();
    assert_eq!(read_frame(&mut wire).expect("first").1, largest);
    let (header, body) = read_frame(&mut wire).expect("second");
    assert_eq!((header.sequence_id, body.as_slice()), (2, &b"second"[..]));
}

#[test]
fn receiving_a_frame_allocates_its_payload_once() {
    for length in [1, 17, 4096, MAX_PAYLOAD_SIZE] {
        let body = payload(length);
        let mut wire = Vec::new();
        build_frame(MessageType::ClinicalCalcReq, 5, &body, &mut wire).expect("frame");
        let (received, allocations) = allocations_during(|| read_frame(&mut wire.as_slice()));
        let (header, decoded) = received.expect("frame decodes");
        assert_eq!(allocations, 1, "payload of {length} bytes");
        assert_eq!(header.sequence_id, 5);
        assert_eq!(decoded, body);
    }
    let mut empty = Vec::new();
    build_frame(MessageType::HeartbeatReq, 6, &[], &mut empty).expect("empty frame");
    let (received, allocations) = allocations_during(|| read_frame(&mut empty.as_slice()));
    assert_eq!(received.expect("empty payload").1, b"");
    assert_eq!(allocations, 0);
}

/// A header announcing `length` payload bytes, with no payload behind it.
fn header_claiming(length: usize) -> [u8; HEADER_SIZE] {
    FrameHeader {
        msg_type: MessageType::HeartbeatReq,
        sequence_id: 1,
        payload_crc32: 0,
        payload_len: u32::try_from(length).expect("claimed length fits u32"),
    }
    .encode()
}

#[test]
fn a_hostile_length_field_never_sizes_an_allocation() {
    // Header validation alone produces the rejection, so a rejected read must
    // allocate exactly what that validation allocates, the typed error, and
    // nothing sized by the claimed length.
    for claimed in [
        usize::try_from(u32::MAX).expect("u32 fits usize"),
        1 << 31,
        1 << 20,
        MAX_PAYLOAD_SIZE + 1,
    ] {
        let header = header_claiming(claimed);
        let (decoded, validation) = usage_during(|| FrameHeader::decode(&header));
        assert_eq!(
            decoded.expect_err("length beyond the wire bound").code,
            ErrorCode::PayloadTooLarge
        );
        let (result, read) = usage_during(|| read_frame(&mut header.as_slice()));
        assert_eq!(
            result.expect_err("length beyond the wire bound").code,
            ErrorCode::PayloadTooLarge
        );
        assert_eq!(read.bytes, validation.bytes, "claimed length {claimed}");
    }

    // A claim at the bound reserves exactly the payload it announces, then the
    // missing bytes surface as truncation. The truncation error costs what it
    // costs for a one-byte claim, so only the reservation differs.
    let truncated = |claimed: usize| {
        let header = header_claiming(claimed);
        let (result, usage) = usage_during(|| read_frame(&mut header.as_slice()));
        assert_eq!(
            result.expect_err("missing payload").code,
            ErrorCode::FrameTruncated
        );
        usage.bytes - claimed
    };
    assert_eq!(truncated(MAX_PAYLOAD_SIZE), truncated(1));
}

#[test]
fn a_memory_transport_decodes_the_received_frame_in_place() {
    let (mut near, mut far) = MemoryTransport::pair();
    for (sequence, length) in (1..).zip([0, 3, 4096, MAX_PAYLOAD_SIZE]) {
        let body = payload(length);
        near.send_message(MessageType::HeartbeatReq, sequence, &body)
            .expect("queued");
        let (message, allocations) = allocations_during(|| far.recv_message());
        let (header, decoded) = message.expect("frame");
        assert_eq!(allocations, 0, "payload of {length} bytes");
        assert_eq!(header.sequence_id, sequence);
        assert_eq!(decoded, body);
    }
}
