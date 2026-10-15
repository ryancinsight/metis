//! Private identity representation; domain invariants precede admission.

use super::super::identity::invalid;
use super::super::{PAGE_SCHEMA_VERSION, PageIdentity, PageRole, SequenceId};
use metis_core::embedded_assets::Digest;
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::embedded) struct WireIdentity {
    schema: u16,
    role: String,
    mount_id: String,
    module_digest: String,
}

impl From<PageIdentity> for WireIdentity {
    fn from(identity: PageIdentity) -> Self {
        Self {
            schema: identity.schema(),
            role: identity.role().as_str().to_owned(),
            mount_id: identity.mount().to_string(),
            module_digest: identity.module_digest().to_string(),
        }
    }
}

impl TryFrom<WireIdentity> for PageIdentity {
    type Error = io::Error;
    fn try_from(wire: WireIdentity) -> io::Result<Self> {
        if wire.schema != PAGE_SCHEMA_VERSION {
            return Err(invalid("Page schema version does not match"));
        }
        Ok(Self::new(
            PageRole::parse(&wire.role)?,
            SequenceId::parse(&wire.mount_id)?,
            wire.module_digest
                .parse::<Digest>()
                .map_err(|error| invalid(format!("Page module digest is invalid: {error}")))?,
        ))
    }
}
