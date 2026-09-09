//! Bounded sorting, filtering, selection, and tree state for result views.

use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::ClinicalCalcResponsePayload;
use std::num::NonZeroU64;

/// Maximum number of calculation rows retained by one explorer.
pub const MAX_RESULT_ROWS: usize = 256;
/// Maximum number of patient groups retained by one explorer.
pub const MAX_RESULT_GROUPS: usize = MAX_RESULT_ROWS;
/// Maximum UTF-8 bytes retained for one patient label in the explorer.
pub const MAX_PATIENT_LABEL_BYTES: usize = 1024;
/// Maximum UTF-8 bytes retained for the explorer filter.
pub const MAX_RESULT_FILTER_BYTES: usize = 128;
/// Number of visible tree entries in one explorer page.
pub const RESULT_PAGE_SIZE: usize = 8;

/// Stable identity of one result row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResultId(NonZeroU64);

impl ResultId {
    /// Creates an identity from a nonzero audit sequence.
    ///
    /// # Errors
    /// Returns a malformed-payload error when `value` is zero.
    pub fn new(value: u64) -> Result<Self> {
        NonZeroU64::new(value).map(Self).ok_or_else(|| {
            MetisError::protocol(
                ErrorCode::MalformedPayload,
                "A result row requires a nonzero audit sequence",
            )
        })
    }

    /// Returns the wire audit sequence represented by this identity.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Stable identity of one patient group in the result tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupId(NonZeroU64);

impl GroupId {
    /// Returns the numeric group identity.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// One validated calculation result displayed by [`ResultExplorer`].
pub struct ResultRow {
    id: ResultId,
    patient_id: Box<str>,
    response: ClinicalCalcResponsePayload,
}

impl ResultRow {
    /// Builds a row from a backend response and its submitted patient label.
    ///
    /// The response is copied because the frontend owns a bounded history
    /// independent of the current form state. The cryptographic signature is
    /// retained exactly and is never formatted into the UI.
    ///
    /// # Errors
    /// Rejects zero or non-finite response values, empty/control-containing
    /// labels, and labels above [`MAX_PATIENT_LABEL_BYTES`].
    pub fn try_new(patient_id: &str, response: &ClinicalCalcResponsePayload) -> Result<Self> {
        validate_patient_label(patient_id)?;
        if !response.rate_ml_hr.is_finite() || !response.drug_rate_mg_hr.is_finite() {
            return Err(MetisError::protocol(
                ErrorCode::MalformedPayload,
                "A result row requires finite rate values",
            ));
        }
        Ok(Self {
            id: ResultId::new(response.audit_sequence_id)?,
            patient_id: patient_id.into(),
            response: response.clone(),
        })
    }

    /// Returns the stable audit identity.
    #[must_use]
    pub const fn id(&self) -> ResultId {
        self.id
    }

    /// Returns the submitted patient label without copying it.
    #[must_use]
    pub fn patient_id(&self) -> &str {
        &self.patient_id
    }

    /// Returns the infusion rate in milliliters per hour.
    #[must_use]
    pub const fn rate_ml_hr(&self) -> f64 {
        self.response.rate_ml_hr
    }

    /// Returns the drug delivery rate in milligrams per hour.
    #[must_use]
    pub const fn drug_rate_mg_hr(&self) -> f64 {
        self.response.drug_rate_mg_hr
    }

    /// Returns whether the pediatric interlock applies.
    #[must_use]
    pub const fn is_pediatric(&self) -> bool {
        self.response.is_pediatric
    }
}

impl Clone for ResultRow {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            patient_id: self.patient_id.clone(),
            response: self.response.clone(),
        }
    }
}

impl PartialEq for ResultRow {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.patient_id == other.patient_id
            && self.response == other.response
    }
}

/// Column used to order visible result rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    /// Order by audit sequence.
    Sequence,
    /// Order by patient label.
    Patient,
    /// Order by infusion volume rate.
    VolumeRate,
    /// Order by drug mass rate.
    DrugRate,
}

impl SortKey {
    /// Returns the stable query value used by browser controls.
    #[must_use]
    pub const fn value(self) -> &'static str {
        match self {
            Self::Sequence => "sequence",
            Self::Patient => "patient",
            Self::VolumeRate => "volume",
            Self::DrugRate => "drug",
        }
    }

    /// Returns the reader-facing column label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sequence => "audit sequence",
            Self::Patient => "patient",
            Self::VolumeRate => "volume rate",
            Self::DrugRate => "drug rate",
        }
    }
}

