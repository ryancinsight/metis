#![deny(missing_docs)]
#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
pub mod app;
pub mod async_app;
mod presentation;
pub use app::{FormInputs, FormState, FrontendApp, MAX_COMPOSITION_BYTES};
pub use async_app::AsyncFrontendApp;
pub use presentation::CLINICAL_SCREEN_XML;
