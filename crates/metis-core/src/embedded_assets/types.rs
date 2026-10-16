//! Canonical inventory owners; compiler settings retain literal path values.
use super::{Digest, FileIdentity};

/// Native consumer platform of an embedded module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativePlatform {
    /// Windows `WebView2`.
    Windows,
}
/// Generated artifact role, encoded with one closed byte tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetKind {
    /// Generated wasm-bindgen ES module.
    JavaScript,
    /// Generated WebAssembly module.
    WebAssembly,
}
/// Build tool role; the array order is Cargo, rustc, wasm-bindgen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    /// Cargo orchestration.
    Cargo,
    /// Rust compiler.
    Rustc,
    /// wasm-bindgen interoperability generation.
    WasmBindgen,
}
/// Compiled module and consumer schema identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleIdentity {
    /// Cargo package name.
    pub package: String,
    /// Path-independent locked package identity.
    pub package_id: String,
    /// Selected cdylib target name.
    pub target: String,
    /// Compiler target triple.
    pub wasm_target: String,
    /// Rust-owned page entry point.
    pub export: String,
    /// Native consumer platform.
    pub native_platform: NativePlatform,
    /// Closed page protocol schema.
    pub page_schema: u16,
}
/// Source identity from the locked dependency graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderSource {
    /// A package belonging to this workspace.
    Workspace,
    /// Git source with its exact commit.
    Git {
        /// Repository URL, without local path syntax.
        url: String,
        /// Full locked revision.
        revision: String,
    },
    /// Registry source with the lock's package checksum.
    Registry {
        /// Cargo registry source URL.
        url: String,
        /// Package archive checksum.
        checksum: Digest,
    },
}
/// One package in the actual resolved module dependency closure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderIdentity {
    /// Path-independent Cargo identity.
    pub package_id: String,
    /// Cargo package name.
    pub package: String,
    /// Locked version.
    pub version: String,
    /// Locked source identity.
    pub source: ProviderSource,
}
impl ProviderIdentity {
    /// Constructs the path-independent identity from its locked source fields.
    #[must_use]
    pub fn new(package: String, version: String, source: ProviderSource) -> Self {
        let package_id = provider_id(&package, &version, &source);
        Self {
            package_id,
            package,
            version,
            source,
        }
    }
    pub(super) fn canonical_id(&self) -> String {
        provider_id(&self.package, &self.version, &self.source)
    }
}
fn provider_id(package: &str, version: &str, source: &ProviderSource) -> String {
    match source {
        ProviderSource::Workspace => format!("workspace:{package}@{version}"),
        ProviderSource::Git { url, revision } => format!("{url}#{revision}:{package}@{version}"),
        ProviderSource::Registry { url, .. } => format!("{url}#{package}@{version}"),
    }
}
/// Observed and admitted build tool identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolIdentity {
    /// Tool role.
    pub kind: ToolKind,
    /// Exact version command output.
    pub version: String,
    /// SHA-256 of the resolved executable bytes.
    pub executable: FileIdentity,
}
/// Effective module build configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildConfiguration {
    /// Cargo profile, currently release.
    pub profile: String,
    /// Sorted unique selected feature names.
    pub features: Vec<String>,
    /// Ordered effective compiler/build arguments.
    pub compiler_args: Vec<String>,
    /// Sorted unique environment entries affecting generation.
    pub environment: Vec<(String, Option<String>)>,
}
/// An actual compiler or build-script input in its canonical owner namespace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputRecord {
    /// Locked package ID, workspace namespace, or generated-output owner ID.
    pub owner_id: String,
    /// Portable path relative to the admitted owner root.
    pub path: String,
    /// Exact byte length and SHA-256.
    pub identity: FileIdentity,
}
/// A declared compiler/build-script directory and its complete membership digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryRecord {
    /// Canonical owner namespace.
    pub owner_id: String,
    /// Relative scope, or the typed owner-root scope.
    pub path: Option<String>,
    /// Declared-directory or Cargo package-domain membership policy.
    pub policy: DirectoryPolicy,
    /// Digest including all names, empty directories, lengths and file hashes.
    pub digest: Digest,
}
/// Directory membership semantics supplied by the actual build declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DirectoryPolicy {
    /// Every entry under an explicitly declared directory, including target names.
    Declared,
    /// Conservative package tree, excluding root `.git` and root `target`.
    CargoPackage,
}
/// One generated module artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetRecord {
    /// Closed JavaScript/WASM role.
    pub kind: AssetKind,
    /// Portable destination relative to the inventory directory.
    pub destination: String,
    /// Exact byte length and SHA-256.
    pub identity: FileIdentity,
}
/// Complete unvalidated fields consumed by the inventory constructor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventorySpec {
    /// Selected module and consumer.
    pub module: ModuleIdentity,
    /// Exact current workspace Cargo.lock SHA-256.
    pub lock_digest: Digest,
    /// Sorted unique resolved providers.
    pub providers: Vec<ProviderIdentity>,
    /// Three ordered tool identities.
    pub tools: [ToolIdentity; 3],
    /// Effective build configuration.
    pub configuration: BuildConfiguration,
    /// Sorted unique owner/path records.
    pub inputs: Vec<InputRecord>,
    /// Sorted declared directories whose complete membership affects generation.
    pub directories: Vec<DirectoryRecord>,
    /// Exactly JavaScript and WASM, sorted by destination.
    pub assets: [AssetRecord; 2],
}
