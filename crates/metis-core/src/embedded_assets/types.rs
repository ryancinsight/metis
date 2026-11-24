use core::fmt;

use moirai_crypto::sha256;

/// Eight ASCII magic bytes opening every inventory encoding.
pub const INVENTORY_MAGIC: &[u8; 8] = b"METISAS1";
/// The only accepted schema version.
pub const INVENTORY_SCHEMA: u16 = 1;
/// Longest accepted owner-relative path, in bytes.
pub const MAX_PATH_BYTES: usize = 240;
/// Most records accepted in one list.
pub const MAX_RECORDS: usize = 4096;
/// Largest accepted manifest encoding, in bytes.
pub const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
/// Largest accepted generated-module payload, in bytes.
pub const MAX_MODULE_BYTES: u64 = 64 * 1024 * 1024;
/// Expected first four bytes of a WASM artifact.
pub const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6d];

/// Failures decoding or validating a module inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryError {
    /// Fewer bytes remain than the field requires.
    Truncated,
    /// Bytes remain after the last asset record.
    TrailingBytes,
    /// The schema tag is not [`INVENTORY_SCHEMA`].
    UnknownSchema(u16),
    /// A count exceeds [`MAX_RECORDS`].
    TooManyRecords,
    /// The encoded manifest exceeds [`MAX_MANIFEST_BYTES`].
    OversizeManifest,
    /// An asset record exceeds [`MAX_MODULE_BYTES`].
    OversizeAsset,
    /// A string field is not valid UTF-8.
    InvalidUtf8,
    /// A path breaks the portable-path rule.
    BadPath,
    /// Two records share one owner and path.
    DuplicatePath,
    /// Two paths differ only in ASCII case.
    CaseCollidingPaths,
    /// Records are not in canonical sorted order.
    UnsortedRecords,
    /// An enum tag names no known variant.
    UnknownTag(u8),
    /// An asset list is not exactly one JavaScript plus one WASM record.
    BadAssetSet,
    /// A tool list is not exactly three records.
    BadToolSet,
    /// A WASM artifact lacks its magic or version header.
    BadWasmArtifact,
}

impl fmt::Display for InventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InventoryError::Truncated => write!(f, "inventory truncated"),
            InventoryError::TrailingBytes => write!(f, "trailing bytes after inventory"),
            InventoryError::UnknownSchema(v) => write!(f, "unknown inventory schema {v}"),
            InventoryError::TooManyRecords => write!(f, "record count over budget"),
            InventoryError::OversizeManifest => write!(f, "manifest over size budget"),
            InventoryError::OversizeAsset => write!(f, "asset over module size budget"),
            InventoryError::InvalidUtf8 => write!(f, "string field is not UTF-8"),
            InventoryError::BadPath => write!(f, "path breaks the portable-path rule"),
            InventoryError::DuplicatePath => write!(f, "duplicate owner/path record"),
            InventoryError::CaseCollidingPaths => {
                write!(f, "paths collide ignoring ASCII case")
            }
            InventoryError::UnsortedRecords => write!(f, "records not in canonical order"),
            InventoryError::UnknownTag(t) => write!(f, "unknown enum tag {t}"),
            InventoryError::BadAssetSet => {
                write!(f, "asset set is not one JavaScript plus one WASM record")
            }
            InventoryError::BadToolSet => write!(f, "tool set is not exactly three records"),
            InventoryError::BadWasmArtifact => {
                write!(f, "WASM artifact lacks magic or version header")
            }
        }
    }
}

impl std::error::Error for InventoryError {}

/// Shorthand for inventory results.
pub type Result<T> = std::result::Result<T, InventoryError>;

/// Where one provider package came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSource {
    /// A Git checkout: repository URL plus resolved revision.
    Git {
        /// Repository URL.
        url: String,
        /// Resolved commit revision.
        revision: String,
    },
    /// A registry package: SHA-256 of the published `.crate` file.
    Registry {
        /// The published archive digest.
        checksum: [u8; 32],
    },
    /// A workspace member compiled from its local path.
    Path {
        /// Owning member name.
        member: String,
    },
}

