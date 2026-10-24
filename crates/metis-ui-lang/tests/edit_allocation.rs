//! Heap requests of a relayout and a reprojection, against the document's size.
//!
//! `METIS-MEM-RENDER-002` requires a render that changes one label to request
//! memory in proportion to what it changed, not to the form it did not. The
//! oracles here are structural rather than a pinned count: the same edit on a
//! ten-times larger document must request no more than a small constant more,
//! while a projection built from scratch must scale with the document. That
//! pair is what separates reuse from a smaller constant factor.
#![cfg(not(target_arch = "wasm32"))]

use metis_ui_lang::{
    LayoutViewport, SemanticTree, compute_layout, compute_layout_into, parse_markup,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::fmt::Write;

thread_local! {
    /// Whether this thread is recording; other test threads never are.
    static RECORDING: Cell<bool> = const { Cell::new(false) };
    /// Requests recorded on this thread.
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

fn record() {
    // `try_with` because the allocator also serves thread teardown, after
    // these slots are destroyed; a destroyed slot is not recording.
    if RECORDING.try_with(Cell::get) == Ok(true) {
        COUNT.set(COUNT.get() + 1);
    }
}

struct Counting;

#[expect(
    unsafe_code,
    reason = "A global allocator must implement the unsafe allocator contract; every method forwards its arguments unchanged to the system allocator"
)]
// SAFETY: each method forwards to `System` with the caller's layout and
// pointer, so the `GlobalAlloc` contract holds exactly as it does for `System`.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        // SAFETY: the caller upholds the `GlobalAlloc::alloc` contract for `layout`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        // SAFETY: the caller upholds the `GlobalAlloc::alloc_zeroed` contract for `layout`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller passes a pointer this allocator returned for `layout`.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record();
        // SAFETY: the caller upholds the `GlobalAlloc::realloc` contract for the
        // pointer, its layout and the new size.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Requests `work` makes on this thread.
fn requests(work: impl FnOnce()) -> usize {
    COUNT.set(0);
    RECORDING.set(true);
    work();
    RECORDING.set(false);
    COUNT.get()
}

/// A column of `rows` rows, each a label and a button, under one header.
fn document(rows: usize) -> metis_ui_lang::DomDocument {
    let mut markup =
        String::from("<screen id='root' style='display: flex; flex-direction: column;'>");
    markup.push_str("<text id='header'>Study rows</text>");
    for row in 0..rows {
        write!(
            markup,
            "<div id='row-{row}' style='display: flex; flex-direction: row;'><text id='label-{row}'>Row {row} of the study</text><button id='go-{row}'>Open</button></div>"
        )
        .expect("a String accepts formatted output");
    }
    markup.push_str("</screen>");
    parse_markup(&markup).expect("generated markup")
}

/// A relayout into the storage the previous layout filled.
fn relayout(document: &metis_ui_lang::DomDocument) -> usize {
    let viewport = LayoutViewport::new(800, 600);
    let mut list = compute_layout(document, viewport).expect("first layout");
    requests(|| compute_layout_into(document, viewport, &mut list).expect("relayout"))
}

#[test]
fn a_relayout_requests_the_same_storage_whatever_the_document_holds() {
    let small = document(20);
    let large = document(200);
    let small_requests = relayout(&small);
    let large_requests = relayout(&large);
    assert!(
        small_requests <= 8,
        "a relayout of 20 rows requested {small_requests}"
    );
    assert!(
        large_requests <= small_requests + 2,
        "a ten-times larger document requested {large_requests} against {small_requests}: \
         the relayout is not reusing its storage"
    );
}

#[test]
fn a_reprojection_after_an_edit_requests_the_same_whatever_the_document_holds() {
    // The projection refuses an element whose aggregated content exceeds its
    // text bound, and the root aggregates every row, so the larger document
    // stays under it.
    let mut small = document(20);
    let mut large = document(80);
    let small_tree = SemanticTree::from_document(&small).expect("projection");
    let large_tree = SemanticTree::from_document(&large).expect("projection");
    assert_eq!(
        small.set_text_content("label-0", "Row 0 edited"),
        metis_ui_lang::Edit::Changed
    );
    assert_eq!(
        large.set_text_content("label-0", "Row 0 edited"),
        metis_ui_lang::Edit::Changed
    );
    let changed = vec!["label-0".to_owned()];
    let small_requests = requests(|| drop(small_tree.refresh(&small, &changed).expect("refresh")));
    let large_requests = requests(|| drop(large_tree.refresh(&large, &changed).expect("refresh")));
    assert!(
        large_requests <= small_requests + 4,
        "one edited label in a ten-times larger document requested {large_requests} against \
         {small_requests}: the projection is not reusing its nodes"
    );
}

/// The reuse above is only worth anything if the fresh projection it replaces
/// does scale with the document, which this asserts the same way.
#[test]
fn a_fresh_projection_requests_memory_for_every_element() {
    let small = document(20);
    let large = document(80);
    let small_requests =
        requests(|| drop(SemanticTree::from_document(&small).expect("projection")));
    let large_requests =
        requests(|| drop(SemanticTree::from_document(&large).expect("projection")));
    assert!(
        large_requests > small_requests * 3,
        "a ten-times larger document projected in {large_requests} against {small_requests}, \
         so the measurement cannot tell reuse from a smaller constant"
    );
}
