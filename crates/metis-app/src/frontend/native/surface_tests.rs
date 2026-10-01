//! A batch of native resize events allocates only the surface it ends on.

use super::NativeForm;
use metis_core::ErrorCode;
use metis_frontend::FrontendApp;
use metis_ipc::MemoryTransport;
use metis_platform::Damage;
use metis_platform::framebuffer::MAX_PIXELS;
use metis_platform::native::{NativeApplication, NativeFlow, WindowEvent};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// Byte sizes of the 640x480, 700x500 and 800x600 surfaces; a surface is
/// reserved at its exact size, so a request of one of them is one surface.
const SURFACE_BYTES: [usize; 3] = [640 * 480 * 4, 700 * 500 * 4, 800 * 600 * 4];

thread_local! {
    /// Requests per watched size made on this thread, so concurrent tests
    /// cannot count each other's allocations.
    static REQUESTS: Cell<[usize; 3]> = const { Cell::new([0; 3]) };
}

fn record(bytes: usize) {
    if let Some(slot) = SURFACE_BYTES.iter().position(|&watched| watched == bytes) {
        REQUESTS.with(|requests| {
            let mut counts = requests.get();
            counts[slot] += 1;
            requests.set(counts);
        });
    }
}

struct CountingAllocator;

#[expect(
    unsafe_code,
    reason = "A global allocator must implement the unsafe allocator contract; every method forwards its arguments unchanged to the system allocator"
)]
// SAFETY: each method forwards to `System` with the caller's layout and
// pointer, so the `GlobalAlloc` contract holds exactly as it does for `System`.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        // SAFETY: the caller upholds the `GlobalAlloc::alloc` contract for `layout`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        // SAFETY: the caller upholds the `GlobalAlloc::alloc_zeroed` contract for `layout`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller passes a pointer this allocator returned for `layout`.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record(new_size);
        // SAFETY: the caller upholds the `GlobalAlloc::realloc` contract for the
        // pointer, its layout and the new size.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn resized(width: u32, height: u32) -> WindowEvent {
    WindowEvent::Resized { width, height }
}

fn form(width: u32, height: u32) -> NativeForm<MemoryTransport> {
    let (transport, _peer) = MemoryTransport::pair();
    let mut app = FrontendApp::new(transport, width, height).expect("form");
    assert_eq!(app.take_damage(), Damage::Full);
    NativeForm {
        app,
        pid: 1,
        patient_id: String::new(),
        focused: true,
    }
}

#[test]
fn a_resize_batch_allocates_one_surface_and_presents_the_last_size() {
    let mut form = form(400, 300);
    let reference = form_pixels(800, 600);
    REQUESTS.set([0; 3]);
    let flow = form
        .handle_events(&[resized(640, 480), resized(700, 500), resized(800, 600)])
        .expect("resize batch");
    assert_eq!(REQUESTS.get(), [0, 0, 1]);
    assert!(matches!(flow, NativeFlow::Continue { repaint: true }));
    assert_eq!(form.framebuffer().width(), 800);
    assert_eq!(form.framebuffer().height(), 600);
    assert_eq!(form.take_damage(), Damage::Full);
    assert_eq!(form.framebuffer().pixels(), reference.as_slice());
}

fn form_pixels(width: u32, height: u32) -> Vec<u32> {
    form(width, height).framebuffer().pixels().to_vec()
}

/// One event batch and what it leaves behind.
struct Batch {
    case: &'static str,
    events: Vec<WindowEvent>,
    /// Surfaces allocated at each of [`SURFACE_BYTES`].
    surfaces: [usize; 3],
    size: (u32, u32),
}

#[test]
fn only_a_directly_following_usable_size_supersedes_a_resize() {
    let batches = [
        Batch {
            case: "an unusable size ends the run",
            events: vec![resized(640, 480), resized(0, 0)],
            surfaces: [1, 0, 0],
            size: (640, 480),
        },
        Batch {
            case: "a zero height ends the run",
            events: vec![resized(640, 480), resized(0, 300)],
            surfaces: [1, 0, 0],
            size: (640, 480),
        },
        Batch {
            case: "a zero width ends the run",
            events: vec![resized(640, 480), resized(300, 0)],
            surfaces: [1, 0, 0],
            size: (640, 480),
        },
        Batch {
            case: "an unusable size between usable ones",
            events: vec![resized(640, 480), resized(0, 300), resized(700, 500)],
            surfaces: [0, 1, 0],
            size: (700, 500),
        },
        Batch {
            case: "another event between resizes",
            events: vec![
                resized(640, 480),
                WindowEvent::FocusGained,
                resized(700, 500),
            ],
            surfaces: [1, 1, 0],
            size: (700, 500),
        },
        Batch {
            case: "a run ending on the current size",
            events: vec![resized(640, 480), resized(400, 300)],
            surfaces: [0, 0, 0],
            size: (400, 300),
        },
        Batch {
            case: "a single resize",
            events: vec![resized(700, 500)],
            surfaces: [0, 1, 0],
            size: (700, 500),
        },
    ];
    for Batch {
        case,
        events,
        surfaces,
        size: (width, height),
    } in batches
    {
        let mut form = form(400, 300);
        REQUESTS.set([0; 3]);
        form.handle_events(&events).expect("resize batch");
        assert_eq!(REQUESTS.get(), surfaces, "{case}");
        assert_eq!(form.framebuffer().width(), width, "{case}");
        assert_eq!(form.framebuffer().height(), height, "{case}");
    }
}

/// One row taller than the largest surface the framebuffer accepts.
fn oversize() -> WindowEvent {
    let width = 4_096;
    let height = 4_097;
    assert!(
        u64::from(width) * u64::from(height)
            > u64::try_from(MAX_PIXELS).expect("invariant: the pixel limit fits u64")
    );
    resized(width, height)
}

#[test]
fn a_superseded_oversize_size_is_never_allocated_so_its_error_is_not_observed() {
    let mut form = form(400, 300);
    REQUESTS.set([0; 3]);
    let flow = form
        .handle_events(&[oversize(), resized(640, 480)])
        .expect("the oversize size is skipped");
    assert!(matches!(flow, NativeFlow::Continue { repaint: true }));
    assert_eq!(REQUESTS.get(), [1, 0, 0]);
    assert_eq!(form.framebuffer().width(), 640);
    assert_eq!(form.framebuffer().height(), 480);
}

#[test]
fn a_trailing_oversize_size_fails_without_applying_the_sizes_it_supersedes() {
    let mut form = form(400, 300);
    REQUESTS.set([0; 3]);
    let error = form
        .handle_events(&[resized(640, 480), oversize()])
        .expect_err("the oversize size is allocated");
    assert_eq!(error.code, ErrorCode::SurfaceAllocationError);
    assert_eq!(REQUESTS.get(), [0, 0, 0]);
    assert_eq!(form.framebuffer().width(), 400);
    assert_eq!(form.framebuffer().height(), 300);
}
