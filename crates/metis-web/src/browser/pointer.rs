use super::view;
use moirai_pal::wasm::{
    PointerMetadata, PointerModifiers, WebDocument, WebElement, WebEventListener,
};
use std::cell::Cell;
use std::io;
use std::rc::Rc;

pub(super) fn listeners(document: &WebDocument) -> io::Result<Vec<WebEventListener>> {
    let surface = view::element(document, "pointer-surface")?;
    let status = view::element(document, "pointer-status")?;
    let active_pointer = Rc::new(Cell::new(None));
    let mut listeners = Vec::with_capacity(4);

    let listener_document = document.clone();
    let listener_surface = surface.clone();
    let listener_status = status.clone();
    let listener_active_pointer = Rc::clone(&active_pointer);
    listeners.push(surface.add_event_listener("pointerdown", move |event| {
        let Some(metadata) = event.pointer_metadata() else {
            set_error(
                &listener_document,
                &listener_status,
                "Pointer event did not carry metadata",
            );
            return;
        };
        let pointer_id = metadata.pointer_id();
        event.prevent_default();
        if listener_active_pointer.get().is_some() {
            set_error(
                &listener_document,
                &listener_status,
                "Another pointer is already captured",
            );
            return;
        }
        if let Err(error) = listener_surface.set_pointer_capture(pointer_id) {
            set_error(&listener_document, &listener_status, &error.to_string());
            return;
        }
        if !listener_surface.has_pointer_capture(pointer_id) {
            set_error(
                &listener_document,
                &listener_status,
                "Browser did not retain pointer capture",
            );
            return;
        }
        listener_active_pointer.set(Some(pointer_id));
        listener_status.set_text(&format!(
            "Pointer capture: active ({pointer_id}) — {}",
            metadata_summary(metadata)
        ));
    })?);

    let listener_status = status.clone();
    let listener_active_pointer = Rc::clone(&active_pointer);
    listeners.push(surface.add_event_listener("pointermove", move |event| {
        let Some(metadata) = event.pointer_metadata() else {
            return;
        };
        if listener_active_pointer.get() == Some(metadata.pointer_id()) {
            listener_status.set_text(&format!(
                "Pointer capture: active ({}) — {}",
                metadata.pointer_id(),
                metadata_summary(metadata)
            ));
        }
    })?);

    let listener_document = document.clone();
    let listener_surface = surface.clone();
    let listener_status = status.clone();
    let listener_active_pointer = Rc::clone(&active_pointer);
    listeners.push(surface.add_event_listener("pointerup", move |event| {
        release(
            &listener_document,
            &listener_surface,
            &listener_status,
            &listener_active_pointer,
            event.pointer_metadata(),
        );
    })?);

    let listener_document = document.clone();
    let listener_surface = surface.clone();
    let callback_surface = listener_surface.clone();
    let listener_status = status;
    let listener_active_pointer = active_pointer;
    listeners.push(
        listener_surface.add_event_listener("pointercancel", move |event| {
            release(
                &listener_document,
                &callback_surface,
                &listener_status,
                &listener_active_pointer,
                event.pointer_metadata(),
            );
        })?,
    );

    Ok(listeners)
}

fn release(
    document: &WebDocument,
    surface: &WebElement,
    status: &WebElement,
    active_pointer: &Cell<Option<i32>>,
    metadata: Option<PointerMetadata>,
) {
    let Some(metadata) = metadata else {
        set_error(document, status, "Pointer event did not carry metadata");
        return;
    };
    let pointer_id = metadata.pointer_id();
    if active_pointer.get() != Some(pointer_id) {
        set_error(
            document,
            status,
            "Pointer release did not match capture state",
        );
        return;
    }
    if !surface.has_pointer_capture(pointer_id) {
        set_error(
            document,
            status,
            "Browser lost pointer capture before release",
        );
        active_pointer.set(None);
        return;
    }
    if let Err(error) = surface.release_pointer_capture(pointer_id) {
        set_error(document, status, &error.to_string());
        return;
    }
    active_pointer.set(None);
    status.set_text(&format!(
        "Pointer capture: released ({pointer_id}) — {}",
        metadata_summary(metadata)
    ));
}

fn metadata_summary(metadata: PointerMetadata) -> String {
    format!(
        "{} at ({}, {}), button {}, buttons {}, modifiers {}, {}",
        metadata.pointer_type().as_str(),
        metadata.client_x(),
        metadata.client_y(),
        metadata.button(),
        metadata.buttons(),
        modifier_summary(metadata.modifiers()),
        if metadata.is_primary() {
            "primary"
        } else {
            "secondary"
        }
    )
}

pub(super) fn modifier_summary(modifiers: PointerModifiers) -> String {
    let mut summary = String::new();
    for (active, label) in [
        (modifiers.ctrl(), "Ctrl"),
        (modifiers.shift(), "Shift"),
        (modifiers.alt(), "Alt"),
        (modifiers.meta(), "Meta"),
    ] {
        if active {
            if !summary.is_empty() {
                summary.push('+');
            }
            summary.push_str(label);
        }
    }
    if summary.is_empty() {
        summary.push_str("none");
    }
    summary
}

fn set_error(document: &WebDocument, status: &WebElement, message: &str) {
    view::set_status_error(document, status, "Pointer capture", message);
}