/// One locked provider package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRecord {
    /// Cargo package id (`name version`).
    pub package_id: String,
    /// Resolved version.
    pub version: String,
    /// Where the package came from.
    pub source: ProviderSource,
}

/// One pinned build tool (Cargo, rustc, wasm-bindgen).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRecord {
    /// Tool name.
    pub name: String,
    /// The tool's own version output.
    pub version_output: String,
    /// SHA-256 of the executed tool binary.
    pub executable_digest: [u8; 32],
}

/// The build configuration the module was prepared under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfiguration {
    /// Cargo profile name.
    pub profile: String,
    /// Sorted unique enabled features.
    pub features: Vec<String>,
    /// Ordered compiler arguments (order is significant).
    pub rustflags: Vec<String>,
    /// Sorted unique `KEY=value` environment entries.
    pub environment: Vec<(String, String)>,
}

/// One hashed source input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputRecord {
    /// Owning crate or workspace name.
    pub owner: String,
    /// Owner-relative portable path.
    pub path: String,
    /// Byte length of the input.
    pub length: u64,
    /// SHA-256 of the input bytes.
    pub digest: [u8; 32],
}

/// A generated module asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetKind {
    /// Generated JavaScript loader.
    JavaScript,
    /// Generated WASM module.
    Wasm,
}

/// One hashed generated asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetRecord {
    /// Which generated module this is.
    pub kind: AssetKind,
    /// Package destination path.
    pub destination: String,
    /// Byte length of the asset.
    pub length: u64,
    /// SHA-256 of the asset bytes.
    pub digest: [u8; 32],
}

/// One prepared embedded module: identity, provenance and hashed contents.
///
/// All lists are stored in canonical sorted order (see `encode`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedModuleInventory {
    /// Cargo package name, e.g. `metis-web`.
    pub package: String,
    /// Cargo package id, e.g. `metis-web 0.5.0`.
    pub package_id: String,
    /// The cdylib target name.
    pub cdylib_target: String,
    /// The WASM target triple.
    pub wasm_target: String,
    /// The exported page entry point.
    pub export_name: String,
    /// Native platform tag consuming the module.
    pub platform_tag: String,
    /// Page schema version the module implements.
    pub page_schema: u16,
    /// SHA-256 of the locked `Cargo.lock` the module was built from.
    pub lock_digest: [u8; 32],
    /// Locked provider packages, sorted by package id.
    pub providers: Vec<ProviderRecord>,
    /// Exactly three pinned tool records.
    pub tools: Vec<ToolRecord>,
    /// The build configuration.
    pub config: BuildConfiguration,
    /// Hashed source inputs, sorted by owner then path.
    pub inputs: Vec<InputRecord>,
    /// Exactly two generated assets: one JavaScript, one WASM.
    pub assets: Vec<AssetRecord>,
}

pub(super) fn check_string(value: &str) -> Result<()> {
    if value.len() > u32::MAX as usize {
        return Err(InventoryError::OversizeManifest);
    }
    Ok(())
}

/// Enforce the portable-path rule: non-empty, at most [`MAX_PATH_BYTES`]
/// bytes, relative with forward slashes, no empty or dot segments, no
/// absolute paths, no backslashes, no control characters, no trailing
/// space or dot in any segment, and no reserved Windows basenames.
pub(super) fn check_path(path: &str) -> Result<()> {
    if path.is_empty() || path.len() > MAX_PATH_BYTES {
        return Err(InventoryError::BadPath);
    }
    if path.starts_with('/') || path.contains('\\') {
        return Err(InventoryError::BadPath);
    }
    for segment in path.split('/') {
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.ends_with(' ')
            || segment.ends_with('.')
            || segment.chars().any(|c| c.is_control())
        {
            return Err(InventoryError::BadPath);
        }
        let stem = segment.split('.').next().unwrap_or(segment);
        if is_reserved_stem(stem) {
            return Err(InventoryError::BadPath);
        }
    }
    Ok(())
}

