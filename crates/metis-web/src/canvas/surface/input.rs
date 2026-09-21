//! Browser input for the canvas surface.
//!
//! The DOM listeners, the translation of each metadata payload into a
//! `CanvasEvent`, and the pointer-capture bookkeeping over a fixed slot
//! array. The surface holds one of these and asks it for events; nothing
//! else crosses.

use super::super::events::CanvasEventQueue;
use super::super::{
    CanvasEvent, CanvasEventError, CanvasEventTrust, CanvasKeyboardEvent, CanvasKeyboardPhase,
    CanvasModifiers, CanvasPointerEvent, CanvasPointerPhase, CanvasPointerType, CanvasWheelEvent,
    CanvasWheelUnit,
};
use moirai_pal::wasm::{
    ContentBoxPoint, KeyboardMetadata, PointerMetadata, PointerType, WebElement, WebEventListener,
    WheelDeltaMode, WheelMetadata,
};
use std::cell::{Cell, RefCell};
use std::io;
use std::rc::Rc;

const MAX_ACTIVE_POINTERS: usize = 4;

pub(super) struct CanvasInput {
    element: WebElement,
    queue: Rc<RefCell<CanvasEventQueue>>,
    active: Rc<Cell<[Option<i32>; MAX_ACTIVE_POINTERS]>>,
    listeners: Vec<WebEventListener>,
}

impl CanvasInput {
    pub(super) fn attach(element: WebElement) -> io::Result<Self> {
        let queue = Rc::new(RefCell::new(CanvasEventQueue::new()));
        let active = Rc::new(Cell::new([None; MAX_ACTIVE_POINTERS]));
        let mut listeners = Vec::with_capacity(7);
        attach_pointer_listeners(&element, &queue, &active, &mut listeners)?;
        attach_keyboard_listeners(&element, &queue, &mut listeners)?;
        attach_wheel_listener(&element, &queue, &active, &mut listeners)?;
        Ok(Self {
            element,
            queue,
            active,
            listeners,
        })
    }

    pub(super) fn take_events(&self) -> Result<Box<[CanvasEvent]>, CanvasEventError> {
        self.queue.borrow_mut().take()
    }

    /// The number of DOM listener guards this input owns.
    ///
    /// The surface reports this; it reaches for the count rather than the
    /// collection so the guard list stays private to the module that
    /// installs and drops it.
    pub(super) fn listener_count(&self) -> usize {
        self.listeners.len()
    }
}

fn attach_pointer_listeners(
    element: &WebElement,
    queue: &Rc<RefCell<CanvasEventQueue>>,
    active: &Rc<Cell<[Option<i32>; MAX_ACTIVE_POINTERS]>>,
    listeners: &mut Vec<WebEventListener>,
) -> io::Result<()> {
    for (name, phase) in [
        ("pointerdown", CanvasPointerPhase::Down),
        ("pointermove", CanvasPointerPhase::Move),
        ("pointerup", CanvasPointerPhase::Up),
        ("pointercancel", CanvasPointerPhase::Cancel),
    ] {
        let listener_element = element.clone();
        let listener_queue = Rc::clone(queue);
        let listener_active = Rc::clone(active);
        listeners.push(element.add_event_listener(name, move |event| {
            event.prevent_default();
            let Some(metadata) = event.pointer_metadata() else {
                listener_queue
                    .borrow_mut()
                    .fail(CanvasEventError::InvalidMetadata);
                release_all(&listener_element, &listener_active);
                return;
            };
            if phase == CanvasPointerPhase::Down
                && let Err(error) =
                    capture(&listener_element, &listener_active, metadata.pointer_id())
            {
                listener_queue.borrow_mut().fail(error);
                release_all(&listener_element, &listener_active);
                return;
            }
            let Ok(point) =
                listener_element.content_box_point(metadata.client_x(), metadata.client_y())
            else {
                listener_queue
                    .borrow_mut()
                    .fail(CanvasEventError::LocalCoordinates);
                release_all(&listener_element, &listener_active);
                return;
            };
            let canvas_event = CanvasEvent::Pointer(pointer_event(phase, metadata, point));
            let accepted = listener_queue.borrow_mut().push(canvas_event);
            if !accepted {
                release_all(&listener_element, &listener_active);
            }
            if matches!(phase, CanvasPointerPhase::Up | CanvasPointerPhase::Cancel) {
                release_one(&listener_element, &listener_active, metadata.pointer_id());
            }
        })?);
    }
    Ok(())
}

