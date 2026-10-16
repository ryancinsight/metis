//! Resource budgets shared by packaging and prepared modules.
/// Maximum UTF-8 bytes in a portable packaged path.
pub const PORTABLE_PATH_LIMIT: usize = 240;
/// Maximum number of records in a manifest or prepared inventory.
pub const FILE_LIMIT: usize = 4096;
/// Maximum encoded manifest or prepared-inventory bytes.
pub const MANIFEST_LIMIT: u64 = 1024 * 1024;
/// Maximum retained bytes of one generated JavaScript or WASM module.
pub const MODULE_LIMIT: u64 = 64 * 1024 * 1024;
/// Maximum aggregate packaged payload bytes, fitting one cabinet.
pub const PAYLOAD_LIMIT: u64 = 1024 * 1024 * 1024;