fn is_reserved_stem(stem: &str) -> bool {
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let upper = stem.to_ascii_uppercase();
    RESERVED.contains(&upper.as_str())
}

fn lower_key(owner: &str, path: &str) -> (String, String) {
    (owner.to_ascii_lowercase(), path.to_ascii_lowercase())
}

/// Validate input content independent of order: portable paths, bounded
/// owners, the module size budget, exact duplicates and ASCII-case
/// collisions. Sorted copies make duplicate detection order-independent.
pub(super) fn check_inputs(inputs: &[InputRecord]) -> Result<()> {
    for input in inputs {
        check_path(&input.path)?;
        check_string(&input.owner)?;
        if input.length > MAX_MODULE_BYTES {
            return Err(InventoryError::OversizeAsset);
        }
    }
    let mut keys: Vec<(String, String)> = inputs
        .iter()
        .map(|input| (input.owner.clone(), input.path.clone()))
        .collect();
    keys.sort();
    for pair in keys.windows(2) {
        if pair[0] == pair[1] {
            return Err(InventoryError::DuplicatePath);
        }
    }
    let mut lowered: Vec<(String, String)> = inputs
        .iter()
        .map(|input| lower_key(&input.owner, &input.path))
        .collect();
    lowered.sort();
    for pair in lowered.windows(2) {
        if pair[0] == pair[1] {
            return Err(InventoryError::CaseCollidingPaths);
        }
    }
    Ok(())
}

/// Verify input records arrive in canonical (owner, path) order.
pub(super) fn check_input_order(inputs: &[InputRecord]) -> Result<()> {
    for pair in inputs.windows(2) {
        let first = (&pair[0].owner, &pair[0].path);
        let second = (&pair[1].owner, &pair[1].path);
        if first >= second {
            return Err(InventoryError::UnsortedRecords);
        }
    }
    Ok(())
}

pub(super) fn check_assets(assets: &[AssetRecord]) -> Result<()> {
    if assets.len() != 2 {
        return Err(InventoryError::BadAssetSet);
    }
    let mut kinds = [false, false];
    for asset in assets {
        check_path(&asset.destination)?;
        if asset.length > MAX_MODULE_BYTES {
            return Err(InventoryError::OversizeAsset);
        }
        match asset.kind {
            AssetKind::JavaScript => kinds[0] = true,
            AssetKind::Wasm => kinds[1] = true,
        }
    }
    if kinds != [true, true] {
        return Err(InventoryError::BadAssetSet);
    }
    if assets[0].destination >= assets[1].destination {
        return Err(InventoryError::UnsortedRecords);
    }
    if assets[0].destination != assets[1].destination
        && assets[0]
            .destination
            .eq_ignore_ascii_case(&assets[1].destination)
    {
        return Err(InventoryError::CaseCollidingPaths);
    }
    Ok(())
}

/// Validate a WASM artifact's magic and version header.
///
/// Returns the binary `u32` version on success.
///
/// # Errors
///
/// Returns [`InventoryError::BadWasmArtifact`] when `bytes` is shorter than
/// eight bytes or lacks the WASM magic.
pub fn validate_wasm_artifact(bytes: &[u8]) -> Result<u32> {
    if bytes.len() < 8 {
        return Err(InventoryError::BadWasmArtifact);
    }
    let (magic, version) = bytes.split_at(4);
    if magic != WASM_MAGIC {
        return Err(InventoryError::BadWasmArtifact);
    }
    let version = u32::from_le_bytes([version[0], version[1], version[2], version[3]]);
    Ok(version)
}

/// SHA-256 digest of canonical inventory bytes.
#[must_use]
pub fn digest_inventory(bytes: &[u8]) -> [u8; 32] {
    sha256(bytes)
}

/// Lowercase hexadecimal rendering of a digest, for readable reports.
#[must_use]
pub fn hex_digest(digest: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
