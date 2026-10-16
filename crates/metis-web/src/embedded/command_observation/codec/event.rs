//! Closed serialized events remain separate from public domain observations.

use super::{required_option, words};
use crate::embedded::{
    command_observation::{Event, KeyboardSnapshot},
    identity::decimal,
};
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum WireEvent {
    Selected {},
    Entered {
        stage: words::Stage,
    },
    MetadataDecision {
        #[serde(deserialize_with = "required_option")]
        keyboard: Option<WireKeyboard>,
        admission: words::Admission,
        decision: words::Decision,
    },
    Returned {
        outcome: words::Outcome,
    },
    ClosingIntent {},
    Quota {
        prior_records: u32,
        prior_bytes: u32,
        omitted_at_notice: String,
        failed: bool,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Six independent observed KeyboardEvent flags admit all combinations; they are not alternative control states"
)]
pub(super) struct WireKeyboard {
    key: String,
    code: String,
    ctrl: bool,
    alt: bool,
    shift: bool,
    meta: bool,
    repeat: bool,
    trusted: bool,
}

impl From<&Event> for WireEvent {
    fn from(event: &Event) -> Self {
        match event {
            Event::Selected => Self::Selected {},
            Event::Entered { stage } => Self::Entered {
                stage: (*stage).into(),
            },
            Event::MetadataDecision {
                keyboard,
                admission,
                decision,
            } => Self::MetadataDecision {
                keyboard: keyboard.as_ref().map(WireKeyboard::from),
                admission: (*admission).into(),
                decision: (*decision).into(),
            },
            Event::Returned { outcome } => Self::Returned {
                outcome: (*outcome).into(),
            },
            Event::ClosingIntent => Self::ClosingIntent {},
            Event::Quota {
                prior_records,
                prior_bytes,
                omitted_at_notice,
                failed,
            } => Self::Quota {
                prior_records: *prior_records,
                prior_bytes: *prior_bytes,
                omitted_at_notice: omitted_at_notice.to_string(),
                failed: *failed,
            },
        }
    }
}

impl TryFrom<WireEvent> for Event {
    type Error = io::Error;

    fn try_from(event: WireEvent) -> io::Result<Self> {
        Ok(match event {
            WireEvent::Selected {} => Self::Selected,
            WireEvent::Entered { stage } => Self::Entered {
                stage: stage.into(),
            },
            WireEvent::MetadataDecision {
                keyboard,
                admission,
                decision,
            } => Self::MetadataDecision {
                keyboard: keyboard.map(KeyboardSnapshot::from),
                admission: admission.into(),
                decision: decision.try_into()?,
            },
            WireEvent::Returned { outcome } => Self::Returned {
                outcome: outcome.into(),
            },
            WireEvent::ClosingIntent {} => Self::ClosingIntent,
            WireEvent::Quota {
                prior_records,
                prior_bytes,
                omitted_at_notice,
                failed,
            } => Self::Quota {
                prior_records,
                prior_bytes,
                omitted_at_notice: decimal(&omitted_at_notice)?,
                failed,
            },
        })
    }
}

impl From<&KeyboardSnapshot> for WireKeyboard {
    fn from(value: &KeyboardSnapshot) -> Self {
        Self {
            key: value.key.clone(),
            code: value.code.clone(),
            ctrl: value.ctrl,
            alt: value.alt,
            shift: value.shift,
            meta: value.meta,
            repeat: value.repeat,
            trusted: value.trusted,
        }
    }
}

impl From<WireKeyboard> for KeyboardSnapshot {
    fn from(value: WireKeyboard) -> Self {
        Self {
            key: value.key,
            code: value.code,
            ctrl: value.ctrl,
            alt: value.alt,
            shift: value.shift,
            meta: value.meta,
            repeat: value.repeat,
            trusted: value.trusted,
        }
    }
}
