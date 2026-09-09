//! Bounded sorting, filtering, selection, and tree state for result views.

mod state;
mod types;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

pub use state::ResultExplorer;
pub use types::{
    ExplorerStatus, GroupId, RecordOutcome, ResultId, ResultRow, SortDirection, SortKey, SortOrder,
    VisibleEntry,
};
pub use types::{
    MAX_PATIENT_LABEL_BYTES, MAX_RESULT_FILTER_BYTES, MAX_RESULT_GROUPS, MAX_RESULT_ROWS,
    RESULT_PAGE_SIZE,
};