/// Direction used to order one result column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    /// Lowest lexical or numeric value first.
    Ascending,
    /// Highest lexical or numeric value first.
    Descending,
}

impl SortDirection {
    /// Returns the stable query value used by browser controls.
    #[must_use]
    pub const fn value(self) -> &'static str {
        match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
        }
    }
}

/// A complete result ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortOrder {
    key: SortKey,
    direction: SortDirection,
}

impl SortOrder {
    /// Creates an ordering from a column and direction.
    #[must_use]
    pub const fn new(key: SortKey, direction: SortDirection) -> Self {
        Self { key, direction }
    }

    /// Parses the browser control value, such as `patient-descending`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let (key, direction) = value.split_once('-')?;
        let key = match key {
            "sequence" => SortKey::Sequence,
            "patient" => SortKey::Patient,
            "volume" => SortKey::VolumeRate,
            "drug" => SortKey::DrugRate,
            _ => return None,
        };
        let direction = match direction {
            "ascending" => SortDirection::Ascending,
            "descending" => SortDirection::Descending,
            _ => return None,
        };
        Some(Self::new(key, direction))
    }

    /// Returns the ordered column.
    #[must_use]
    pub const fn key(self) -> SortKey {
        self.key
    }

    /// Returns the ordered direction.
    #[must_use]
    pub const fn direction(self) -> SortDirection {
        self.direction
    }

    /// Returns the stable browser control value.
    #[must_use]
    pub const fn value(self) -> &'static str {
        match (self.key, self.direction) {
            (SortKey::Sequence, SortDirection::Ascending) => "sequence-ascending",
            (SortKey::Sequence, SortDirection::Descending) => "sequence-descending",
            (SortKey::Patient, SortDirection::Ascending) => "patient-ascending",
            (SortKey::Patient, SortDirection::Descending) => "patient-descending",
            (SortKey::VolumeRate, SortDirection::Ascending) => "volume-ascending",
            (SortKey::VolumeRate, SortDirection::Descending) => "volume-descending",
            (SortKey::DrugRate, SortDirection::Ascending) => "drug-ascending",
            (SortKey::DrugRate, SortDirection::Descending) => "drug-descending",
        }
    }
}

impl Default for SortOrder {
    fn default() -> Self {
        Self::new(SortKey::Sequence, SortDirection::Descending)
    }
}

/// Current result-explorer lifecycle state.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExplorerStatus {
    /// No rows have been recorded.
    Empty,
    /// A producer is replacing or appending rows.
    Loading,
    /// At least one valid row is available.
    Ready,
    /// The latest producer operation failed while existing rows remain usable.
    Error(MetisError),
}

/// Outcome of recording one backend response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordOutcome {
    /// A new row was retained.
    Inserted,
    /// An existing row with the same audit identity was replaced.
    Updated,
    /// A new row was retained after the oldest row was evicted at the bound.
    Evicted(ResultId),
}

/// One item in the bounded, accessible result tree.
pub enum VisibleEntry<'a> {
    /// A patient group that can be expanded or collapsed.
    Group {
        /// Stable group identity.
        id: GroupId,
        /// Patient label shown by the tree control.
        label: &'a str,
        /// Whether child rows are currently visible.
        expanded: bool,
        /// Number of retained rows in this group.
        row_count: usize,
    },
    /// A calculation row inside an expanded group.
    Row(&'a ResultRow),
}

impl TryFrom<u64> for GroupId {
    type Error = MetisError;

    fn try_from(value: u64) -> Result<Self> {
        NonZeroU64::new(value).map(Self).ok_or_else(|| {
            MetisError::protocol(
                ErrorCode::MalformedPayload,
                "A result group requires a nonzero identity",
            )
        })
    }
}

fn validate_patient_label(value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(MetisError::protocol(
            ErrorCode::MalformedPayload,
            "A result row requires a patient label",
        ));
    }
    if value.len() > MAX_PATIENT_LABEL_BYTES {
        return Err(MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Result patient label exceeds the bounded display limit",
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(MetisError::protocol(
            ErrorCode::MalformedPayload,
            "Result patient label contains a control character",
        ));
    }
    Ok(())
}
