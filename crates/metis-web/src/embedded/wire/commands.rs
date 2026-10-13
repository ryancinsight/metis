//! Closed native-to-page command representations.

use super::super::identity::{decimal, invalid};
use super::super::{PageCapability, PageCommand, SequenceId};
use super::{finite, shape::WireIdentity};
use metis_core::protocol::ClinicalCalcResponsePayload;
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum WireCommand {
    Mount {
        identity: WireIdentity,
    },
    Stop {
        identity: WireIdentity,
    },
    Result {
        identity: WireIdentity,
        request_id: String,
        audit_sequence_id: String,
        #[serde(with = "super::number")]
        rate_ml_hr: f64,
        #[serde(with = "super::number")]
        drug_rate_mg_hr: f64,
        is_pediatric: bool,
        result_signature: [u8; 32],
    },
    Error {
        identity: WireIdentity,
        request_id: Option<String>,
        status: String,
        error_code: u16,
        message: String,
    },
    PermissionDenied {
        identity: WireIdentity,
        permission: String,
        user_initiated: bool,
    },
}

impl TryFrom<&PageCommand> for WireCommand {
    type Error = io::Error;
    fn try_from(value: &PageCommand) -> io::Result<Self> {
        Ok(match value {
            PageCommand::Mount { identity } => Self::Mount {
                identity: (*identity).into(),
            },
            PageCommand::Stop { identity } => Self::Stop {
                identity: (*identity).into(),
            },
            PageCommand::Result {
                identity,
                request,
                response,
            } => {
                finite(&[response.rate_ml_hr, response.drug_rate_mg_hr])?;
                Self::Result {
                    identity: (*identity).into(),
                    request_id: request.to_string(),
                    audit_sequence_id: response.audit_sequence_id.to_string(),
                    rate_ml_hr: response.rate_ml_hr,
                    drug_rate_mg_hr: response.drug_rate_mg_hr,
                    is_pediatric: response.is_pediatric,
                    result_signature: response.result_signature,
                }
            }
            PageCommand::Error {
                identity,
                request,
                status,
                error_code,
                message,
            } => Self::Error {
                identity: (*identity).into(),
                request_id: request.map(|id| id.to_string()),
                status: status.clone(),
                error_code: *error_code,
                message: message.clone(),
            },
            PageCommand::PermissionDenied {
                identity,
                kind,
                user_initiated,
            } => Self::PermissionDenied {
                identity: (*identity).into(),
                permission: kind.as_str().to_owned(),
                user_initiated: *user_initiated,
            },
        })
    }
}

impl TryFrom<WireCommand> for PageCommand {
    type Error = io::Error;
    fn try_from(value: WireCommand) -> io::Result<Self> {
        Ok(match value {
            WireCommand::Mount { identity } => Self::Mount {
                identity: identity.try_into()?,
            },
            WireCommand::Stop { identity } => Self::Stop {
                identity: identity.try_into()?,
            },
            WireCommand::Result {
                identity,
                request_id,
                audit_sequence_id,
                rate_ml_hr,
                drug_rate_mg_hr,
                is_pediatric,
                result_signature,
            } => {
                finite(&[rate_ml_hr, drug_rate_mg_hr])?;
                Self::Result {
                    identity: identity.try_into()?,
                    request: SequenceId::parse(&request_id)?,
                    response: ClinicalCalcResponsePayload {
                        audit_sequence_id: decimal(&audit_sequence_id)?,
                        rate_ml_hr,
                        drug_rate_mg_hr,
                        is_pediatric,
                        result_signature,
                    },
                }
            }
            WireCommand::Error {
                identity,
                request_id,
                status,
                error_code,
                message,
            } => Self::Error {
                identity: identity.try_into()?,
                request: request_id.as_deref().map(SequenceId::parse).transpose()?,
                status,
                error_code,
                message,
            },
            WireCommand::PermissionDenied {
                identity,
                permission,
                user_initiated,
            } => Self::PermissionDenied {
                identity: identity.try_into()?,
                kind: PageCapability::parse(&permission)
                    .ok_or_else(|| invalid("Native permission kind is not declared by the page"))?,
                user_initiated,
            },
        })
    }
}
