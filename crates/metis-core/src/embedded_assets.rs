//! Canonical prepared-module inventory, portable paths and verified asset bytes.
//!
//! The inventory describes semantic build identity. Local absolute roots live
//! in a separately bounded binding file and are admitted against the consumer's
//! workspace, Cargo home and generated output ownership before file access.
mod budget;
mod codec;
mod content;
mod digest;
#[cfg(not(target_arch = "wasm32"))]
mod fs;
mod inventory;
mod path;
mod types;

pub use budget::{FILE_LIMIT, MANIFEST_LIMIT, MODULE_LIMIT, PAYLOAD_LIMIT, PORTABLE_PATH_LIMIT};
pub use content::FileIdentity;
pub use digest::Digest;
#[cfg(not(target_arch = "wasm32"))]
pub use fs::{
    CargoLocations, DirectorySnapshot, LocalBindings, NativeContext, OwnerBinding, PreparedModule,
    ProducerContext, TOOL_IDENTITY_DEADLINE, TOOL_OUTPUT_LIMIT, ToolBinding, VerificationContext,
    VerifiedAsset, build_environment_value, canonical_directory, canonical_file, capture_command,
    codegen_environment_keys, compiler_command, configuration, configuration_inputs,
    environment_value, linked, resolve_tool, source, verify_compiler_pin,
};
pub use inventory::EmbeddedModuleInventory;
pub use path::relative;
pub use types::{
    AssetKind, AssetRecord, BuildConfiguration, DirectoryPolicy, DirectoryRecord, InputRecord,
    InventorySpec, ModuleIdentity, NativePlatform, ProviderIdentity, ProviderSource, ToolIdentity,
    ToolKind,
};

#[cfg(test)]
mod tests;
