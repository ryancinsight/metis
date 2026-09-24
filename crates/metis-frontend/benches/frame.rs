//! Repaint cost of the authored form after the edits that drive it: a
//! keystroke, which changes one field, and a theme change, which recolors
//! nearly every command and so approaches a full repaint.
//!
//! Two surfaces bound the regimes a desktop presents: the 800x600 logical form
//! at 100% scale, and the same form at 200% on a 1600x1200 surface, where
//! per-pixel work dominates. Each iteration alternates between two states so
//! every measured render has real work to do.

use criterion::{Criterion, criterion_group, criterion_main};
use metis_frontend::{ApplicationCommand, FrontendApp};
use metis_ipc::MemoryTransport;
use metis_platform::DisplayScale;
use std::hint::black_box;

fn form(width: u32, height: u32, milli: u32) -> FrontendApp<MemoryTransport> {
    let (transport, _peer) = MemoryTransport::pair();
    let mut app = FrontendApp::new(transport, width, height)
        .expect("invariant: the fixture surface is a valid form extent");
    let scale = DisplayScale::from_milli(milli)
        .expect("invariant: the fixture scale is a supported display scale");
    app.set_display_scale(scale)
        .expect("invariant: the fixture scale is a supported display scale");
    app
}

fn repaint(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame");
    for (surface, width, height, milli) in [
        ("800x600_scale_1", 800, 600, 1_000),
        ("1600x1200_scale_2", 1_600, 1_200, 2_000),
    ] {
        let mut app = form(width, height, milli);
        let mut typed = false;
        group.bench_function(format!("keystroke_{surface}"), |b| {
            b.iter(|| {
                typed = !typed;
                let patient = if typed {
                    "PT-9042-ALPHAB"
                } else {
                    "PT-9042-ALPHA"
                };
                black_box(&mut app)
                    .set_inputs(black_box(patient), 72.5, 4.0, 0.5)
                    .expect("invariant: the fixture input is valid");
                app.framebuffer().get_pixel(1, 1)
            });
        });
        let mut app = form(width, height, milli);
        let mut dark = false;
        group.bench_function(format!("theme_{surface}"), |b| {
            b.iter(|| {
                dark = !dark;
                let command = if dark {
                    ApplicationCommand::ThemeDark
                } else {
                    ApplicationCommand::ThemeSystem
                };
                black_box(&mut app)
                    .activate_command(black_box(command))
                    .expect("invariant: theme commands apply");
                app.framebuffer().get_pixel(1, 1)
            });
        });
    }
    group.finish();
}

criterion_group! {
    name = frame;
    // Four cases at about two and a half seconds each.
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_millis(500))
        .measurement_time(std::time::Duration::from_secs(2))
        .sample_size(20);
    targets = repaint
}
criterion_main!(frame);
