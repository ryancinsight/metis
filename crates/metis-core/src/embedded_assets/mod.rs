//! Embedded `WebView2` module inventory and canonical binary codec.
//!
//! Entry point for ADR 0059's preparation contract: one typed description of
//! a prepared WASM page module plus the single binary encoding every
//! producer and consumer agrees on. See [`EmbeddedModuleInventory`] for the
//! record layout and [`encode`] for the canonical writer.
//!
//! Every successful encoding can be decoded, and re-encoding any accepted
//! byte sequence reproduces it exactly. The writer sorts records and deduplicates
//! enabled features. On the wire, provider IDs, tool names, features, and
//! environment pairs must each be in strictly increasing order. Compiler flags
//! retain their order and may repeat. Both directions admit at most
//! [`MAX_MANIFEST_BYTES`] per inventory, including its header.

mod codec;
mod types;

#[cfg(test)]
mod tests;

pub use codec::{decode, encode};
pub use types::{
    AssetKind, AssetRecord, BuildConfiguration, EmbeddedModuleInventory, INVENTORY_MAGIC,
    INVENTORY_SCHEMA, InputRecord, InventoryError, MAX_MANIFEST_BYTES, MAX_MODULE_BYTES,
    MAX_PATH_BYTES, MAX_RECORDS, ProviderRecord, ProviderSource, Result, ToolRecord, WASM_MAGIC,
    digest_inventory, hex_digest, validate_wasm_artifact,
};
