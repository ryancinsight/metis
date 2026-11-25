//! `metis prepare`: build the embedded WASM page module and record its inventory.
//!
//! Reads a native manifest with an `embedded_module` declaration, builds the
//! declared workspace package to WebAssembly, generates the loader with the
//! locked wasm-bindgen CLI, hashes every source input, and writes the
//! canonical `inventory.metisas1` plus a readable `report.json` derived from
//! the decoded inventory (so the report proves what the bytes say). The
//! native build consumes the prepared directory through
//! `METIS_WEBVIEW_ASSET_INVENTORY` and nothing else.

use crate::{
    Result,
    build::{
        self,
        artifacts::{closure, module, module_target},
    },
    manifest::{self, Application},
    process,
};
use metis_core::embedded_assets::{
    AssetKind, AssetRecord, BuildConfiguration, EmbeddedModuleInventory, INVENTORY_SCHEMA,
    InputRecord, MAX_RECORDS, ProviderRecord, ProviderSource, ToolRecord, decode, digest_inventory,
    encode, hex_digest, validate_wasm_artifact,
};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

/// WebAssembly target triple for prepared page modules.
const WASM_TARGET: &str = "wasm32-unknown-unknown";
/// Canonical inventory file written into the prepared directory.
const INVENTORY_FILE: &str = "inventory.metisas1";
/// Human-readable report derived from the decoded inventory.
const REPORT_FILE: &str = "report.json";
/// The generated loader and module file stems (`--out-name page`).
const LOADER_FILE: &str = "page.js";
const MODULE_FILE: &str = "page_bg.wasm";

const METADATA_DEADLINE: Duration = Duration::from_mins(1);
const WASM_BUILD_DEADLINE: Duration = Duration::from_mins(10);
const BINDGEN_DEADLINE: Duration = Duration::from_mins(2);
const TOOL_DEADLINE: Duration = Duration::from_secs(30);

/// Runs `prepare MANIFEST OUT_DIR`: records the module inventory for the
/// manifest's `embedded_module` declaration.
pub(crate) fn run(manifest: &Path, output: &Path) -> Result<()> {
    let (application, root) = Application::read(manifest)?;
    let declared = application
        .embedded_module
        .as_ref()
        .ok_or("manifest declares no embedded_module; nothing to prepare")?;
    if declared.platform != "windows" {
        return Err("embedded platform admits only windows".into());
    }
    let out_dir = build::absolute_output(output)?;
    if out_dir.try_exists()? {
        return Err("output already exists; choose a new output directory".into());
    }
    let cargo = process::tool("cargo")?;
    let cargo_manifest = manifest::source(&root, &application.cargo_manifest)?;
    let metadata = process::capture(
        &cargo,
        &[
            OsString::from("metadata"),
            OsString::from("--format-version=1"),
            OsString::from("--locked"),
            OsString::from("--manifest-path"),
            cargo_manifest.as_os_str().to_owned(),
        ],
        METADATA_DEADLINE,
        &[],
    )?;
    let selected = module_target(&metadata, &declared.package)?;
    let package_dir = package_dir(&metadata, &selected.package_id)?;
    let messages = build_wasm(&cargo, &cargo_manifest, &declared.package)?;
    let wasm_module = module(&messages, &selected)?;
    let bindgen = match std::env::var_os("WASM_BINDGEN") {
        Some(program) => PathBuf::from(program),
        None => process::tool("wasm-bindgen")?,
    };
    let (loader, wasm) =
        generate_assets(&bindgen, &wasm_module, &out_dir, &selected.bindgen_version)?;
    let inputs = collect_inputs(&package_dir, &declared.package)?;
    let lock_path = lock_file(&metadata)?;
    let lock_digest = digest_file(&lock_path)?;
    let lock_text = fs::read_to_string(&lock_path).map_err(|_| "workspace lock is unreadable")?;
    let checksums = lock_checksums(&lock_text)?;
    let tools = tool_records(&cargo, &bindgen)?;
    let inventory = assemble(Assembly {
        package: declared.package.clone(),
        package_id: selected.package_id.clone(),
        target: selected.target.clone(),
        export: declared.export.clone(),
        platform: declared.platform.clone(),
        page_schema: declared.page_schema,
        lock_digest,
        providers: providers(&metadata, &selected.package_id, &checksums)?,
        tools,
        rustflags: rustflags(),
        inputs,
        loader,
        wasm,
    })?;
    let encoded =
        encode(&inventory).map_err(|error| format!("inventory is not canonical: {error}"))?;
    let decoded =
        decode(&encoded).map_err(|error| format!("encoded inventory does not decode: {error}"))?;
    let digest = digest_inventory(&encoded);
    let mut report = serde_json::to_vec_pretty(&report_json(&decoded, &digest))?;
    report.push(b'\n');
    write_new(&out_dir.join(INVENTORY_FILE), &encoded)?;
    write_new(&out_dir.join(REPORT_FILE), &report)?;
    println!("{}", out_dir.join(INVENTORY_FILE).display());
    println!("{}", out_dir.join(REPORT_FILE).display());
    Ok(())
}

