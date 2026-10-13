//! Exact identity and sequence contracts at the private page boundary.

use metis_core::embedded_assets::Digest;
use std::{fmt, io, num::NonZeroU64};

/// The inaugural closed Rust embedded-page schema.
pub const PAGE_SCHEMA_VERSION: u16 = 1;

/// A positive sequence whose decimal wire representation is lossless.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SequenceId(NonZeroU64);

impl SequenceId {
    /// Admits a positive domain sequence.
    /// # Errors
    /// Rejects zero rather than reserving it as a reusable identifier.
    pub fn new(value: u64) -> io::Result<Self> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or_else(|| invalid("Page sequence must be positive"))
    }

    /// Returns the exact backend-independent sequence value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    /// Advances monotonically without wraparound or identifier reuse.
    /// # Errors
    /// Rejects exhaustion at the maximum domain value.
    pub fn next(self) -> io::Result<Self> {
        self.get()
            .checked_add(1)
            .ok_or_else(|| invalid("Page sequence is exhausted"))
            .and_then(Self::new)
    }

    pub(super) fn parse(value: &str) -> io::Result<Self> {
        decimal(value).and_then(Self::new)
    }
}

impl fmt::Display for SequenceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(formatter)
    }
}

pub(super) fn decimal(value: &str) -> io::Result<u64> {
    // ceil(log10(2^64)) = 20; a leading zero admits only the representation "0".
    if value.is_empty()
        || value.len() > 20
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid("Page integer is not canonical decimal text"));
    }
    value
        .parse()
        .map_err(|error| invalid(format!("Page integer exceeds its domain: {error}")))
}

/// The packaged document's application role.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PageRole {
    /// The supervised clinical calculation form.
    Form,
    /// The real browser capability probe.
    PermissionProbe,
}

impl PageRole {
    /// Returns the canonical configuration and wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Form => "form",
            Self::PermissionProbe => "permission_probe",
        }
    }

    pub(super) fn parse(value: &str) -> io::Result<Self> {
        match value {
            "form" => Ok(Self::Form),
            "permission_probe" => Ok(Self::PermissionProbe),
            _ => Err(invalid("Page role is not declared by the schema")),
        }
    }
}

/// The exact role, mount and prepared module admitted by the native host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageIdentity {
    role: PageRole,
    mount: SequenceId,
    module_digest: Digest,
}

impl PageIdentity {
    /// Constructs an identity using the current closed schema.
    #[must_use]
    pub const fn new(role: PageRole, mount: SequenceId, module_digest: Digest) -> Self {
        Self {
            role,
            mount,
            module_digest,
        }
    }
    /// Returns the fixed wire schema.
    #[must_use]
    pub const fn schema(self) -> u16 {
        PAGE_SCHEMA_VERSION
    }
    /// Returns the document role.
    #[must_use]
    pub const fn role(self) -> PageRole {
        self.role
    }
    /// Returns this document mount's positive sequence.
    #[must_use]
    pub const fn mount(self) -> SequenceId {
        self.mount
    }
    /// Returns the canonical prepared inventory digest.
    #[must_use]
    pub const fn module_digest(self) -> Digest {
        self.module_digest
    }
}

pub(super) fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
