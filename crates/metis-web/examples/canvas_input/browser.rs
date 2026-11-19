use metis_web::{CanvasEvent, CanvasFrame, CanvasPointerEvent, CanvasSurface};
use moirai_pal::wasm::WebDocument;
use std::cell::RefCell;
use std::fmt::Write;
use std::io;

const WIDTH: u32 = 100;
const HEIGHT: u32 = 80;
const PIXELS: usize = 8_000;
const COLOR: [u8; 4] = [17, 34, 51, 255];

thread_local! {
    static SURFACE: RefCell<Option<CanvasSurface>> = const { RefCell::new(None) };
}

struct Frame(Vec<u8>);

impl CanvasFrame for Frame {
    fn width(&self) -> u32 {
        WIDTH
    }

    fn height(&self) -> u32 {
        HEIGHT
    }

    fn rgba(&self) -> &[u8] {
        &self.0
    }
}

fn status(text: &str) -> io::Result<()> {
    WebDocument::current()?
        .get_element_by_id("canvas-input-status")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "fixture status is missing"))?
        .set_text(text);
    Ok(())
}

fn report(result: io::Result<()>) {
    if let Err(error) = result {
        if let Ok(document) = WebDocument::current()
            && let Some(element) = document.get_element_by_id("canvas-input-status")
        {
            element.set_text(&format!("error:{:?}:{error}", error.kind()));
        }
    }
}

fn write_pointer(output: &mut String, event: CanvasPointerEvent) {
    let modifiers = event.modifiers();
    write!(
        output,
        "[\"{:?}\",{},{:.3},{:.3},{},{},{},{},{},{},{}]",
        event.phase(),
        event.pointer_id(),
        event.x(),
        event.y(),
        event.button(),
        event.buttons(),
        modifiers.ctrl(),
        modifiers.shift(),
        modifiers.alt(),
        modifiers.meta(),
        event.is_trusted(),
    )
    .expect("invariant: writing to a String is infallible");
}

#[expect(unsafe_code, reason = "raw fixture WASM ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn canvas_start() {
    report((|| {
        let surface = CanvasSurface::from_current_document_with_input("canvas-input")?;
        surface.present(&Frame(COLOR.repeat(PIXELS)))?;
        SURFACE.with_borrow_mut(|slot| *slot = Some(surface));
        status("ready")
    })());
}

#[expect(unsafe_code, reason = "raw fixture WASM ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn canvas_drain() {
    report(SURFACE.with_borrow(|slot| {
        let Some(surface) = slot.as_ref() else {
            return status("stopped");
        };
        let events = surface.take_events().map_err(io::Error::other)?;
        let mut output = String::from("{\"events\":[");
        for (index, event) in events.into_iter().enumerate() {
            let CanvasEvent::Pointer(pointer) = event else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "fixture received a non-pointer event",
                ));
            };
            if index != 0 {
                output.push(',');
            }
            write_pointer(&mut output, pointer);
        }
        output.push_str("]}");
        status(&output)
    }));
}

#[expect(unsafe_code, reason = "raw fixture WASM ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn canvas_stop() {
    SURFACE.with_borrow_mut(|slot| *slot = None);
    report(status("stopped"));
}
