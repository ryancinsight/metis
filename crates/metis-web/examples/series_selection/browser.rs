//! Moirai owns DOM handles; this adapter retains the mounted click guard.

use super::model::{Action, SelectionState, VIEWPORT_COUNT};
use super::presentation;
use moirai_pal::wasm::{WebDocument, WebEventListener};
use std::{cell::RefCell, io};

thread_local! {
    static APPLICATION: RefCell<Option<WebEventListener>> = const { RefCell::new(None) };
}

fn mount(document: &WebDocument) -> io::Result<WebEventListener> {
    let root = presentation::element(document, "selection-app")?;
    root.set_inner_html(&presentation::markup());
    // One FnMut callback owns the state; selection needs no shared allocation
    // or interior-mutability check beyond Moirai's callback lifecycle.
    let mut state = SelectionState::<VIEWPORT_COUNT>::new();
    presentation::render(document, &state)?;
    let listener_document = document.clone();
    root.add_event_listener("click", move |event| {
        let result = (|| {
            let control = event.target().map_or(Ok(None), |target| {
                target.closest("[data-selection-control]")
            })?;
            let Some(control) = control else {
                return Ok(());
            };
            event.prevent_default();
            let result = Action::parse(&control.id()).and_then(|action| state.apply(action));
            let status = presentation::element(&listener_document, "selection-error")?;
            match result {
                Ok(()) => {
                    presentation::render(&listener_document, &state)?;
                    status.set_text("");
                }
                Err(error) => status.set_text(&format!("Selection rejected: {error}")),
            }
            Ok::<_, io::Error>(())
        })();
        if let Err(error) = result {
            presentation::error(
                &listener_document,
                &format!("Selection presentation failed: {error}"),
            );
        }
    })
}

/// Mounts fresh fixture selection state, releasing the preceding listener.
#[expect(unsafe_code, reason = "raw demonstration WASM export ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn series_selection_start() {
    APPLICATION.with_borrow_mut(|slot| *slot = None);
    if let Ok(document) = WebDocument::current() {
        presentation::error(&document, "");
        let result = (|| {
            let root = presentation::element(&document, "selection-app")?;
            root.set_attribute("data-selection-listener-count", "0")?;
            let listener = mount(&document)?;
            root.set_attribute("data-selection-listener-count", "1")?;
            Ok::<_, io::Error>(listener)
        })();
        match result {
            Ok(listener) => APPLICATION.with_borrow_mut(|slot| *slot = Some(listener)),
            Err(error) => {
                presentation::error(&document, &format!("Selection mount failed: {error}"));
            }
        }
    }
}

/// Releases the listener and removes the selection presentation.
#[expect(unsafe_code, reason = "raw demonstration WASM export ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn series_selection_stop() {
    APPLICATION.with_borrow_mut(|slot| *slot = None);
    if let Ok(document) = WebDocument::current() {
        let result = (|| {
            let root = presentation::element(&document, "selection-app")?;
            root.set_attribute("data-selection-listener-count", "0")?;
            root.set_inner_html("<p>Selection demonstration stopped.</p>");
            root.set_attribute("data-active-viewport", "")?;
            root.set_attribute("data-selected-series", "")?;
            Ok::<_, io::Error>(())
        })();
        if let Err(error) = result {
            presentation::error(&document, &format!("Selection stop failed: {error}"));
        }
    }
}
