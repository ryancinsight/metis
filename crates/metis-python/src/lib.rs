//! Thin `PyO3` bindings for the Metis Rust application framework.
//!
//! The binding converts Python values into validated Rust types, delegates
//! computation to `metis-backend`, and converts the result back to Python.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod application;
mod clinical;
mod error;
mod module;
mod presentation;
