#![deny(missing_docs)]
#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
pub mod app;
mod presentation;
pub use app::{FormInputs, FormState, FrontendApp};
pub use presentation::CLINICAL_SCREEN_XML;
