//! Whole-frame repaint cost of the authored form: text projection, semantic
//! validation, layout and rasterization, as `FrontendApp::render` runs them on
//! every input event and resize.
//!
//! Two surfaces bound the regimes a desktop presents: the 800x600 logical form
//! at 100% scale, and the same form at 200% on a 1600x1200 surface, where
//! per-pixel work dominates.

use criterion::{Criterion, criterion_group, criterion_main};
use metis_frontend::FrontendApp;
use metis_ipc::MemoryTransport;
use metis_platform::DisplayScale;
use std::hint::black_box;

fn form(width: u32, height: u32, scale: DisplayScale) -> FrontendApp<MemoryTransport> {
    let (transport, _peer) = MemoryTransport::pair();
    let mut app = FrontendApp::new(transport, width, height)
        .expect("invariant: the fixture surface is a valid form extent");
    app.set_display_scale(scale)
        .expect("invariant: the fixture scale is a supported display scale");
    app
}

fn repaint(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame");
    for (name, width, height, milli) in [
        ("form_800x600_scale_1", 800, 600, 1_000),
        ("form_1600x1200_scale_2", 1_600, 1_200, 2_000),
    ] {
        let scale = DisplayScale::from_milli(milli)
            .expect("invariant: the fixture scale is a supported display scale");
        let mut app = form(width, height, scale);
        group.bench_function(name, |b| {
            b.iter(|| {
                black_box(&mut app)
                    .render()
                    .expect("invariant: the authored form renders");
                app.framebuffer().get_pixel(1, 1)
            });
        });
    }
    group.finish();
}

criterion_group! {
    name = frame;
    // Two cases at about two and a half seconds each.
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_millis(500))
        .measurement_time(std::time::Duration::from_secs(2))
        .sample_size(20);
    targets = repaint
}
criterion_main!(frame);
