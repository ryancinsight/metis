use metis_web::{CanvasFrame, CanvasSurface};
use moirai_pal::wasm::{WebDocument, spawn_local};
use std::cell::{Cell, RefCell};
use std::io;

thread_local! {
    static SURFACE: RefCell<Option<CanvasSurface>> = const { RefCell::new(None) };
    static STATUS: Cell<u32> = const { Cell::new(0) };
}

struct Frame([u8; 32 * 32 * 4]);

impl Frame {
    fn new(left: [u8; 4], right: [u8; 4]) -> Self {
        let mut frame = Self([0; 32 * 32 * 4]);
        for (index, pixel) in frame.0.chunks_exact_mut(4).enumerate() {
            pixel.copy_from_slice(if index % 32 < 16 { &left } else { &right });
        }
        frame
    }
}

impl CanvasFrame for Frame {
    fn width(&self) -> u32 {
        32
    }
    fn height(&self) -> u32 {
        32
    }
    fn rgba(&self) -> &[u8] {
        &self.0
    }
}

fn finish(result: io::Result<()>, success: u32) {
    match result {
        Ok(()) => STATUS.set(success),
        Err(error) => {
            STATUS.set(255);
            if let Ok(document) = WebDocument::current()
                && let Some(element) = document.get_element_by_id("recovery-error")
            {
                element.set_text(&format!("{:?}: {error}", error.kind()));
            }
        }
    }
}

// Raw exports let the existing wasm-bindgen loader drive the real Rust surface.
#[expect(unsafe_code, reason = "raw demonstration WASM ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn canvas_start() {
    STATUS.set(1);
    spawn_local(async {
        match CanvasSurface::from_current_document_gpu_with_input("recovery").await {
            Ok(surface) => {
                let result = surface.present(&Frame::new([255, 0, 0, 255], [0, 255, 0, 255]));
                SURFACE.with_borrow_mut(|slot| *slot = Some(surface));
                finish(result, 2);
            }
            Err(error) => finish(Err(error), 2),
        }
    });
}

#[expect(unsafe_code, reason = "raw demonstration WASM ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn canvas_recreate() {
    let surface = SURFACE.with_borrow_mut(Option::take);
    STATUS.set(1);
    spawn_local(async move {
        let Some(mut surface) = surface else {
            STATUS.set(255);
            return;
        };
        let result = match surface.recreate().await {
            Ok(()) => surface.present(&Frame::new([0, 0, 255, 255], [255, 255, 255, 255])),
            Err(error) => Err(error),
        };
        SURFACE.with_borrow_mut(|slot| *slot = Some(surface));
        finish(result, 3);
    });
}

#[expect(unsafe_code, reason = "raw demonstration WASM ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn canvas_redraw() {
    let result = SURFACE.with_borrow(|slot| {
        slot.as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "canvas is not mounted"))?
            .present(&Frame::new([255, 0, 0, 255], [0, 255, 0, 255]))
    });
    finish(result, 5);
}

#[expect(unsafe_code, reason = "raw demonstration WASM ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn canvas_stop() {
    SURFACE.with_borrow_mut(|slot| *slot = None);
    STATUS.set(4);
}

#[expect(unsafe_code, reason = "raw demonstration WASM ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn canvas_status() -> u32 {
    STATUS.get()
}

#[expect(unsafe_code, reason = "raw demonstration WASM ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn canvas_listeners() -> usize {
    SURFACE.with_borrow(|slot| slot.as_ref().map_or(0, CanvasSurface::listener_count))
}