fn build_wasm(cargo: &Path, cargo_manifest: &Path, package: &str) -> Result<Vec<u8>> {
    let mut args = [
        "build",
        "--release",
        "--locked",
        "--message-format=json-render-diagnostics",
        "--manifest-path",
    ]
    .into_iter()
    .map(OsString::from)
    .collect::<Vec<_>>();
    args.push(cargo_manifest.as_os_str().to_owned());
    for flag in ["--target", WASM_TARGET, "--package", package] {
        args.push(OsString::from(flag));
    }
    process::capture(cargo, &args, WASM_BUILD_DEADLINE, &[])
}

fn generate_assets(
    bindgen: &Path,
    wasm_module: &Path,
    out_dir: &Path,
    expected_version: &str,
) -> Result<(Vec<u8>, Vec<u8>)> {
    let reported = process::capture(bindgen, &[OsString::from("--version")], TOOL_DEADLINE, &[])?;
    let version = parse_bindgen_version(&reported)?;
    if version != expected_version {
        return Err(format!(
            "wasm-bindgen CLI {version} does not match the locked crate {expected_version}"
        )
        .into());
    }
    let mut args = vec![
        OsString::from(wasm_module.as_os_str()),
        OsString::from("--out-dir"),
    ];
    args.push(out_dir.as_os_str().to_owned());
    args.extend([OsString::from("--target"), OsString::from("web")]);
    args.extend([OsString::from("--out-name"), OsString::from("page")]);
    process::run(bindgen, &args, BINDGEN_DEADLINE)?;
    let loader = read_generated(&out_dir.join(LOADER_FILE), LOADER_FILE)?;
    let wasm = read_generated(&out_dir.join(MODULE_FILE), MODULE_FILE)?;
    validate_wasm_artifact(&wasm)
        .map_err(|_| "generated module failed WASM magic and version validation")?;
    Ok((loader, wasm))
}

fn read_generated(path: &Path, name: &str) -> Result<Vec<u8>> {
    let bytes = fs::read(path).map_err(|_| format!("bindgen did not emit {name}"))?;
    if bytes.is_empty() {
        return Err(format!("generated {name} is empty").into());
    }
    Ok(bytes)
}

fn parse_bindgen_version(output: &[u8]) -> Result<String> {
    let text =
        std::str::from_utf8(output).map_err(|_| "wasm-bindgen printed a non-UTF-8 version")?;
    text.split_whitespace()
        .nth(1)
        .ok_or_else(|| "wasm-bindgen --version has no version field".into())
        .map(str::to_owned)
}

/// A hashed tool binary: name, reported version and executable digest.
struct ToolRecorded {
    name: String,
    version: String,
    digest: [u8; 32],
}

