//! Moirai-backed browser canvas surface.

use super::events::CanvasEventQueue;
use super::{
    CanvasEvent, CanvasEventError, CanvasFrame, CanvasModifiers, CanvasPointerEvent,
    CanvasPointerPhase, CanvasPointerType, CanvasWheelEvent, CanvasWheelUnit,
};
use moirai_pal::wasm::{
    CanvasSize, PointerMetadata, PointerType, RgbaFrame, WebCanvas, WebDocument, WebElement,
    WebEventListener, WheelDeltaMode, WheelMetadata,
};
use std::cell::{Cell, RefCell};
use std::io;
use std::rc::Rc;

const MAX_ACTIVE_POINTERS: usize = 4;

/// A browser canvas surface owned by the Metis host.
pub struct CanvasSurface {
    canvas: WebCanvas,
    input: Option<CanvasInput>,
}

impl CanvasSurface {
    /// Resolves a canvas from the current browser document by identifier.
    ///
    /// # Errors
    /// Returns a typed I/O error when no browser document exists, the element
    /// is absent, is not a canvas, or cannot provide a two-dimensional context.
    pub fn from_current_document(id: &str) -> io::Result<Self> {
        let document = WebDocument::current()?;
        Self::from_document(&document, id)
    }

    /// Resolves a canvas from a document by its stable identifier.
    ///
    /// # Errors
    /// Returns a typed I/O error when the element is absent, is not a canvas,
    /// or cannot provide a two-dimensional rendering context.
    pub fn from_document(document: &WebDocument, id: &str) -> io::Result<Self> {
        Ok(Self {
            canvas: document.canvas_by_id(id)?,
            input: None,
        })
    }

    /// Resolves a canvas and retains bounded pointer and wheel listeners.
    ///
    /// The listeners are removed when this surface is dropped. Events remain
    /// format-neutral; the consuming application decides how coordinates,
    /// buttons and deltas affect its state.
    ///
    /// # Errors
    /// Returns a typed I/O error when the canvas cannot be resolved or the
    /// browser rejects one of the listener registrations.
    pub fn from_current_document_with_input(id: &str) -> io::Result<Self> {
        let document = WebDocument::current()?;
        Self::from_document_with_input(&document, id)
    }

    /// Resolves a canvas and retains bounded pointer and wheel listeners.
    ///
    /// # Errors
    /// Returns a typed I/O error when the canvas cannot be resolved or the
    /// browser rejects one of the listener registrations.
    pub fn from_document_with_input(document: &WebDocument, id: &str) -> io::Result<Self> {
        let element = document.get_element_by_id(id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "canvas element identifier is absent",
            )
        })?;
        let canvas = WebCanvas::from_element(&element)?;
        let input = CanvasInput::attach(element)?;
        Ok(Self {
            canvas,
            input: Some(input),
        })
    }

    /// Returns the resolved canvas identifier.
    #[must_use]
    pub fn id(&self) -> String {
        self.canvas.id()
    }

    /// Presents one borrowed RGBA8 frame without retaining its bytes.
    ///
    /// Dimensions and exact storage length are validated by Moirai's shared
    /// browser frame contract before the Web API upload. The consumer remains
    /// the owner of the source bytes and any domain meaning attached to them.
    ///
    /// # Errors
    /// Returns [`io::ErrorKind::InvalidInput`] for zero, oversized, or
    /// inconsistent dimensions/storage, or when the browser rejects the
    /// upload.
    pub fn present<F>(&self, frame: &F) -> io::Result<()>
    where
        F: CanvasFrame + ?Sized,
    {
        let size = CanvasSize::new(frame.width(), frame.height())?;
        let frame = RgbaFrame::new(size, frame.rgba())?;
        self.canvas.present(frame)
    }

    /// Takes all events captured since the previous call.
    ///
    /// The returned batch is bounded by [`super::CANVAS_EVENT_CAPACITY`]. A
    /// queue overflow or browser metadata/capture failure clears the pending
    /// batch and returns a typed error so the consumer can cancel its gesture
    /// and decide whether to remount the surface.
    pub fn take_events(&self) -> Result<Box<[CanvasEvent]>, CanvasEventError> {
        self.input.as_ref().map_or_else(
            || Ok(Vec::new().into_boxed_slice()),
            CanvasInput::take_events,
        )
    }
}

struct CanvasInput {
    element: WebElement,
    queue: Rc<RefCell<CanvasEventQueue>>,
    active: Rc<Cell<[Option<i32>; MAX_ACTIVE_POINTERS]>>,
    listeners: Vec<WebEventListener>,
}

