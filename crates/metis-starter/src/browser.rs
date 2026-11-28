//! Binds the starter page's form to [`crate::greet`].
//!
//! The page ships static markup; Rust finds the form by id, cancels its
//! navigation, reads the name and writes the greeting. The generated module
//! loader invokes [`start`] as part of WebAssembly initialization.

use crate::greet;
use moirai_pal::wasm::{WebDocument, WebElement, WebEventListener};
use std::cell::RefCell;
use std::io;

const FORM: &str = "greet-form";
const INPUT: &str = "greet-input";
const MESSAGE: &str = "greet-msg";

thread_local! {
    /// The submit listener; dropping it would detach the handler.
    static SUBMIT: RefCell<Option<WebEventListener>> = const { RefCell::new(None) };
}

/// Starts the page and marks the form `data-metis-ready` once Rust owns submission.
///
/// `wasm-bindgen` calls this hook once while initializing a module instance.
/// The listener remains retained for that instance's lifetime.
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    if let Err(error) = bind() {
        show(&format!("The starter could not start: {error}"));
    }
}

fn bind() -> io::Result<()> {
    let document = WebDocument::current()?;
    let form = element(&document, FORM)?;
    let listener = form.add_event_listener("submit", |event| {
        event.prevent_default();
        let greeting = WebDocument::current()
            .and_then(|document| element(&document, INPUT))
            .and_then(|input| {
                input.value().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "#greet-input is not an input")
                })
            })
            .map(|name| greet(&name));
        match greeting {
            Ok(greeting) => show(&greeting),
            Err(error) => show(&format!("The name could not be read: {error}")),
        }
    })?;
    SUBMIT.with_borrow_mut(|slot| *slot = Some(listener));
    form.set_attribute("data-metis-ready", "true")
}

fn element(document: &WebDocument, id: &str) -> io::Result<WebElement> {
    document.get_element_by_id(id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("the page has no #{id} element"),
        )
    })
}

/// Writes `text` into the message line, the page's one output.
fn show(text: &str) {
    if let Ok(document) = WebDocument::current()
        && let Ok(message) = element(&document, MESSAGE)
    {
        message.set_text(text);
    }
}
