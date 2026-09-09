#![deny(missing_docs)]
#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
pub mod app;
pub mod async_app;
mod presentation;
#[path = "result_explorer/mod.rs"]
mod result_explorer;
pub use app::{FormInputs, FormState, FrontendApp, MAX_COMPOSITION_BYTES};
pub use async_app::AsyncFrontendApp;
pub use presentation::CLINICAL_SCREEN_XML;
pub use result_explorer::{
    ExplorerStatus, GroupId, MAX_PATIENT_LABEL_BYTES, MAX_RESULT_FILTER_BYTES, MAX_RESULT_GROUPS,
    MAX_RESULT_ROWS, RESULT_PAGE_SIZE, RecordOutcome, ResultExplorer, ResultId, ResultRow,
    SortDirection, SortKey, SortOrder, VisibleEntry,
};
