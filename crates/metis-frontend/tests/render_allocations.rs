//! Heap requests of a render whose state has not changed.
//!
//! A host renders on every event, and most events change nothing the form
//! shows. Such a render keeps every document string, the semantic
//! projection and the layout scratch it already holds; the only request left
//! is the new display list's command storage.

use metis_frontend::{FocusDirection, FrontendApp};
use metis_ipc::MemoryTransport;
use metis_ui_lang::DisplayCommand;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// Most requests one test records; a render beyond it fails the test.
const RECORDED: usize = 512;

thread_local! {
    /// Whether this thread is recording; other test threads never are.
    static RECORDING: Cell<bool> = const { Cell::new(false) };
    /// Requests recorded on this thread.
    static COUNT: Cell<usize> = const { Cell::new(0) };
    /// Byte size of each recorded request, in request order.
    static SIZES: Cell<[usize; RECORDED]> = const { Cell::new([0; RECORDED]) };
}

fn record(bytes: usize) {
    // `try_with` because the allocator also serves thread teardown, after
    // these slots are destroyed; a destroyed slot is not recording.
    if RECORDING.try_with(Cell::get) != Ok(true) {
        return;
    }
    let index = COUNT.get();
    if index < RECORDED {
        SIZES.with(|sizes| {
            let mut all = sizes.get();
            all[index] = bytes;
            sizes.set(all);
        });
    }
    COUNT.set(index + 1);
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

/// Byte sizes of the heap requests `render` makes, in request order.
fn render_requests(app: &mut FrontendApp<MemoryTransport>) -> Vec<usize> {
    COUNT.set(0);
    RECORDING.set(true);
    let rendered = app.render();
    RECORDING.set(false);
    rendered.expect("invariant: the authored form renders");
    let count = COUNT.get();
    assert!(count <= RECORDED, "{count} requests overflow the record");
    SIZES.get()[..count].to_vec()
}

fn form() -> FrontendApp<MemoryTransport> {
    let (transport, _peer) = MemoryTransport::pair();
    FrontendApp::new(transport, 800, 600).expect("invariant: 800x600 is a valid form surface")
}

/// The one request an unchanged render may make: command storage holding
/// at least every command of the frame it painted.
fn assert_spine_only(requests: &[usize], commands: usize) {
    let command = size_of::<DisplayCommand>();
    assert_eq!(requests.len(), 1, "requests: {requests:?}");
    assert_eq!(requests[0] % command, 0, "requests: {requests:?}");
    assert!(requests[0] / command >= commands, "requests: {requests:?}");
}

/// Commands the form paints, counted from a fresh layout of its document.
fn command_count(app: &FrontendApp<MemoryTransport>, focus_ring: bool) -> usize {
    let viewport = metis_ui_lang::LayoutViewport::new(800, 600);
    let layout = metis_ui_lang::compute_layout(app.document(), viewport)
        .expect("invariant: the authored form lays out");
    layout.commands.len() + usize::from(focus_ring)
}

#[test]
fn an_unchanged_render_requests_only_the_display_list_storage() {
    let mut app = form();
    let commands = command_count(&app, false);
    assert_spine_only(&render_requests(&mut app), commands);
    // Steady state: a second unchanged render requests the same.
    assert_spine_only(&render_requests(&mut app), commands);
}

#[test]
fn an_unchanged_render_with_a_focus_ring_requests_only_the_display_list_storage() {
    let mut app = form();
    app.move_focus(FocusDirection::Forward)
        .expect("invariant: the authored form has focusable controls");
    assert!(app.focus_visible());
    let commands = command_count(&app, true);
    assert_spine_only(&render_requests(&mut app), commands);
}
