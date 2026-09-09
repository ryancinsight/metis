use super::view;
use moirai_pal::wasm::{
    PointerMetadata, PointerModifiers, WebDocument, WebElement, WebEventListener,
};
use std::cell::Cell;
use std::io;
use std::rc::Rc;

const MAX_CAPTURED_POINTERS: usize = 2;

pub(super) fn listeners(document: &WebDocument) -> io::Result<Vec<WebEventListener>> {
    let surface = view::element(document, "pointer-surface")?;
    let status = view::element(document, "pointer-status")?;
    let active_pointers = Rc::new(Cell::new([None; MAX_CAPTURED_POINTERS]));
    let mut listeners = Vec::with_capacity(4);

    let listener_document = document.clone();
    let listener_surface = surface.clone();
    let listener_status = status.clone();
    let listener_active_pointers = Rc::clone(&active_pointers);
    listeners.push(surface.add_event_listener("pointerdown", move |event| {
        event.prevent_default();
        pointer_down(
            &listener_document,
            &listener_surface,
            &listener_status,
            &listener_active_pointers,
            event.pointer_metadata(),
        );
    })?);

    let listener_status = status.clone();
    let listener_active_pointers = Rc::clone(&active_pointers);
    listeners.push(surface.add_event_listener("pointermove", move |event| {
        pointer_move(
            &listener_status,
            &listener_active_pointers,
            event.pointer_metadata(),
        );
    })?);

    let listener_document = document.clone();
    let listener_surface = surface.clone();
    let listener_status = status.clone();
    let listener_active_pointers = Rc::clone(&active_pointers);
    listeners.push(surface.add_event_listener("pointerup", move |event| {
        release(
            &listener_document,
            &listener_surface,
            &listener_status,
            &listener_active_pointers,
            event.pointer_metadata(),
        );
    })?);

    let listener_document = document.clone();
    let listener_surface = surface.clone();
    let callback_surface = listener_surface.clone();
    let listener_status = status;
    let listener_active_pointers = active_pointers;
    listeners.push(
        listener_surface.add_event_listener("pointercancel", move |event| {
            release(
                &listener_document,
                &callback_surface,
                &listener_status,
                &listener_active_pointers,
                event.pointer_metadata(),
            );
        })?,
    );

    Ok(listeners)
}

fn pointer_down(
    document: &WebDocument,
    surface: &WebElement,
    status: &WebElement,
    active_pointers: &Cell<[Option<i32>; MAX_CAPTURED_POINTERS]>,
    metadata: Option<PointerMetadata>,
) {
    let Some(metadata) = metadata else {
        set_error(document, status, "Pointer event did not carry metadata");
        return;
    };
    let pointer_id = metadata.pointer_id();
    let active = active_pointers.get();
    if active
        .iter()
        .flatten()
        .any(|active_id| *active_id == pointer_id)
    {
        set_error(document, status, "Pointer identifier is already captured");
        return;
    }
    let Some(slot) = active.iter().position(Option::is_none) else {
        set_error(
            document,
            status,
            "The gesture surface already captures two pointers",
        );
        return;
    };
    if let Err(error) = surface.set_pointer_capture(pointer_id) {
        set_error(document, status, &error.to_string());
        return;
    }
    if !surface.has_pointer_capture(pointer_id) {
        set_error(document, status, "Browser did not retain pointer capture");
        return;
    }
    let mut next = active;
    next[slot] = Some(pointer_id);
    active_pointers.set(next);
    status.set_text(&format!(
        "Pointer capture: {} — {}",
        capture_summary(next),
        metadata_summary(metadata)
    ));
}

fn pointer_move(
    status: &WebElement,
    active_pointers: &Cell<[Option<i32>; MAX_CAPTURED_POINTERS]>,
    metadata: Option<PointerMetadata>,
) {
    let Some(metadata) = metadata else {
        return;
    };
    if active_pointers
        .get()
        .iter()
        .flatten()
        .any(|active_id| *active_id == metadata.pointer_id())
    {
        status.set_text(&format!(
            "Pointer capture: {} — {}",
            capture_summary(active_pointers.get()),
            metadata_summary(metadata)
        ));
    }
}

fn release(
    document: &WebDocument,
    surface: &WebElement,
    status: &WebElement,
    active_pointers: &Cell<[Option<i32>; MAX_CAPTURED_POINTERS]>,
    metadata: Option<PointerMetadata>,
) {
    let Some(metadata) = metadata else {
        set_error(document, status, "Pointer event did not carry metadata");
        return;
    };
    let pointer_id = metadata.pointer_id();
    let active = active_pointers.get();
    let Some(slot) = active
        .iter()
        .position(|active_id| *active_id == Some(pointer_id))
    else {
        set_error(
            document,
            status,
            "Pointer release did not match capture state",
        );
        return;
    };
    if !surface.has_pointer_capture(pointer_id) {
        set_error(
            document,
            status,
            "Browser lost pointer capture before release",
        );
        let mut next = active;
        next[slot] = None;
        active_pointers.set(next);
        return;
    }
    if let Err(error) = surface.release_pointer_capture(pointer_id) {
        set_error(document, status, &error.to_string());
        return;
    }
    let mut next = active;
    next[slot] = None;
    active_pointers.set(next);
    status.set_text(&format!(
        "Pointer capture: {} — released ({pointer_id}) — {}",
        capture_summary(next),
        metadata_summary(metadata)
    ));
}

fn capture_summary(active: [Option<i32>; MAX_CAPTURED_POINTERS]) -> String {
    let mut summary = String::new();
    let mut first = true;
    for pointer_id in active.into_iter().flatten() {
        if first {
            summary.push_str(" (");
            first = false;
        } else {
            summary.push_str(", ");
        }
        summary.push_str(&pointer_id.to_string());
    }
    if first {
        "none".to_owned()
    } else {
        summary.push(')');
        summary
    }
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
