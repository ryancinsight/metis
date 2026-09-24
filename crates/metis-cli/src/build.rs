//! One compiler-produced inventory feeds portable directories and installers.
use crate::{
    Result,
    manifest::{self, Application, PAYLOAD_LIMIT},
    process,
};
use artifacts::{artifacts, targets};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

mod artifacts;
#[cfg(any(not(windows), test))]
mod install;
mod platform;
pub(crate) mod web;

#[cfg(any(not(windows), test))]
pub(crate) use platform::{desktop_icon, desktop_word};

// A cold RITK workspace can exceed five minutes on a hosted Windows runner;
// fifteen minutes leaves the packaging workflow's remaining budget for MSI
// authoring and inventory verification while keeping compiler ownership finite.
const CARGO_BUILD_DEADLINE: Duration = Duration::from_mins(15);

#[derive(Clone, Copy)]
pub(crate) enum OutputKind {
    Portable,
    Installer,
}

#[derive(Serialize)]
struct Inventory {
    schema: u32,
    application: Application,
    entry: String,
    files: Vec<FileRecord>,
    installer: Option<InstallerRecord>,
    platform_package: Option<platform::PlatformPackageRecord>,
}

#[derive(Serialize)]
pub(crate) struct FileRecord {
    pub(crate) destination: String,
    pub(crate) bytes: u64,
    pub(crate) sha256: String,
}
#[derive(Serialize)]
struct InstallerRecord {
    file: String,
    product_code: String,
    sha256: String,
}

struct StagedPayload {
    files: Vec<FileRecord>,
    entries: Vec<(PathBuf, String)>,
}

