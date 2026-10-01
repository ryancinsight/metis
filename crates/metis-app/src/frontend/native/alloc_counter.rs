//! The counting global allocator shared by the native host's allocation tests.
//!
//! Counts are per thread, so concurrent tests cannot count each other's
//! allocations.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// Byte sizes of the 640x480, 700x500 and 800x600 surfaces; a surface is
/// reserved at its exact size, so a request of one of them is one surface.
pub(super) const SURFACE_BYTES: [usize; 3] = [640 * 480 * 4, 700 * 500 * 4, 800 * 600 * 4];

thread_local! {
    /// Requests per watched size made on this thread.
    pub(super) static REQUESTS: Cell<[usize; 3]> = const { Cell::new([0; 3]) };
    /// Every allocation, zeroed allocation and reallocation made on this thread.
    pub(super) static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn record(bytes: usize) {
    ALLOCATIONS.set(ALLOCATIONS.get() + 1);
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
