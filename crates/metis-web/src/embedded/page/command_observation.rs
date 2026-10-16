//! Selected keyboard observations reuse the functional handler's owned metadata.

use super::coordinator::Coordinator;
use crate::embedded::{
    PageIdentity, PageRole, SequenceId,
    command_observation::{
        Admission, Decision, Event, KeyboardSnapshot, MARKER_VALUE, Outcome, producer::Producer,
    },
};
use metis_frontend::ApplicationCommand;
use moirai_pal::wasm::KeyboardMetadata;
use std::{cell::RefCell, io};

pub(super) struct Invocation {
    sequence: SequenceId,
    error: Outcome,
    closing: bool,
}

pub(super) fn configured(
    identity: PageIdentity,
    marker: Option<&str>,
) -> Option<RefCell<Producer>> {
    (identity.role() == PageRole::Form && marker == Some(MARKER_VALUE))
        .then(|| RefCell::new(Producer::new(identity)))
}

pub(super) fn selected(page: &Coordinator) {
    let Some(owner) = &page.command_observation else {
        return;
    };
    let identity = page.identity();
    let encoded = {
        let mut owner = owner.borrow_mut();
        if owner.identity() != identity {
            owner.disable(io::Error::other(
                "Command observation belongs to a different mount",
            ));
            return;
        }
        owner.selected()
    };
    publish(page, encoded);
}

/// After arming, this snapshot precedes the existing Ready and metadata reads.
pub(super) fn entered(page: &Coordinator) -> Option<Invocation> {
    let owner = page.command_observation.as_ref()?;
    if !owner.borrow().armed() {
        return None;
    }
    let identity = page.identity();
    let stage = page.session.borrow().stage();
    let (sequence, encoded) = {
        let mut owner = owner.borrow_mut();
        if owner.identity() != identity {
            owner.disable(io::Error::other(
                "Command observation belongs to a different mount",
            ));
            return None;
        }
        let sequence = owner.next_invocation()?;
        let encoded = owner.prepare(Some(sequence), Event::Entered { stage });
        (sequence, encoded)
    };
    publish(page, encoded);
    Some(Invocation {
        sequence,
        error: Outcome::ReadyRejected,
        closing: false,
    })
}

pub(super) fn phase(invocation: &mut Option<Invocation>, error: Outcome) {
    if let Some(invocation) = invocation {
        invocation.error = error;
    }
}

pub(super) fn metadata(
    page: &Coordinator,
    invocation: &mut Option<Invocation>,
    keyboard: Option<&KeyboardMetadata>,
    admission: Admission,
    decision: Decision,
) {
    let Some(owner) = &page.command_observation else {
        return;
    };
    let encoded = {
        let mut owner = owner.borrow_mut();
        if !owner.active() {
            return;
        }
        if invocation.is_none()
            && matches!(decision, Decision::Command(ApplicationCommand::ThemeDark))
        {
            let Some(sequence) = owner.arm() else {
                return;
            };
            // The first D observation is made after its real metadata and
            // command resolution. It does not invent an earlier entry record.
            *invocation = Some(Invocation {
                sequence,
                error: Outcome::ActionError,
                closing: false,
            });
        }
        let Some(invocation) = invocation else {
            return;
        };
        let keyboard = keyboard.map(|metadata| {
            let modifiers = metadata.modifiers();
            KeyboardSnapshot {
                key: metadata.key().to_owned(),
                code: metadata.code().to_owned(),
                ctrl: modifiers.ctrl(),
                alt: modifiers.alt(),
                shift: modifiers.shift(),
                meta: modifiers.meta(),
                repeat: metadata.is_repeat(),
                trusted: metadata.is_trusted(),
            }
        });
        owner.prepare(
            Some(invocation.sequence),
            Event::MetadataDecision {
                keyboard,
                admission,
                decision,
            },
        )
    };
    publish(page, encoded);
}

pub(super) fn closing(page: &Coordinator, invocation: &mut Option<Invocation>) {
    let (Some(owner), Some(invocation)) = (&page.command_observation, invocation) else {
        return;
    };
    invocation.closing = true;
    let encoded = owner
        .borrow_mut()
        .prepare(Some(invocation.sequence), Event::ClosingIntent);
    publish(page, encoded);
}

pub(super) fn returned(
    page: &Coordinator,
    invocation: Option<Invocation>,
    result: &io::Result<()>,
) {
    let (Some(owner), Some(invocation)) = (&page.command_observation, invocation) else {
        return;
    };
    if invocation.closing {
        return;
    }
    let outcome = if result.is_ok() {
        Outcome::Ok
    } else {
        invocation.error
    };
    let encoded = owner
        .borrow_mut()
        .prepare(Some(invocation.sequence), Event::Returned { outcome });
    publish(page, encoded);
}

fn publish(page: &Coordinator, encoded: Option<String>) {
    // All producer/session/application borrows end before JSON.parse and the
    // actual bridge method. A diagnostic error stays secondary and disables
    // this owner; it never calls the functional page failure path.
    if let Some(encoded) = encoded
        && let Err(error) = page.bridge.post_json(&encoded)
        && let Some(owner) = &page.command_observation
    {
        owner.borrow_mut().disable(error);
    }
}