fn attach_keyboard_listeners(
    element: &WebElement,
    queue: &Rc<RefCell<CanvasEventQueue>>,
    listeners: &mut Vec<WebEventListener>,
) -> io::Result<()> {
    for (name, phase) in [
        ("keydown", CanvasKeyboardPhase::Down),
        ("keyup", CanvasKeyboardPhase::Up),
    ] {
        let listener_queue = Rc::clone(queue);
        listeners.push(element.add_event_listener(name, move |event| {
            event.prevent_default();
            let Ok(Some(metadata)) = event.keyboard_metadata() else {
                listener_queue
                    .borrow_mut()
                    .fail(CanvasEventError::InvalidMetadata);
                return;
            };
            let keyboard_event = match keyboard_event(phase, &metadata) {
                Ok(event) => event,
                Err(error) => {
                    listener_queue.borrow_mut().fail(error);
                    return;
                }
            };
            listener_queue
                .borrow_mut()
                .push(CanvasEvent::Keyboard(keyboard_event));
        })?);
    }
    Ok(())
}

fn attach_wheel_listener(
    element: &WebElement,
    queue: &Rc<RefCell<CanvasEventQueue>>,
    active: &Rc<Cell<[Option<i32>; MAX_ACTIVE_POINTERS]>>,
    listeners: &mut Vec<WebEventListener>,
) -> io::Result<()> {
    let listener_element = element.clone();
    let listener_queue = Rc::clone(queue);
    let listener_active = Rc::clone(active);
    listeners.push(element.add_event_listener("wheel", move |event| {
        event.prevent_default();
        let Some(metadata) = event.wheel_metadata() else {
            listener_queue
                .borrow_mut()
                .fail(CanvasEventError::InvalidMetadata);
            return;
        };
        let Ok(point) =
            listener_element.content_box_point(metadata.client_x(), metadata.client_y())
        else {
            listener_queue
                .borrow_mut()
                .fail(CanvasEventError::LocalCoordinates);
            release_all(&listener_element, &listener_active);
            return;
        };
        let accepted = listener_queue
            .borrow_mut()
            .push(CanvasEvent::Wheel(wheel_event(metadata, point)));
        if !accepted {
            release_all(&listener_element, &listener_active);
        }
    })?);
    Ok(())
}

impl Drop for CanvasInput {
    fn drop(&mut self) {
        release_all(&self.element, &self.active);
        self.listeners.clear();
    }
}

fn pointer_event(
    phase: CanvasPointerPhase,
    metadata: PointerMetadata,
    point: ContentBoxPoint,
) -> CanvasPointerEvent {
    CanvasPointerEvent {
        phase,
        pointer_id: metadata.pointer_id(),
        pointer_type: match metadata.pointer_type() {
            PointerType::Mouse => CanvasPointerType::Mouse,
            PointerType::Pen => CanvasPointerType::Pen,
            PointerType::Touch => CanvasPointerType::Touch,
            _ => CanvasPointerType::Other,
        },
        x: point.x(),
        y: point.y(),
        content_width: point.width(),
        content_height: point.height(),
        button: metadata.button(),
        buttons: metadata.buttons(),
        modifiers: modifiers(metadata.modifiers()),
        primary: metadata.is_primary(),
        trust: CanvasEventTrust::from(metadata.is_trusted()),
    }
}

fn wheel_event(metadata: WheelMetadata, point: ContentBoxPoint) -> CanvasWheelEvent {
    CanvasWheelEvent {
        delta_x: metadata.delta_x(),
        delta_y: metadata.delta_y(),
        delta_z: metadata.delta_z(),
        unit: match metadata.delta_mode() {
            WheelDeltaMode::Pixel => CanvasWheelUnit::Pixel,
            WheelDeltaMode::Line => CanvasWheelUnit::Line,
            WheelDeltaMode::Page => CanvasWheelUnit::Page,
            _ => CanvasWheelUnit::Other,
        },
        x: point.x(),
        y: point.y(),
        content_width: point.width(),
        content_height: point.height(),
        modifiers: modifiers(metadata.modifiers()),
        trust: CanvasEventTrust::from(metadata.is_trusted()),
    }
}

fn keyboard_event(
    phase: CanvasKeyboardPhase,
    metadata: &KeyboardMetadata,
) -> Result<CanvasKeyboardEvent, CanvasEventError> {
    CanvasKeyboardEvent::try_new(
        phase,
        metadata.key().to_owned(),
        metadata.code().to_owned(),
        metadata.is_repeat(),
        modifiers(metadata.modifiers()),
        CanvasEventTrust::from(metadata.is_trusted()),
    )
}

fn modifiers(value: moirai_pal::wasm::PointerModifiers) -> CanvasModifiers {
    CanvasModifiers::from_bits(
        (u8::from(value.ctrl()) * CanvasModifiers::CTRL)
            | (u8::from(value.shift()) * CanvasModifiers::SHIFT)
            | (u8::from(value.alt()) * CanvasModifiers::ALT)
            | (u8::from(value.meta()) * CanvasModifiers::META),
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
