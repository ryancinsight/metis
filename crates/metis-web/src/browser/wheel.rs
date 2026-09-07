use super::{pointer, view};
use moirai_pal::wasm::{WebDocument, WebElement, WebEventListener, WheelMetadata};
use std::io;

pub(super) fn listeners(document: &WebDocument) -> io::Result<Vec<WebEventListener>> {
    let surface = view::element(document, "pointer-surface")?;
    let status = view::element(document, "wheel-status")?;
    let listener_document = document.clone();
    let listener_status = status.clone();
    Ok(vec![surface.add_event_listener("wheel", move |event| {
        let Some(metadata) = event.wheel_metadata() else {
            set_error(
                &listener_document,
                &listener_status,
                "Wheel event did not carry metadata",
            );
            return;
        };
        event.prevent_default();
        listener_status.set_text(&format!("Wheel: {}", metadata_summary(metadata)));
    })?])
}

fn metadata_summary(metadata: WheelMetadata) -> String {
    format!(
        "delta ({:.2}, {:.2}, {:.2}) {} at ({}, {}), modifiers {}",
        metadata.delta_x(),
        metadata.delta_y(),
        metadata.delta_z(),
        metadata.delta_mode().as_str(),
        metadata.client_x(),
        metadata.client_y(),
        pointer::modifier_summary(metadata.modifiers()),
    )
}

fn set_error(document: &WebDocument, status: &WebElement, message: &str) {
    view::set_status_error(document, status, "Wheel", message);
}