pub(crate) fn application(
    application: Application,
    root: &Path,
    output: &Path,
    kind: OutputKind,
) -> Result<()> {
    if matches!(kind, OutputKind::Installer) && !supports_host_package() {
        return Err("package requires an x86-64 Windows, macOS or Linux host".into());
    }
    let cargo_manifest = manifest::source(root, &application.cargo_manifest)?;
    #[cfg(windows)]
    let icon = icon(&application, root)?;
    let output = absolute_output(output)?;
    if output.try_exists()? {
        return Err("output already exists; choose a new output directory".into());
    }
    let mut resources = Vec::new();
    for resource in &application.resources {
        let source = manifest::source(root, &resource.source)?;
        manifest::validate_resource_file(&source)?;
        resources.push((source, resource.destination.clone()));
    }
    let artifacts = compile(cargo_manifest, &application)?;
    let entry = format!("{}{}", application.entry, std::env::consts::EXE_SUFFIX);
    let payload = artifacts
        .into_iter()
        .map(|(name, path)| (path, format!("{name}{}", std::env::consts::EXE_SUFFIX)))
        .chain(resources)
        .collect::<Vec<_>>();
    validate_payload(&payload)?;
    // Create-new ownership is the rollback boundary: no pre-existing output is
    // overwritten or recursively erased, including on partial build failure.
    fs::create_dir(&output)?;
    let portable = output.join("app");
    fs::create_dir(&portable)?;
    let staged = stage_payload(&payload, &portable)?;
    #[cfg(windows)]
    let installer = installer(
        kind,
        &application,
        &output,
        &entry,
        icon.as_deref(),
        &staged.files,
        &staged.entries,
    )?;
    #[cfg(not(windows))]
    let installer = installer(kind);
    #[cfg(windows)]
    let platform_package = platform_package(kind);
    #[cfg(not(windows))]
    let platform_package = platform_package(kind, &application, &output, &entry, &staged.entries)?;
    let inventory = Inventory {
        schema: 1,
        application,
        entry,
        files: staged.files,
        installer,
        platform_package,
    };
    let mut report = fs::File::options()
        .write(true)
        .create_new(true)
        .open(output.join("inventory.json"))?;
    serde_json::to_writer_pretty(&mut report, &inventory)?;
    report.write_all(b"\n")?;
    report.sync_all()?;
    println!("{}", output.join("inventory.json").display());
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn install(archive: &Path, prefix: &Path) -> Result<()> {
    install::run(archive, prefix)
}

#[cfg(windows)]
pub(crate) fn install(_archive: &Path, _prefix: &Path) -> Result<()> {
    Err("Linux package installation requires a Linux host".into())
}

#[cfg(not(windows))]
pub(crate) fn uninstall(application_id: &str, prefix: &Path) -> Result<()> {
    install::uninstall(application_id, prefix)
}

#[cfg(windows)]
pub(crate) fn uninstall(_application_id: &str, _prefix: &Path) -> Result<()> {
    Err("Linux package removal requires a Linux host".into())
}

fn supports_host_package() -> bool {
    cfg!(all(windows, target_arch = "x86_64"))
        || cfg!(target_os = "macos")
        || cfg!(target_os = "linux")
}

fn stage_payload(payload: &[(PathBuf, String)], portable: &Path) -> Result<StagedPayload> {
    let mut files = Vec::new();
    let mut staged = Vec::new();
    let mut written = 0_u64;
    for (path, destination) in payload {
        let target = portable.join(destination);
        fs::create_dir_all(target.parent().ok_or("payload destination has no parent")?)?;
        files.push(copy(path, &target, destination.clone(), &mut written)?);
        staged.push((target, destination.clone()));
    }
    Ok(StagedPayload {
        files,
        entries: staged,
    })
}

#[cfg(windows)]
fn installer(
    kind: OutputKind,
    application: &Application,
    output: &Path,
    entry: &str,
    icon: Option<&Path>,
    files: &[FileRecord],
    staged: &[(PathBuf, String)],
) -> Result<Option<InstallerRecord>> {
    if matches!(kind, OutputKind::Portable) {
        return Ok(None);
    }
    let file = format!("{}.msi", application.id);
    let path = output.join(&file);
    let spec = crate::windows::InstallerSpec {
        id: &application.id,
        name: &application.name,
        version: &application.version,
        manufacturer: &application.manufacturer,
        upgrade_code: &application.upgrade_code,
        entry,
        arguments: &application.arguments,
        url_schemes: &application.url_schemes,
        file_associations: &application.file_associations,
        files: staged,
        icon,
    };
    let product_code = crate::windows::build(&spec, &path)?;
    let (stored_code, stored_files) = crate::windows::inspect(&path)?;
    if stored_code != product_code || stored_files.len() != files.len() {
        return Err("installer inventory does not match staged application".into());
    }
    Ok(Some(InstallerRecord {
        file,
        product_code,
        sha256: digest_file(&path)?,
    }))
}

#[cfg(not(windows))]
fn installer(_kind: OutputKind) -> Option<InstallerRecord> {
    None
}

#[cfg(windows)]
fn platform_package(_kind: OutputKind) -> Option<platform::PlatformPackageRecord> {
    None
}

#[cfg(not(windows))]
fn platform_package(
    kind: OutputKind,
    application: &Application,
    output: &Path,
    entry: &str,
    staged: &[(PathBuf, String)],
) -> Result<Option<platform::PlatformPackageRecord>> {
    if matches!(kind, OutputKind::Portable) {
        return Ok(None);
    }
    Ok(Some(platform::build(application, output, entry, staged)?))
}

#[cfg(windows)]
fn icon(application: &Application, root: &Path) -> Result<Option<PathBuf>> {
    application
        .icon
        .as_deref()
        .map(|path| manifest::icon_source(root, path))
        .transpose()
}

fn validate_payload(payload: &[(PathBuf, String)]) -> Result<()> {
    let mut total = 0_u64;
    for (path, _) in payload {
        let meta = fs::symlink_metadata(path)?;
        if !meta.is_file() || manifest::linked(&meta) {
            return Err("artifact is not a regular, unlinked file".into());
        }
        total = total
            .checked_add(meta.len())
            .ok_or("payload size overflow")?;
        if total > PAYLOAD_LIMIT {
            return Err("application payload exceeds the 1 GiB budget".into());
        }
    }
    Ok(())
}

fn compile(
    cargo_manifest: PathBuf,
    application: &Application,
) -> Result<BTreeMap<String, PathBuf>> {
    let cargo = process::tool("cargo")?;
    let metadata_args = [
        OsString::from("metadata"),
        OsString::from("--no-deps"),
        OsString::from("--format-version=1"),
        OsString::from("--locked"),
        OsString::from("--manifest-path"),
        cargo_manifest.as_os_str().to_owned(),
    ];
    let metadata = process::capture(&cargo, &metadata_args, Duration::from_mins(1))?;
    let selected = targets(&metadata, application)?;
    let mut args: Vec<OsString> = [
        "build",
        "--release",
        "--locked",
        "--message-format=json-render-diagnostics",
        "--manifest-path",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    if let Some(target) = cargo_target(std::env::consts::OS, std::env::consts::ARCH) {
        args.insert(2, format!("--target={target}").into());
    }
    args.push(cargo_manifest.into_os_string());
    for binary in &application.binaries {
        args.extend([
            OsString::from("--package"),
            OsString::from(&binary.package),
            OsString::from("--bin"),
            OsString::from(&binary.bin),
        ]);
    }
    let messages = process::capture(&cargo, &args, CARGO_BUILD_DEADLINE)?;
    artifacts(&messages, &selected)
}

fn cargo_target(os: &str, architecture: &str) -> Option<&'static str> {
    (os == "windows" && architecture == "x86_64").then_some("x86_64-pc-windows-msvc")
}

fn absolute_output(path: &Path) -> Result<PathBuf> {
    let name = path.file_name().ok_or("output requires a directory name")?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // Preserve parent components until link checks finish: normalizing `..`
    // first could erase the very junction that must be rejected.
    let parent = std::env::current_dir()?.join(parent);
    let mut cursor = parent.as_path();
    loop {
        if manifest::linked(&fs::symlink_metadata(cursor)?) {
            return Err("linked output parent is not admitted".into());
        }
        let Some(next) = cursor.parent() else {
            break;
        };
        cursor = next;
    }
    Ok(parent.canonicalize()?.join(name))
}

pub(super) fn copy(
    source: &Path,
    target: &Path,
    destination: String,
    total: &mut u64,
) -> Result<FileRecord> {
    let mut source = fs::File::open(source)?;
    let expected = source.metadata()?.len();
    let mut output = fs::File::options()
        .create_new(true)
        .write(true)
        .open(target)?;
    let mut hash = moirai_crypto::Sha256::new();
    let mut buffer = [0; 16 * 1024];
    let mut bytes = 0_u64;
    loop {
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes = bytes
            .checked_add(count as u64)
            .ok_or("file size overflow")?;
        *total = total
            .checked_add(count as u64)
            .ok_or("payload size overflow")?;
        if bytes > expected || *total > PAYLOAD_LIMIT {
            return Err("payload grew beyond its validated budget".into());
        }
        output.write_all(&buffer[..count])?;
        hash.update(&buffer[..count]);
    }
    if bytes != expected {
        return Err("payload changed size during staging".into());
    }
    output.set_permissions(source.metadata()?.permissions())?;
    output.sync_all()?;
    Ok(FileRecord {
        destination,
        bytes,
        sha256: hex(&hash.finalize()),
    })
}

fn digest_file(path: &Path) -> Result<String> {
    let mut source = fs::File::open(path)?;
    let mut hash = moirai_crypto::Sha256::new();
    let mut buffer = [0; 16 * 1024];
    loop {
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hex(&hash.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("invariant: String formatting cannot fail");
    }
    output
}

#[cfg(test)]
mod tests;