impl CanvasInput {
    fn attach(element: WebElement) -> io::Result<Self> {
        let queue = Rc::new(RefCell::new(CanvasEventQueue::new()));
        let active = Rc::new(Cell::new([None; MAX_ACTIVE_POINTERS]));
        let mut listeners = Vec::with_capacity(5);
        for (name, phase) in [
            ("pointerdown", CanvasPointerPhase::Down),
            ("pointermove", CanvasPointerPhase::Move),
            ("pointerup", CanvasPointerPhase::Up),
            ("pointercancel", CanvasPointerPhase::Cancel),
        ] {
            let listener_element = element.clone();
            let listener_queue = Rc::clone(&queue);
            let listener_active = Rc::clone(&active);
            listeners.push(element.add_event_listener(name, move |event| {
                event.prevent_default();
                let Some(metadata) = event.pointer_metadata() else {
                    listener_queue
                        .borrow_mut()
                        .fail(CanvasEventError::InvalidMetadata);
                    release_all(&listener_element, &listener_active);
                    return;
                };
                if phase == CanvasPointerPhase::Down {
                    if let Err(error) =
                        capture(&listener_element, &listener_active, metadata.pointer_id())
                    {
                        listener_queue.borrow_mut().fail(error);
                        release_all(&listener_element, &listener_active);
                        return;
                    }
                }
                let canvas_event = CanvasEvent::Pointer(pointer_event(phase, metadata));
                let accepted = listener_queue.borrow_mut().push(canvas_event);
                if !accepted {
                    release_all(&listener_element, &listener_active);
                }
                if matches!(phase, CanvasPointerPhase::Up | CanvasPointerPhase::Cancel) {
                    release_one(&listener_element, &listener_active, metadata.pointer_id());
                }
            })?);
        }
        let listener_element = element.clone();
        let listener_queue = Rc::clone(&queue);
        let listener_active = Rc::clone(&active);
        listeners.push(element.add_event_listener("wheel", move |event| {
            event.prevent_default();
            let Some(metadata) = event.wheel_metadata() else {
                listener_queue
                    .borrow_mut()
                    .fail(CanvasEventError::InvalidMetadata);
                return;
            };
            let accepted = listener_queue
                .borrow_mut()
                .push(CanvasEvent::Wheel(wheel_event(metadata)));
            if !accepted {
                release_all(&listener_element, &listener_active);
            }
        })?);
        Ok(Self {
            element,
            queue,
            active,
            listeners,
        })
    }

    fn take_events(&self) -> Result<Box<[CanvasEvent]>, CanvasEventError> {
        self.queue.borrow_mut().take()
    }
}

impl Drop for CanvasInput {
    fn drop(&mut self) {
        release_all(&self.element, &self.active);
        self.listeners.clear();
    }
}

fn pointer_event(phase: CanvasPointerPhase, metadata: PointerMetadata) -> CanvasPointerEvent {
    CanvasPointerEvent {
        phase,
        pointer_id: metadata.pointer_id(),
        pointer_type: match metadata.pointer_type() {
            PointerType::Mouse => CanvasPointerType::Mouse,
            PointerType::Pen => CanvasPointerType::Pen,
            PointerType::Touch => CanvasPointerType::Touch,
            PointerType::Other => CanvasPointerType::Other,
            _ => CanvasPointerType::Other,
        },
        x: metadata.offset_x(),
        y: metadata.offset_y(),
        button: metadata.button(),
        buttons: metadata.buttons(),
        modifiers: modifiers(metadata.modifiers()),
        primary: metadata.is_primary(),
    }
}

fn wheel_event(metadata: WheelMetadata) -> CanvasWheelEvent {
    CanvasWheelEvent {
        delta_x: metadata.delta_x(),
        delta_y: metadata.delta_y(),
        delta_z: metadata.delta_z(),
        unit: match metadata.delta_mode() {
            WheelDeltaMode::Pixel => CanvasWheelUnit::Pixel,
            WheelDeltaMode::Line => CanvasWheelUnit::Line,
            WheelDeltaMode::Page => CanvasWheelUnit::Page,
            WheelDeltaMode::Other => CanvasWheelUnit::Other,
            _ => CanvasWheelUnit::Other,
        },
        x: metadata.offset_x(),
        y: metadata.offset_y(),
        modifiers: modifiers(metadata.modifiers()),
    }
}

fn modifiers(value: moirai_pal::wasm::PointerModifiers) -> CanvasModifiers {
    CanvasModifiers::from_bits(
        u8::from(value.ctrl()) * CanvasModifiers::CTRL
            | u8::from(value.shift()) * CanvasModifiers::SHIFT
            | u8::from(value.alt()) * CanvasModifiers::ALT
            | u8::from(value.meta()) * CanvasModifiers::META,
    )
}

fn capture(
    element: &WebElement,
    active: &Cell<[Option<i32>; MAX_ACTIVE_POINTERS]>,
    pointer_id: i32,
) -> Result<(), CanvasEventError> {
    let current = active.get();
    if current.iter().flatten().any(|id| *id == pointer_id) {
        return Err(CanvasEventError::PointerCapture);
    }
    let Some(slot) = current.iter().position(Option::is_none) else {
        return Err(CanvasEventError::PointerLimit);
    };
    element
        .set_pointer_capture(pointer_id)
        .map_err(|_| CanvasEventError::PointerCapture)?;
    let mut next = current;
    next[slot] = Some(pointer_id);
    active.set(next);
    Ok(())
}

fn release_one(
    element: &WebElement,
    active: &Cell<[Option<i32>; MAX_ACTIVE_POINTERS]>,
    pointer_id: i32,
) {
    let current = active.get();
    let Some(slot) = current
        .iter()
        .position(|active_id| *active_id == Some(pointer_id))
    else {
        return;
    };
    let _ = element.release_pointer_capture(pointer_id);
    let mut next = current;
    next[slot] = None;
    active.set(next);
}

fn release_all(element: &WebElement, active: &Cell<[Option<i32>; MAX_ACTIVE_POINTERS]>) {
    let current = active.get();
    for pointer_id in current.into_iter().flatten() {
        let _ = element.release_pointer_capture(pointer_id);
    }
    active.set([None; MAX_ACTIVE_POINTERS]);
}
