//! Cost of the operations a host performs on a virtual list: constructing the
//! extent index, changing one row, and querying the visible window.
//!
//! The pinned model is 10^5 rows of four repeating extents, the shape
//! `METIS-LIST-003` measures before any per-row update is added. The build
//! case is the O(n) construction; the change case is one row per iteration,
//! which the Fenwick tree updates in O(log n); the window case is the query a
//! scroll performs. Reporting all three separately is what distinguishes a
//! rebuild from an update: if a change cost what a build costs, the update
//! path would not be the one being exercised.

use criterion::{Criterion, criterion_group, criterion_main};
use metis_ui_lang::VirtualList;
use std::hint::black_box;

/// Rows in the pinned model.
const ROWS: usize = 100_000;
/// Logical viewport the window query spans, in pixels.
const VIEWPORT: u32 = 800;
/// Rows a wheel step advances the window by, in the query loop.
const STEP: u64 = 4_096;

/// Row extents with a repeating shape, so the prefix sums are not uniform and
/// a window query cannot be answered by arithmetic alone.
fn extents() -> Vec<u32> {
    (0..ROWS)
        .map(|index| match index % 4 {
            0 => 18,
            1 => 22,
            2 => 24,
            _ => 30,
        })
        .collect()
}

fn virtual_list(c: &mut Criterion) {
    let extents = extents();
    let mut group = c.benchmark_group("virtual_list_100k");

    group.bench_function("build", |b| {
        b.iter(|| {
            let list = VirtualList::variable(black_box(&extents))
                .expect("invariant: the pinned extents are within the item bound");
            black_box(list.total_extent())
        });
    });

    let mut list = VirtualList::variable(&extents)
        .expect("invariant: the pinned extents are within the item bound");
    let mut changed = 0usize;
    group.bench_function("set_extent", |b| {
        b.iter(|| {
            changed = (changed + 1) % ROWS;
            // Alternate the extent so the tree's sums actually move.
            let extent = if changed.is_multiple_of(2) { 24 } else { 26 };
            list.set_extent(black_box(changed), extent)
                .expect("invariant: the index is inside the pinned list");
        });
    });

    let total = list.total_extent().max(1);
    let mut offset = 0u64;
    group.bench_function("window", |b| {
        b.iter(|| {
            offset = (offset + STEP) % total;
            black_box(list.window(offset, VIEWPORT).items.len())
        });
    });

    group.finish();
}

criterion_group!(benches, virtual_list);
criterion_main!(benches);
