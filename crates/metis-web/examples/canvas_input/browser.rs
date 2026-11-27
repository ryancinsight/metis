use metis_web::{CanvasEvent, CanvasFrame, CanvasPointerEvent, CanvasSurface};
use moirai_pal::wasm::{WebDocument, WebEventListener};
use std::cell::RefCell;
use std::fmt::Write;
use std::io;

const WIDTH: u32 = 100;
const HEIGHT: u32 = 80;
const PIXELS: usize = 8_000;
const COLOR: [u8; 4] = [17, 34, 51, 255];

thread_local! {
    static APPLICATION: RefCell<Option<Application>> = const { RefCell::new(None) };
}

struct Application {
    surface: Option<CanvasSurface>,
    controls: [WebEventListener; 3],
}

impl Application {
    fn close(mut self) -> io::Result<()> {
        self.surface = None;
        for control in &mut self.controls {
            control.close()?;
        }
        Ok(())
    }
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

fn start() -> io::Result<()> {
    APPLICATION.with_borrow_mut(|application| {
        let application = application.as_mut().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotConnected,
                "canvas application is not mounted",
            )
        })?;
        let surface = CanvasSurface::from_current_document_with_input("canvas-input")?;
        surface.present(&Frame(COLOR.repeat(PIXELS)))?;
        application.surface = Some(surface);
        status("ready")
    })
}

fn drain() -> io::Result<()> {
    APPLICATION.with_borrow(|application| {
        let Some(surface) = application
            .as_ref()
            .and_then(|application| application.surface.as_ref())
        else {
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
    })
}

fn stop() -> io::Result<()> {
    APPLICATION.with_borrow_mut(|application| {
        let application = application.as_mut().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotConnected,
                "canvas application is not mounted",
            )
        })?;
        application.surface = None;
        status("stopped")
    })
}

fn mount() -> io::Result<()> {
    if let Some(application) = APPLICATION.with_borrow_mut(Option::take) {
        application.close()?;
    }
    let document = WebDocument::current()?;
    let start_button = document.get_element_by_id("canvas-start").ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "canvas start control is missing")
    })?;
    let drain_button = document.get_element_by_id("canvas-drain").ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "canvas drain control is missing")
    })?;
    let stop_button = document
        .get_element_by_id("canvas-stop")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "canvas stop control is missing"))?;
    let controls = [
        start_button.add_event_listener("click", |_| report(start()))?,
        drain_button.add_event_listener("click", |_| report(drain()))?,
        stop_button.add_event_listener("click", |_| report(stop()))?,
    ];
    APPLICATION.with_borrow_mut(|application| {
        *application = Some(Application {
            surface: None,
            controls,
        });
    });
    status("loaded")
}

#[expect(unsafe_code, reason = "raw fixture WASM ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn canvas_mount() {
    report(mount());
}
