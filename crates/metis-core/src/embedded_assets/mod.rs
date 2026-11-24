//! Embedded WebView2 module inventory and canonical binary codec.
//!
//! Entry point for ADR 0059's preparation contract: one typed description of
//! a prepared WASM page module plus the single binary encoding every
//! producer and consumer agrees on. Concrete record types and validation
//! live in [`types`]; the encoding lives in [`codec`].

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
