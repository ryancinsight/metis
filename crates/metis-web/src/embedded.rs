//! Rust-owned pages and closed messages for an embedded native `WebView`.
//!
//! Domain messages retain exact integers and native backend values. JSON DTOs
//! remain private. Page readiness and resource observations do not confer
//! backend authority or prove trusted input.

#[doc(hidden)]
pub mod command_observation;
mod identity;
mod lifecycle;
mod message;
#[cfg(target_arch = "wasm32")]
mod page;
mod permission;
mod resources;
#[cfg(any(target_arch = "wasm32", test))]
mod theme;
mod wire;

#[cfg(target_arch = "wasm32")]
pub use page::{metis_embedded_start, metis_embedded_stop};

pub use identity::{PAGE_SCHEMA_VERSION, PageIdentity, PageRole, SequenceId};
pub use lifecycle::{PageSession, PageStage};
pub use message::{PageCommand, PageEvent};
pub use permission::{BrowserPermissionResult, CapabilityObservation, PageCapability};
pub use resources::ResourceCounts;
pub use wire::{decode_page_command, decode_page_event, encode_page_command, encode_page_event};
