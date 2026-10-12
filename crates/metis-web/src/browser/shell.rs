//! Rust owns configuration and shell listeners beyond application remounts.
use super::{metis_start, metis_stop, view};
use moirai_pal::wasm::{WebDocument, WebEventListener, WebLocation};
use std::{cell::RefCell, io};
thread_local! { static SHELL: RefCell<Option<Vec<WebEventListener>>> = const { RefCell::new(None) }; }

/// Initializes the workbench shell and application from the current document.
///
/// Rust owns query values and listeners. The module loader instantiates WASM
/// and calls this boundary. Reinitialization releases the prior shell; opaque
/// destination origins never pass the navigation policy.
#[expect(unsafe_code, reason = "stable raw WASM export ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn metis_browser_start() {
    metis_browser_stop();
    let result = WebDocument::current().and_then(|document| {
        configure(&document)?;
        let start = view::element(&document, "metis-start")?
            .add_event_listener("click", |_| metis_start())?;
        let stop = view::element(&document, "metis-stop")?
            .add_event_listener("click", |_| metis_stop())?;
        let navigation = document.add_event_listener("click", |event| {
            let destination = event
                .target()
                .and_then(|target| target.closest("a[href]").ok().flatten())
                .and_then(|link| link.attribute("href"));
            if let Some(destination) = destination {
                let admitted = WebLocation::current()
                    .and_then(|current| {
                        current.resolve(&destination).map(|target| {
                            let origin = current.origin();
                            origin != "null" && origin == target.origin()
                        })
                    })
                    .unwrap_or(false);
                if !admitted {
                    event.prevent_default();
                }
            }
        })?;
        let listeners = vec![start, stop, navigation];
        document.body()?.set_attribute(
            "data-metis-shell-listener-count",
            &listeners.len().to_string(),
        )?;
        Ok(listeners)
    });
    match result {
        Ok(listeners) => {
            SHELL.with_borrow_mut(|slot| *slot = Some(listeners));
            metis_start();
        }
        Err(error) => {
            if let Ok(document) = WebDocument::current() {
                view::set_mount_error(&document, &error);
            }
        }
    }
}

/// Releases the workbench shell and application listeners.
#[expect(unsafe_code, reason = "stable raw WASM export ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn metis_browser_stop() {
    SHELL.with_borrow_mut(|slot| *slot = None);
    metis_stop();
    if let Ok(body) = WebDocument::current().and_then(|document| document.body())
        && let Err(error) = body.set_attribute("data-metis-shell-listener-count", "0")
        && let Ok(document) = WebDocument::current()
    {
        view::set_mount_error(&document, &error);
    }
}

fn configure(document: &WebDocument) -> io::Result<()> {
    let location = WebLocation::current()?;
    for (name, id, bound) in [
        ("endpoint", "metis-websocket-endpoint", 2048),
        ("process", "metis-process-id", 10),
        ("principal", "metis-principal", 32),
    ] {
        if let Some(value) = location.parameter(name, bound)? {
            view::element(document, id)?.set_value(&value)?;
        }
    }
    Ok(())
}
