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
        artifacts::{module, module_target},
    },
    manifest::{self, Application},
    process,
};
use metis_core::embedded_assets::{
    AssetKind, AssetRecord, BuildConfiguration, EmbeddedModuleInventory, InputRecord, MAX_RECORDS,
    ProviderRecord, ToolRecord, decode, digest_inventory, encode, validate_wasm_artifact,
};
use providers::{LockedChecksums, lock_checksums, providers};
use report::report_json;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

mod providers;
mod report;

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

#[cfg(test)]
mod tests;
