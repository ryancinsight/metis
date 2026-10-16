//! Captures sanitized HTML numeric values and consumes only correlated replies.

use super::{
    application::State,
    coordinator::{Coordinator, invalid},
    view,
};
use crate::embedded::{PageCommand, PageEvent};
use metis_frontend::FormInputs;
use moirai_pal::wasm::{WebEventListener, WebNumber};
use std::{cell::RefCell, io, rc::Rc};

/// Reset request presentation after stop invalidates the previous pending ID.
pub(super) fn initialize(page: &Coordinator) -> io::Result<()> {
    view::element(&page.document, "calculation-submit")?.set_disabled(false)?;
    view::element(&page.document, "result")?.set_text("No calculation submitted.");
    Ok(())
}

pub(super) fn listener(
    page: &Rc<Coordinator>,
    state: &Rc<RefCell<State>>,
) -> io::Result<WebEventListener> {
    let weak = Rc::downgrade(page);
    let state = Rc::clone(state);
    view::element(&page.document, "calculation")?.add_event_listener("submit", move |event| {
        event.prevent_default();
        let Some(page) = weak.upgrade() else {
            return;
        };
        let result = submit(&page, &state);
        if let Err(error) = result {
            page.failure(&error);
        }
    })
}

fn submit(page: &Coordinator, state: &RefCell<State>) -> io::Result<()> {
    page.require_ready(page.identity())?;
    let inputs = FormInputs::new(
        value(page, "patient-id")?,
        number(page, "weight")?,
        number(page, "concentration")?,
        number(page, "dose")?,
    );
    let request = state
        .borrow()
        .next_request
        .ok_or_else(|| invalid("Form request sequence is exhausted"))?;
    let identity = page.identity();
    page.session.borrow_mut().begin_request(identity, request)?;
    state.borrow_mut().next_request = request.next().ok();
    view::element(&page.document, "calculation-submit")?.set_disabled(true)?;
    view::element(&page.document, "result")?.set_text("Submitting to the supervised backend…");
    page.post(&PageEvent::Submit {
        identity: page.identity(),
        request,
        inputs,
    })
}

fn value(page: &Coordinator, id: &str) -> io::Result<String> {
    view::element(&page.document, id)?
        .value()
        .ok_or_else(|| invalid(format!("Control {id} has no value")))
}

fn number(page: &Coordinator, id: &str) -> io::Result<f64> {
    let value = value(page, id)?;
    // HTML number controls sanitize non-numeric editing to the empty value.
    // Number("") in the former controller is exactly zero; native validation
    // remains responsible for domain positivity and backend admission.
    if value.is_empty() {
        return Ok(0.0);
    }
    let value = value
        .parse::<f64>()
        .map_err(|error| invalid(format!("Control {id} numeric value: {error}")))?;
    if !value.is_finite() {
        return Err(invalid(format!("Control {id} is not finite")));
    }
    Ok(value)
}

pub(super) fn reply(page: &Coordinator, command: PageCommand) -> io::Result<()> {
    match command {
        PageCommand::Result {
            identity,
            request,
            response,
        } => {
            page.session
                .borrow_mut()
                .complete_request(identity, request)?;
            view::element(&page.document, "result")?.set_text(&format!(
                "Rate {} mL/hour; drug {} mg/hour; audit {}",
                WebNumber::decimal(response.rate_ml_hr)?,
                WebNumber::decimal(response.drug_rate_mg_hr)?,
                response.audit_sequence_id
            ));
            view::element(&page.document, "calculation-submit")?.set_disabled(false)
        }
        PageCommand::Error {
            identity,
            request,
            status,
            error_code,
            message,
        } => {
            page.require_ready(identity)?;
            if let Some(request) = request {
                page.session
                    .borrow_mut()
                    .complete_request(identity, request)?;
                view::element(&page.document, "calculation-submit")?.set_disabled(false)?;
            }
            view::element(&page.document, "result")?
                .set_text(&format!("{status}: {message} [0x{error_code:04x}]"));
            Ok(())
        }
        _ => Err(invalid("Form reply is not a terminal native command")),
    }
}
