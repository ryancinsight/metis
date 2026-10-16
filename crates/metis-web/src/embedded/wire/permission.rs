//! Closed actual browser outcome encoding, separate from native observations.

use super::super::identity::invalid;
use super::super::{BrowserPermissionResult, CapabilityObservation, PageCapability};
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireObservation {
    kind: String,
    browser: WireBrowserResult,
    host_denial: Option<bool>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum WireBrowserResult {
    Granted,
    Rejected {
        name: Option<String>,
        code: Option<u32>,
        message: String,
    },
    Denied,
    Default,
    Unavailable {
        message: String,
    },
}

impl From<&CapabilityObservation> for WireObservation {
    fn from(value: &CapabilityObservation) -> Self {
        Self {
            kind: value.kind.as_str().to_owned(),
            host_denial: value.host_denial,
            browser: match &value.browser {
                BrowserPermissionResult::Granted => WireBrowserResult::Granted,
                BrowserPermissionResult::Rejected {
                    name,
                    code,
                    message,
                } => WireBrowserResult::Rejected {
                    name: name.clone(),
                    code: *code,
                    message: message.clone(),
                },
                BrowserPermissionResult::Denied => WireBrowserResult::Denied,
                BrowserPermissionResult::Default => WireBrowserResult::Default,
                BrowserPermissionResult::Unavailable { message } => {
                    WireBrowserResult::Unavailable {
                        message: message.clone(),
                    }
                }
            },
        }
    }
}

impl TryFrom<WireObservation> for CapabilityObservation {
    type Error = io::Error;
    fn try_from(value: WireObservation) -> io::Result<Self> {
        let observation = Self {
            kind: PageCapability::parse(&value.kind)
                .ok_or_else(|| invalid("Page capability is not declared"))?,
            host_denial: value.host_denial,
            browser: match value.browser {
                WireBrowserResult::Granted => BrowserPermissionResult::Granted,
                WireBrowserResult::Rejected {
                    name,
                    code,
                    message,
                } => BrowserPermissionResult::Rejected {
                    name,
                    code,
                    message,
                },
                WireBrowserResult::Denied => BrowserPermissionResult::Denied,
                WireBrowserResult::Default => BrowserPermissionResult::Default,
                WireBrowserResult::Unavailable { message } => {
                    BrowserPermissionResult::Unavailable { message }
                }
            },
        };
        observation.validate()?;
        Ok(observation)
    }
}

pub(super) fn observations(values: [WireObservation; 4]) -> io::Result<[CapabilityObservation; 4]> {
    let admitted = values
        .into_iter()
        .map(CapabilityObservation::try_from)
        .collect::<io::Result<Vec<_>>>()?;
    let admitted: [CapabilityObservation; 4] = admitted.try_into().map_err(|values: Vec<_>| {
        invalid(format!(
            "Probe has {} outcomes rather than four",
            values.len()
        ))
    })?;
    if admitted
        .iter()
        .zip(PageCapability::ALL)
        .any(|(value, expected)| value.kind != expected)
    {
        return Err(invalid(
            "Probe outcomes duplicate or reorder the declared request sequence",
        ));
    }
    Ok(admitted)
}
