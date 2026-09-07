use super::view;
use moirai_pal::wasm::{WebDocument, WebEventListener};
use std::io;

pub(super) fn listeners(document: &WebDocument) -> io::Result<Vec<WebEventListener>> {
    let opener = view::element(document, "open-session-dialog")?;
    let dialog = view::element(document, "session-dialog")?;
    let close_button = view::element(document, "session-dialog-close")?;
    let mut listeners = Vec::with_capacity(3);

    let listener_document = document.clone();
    let listener_dialog = dialog.clone();
    listeners.push(opener.add_event_listener("click", move |event| {
        event.prevent_default();
        if listener_dialog.dialog_open() != Some(true)
            && let Err(error) = listener_dialog.show_modal()
        {
            view::set_mount_error(&listener_document, &error);
        }
    })?);

    let listener_document = document.clone();
    let listener_dialog = dialog.clone();
    listeners.push(close_button.add_event_listener("click", move |event| {
        event.prevent_default();
        if let Err(error) = listener_dialog.close_dialog() {
            view::set_mount_error(&listener_document, &error);
        }
    })?);

    let listener_document = document.clone();
    listeners.push(dialog.add_event_listener("close", move |_event| {
        if let Err(error) = opener.focus() {
            view::set_mount_error(&listener_document, &error);
        }
    })?);

    Ok(listeners)
}