fn tool_record(path: &Path, name: &str) -> Result<ToolRecorded> {
    let output = process::capture(path, &[OsString::from("--version")], TOOL_DEADLINE, &[])?;
    let version = std::str::from_utf8(output.as_slice())
        .map_err(|_| format!("{name} printed a non-UTF-8 version"))?;
    let version = version.trim().to_owned();
    if version.is_empty() || version.contains(['\n', '\r']) {
        return Err(format!("{name} printed an unusable version").into());
    }
    let digest = digest_stream(
        &mut fs::File::open(path).map_err(|_| format!("{name} binary is unreadable"))?,
    )?;
    Ok(ToolRecorded {
        name: name.to_owned(),
        version,
        digest,
    })
}

fn tool_records(cargo: &Path, bindgen: &Path) -> Result<Vec<ToolRecorded>> {
    let rustc = process::tool("rustc")?;
    Ok(vec![
        tool_record(cargo, "cargo")?,
        tool_record(&rustc, "rustc")?,
        tool_record(bindgen, "wasm-bindgen")?,
    ])
}

fn digest_stream(source: &mut fs::File) -> Result<[u8; 32]> {
    use moirai_crypto::Sha256;
    use std::io::Read;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash.finalize())
}

fn digest_bytes(data: &[u8]) -> [u8; 32] {
    use moirai_crypto::Sha256;
    let mut hash = Sha256::new();
    hash.update(data);
    hash.finalize()
}

fn digest_file(path: &Path) -> Result<[u8; 32]> {
    digest_stream(&mut fs::File::open(path)?)
}

fn lock_file(metadata: &[u8]) -> Result<PathBuf> {
    let document: serde_json::Value = serde_json::from_slice(metadata)?;
    let root = document
        .get("workspace_root")
        .and_then(serde_json::Value::as_str)
        .ok_or("cargo metadata omitted workspace_root")?;
    let candidate = Path::new(root).join("Cargo.lock");
    if !candidate.is_file() {
        return Err("locked workspace has no Cargo.lock".into());
    }
    Ok(candidate)
}

fn package_dir(metadata: &[u8], package_id: &str) -> Result<PathBuf> {
    let document: serde_json::Value = serde_json::from_slice(metadata)?;
    let packages = document
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or("cargo metadata omitted packages")?;
    let manifest_path = packages
        .iter()
        .find(|candidate| {
            candidate.get("id").and_then(serde_json::Value::as_str) == Some(package_id)
        })
        .and_then(|package| package.get("manifest_path"))
        .and_then(serde_json::Value::as_str)
        .ok_or("resolved package omitted its manifest path")?;
    Path::new(manifest_path)
        .parent()
        .ok_or("package manifest has no parent directory".into())
        .map(Path::to_path_buf)
}

/// Locked registry checksums from `Cargo.lock`, keyed by
/// `(name, version, source)`: the provenance pin `cargo metadata` no longer
/// reports (its graph carries `checksum: null` for every registry package
/// while the lock records them), so the lock is the single source of truth.
type LockedChecksums = BTreeMap<(String, String, String), [u8; 32]>;

/// Parses the workspace lock into its registry checksums. A registry entry
/// without a checksum is unpinned and rejected by name; other sources carry
/// no checksum by construction and are skipped.
fn lock_checksums(lock: &str) -> Result<LockedChecksums> {
    let document: toml::Table = toml::from_str(lock)?;
    let packages = document
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("workspace lock omits its package table")?;
    let mut checksums = LockedChecksums::new();
    for package in packages {
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or("locked package omits its name")?;
        let version = package
            .get("version")
            .and_then(toml::Value::as_str)
            .ok_or("locked package omits its version")?;
        let source = package
            .get("source")
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        if !source.starts_with("registry+") {
            continue;
        }
        let raw = package
            .get("checksum")
            .and_then(toml::Value::as_str)
            .ok_or(format!(
                "locked registry package omits its checksum: {name} {version}"
            ))?;
        checksums.insert(
            (name.to_owned(), version.to_owned(), source.to_owned()),
            hex_to_digest(raw)?,
        );
    }
    Ok(checksums)
}

/// Workspace packages reachable from `root_id` through the resolved graph,
/// as `(name, version, source)` tuples. Registry checksums are resolved
/// separately from the parsed lock (see [`lock_checksums`).
type ProviderRow = (String, String, Option<String>);

