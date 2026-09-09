//! Browser event handlers for the shared result explorer.

use super::BrowserState;
use super::view;
use metis_core::error::{ErrorCode, MetisError};
use metis_frontend::SortOrder;
use moirai_pal::wasm::{WebDocument, WebElement, WebEventListener};
use std::cell::RefCell;
use std::io;
use std::rc::Rc;

pub(super) fn listeners(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
) -> io::Result<Vec<WebEventListener>> {
    let root = view::element(document, "metis-app")?;
    Ok(vec![
        filter_listener(document, state, &root)?,
        sort_listener(document, state, &root)?,
        entry_listener(document, state, &root)?,
    ])
}

fn filter_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    root: &WebElement,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    root.add_event_listener("input", move |event| {
        let Some(target) = event.target() else {
            return;
        };
        if target.id() != "explorer-filter" {
            return;
        }
        let value = event.value().unwrap_or_default();
        let mut state = listener_state.borrow_mut();
        if let Err(error) = state.result_explorer.set_filter(&value) {
            state.result_explorer.fail(error);
        }
        if let Err(error) = view::render(&listener_document, &state) {
            view::set_mount_error(&listener_document, &error);
        }
    })
}

fn sort_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    root: &WebElement,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    root.add_event_listener("change", move |event| {
        let Some(target) = event.target() else {
            return;
        };
        if target.id() != "explorer-sort" {
            return;
        }
        let Some(value) = event.value() else {
            return;
        };
        let mut state = listener_state.borrow_mut();
        let Some(order) = SortOrder::parse(&value) else {
            state.result_explorer.fail(MetisError::protocol(
                ErrorCode::MalformedPayload,
                "Result explorer sort control contains an unsupported value",
            ));
            if let Err(error) = view::render(&listener_document, &state) {
                view::set_mount_error(&listener_document, &error);
            }
            return;
        };
        if let Err(error) = state.result_explorer.set_sort(order) {
            state.result_explorer.fail(error);
        }
        if let Err(error) = view::render(&listener_document, &state) {
            view::set_mount_error(&listener_document, &error);
        }
    })
}

fn entry_listener(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    root: &WebElement,
) -> io::Result<WebEventListener> {
    let listener_document = document.clone();
    let listener_state = Rc::clone(state);
    root.add_event_listener("click", move |event| {
        let Some(target) = event.target() else {
            return;
        };
        let id = target.id();
        let mut state = listener_state.borrow_mut();
        let changed = match id.as_str() {
            "explorer-previous" => {
                state.result_explorer.previous_page();
                true
            }
            "explorer-next" => {
                state.result_explorer.next_page();
                true
            }
            _ => id
                .strip_prefix("explorer-entry-")
                .and_then(|slot| slot.parse::<usize>().ok())
                .is_some_and(|slot| state.result_explorer.activate_visible(slot)),
        };
        if changed && let Err(error) = view::render(&listener_document, &state) {
            view::set_mount_error(&listener_document, &error);
        }
    })
}
