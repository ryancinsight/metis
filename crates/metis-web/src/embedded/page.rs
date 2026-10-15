//! The Rust controller retains one native coordinator across application mounts.

mod application;
mod command_observation;
mod commands;
mod coordinator;
mod form;
mod permissions;
mod resources;
mod view;

use coordinator::Coordinator;
use moirai_pal::wasm::WebDocument;
use std::{cell::RefCell, rc::Rc};

thread_local! {
    static PAGE: RefCell<Option<Rc<Coordinator>>> = const { RefCell::new(None) };
}

/// Installs the exact packaged Rust page and its real native-message bridge.
/// Initialization failure attempts document rendering. Failures after coordinator
/// acquisition also attempt correlated native publication.
#[expect(unsafe_code, reason = "stable raw WASM export ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn metis_embedded_start() {
    let occupied = PAGE.with_borrow(Option::is_some);
    if occupied {
        if let Ok(document) = WebDocument::current() {
            view::failure(&document, "Embedded coordinator already owns this document");
        }
        return;
    }
    match Coordinator::start() {
        Ok(page) => PAGE.with_borrow_mut(|slot| *slot = Some(page)),
        Err(error) => {
            if let Ok(document) = WebDocument::current() {
                view::failure(&document, &error.to_string());
            }
        }
    }
}

/// Requests application teardown while retaining the real coordinator for acknowledgement.
#[expect(unsafe_code, reason = "stable raw WASM export ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn metis_embedded_stop() {
    PAGE.with_borrow(|slot| {
        if let Some(page) = slot
            && let Err(error) = page.stop(page.identity())
        {
            page.failure(&error);
        }
    });
}