fn provider_packages(metadata: &[u8], root_id: &str) -> Result<Vec<ProviderRow>> {
    let document: serde_json::Value = serde_json::from_slice(metadata)?;
    let packages = document
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or("cargo metadata omitted packages")?;
    let mut records = Vec::new();
    for id in closure(&document, root_id)? {
        let package = packages
            .iter()
            .find(|candidate| candidate.get("id") == Some(id))
            .ok_or("resolved package omitted its manifest entry")?;
        let name = package
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or("package omitted its name")?;
        let version = package
            .get("version")
            .and_then(serde_json::Value::as_str)
            .ok_or("package omitted its version")?;
        records.push((
            name.to_owned(),
            version.to_owned(),
            package
                .get("source")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        ));
    }
    Ok(records)
}

fn providers(
    metadata: &[u8],
    root_id: &str,
    checksums: &LockedChecksums,
) -> Result<Vec<ProviderRecord>> {
    let mut records = Vec::new();
    for (name, version, source) in provider_packages(metadata, root_id)? {
        let package_id = format!("{name} {version}");
        let source = match source.as_deref() {
            None => ProviderSource::Path { member: name },
            Some(source) if source.starts_with("registry+") => {
                let key = (name.clone(), version.clone(), source.to_owned());
                let checksum = checksums.get(&key).copied().ok_or(format!(
                    "locked registry checksum is missing for {package_id}"
                ))?;
                ProviderSource::Registry { checksum }
            }
            Some(source) if source.starts_with("git+") => {
                let (url, revision) = split_git_source(source)?;
                ProviderSource::Git { url, revision }
            }
            Some(source) => {
                return Err(
                    format!("unsupported provider source for {package_id}: {source}").into(),
                );
            }
        };
        records.push(ProviderRecord {
            package_id,
            version,
            source,
        });
    }
    Ok(records)
}

fn split_git_source(source: &str) -> Result<(String, String)> {
    let rest = source
        .strip_prefix("git+")
        .ok_or("git source lacks its scheme")?;
    let (url, revision) = rest
        .rsplit_once('#')
        .ok_or("git source lacks its resolved revision")?;
    let url = url.split('?').next().unwrap_or(url).to_owned();
    if url.is_empty() || revision.is_empty() {
        return Err("git source lacks its URL or revision".into());
    }
    Ok((url, revision.to_owned()))
}

fn hex_to_digest(hex: &str) -> Result<[u8; 32]> {
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("registry checksum is not 64 hexadecimal digits".into());
    }
    let mut out = [0u8; 32];
    for (index, chunk) in hex.as_bytes().chunks_exact(2).enumerate() {
        let pair = core::str::from_utf8(chunk).map_err(|_| "registry checksum is not ASCII")?;
        out[index] =
            u8::from_str_radix(pair, 16).map_err(|_| "registry checksum is not hexadecimal")?;
    }
    Ok(out)
}

fn rustflags() -> Vec<String> {
    std::env::var("RUSTFLAGS")
        .map(|flags| flags.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default()
}

fn collect_inputs(package_dir: &Path, owner: &str) -> Result<Vec<InputRecord>> {
    let mut inputs = Vec::new();
    let mut stack = vec![package_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries: Vec<_> = fs::read_dir(&dir)?.collect::<std::result::Result<_, _>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "package input name is not valid Unicode")?;
            if name.starts_with('.') {
                continue;
            }
            let file_type = entry.file_type()?;
            if file_type.is_symlink() || manifest::linked(&fs::symlink_metadata(entry.path())?) {
                return Err(format!("linked input is not admitted: {name}").into());
            }
            if file_type.is_dir() {
                if name != "target" {
                    stack.push(entry.path());
                }
                continue;
            }
            if !file_type.is_file() {
                return Err(format!("special input is not admitted: {name}").into());
            }
            if inputs.len() >= MAX_RECORDS {
                return Err("source inputs exceed the record budget".into());
            }
            let mut parts = Vec::new();
            let full = entry.path();
            let relative = full
                .strip_prefix(package_dir)
                .map_err(|_| "input escaped its package")?;
            for component in relative.components() {
                parts.push(
                    component
                        .as_os_str()
                        .to_str()
                        .ok_or("input name is not valid Unicode")?,
                );
            }
            inputs.push(InputRecord {
                owner: owner.to_owned(),
                path: parts.join("/"),
                length: entry.metadata()?.len(),
                digest: digest_stream(&mut fs::File::open(entry.path())?)?,
            });
        }
    }
    Ok(inputs)
}

