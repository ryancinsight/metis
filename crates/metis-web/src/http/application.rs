//! Mounted HTTP presentation owns every listener and cancellable request.
use super::{
    lifecycle::{Lease, Lifecycle},
    probes, request,
};
use metis_core::{capability::CapabilityToken, host::HostOrigin};
use moirai_pal::wasm::{
    LocalTaskHandle, WebDocument, WebElement, WebEventListener, WebLocation,
    spawn_local_with_handle,
};
use std::{cell::RefCell, io, rc::Rc};

pub(super) struct State {
    pub(super) lifecycle: Lifecycle,
    pub(super) token: Option<CapabilityToken>,
    pub(super) origin: HostOrigin,
}
pub(super) type SharedState = Rc<RefCell<State>>;
type TaskSlot = Rc<RefCell<Option<LocalTaskHandle>>>;
struct HttpApplication {
    document: WebDocument,
    listeners: Vec<WebEventListener>,
    state: SharedState,
    task: TaskSlot,
}
thread_local! { static HTTP_APPLICATION: RefCell<Option<HttpApplication>> = const { RefCell::new(None) }; }

/// Mounts the HTTP demonstration, using Rust codecs and owned browser I/O.
///
/// Reinitialization first drops all prior listeners and requests. The real
/// loopback service validates the page Origin and demonstration principal.
#[expect(unsafe_code, reason = "stable raw WASM export ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn metis_http_start() {
    metis_http_stop();
    match WebDocument::current().and_then(HttpApplication::mount) {
        Ok(application) => HTTP_APPLICATION.with_borrow_mut(|slot| *slot = Some(application)),
        Err(error) => {
            if let Ok(document) = WebDocument::current() {
                report(&document, &error);
            }
        }
    }
}

/// Stops HTTP presentation and cancels all retained requests/listeners.
#[expect(unsafe_code, reason = "stable raw WASM export ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn metis_http_stop() {
    HTTP_APPLICATION.with_borrow_mut(|slot| *slot = None);
}

impl HttpApplication {
    fn mount(document: WebDocument) -> io::Result<Self> {
        for id in [
            "metis-status",
            "http-response",
            "http-origin",
            "metis-health",
            "metis-fragment",
            "metis-reset",
            "metis-http-stop",
            "fragment-input",
            "metis-events",
            "metis-negative",
            "metis-lifecycle",
        ] {
            element(&document, id)?;
        }
        element(&document, "http-origin")?.set_text(&WebLocation::current()?.origin());
        let endpoint = element(&document, "http-endpoint")?
            .text_content()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "HTTP endpoint configuration has no text",
                )
            })?;
        let origin = super::codec::endpoint_origin(&endpoint)?;
        let state = Rc::new(RefCell::new(State {
            lifecycle: Lifecycle::new(),
            token: None,
            origin,
        }));
        let task = Rc::new(RefCell::new(None));
        let mut listeners = Vec::new();
        for (id, probe) in [("metis-health", true), ("metis-fragment", false)] {
            let (document, state, task) = (document.clone(), Rc::clone(&state), Rc::clone(&task));
            listeners.push(
                element(&document, id)?.add_event_listener("click", move |_| {
                    launch(&document, &state, &task, probe);
                })?,
            );
        }
        let (reset_document, reset_state, reset_task) =
            (document.clone(), Rc::clone(&state), Rc::clone(&task));
        listeners.push(element(&document, "metis-reset")?.add_event_listener(
            "click",
            move |_| {
                reset_task.borrow_mut().take();
                let generation = reset_state.borrow_mut().lifecycle.reset();
                let result = generation.and_then(|generation| {
                    clear(&reset_document, generation)?;
                    ready(&reset_document, reset_state.borrow().token.is_some())
                });
                if let Err(error) = result {
                    report(&reset_document, &error);
                }
            },
        )?);
        listeners.push(
            element(&document, "metis-http-stop")?
                .add_event_listener("click", |_| metis_http_stop())?,
        );
        for id in ["metis-health", "metis-reset", "metis-http-stop"] {
            element(&document, id)?.set_disabled(false)?;
        }
        document.body()?.set_attribute(
            "data-metis-http-listener-count",
            &listeners.len().to_string(),
        )?;
        let application = Self {
            document,
            listeners,
            state,
            task,
        };
        clear(
            &application.document,
            application.state.borrow().lifecycle.mount(),
        )?;
        launch(
            &application.document,
            &application.state,
            &application.task,
            true,
        );
        Ok(application)
    }
}

