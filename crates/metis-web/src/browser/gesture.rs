//! Browser listeners for the Rust-owned pan and zoom policy.

use super::view;
use crate::gesture_policy::{GestureViewport, WheelUnit};
use moirai_pal::wasm::{WebDocument, WebElement, WebEventListener, WheelDeltaMode};
use std::cell::RefCell;
use std::io;
use std::rc::Rc;

pub(super) fn listeners(document: &WebDocument) -> io::Result<Vec<WebEventListener>> {
    let surface = view::element(document, "pointer-surface")?;
    let content = view::element(document, "gesture-content")?;
    let status = view::element(document, "gesture-status")?;
    let viewport = Rc::new(RefCell::new(GestureViewport::default()));
    render(document, &content, &status, *viewport.borrow(), "idle")?;

    Ok(vec![
        pointer_down_listener(document, &surface, &content, &status, &viewport)?,
        pointer_move_listener(document, &surface, &content, &status, &viewport)?,
        pointer_lifecycle_listener(
            document,
            &surface,
            &content,
            &status,
            &viewport,
            "pointerup",
        )?,
        pointer_lifecycle_listener(
            document,
            &surface,
            &content,
            &status,
            &viewport,
            "pointercancel",
        )?,
        wheel_listener(document, &surface, &content, &status, viewport)?,
    ])
}

fn pointer_down_listener(
    document: &WebDocument,
    surface: &WebElement,
    content: &WebElement,
    status: &WebElement,
    viewport: &Rc<RefCell<GestureViewport>>,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_surface = surface.clone();
    let listener_content = content.clone();
    let listener_status = status.clone();
    let listener_viewport = Rc::clone(viewport);
    surface.add_event_listener("pointerdown", move |event| {
        let Some(metadata) = event.pointer_metadata() else {
            set_error(
                &listener_document,
                &listener_status,
                "Pointer event did not carry metadata",
            );
            return;
        };
        if !listener_surface.has_pointer_capture(metadata.pointer_id()) {
            return;
        }
        let mut viewport = listener_viewport.borrow_mut();
        if !viewport.press(
            metadata.pointer_id(),
            metadata.client_x(),
            metadata.client_y(),
        ) {
            set_error(
                &listener_document,
                &listener_status,
                "Another pointer is already driving the gesture",
            );
            return;
        }
        event.prevent_default();
        if let Err(error) = render(
            &listener_document,
            &listener_content,
            &listener_status,
            *viewport,
            &format!("press {}", metadata.pointer_type().as_str()),
        ) {
            set_error(&listener_document, &listener_status, &error.to_string());
        }
    })
}

fn pointer_move_listener(
    document: &WebDocument,
    surface: &WebElement,
    content: &WebElement,
    status: &WebElement,
    viewport: &Rc<RefCell<GestureViewport>>,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_surface = surface.clone();
    let listener_content = content.clone();
    let listener_status = status.clone();
    let listener_viewport = Rc::clone(viewport);
    surface.add_event_listener("pointermove", move |event| {
        let Some(metadata) = event.pointer_metadata() else {
            return;
        };
        if !listener_surface.has_pointer_capture(metadata.pointer_id()) {
            return;
        }
        let mut viewport = listener_viewport.borrow_mut();
        if !viewport.move_pointer(
            metadata.pointer_id(),
            metadata.client_x(),
            metadata.client_y(),
        ) {
            return;
        }
        event.prevent_default();
        if let Err(error) = render(
            &listener_document,
            &listener_content,
            &listener_status,
            *viewport,
            "pan",
        ) {
            set_error(&listener_document, &listener_status, &error.to_string());
        }
    })
}

fn pointer_lifecycle_listener(
    document: &WebDocument,
    surface: &WebElement,
    content: &WebElement,
    status: &WebElement,
    viewport: &Rc<RefCell<GestureViewport>>,
    event_name: &'static str,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_content = content.clone();
    let listener_status = status.clone();
    let listener_viewport = Rc::clone(viewport);
    surface.add_event_listener(event_name, move |event| {
        let Some(metadata) = event.pointer_metadata() else {
            set_error(
                &listener_document,
                &listener_status,
                "Pointer event did not carry metadata",
            );
            return;
        };
        let mut viewport = listener_viewport.borrow_mut();
        if !viewport.release(metadata.pointer_id()) {
            return;
        }
        if let Err(error) = render(
            &listener_document,
            &listener_content,
            &listener_status,
            *viewport,
            if event_name == "pointercancel" {
                "cancel"
            } else {
                "release"
            },
        ) {
            set_error(&listener_document, &listener_status, &error.to_string());
        }
    })
}

fn wheel_listener(
    document: &WebDocument,
    surface: &WebElement,
    content: &WebElement,
    status: &WebElement,
    viewport: Rc<RefCell<GestureViewport>>,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_content = content.clone();
    let listener_status = status.clone();
    let listener_viewport = viewport;
    surface.add_event_listener("wheel", move |event| {
        let Some(metadata) = event.wheel_metadata() else {
            set_error(
                &listener_document,
                &listener_status,
                "Wheel event did not carry metadata",
            );
            return;
        };
        let mut viewport = listener_viewport.borrow_mut();
        if !viewport.wheel(
            metadata.delta_x(),
            metadata.delta_y(),
            wheel_unit(metadata.delta_mode()),
            metadata.modifiers().ctrl(),
        ) {
            set_error(
                &listener_document,
                &listener_status,
                "Wheel metadata contained a non-finite delta",
            );
            return;
        }
        event.prevent_default();
        let action = if metadata.modifiers().ctrl() {
            "zoom"
        } else {
            "wheel pan"
        };
        if let Err(error) = render(
            &listener_document,
            &listener_content,
            &listener_status,
            *viewport,
            action,
        ) {
            set_error(&listener_document, &listener_status, &error.to_string());
        }
    })
}

fn render(
    document: &WebDocument,
    content: &WebElement,
    status: &WebElement,
    viewport: GestureViewport,
    action: &str,
) -> io::Result<()> {
    content.set_attribute("style", &viewport.transform())?;
    status.set_text(&viewport.summary(action));
    if let Ok(surface) = view::element(document, "pointer-surface") {
        surface.set_attribute("data-gesture-zoom", &format!("{:.3}", viewport.zoom()))?;
    }
    Ok(())
}

fn set_error(document: &WebDocument, status: &WebElement, message: &str) {
    view::set_status_error(document, status, "Gesture", message);
}

fn wheel_unit(mode: WheelDeltaMode) -> WheelUnit {
    match mode {
        WheelDeltaMode::Pixel => WheelUnit::Pixel,
        WheelDeltaMode::Line => WheelUnit::Line,
        WheelDeltaMode::Page => WheelUnit::Page,
        _ => WheelUnit::Other,
    }
}
