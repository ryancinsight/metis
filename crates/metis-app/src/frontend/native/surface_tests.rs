//! A batch of native resize events allocates only the surface it ends on.

use super::NativeForm;
use super::alloc_counter::REQUESTS;
use metis_core::ErrorCode;
use metis_frontend::FrontendApp;
use metis_ipc::MemoryTransport;
use metis_platform::Damage;
use metis_platform::framebuffer::MAX_PIXELS;
use metis_platform::native::{NativeApplication, NativeFlow, WindowEvent};

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
    /// Surfaces allocated at each of the 640x480, 700x500 and 800x600 sizes.
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
