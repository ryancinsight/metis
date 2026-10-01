//! A batch of resizes allocates one surface, sized by the last of them.

use super::NativeForm;
use metis_frontend::FrontendApp;
use metis_ipc::MemoryTransport;
use metis_platform::DisplayScale;
use metis_platform::native::{NativeApplication, NativeFlow, WindowEvent};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// Smallest allocation counted as a surface: every surface below is at least
/// 640 x 480 pixels of four bytes, and nothing else the form allocates is
/// near a megabyte.
const SURFACE_BYTES_MIN: usize = 1_000_000;

thread_local! {
    static SURFACES: Cell<usize> = const { Cell::new(0) };
    static LAST_SURFACE_BYTES: Cell<usize> = const { Cell::new(0) };
}

/// The system allocator, counting surface-sized requests of the current
/// thread so concurrently running tests cannot disturb a measurement.
struct CountingAllocator;

// SAFETY: every call forwards unchanged to the system allocator, whose
// contract the caller already upholds; the only addition records a size in
// const-initialized thread-local cells, which neither allocate nor run
// destructors, so the allocator never re-enters itself.
#[expect(unsafe_code, reason = "a counting global allocator is an unsafe trait")]
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() >= SURFACE_BYTES_MIN {
            SURFACES.with(|count| count.set(count.get() + 1));
            LAST_SURFACE_BYTES.with(|bytes| bytes.set(layout.size()));
        }
        // SAFETY: the caller upholds `GlobalAlloc::alloc` for `layout`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the caller passes a pointer this allocator returned for
        // `layout`, and `alloc` returned `System`'s.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Surface-sized allocations `run` makes on this thread, and the byte size of
/// the last one.
fn surfaces_allocated(run: impl FnOnce()) -> (usize, usize) {
    SURFACES.with(|count| count.set(0));
    LAST_SURFACE_BYTES.with(|bytes| bytes.set(0));
    run();
    (SURFACES.with(Cell::get), LAST_SURFACE_BYTES.with(Cell::get))
}

fn form(width: u32, height: u32) -> NativeForm<MemoryTransport> {
    let (transport, _peer) = MemoryTransport::pair();
    NativeForm {
        app: FrontendApp::new(transport, width, height).expect("form"),
        pid: 1,
        patient_id: "patient".to_owned(),
        focused: true,
    }
}

fn resized(width: u32, height: u32) -> WindowEvent {
    WindowEvent::Resized { width, height }
}

fn size(form: &NativeForm<MemoryTransport>) -> (u32, u32) {
    (
        form.app.framebuffer().width(),
        form.app.framebuffer().height(),
    )
}

#[test]
fn a_batch_of_resizes_allocates_one_surface_of_the_last_size() {
    let mut form = form(320, 240);
    let mut flow = None;
    let (surfaces, bytes) = surfaces_allocated(|| {
        flow = Some(
            form.handle_events(&[resized(640, 480), resized(700, 500), resized(800, 600)])
                .expect("resize batch"),
        );
    });
    assert!(matches!(flow, Some(NativeFlow::Continue { repaint: true })));
    assert_eq!(size(&form), (800, 600));
    assert_eq!(surfaces, 1);
    assert_eq!(bytes, 800 * 600 * 4);
}

#[test]
fn a_batch_ending_at_the_current_size_allocates_and_repaints_nothing() {
    let mut form = form(800, 600);
    let before = form.app.framebuffer().pixels().to_vec();
    let mut flow = None;
    let (surfaces, _) = surfaces_allocated(|| {
        flow = Some(
            form.handle_events(&[resized(640, 480), resized(800, 600)])
                .expect("resize batch"),
        );
    });
    assert!(matches!(
        flow,
        Some(NativeFlow::Continue { repaint: false })
    ));
    assert_eq!(surfaces, 0);
    assert_eq!(size(&form), (800, 600));
    assert_eq!(form.app.framebuffer().pixels(), before);
}

#[test]
fn an_empty_resize_neither_replaces_a_pending_size_nor_allocates() {
    let mut form = form(320, 240);
    let (surfaces, bytes) = surfaces_allocated(|| {
        form.handle_events(&[resized(640, 480), resized(0, 100), resized(100, 0)])
            .expect("resize batch");
    });
    assert_eq!(size(&form), (640, 480));
    assert_eq!(surfaces, 1);
    assert_eq!(bytes, 640 * 480 * 4);

    let (surfaces, _) = surfaces_allocated(|| {
        let flow = form.handle_events(&[resized(0, 0)]).expect("empty resize");
        assert!(matches!(flow, NativeFlow::Continue { repaint: false }));
    });
    assert_eq!(surfaces, 0);
    assert_eq!(size(&form), (640, 480));
}

#[test]
fn a_resize_takes_effect_before_the_event_after_it() {
    let mut form = form(320, 240);
    let mut flow = None;
    let (surfaces, bytes) = surfaces_allocated(|| {
        flow = Some(
            form.handle_events(&[
                resized(640, 480),
                WindowEvent::DpiChanged { dpi: 144 },
                resized(700, 500),
            ])
            .expect("mixed batch"),
        );
    });
    assert!(matches!(flow, Some(NativeFlow::Continue { repaint: true })));
    // The first size exists when the scale change paints, so the event after
    // it sees the surface it was sent for; the second is a new surface.
    assert_eq!(surfaces, 2);
    assert_eq!(bytes, 700 * 500 * 4);
    assert_eq!(size(&form), (700, 500));
    assert_eq!(
        form.app.display_scale(),
        DisplayScale::from_dpi(144).expect("144 DPI")
    );
}
