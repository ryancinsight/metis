//! Closed page-to-native event representations.

use super::super::{PageEvent, SequenceId};
use super::{
    finite,
    permission::{WireObservation, observations},
    resources::WireResources,
    shape::WireIdentity,
};
use metis_frontend::FormInputs;
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum WireEvent {
    Ready {
        identity: WireIdentity,
    },
    CloseRequested {
        identity: WireIdentity,
    },
    Stopped {
        identity: WireIdentity,
        resources: WireResources,
    },
    Submit {
        identity: WireIdentity,
        request_id: String,
        inputs: WireInputs,
    },
    ProbeCompleted {
        identity: WireIdentity,
        observations: Box<[WireObservation; 4]>,
        resources: WireResources,
    },
    Failed {
        identity: WireIdentity,
        message: String,
    },
}

// The same allocation-shape invariant applies at the private Serde boundary.
const _: () = assert!(size_of::<WireEvent>() < size_of::<[WireObservation; 4]>());

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireInputs {
    patient_id: String,
    #[serde(with = "super::number")]
    weight_kg: f64,
    #[serde(with = "super::number")]
    concentration_mg_ml: f64,
    #[serde(with = "super::number")]
    target_dose_mcg_kg_min: f64,
}

impl TryFrom<&PageEvent> for WireEvent {
    type Error = io::Error;
    fn try_from(value: &PageEvent) -> io::Result<Self> {
        Ok(match value {
            PageEvent::Ready { identity } => Self::Ready {
                identity: (*identity).into(),
            },
            PageEvent::CloseRequested { identity } => Self::CloseRequested {
                identity: (*identity).into(),
            },
            PageEvent::Stopped {
                identity,
                resources,
            } => Self::Stopped {
                identity: (*identity).into(),
                resources: (*resources).into(),
            },
            PageEvent::Submit {
                identity,
                request,
                inputs,
            } => {
                finite(&[
                    inputs.weight_kg,
                    inputs.concentration_mg_ml,
                    inputs.target_dose_mcg_kg_min,
                ])?;
                Self::Submit {
                    identity: (*identity).into(),
                    request_id: request.to_string(),
                    inputs: WireInputs {
                        patient_id: inputs.patient_id.clone(),
                        weight_kg: inputs.weight_kg,
                        concentration_mg_ml: inputs.concentration_mg_ml,
                        target_dose_mcg_kg_min: inputs.target_dose_mcg_kg_min,
                    },
                }
            }
            PageEvent::ProbeCompleted {
                identity,
                observations,
                resources,
            } => {
                for observation in observations.iter() {
                    observation.validate()?;
                }
                if observations
                    .iter()
                    .zip(super::super::PageCapability::ALL)
                    .any(|(value, kind)| value.kind != kind)
                {
                    return Err(super::super::identity::invalid(
                        "Probe outcomes do not match the declared request sequence",
                    ));
                }
                Self::ProbeCompleted {
                    identity: (*identity).into(),
                    observations: Box::new(observations.each_ref().map(WireObservation::from)),
                    resources: (*resources).into(),
                }
            }
            PageEvent::Failed { identity, message } => Self::Failed {
                identity: (*identity).into(),
                message: message.clone(),
            },
        })
    }
}

impl TryFrom<WireEvent> for PageEvent {
    type Error = io::Error;
    fn try_from(value: WireEvent) -> io::Result<Self> {
        Ok(match value {
            WireEvent::Ready { identity } => Self::Ready {
                identity: identity.try_into()?,
            },
            WireEvent::CloseRequested { identity } => Self::CloseRequested {
                identity: identity.try_into()?,
            },
            WireEvent::Stopped {
                identity,
                resources,
            } => Self::Stopped {
                identity: identity.try_into()?,
                resources: resources.into(),
            },
            WireEvent::Submit {
                identity,
                request_id,
                inputs,
            } => {
                finite(&[
                    inputs.weight_kg,
                    inputs.concentration_mg_ml,
                    inputs.target_dose_mcg_kg_min,
                ])?;
                Self::Submit {
                    identity: identity.try_into()?,
                    request: SequenceId::parse(&request_id)?,
                    inputs: FormInputs::new(
                        inputs.patient_id,
                        inputs.weight_kg,
                        inputs.concentration_mg_ml,
                        inputs.target_dose_mcg_kg_min,
                    ),
                }
            }
            WireEvent::ProbeCompleted {
                identity,
                observations: values,
                resources,
            } => Self::ProbeCompleted {
                identity: identity.try_into()?,
                observations: Box::new(observations(*values)?),
                resources: resources.into(),
            },
            WireEvent::Failed { identity, message } => Self::Failed {
                identity: identity.try_into()?,
                message,
            },
        })
    }
}