impl Drop for HttpApplication {
    fn drop(&mut self) {
        self.state.borrow_mut().lifecycle.stop();
        self.task.borrow_mut().take();
        self.listeners.clear();
        let result = (|| {
            self.document
                .body()?
                .set_attribute("data-metis-http-listener-count", "0")?;
            element(&self.document, "metis-status")?.set_attribute("aria-busy", "false")?;
            for id in [
                "metis-health",
                "metis-fragment",
                "metis-reset",
                "metis-http-stop",
            ] {
                element(&self.document, id)?.set_disabled(true)?;
            }
            element(&self.document, "metis-status")?
                .set_text("HTTP host stopped; Rust-owned listeners released");
            Ok(())
        })();
        if let Err(error) = result {
            report(&self.document, &error);
        }
    }
}

fn launch(document: &WebDocument, state: &SharedState, task: &TaskSlot, probe: bool) {
    task.borrow_mut().take();
    let lease = match state.borrow_mut().lifecycle.begin() {
        Ok(lease) => lease,
        Err(error) => {
            report(document, &error);
            return;
        }
    };
    let result = (|| {
        element(document, "metis-status")?.set_attribute("aria-busy", "true")?;
        element(document, "metis-health")?.set_disabled(true)?;
        element(document, "metis-fragment")?.set_disabled(true)?;
        if probe {
            element(document, "metis-status")?.set_text("Probing the local Metis service…");
            element(document, "http-response")?.set_text("—");
            element(document, "metis-negative")?.set_text("—");
        }
        Ok(())
    })();
    if let Err(error) = result {
        state.borrow_mut().lifecycle.finish(lease);
        report(document, &error);
        return;
    }
    let (document, state, cleanup) = (document.clone(), Rc::clone(state), Rc::clone(task));
    let handle = spawn_local_with_handle(async move {
        let result = if probe {
            probes::probe(&document, &state, lease).await
        } else {
            request::render(&document, &state, lease).await
        };
        if !state.borrow().lifecycle.accepts(lease) {
            return;
        }
        if let Err(error) = result {
            report(&document, &error);
        }
        if let Err(error) = ready(&document, state.borrow().token.is_some()) {
            report(&document, &error);
        }
        state.borrow_mut().lifecycle.finish(lease);
        cleanup.borrow_mut().take();
    });
    *task.borrow_mut() = Some(handle);
}

pub(super) fn current(state: &SharedState, lease: Lease) -> io::Result<()> {
    if state.borrow().lifecycle.accepts(lease) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Stale HTTP request generation",
        ))
    }
}
pub(super) fn element(document: &WebDocument, id: &str) -> io::Result<WebElement> {
    document.get_element_by_id(id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("HTTP presentation element {id} is not mounted"),
        )
    })
}
fn ready(document: &WebDocument, authorized: bool) -> io::Result<()> {
    element(document, "metis-health")?.set_disabled(false)?;
    element(document, "metis-fragment")?.set_disabled(!authorized)?;
    element(document, "metis-status")?.set_attribute("aria-busy", "false")
}
fn clear(document: &WebDocument, generation: u64) -> io::Result<()> {
    for id in ["http-response", "metis-events", "metis-negative"] {
        element(document, id)?.set_text("—");
    }
    element(document, "metis-lifecycle")?.set_text(&format!("generation {generation}"));
    element(document, "metis-status")?
        .set_text("Mount reset; the previous fragment generation is stale");
    Ok(())
}
fn report(document: &WebDocument, error: &io::Error) {
    if let Some(status) = document.get_element_by_id("metis-status") {
        status.set_text("Authenticated fragment boundary unavailable");
    }
    if let Some(response) = document.get_element_by_id("http-response") {
        response.set_text(&error.to_string());
    }
}
