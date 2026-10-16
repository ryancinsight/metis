//! Finite wire spellings map to the existing lifecycle and command catalogue.

use crate::embedded::{
    PageStage,
    command_observation::{self as domain},
    identity::invalid,
};
use metis_frontend::ApplicationCommand;
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Stage {
    Loading,
    Ready,
    Stopping,
    Stopped,
    Closed,
}

impl From<PageStage> for Stage {
    fn from(value: PageStage) -> Self {
        match value {
            PageStage::Loading => Self::Loading,
            PageStage::Ready => Self::Ready,
            PageStage::Stopping => Self::Stopping,
            PageStage::Stopped => Self::Stopped,
            PageStage::Closed => Self::Closed,
        }
    }
}

impl From<Stage> for PageStage {
    fn from(value: Stage) -> Self {
        match value {
            Stage::Loading => Self::Loading,
            Stage::Ready => Self::Ready,
            Stage::Stopping => Self::Stopping,
            Stage::Stopped => Self::Stopped,
            Stage::Closed => Self::Closed,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Admission {
    Ready,
    NotReady,
    NotKeyboard,
    MetadataError,
}

impl From<domain::Admission> for Admission {
    fn from(value: domain::Admission) -> Self {
        match value {
            domain::Admission::Ready => Self::Ready,
            domain::Admission::NotReady => Self::NotReady,
            domain::Admission::NotKeyboard => Self::NotKeyboard,
            domain::Admission::MetadataError => Self::MetadataError,
        }
    }
}

impl From<Admission> for domain::Admission {
    fn from(value: Admission) -> Self {
        match value {
            Admission::Ready => Self::Ready,
            Admission::NotReady => Self::NotReady,
            Admission::NotKeyboard => Self::NotKeyboard,
            Admission::MetadataError => Self::MetadataError,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Outcome {
    Ok,
    ReadyRejected,
    MetadataError,
    ActionError,
}

impl From<domain::Outcome> for Outcome {
    fn from(value: domain::Outcome) -> Self {
        match value {
            domain::Outcome::Ok => Self::Ok,
            domain::Outcome::ReadyRejected => Self::ReadyRejected,
            domain::Outcome::MetadataError => Self::MetadataError,
            domain::Outcome::ActionError => Self::ActionError,
        }
    }
}

impl From<Outcome> for domain::Outcome {
    fn from(value: Outcome) -> Self {
        match value {
            Outcome::Ok => Self::Ok,
            Outcome::ReadyRejected => Self::ReadyRejected,
            Outcome::MetadataError => Self::MetadataError,
            Outcome::ActionError => Self::ActionError,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Decision {
    NotEvaluated {},
    Repeat {},
    EscapeMenu {},
    EscapeClose {},
    Command { command: String },
    Ignored {},
}

impl From<domain::Decision> for Decision {
    fn from(value: domain::Decision) -> Self {
        match value {
            domain::Decision::NotEvaluated => Self::NotEvaluated {},
            domain::Decision::Repeat => Self::Repeat {},
            domain::Decision::EscapeMenu => Self::EscapeMenu {},
            domain::Decision::EscapeClose => Self::EscapeClose {},
            domain::Decision::Command(command) => Self::Command {
                command: command.id().to_owned(),
            },
            domain::Decision::Ignored => Self::Ignored {},
        }
    }
}

impl TryFrom<Decision> for domain::Decision {
    type Error = io::Error;

    fn try_from(value: Decision) -> io::Result<Self> {
        Ok(match value {
            Decision::NotEvaluated {} => Self::NotEvaluated,
            Decision::Repeat {} => Self::Repeat,
            Decision::EscapeMenu {} => Self::EscapeMenu,
            Decision::EscapeClose {} => Self::EscapeClose,
            Decision::Command { command } => Self::Command(
                ApplicationCommand::from_id(&command)
                    .ok_or_else(|| invalid("Command observation command is not declared"))?,
            ),
            Decision::Ignored {} => Self::Ignored,
        })
    }
}