/// Everything `assemble` needs, collected by `run`.
struct Assembly {
    package: String,
    package_id: String,
    target: String,
    export: String,
    platform: String,
    page_schema: u16,
    lock_digest: [u8; 32],
    providers: Vec<ProviderRecord>,
    tools: Vec<ToolRecorded>,
    rustflags: Vec<String>,
    inputs: Vec<InputRecord>,
    loader: Vec<u8>,
    wasm: Vec<u8>,
}

fn assemble(parts: Assembly) -> Result<EmbeddedModuleInventory> {
    if parts.loader.is_empty() || parts.wasm.is_empty() {
        return Err("generated module content is empty".into());
    }
    Ok(EmbeddedModuleInventory {
        package: parts.package,
        package_id: parts.package_id,
        cdylib_target: parts.target,
        wasm_target: WASM_TARGET.to_owned(),
        export_name: parts.export,
        platform_tag: parts.platform,
        page_schema: parts.page_schema,
        lock_digest: parts.lock_digest,
        providers: parts.providers,
        tools: parts
            .tools
            .into_iter()
            .map(|tool| ToolRecord {
                name: tool.name,
                version_output: tool.version,
                executable_digest: tool.digest,
            })
            .collect(),
        config: BuildConfiguration {
            profile: "release".to_owned(),
            features: Vec::new(),
            rustflags: parts.rustflags,
            environment: Vec::new(),
        },
        inputs: parts.inputs,
        assets: vec![
            AssetRecord {
                kind: AssetKind::JavaScript,
                destination: "page.js".to_owned(),
                length: u64::try_from(parts.loader.len())
                    .map_err(|_| "generated loader exceeds the module budget")?,
                digest: digest_bytes(&parts.loader),
            },
            AssetRecord {
                kind: AssetKind::Wasm,
                destination: "page_bg.wasm".to_owned(),
                length: u64::try_from(parts.wasm.len())
                    .map_err(|_| "generated module exceeds the module budget")?,
                digest: digest_bytes(&parts.wasm),
            },
        ],
    })
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut file = fs::File::options()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn report_json(inventory: &EmbeddedModuleInventory, digest: &[u8; 32]) -> serde_json::Value {
    use serde_json::{Map, Value};
    let mut root = Map::new();
    root.insert("magic".into(), Value::String("METISAS1".into()));
    root.insert("schema".into(), Value::from(INVENTORY_SCHEMA));
    root.insert("package".into(), Value::String(inventory.package.clone()));
    root.insert(
        "package_id".into(),
        Value::String(inventory.package_id.clone()),
    );
    root.insert(
        "cdylib_target".into(),
        Value::String(inventory.cdylib_target.clone()),
    );
    root.insert(
        "wasm_target".into(),
        Value::String(inventory.wasm_target.clone()),
    );
    root.insert(
        "export".into(),
        Value::String(inventory.export_name.clone()),
    );
    root.insert(
        "platform_tag".into(),
        Value::String(inventory.platform_tag.clone()),
    );
    root.insert("page_schema".into(), Value::from(inventory.page_schema));
    root.insert(
        "lock_digest".into(),
        Value::String(hex_digest(&inventory.lock_digest)),
    );
    root.insert(
        "providers".into(),
        Value::Array(report_providers(&inventory.providers)),
    );
    root.insert("tools".into(), Value::Array(report_tools(&inventory.tools)));
    root.insert(
        "config".into(),
        Value::Object(report_config(&inventory.config)),
    );
    root.insert(
        "inputs".into(),
        Value::Array(report_inputs(&inventory.inputs)),
    );
    root.insert(
        "assets".into(),
        Value::Array(report_assets(&inventory.assets)),
    );
    root.insert("inventory_sha256".into(), Value::String(hex_digest(digest)));
    Value::Object(root)
}

fn report_providers(providers: &[ProviderRecord]) -> Vec<serde_json::Value> {
    use serde_json::{Map, Value};
    providers
        .iter()
        .map(|provider| {
            let mut entry = Map::new();
            entry.insert(
                "package_id".into(),
                Value::String(provider.package_id.clone()),
            );
            entry.insert("version".into(), Value::String(provider.version.clone()));
            entry.insert("source".into(), report_provider_source(&provider.source));
            Value::Object(entry)
        })
        .collect()
}

fn report_provider_source(source: &ProviderSource) -> serde_json::Value {
    use serde_json::{Map, Value};
    let mut entry = Map::new();
    match source {
        ProviderSource::Git { url, revision } => {
            entry.insert("git".into(), Value::String(url.clone()));
            entry.insert("revision".into(), Value::String(revision.clone()));
        }
        ProviderSource::Registry { checksum } => {
            entry.insert("registry".into(), Value::String(hex_digest(checksum)));
        }
        ProviderSource::Path { member } => {
            entry.insert("path".into(), Value::String(member.clone()));
        }
    }
    Value::Object(entry)
}

fn report_tools(tools: &[ToolRecord]) -> Vec<serde_json::Value> {
    use serde_json::{Map, Value};
    tools
        .iter()
        .map(|tool| {
            let mut entry = Map::new();
            entry.insert("name".into(), Value::String(tool.name.clone()));
            entry.insert("version".into(), Value::String(tool.version_output.clone()));
            entry.insert(
                "digest".into(),
                Value::String(hex_digest(&tool.executable_digest)),
            );
            Value::Object(entry)
        })
        .collect()
}

fn report_config(config: &BuildConfiguration) -> serde_json::Map<String, serde_json::Value> {
    use serde_json::{Map, Value};
    let mut entry = Map::new();
    entry.insert("profile".into(), Value::String(config.profile.clone()));
    entry.insert(
        "features".into(),
        Value::Array(
            config
                .features
                .iter()
                .map(|feature| Value::String(feature.clone()))
                .collect(),
        ),
    );
    entry.insert(
        "rustflags".into(),
        Value::Array(
            config
                .rustflags
                .iter()
                .map(|flag| Value::String(flag.clone()))
                .collect(),
        ),
    );
    entry.insert(
        "environment".into(),
        Value::Array(
            config
                .environment
                .iter()
                .map(|(key, value)| {
                    let mut pair = Map::new();
                    pair.insert("key".into(), Value::String(key.clone()));
                    pair.insert("value".into(), Value::String(value.clone()));
                    Value::Object(pair)
                })
                .collect(),
        ),
    );
    entry
}

fn report_inputs(inputs: &[InputRecord]) -> Vec<serde_json::Value> {
    use serde_json::{Map, Value};
    inputs
        .iter()
        .map(|input| {
            let mut entry = Map::new();
            entry.insert("owner".into(), Value::String(input.owner.clone()));
            entry.insert("path".into(), Value::String(input.path.clone()));
            entry.insert("length".into(), Value::from(input.length));
            entry.insert("digest".into(), Value::String(hex_digest(&input.digest)));
            Value::Object(entry)
        })
        .collect()
}

fn report_assets(assets: &[AssetRecord]) -> Vec<serde_json::Value> {
    use serde_json::{Map, Value};
    assets
        .iter()
        .map(|asset| {
            let mut entry = Map::new();
            entry.insert(
                "kind".into(),
                Value::String(
                    match asset.kind {
                        AssetKind::JavaScript => "javascript",
                        AssetKind::Wasm => "wasm",
                    }
                    .into(),
                ),
            );
            entry.insert(
                "destination".into(),
                Value::String(asset.destination.clone()),
            );
            entry.insert("length".into(), Value::from(asset.length));
            entry.insert("digest".into(), Value::String(hex_digest(&asset.digest)));
            Value::Object(entry)
        })
        .collect()
}

#[cfg(test)]
mod tests;
