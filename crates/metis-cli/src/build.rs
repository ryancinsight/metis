//! One compiler-produced inventory feeds portable directories and installers.
use crate::{
    Result,
    manifest::{self, Application, PAYLOAD_LIMIT},
    process,
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

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
}

#[derive(Serialize)]
struct FileRecord {
    destination: String,
    bytes: u64,
    sha256: String,
}
#[derive(Serialize)]
struct InstallerRecord {
    file: String,
    product_code: String,
    sha256: String,
}

pub(crate) fn application(input: &Path, output: &Path, kind: OutputKind) -> Result<()> {
    if !cfg!(all(windows, target_arch = "x86_64")) {
        return Err("application distribution currently requires a Windows x64 host".into());
    }
    let (application, root) = Application::read(input)?;
    let cargo_manifest = manifest::source(&root, &application.cargo_manifest)?;
    let icon = icon(&application, &root)?;
    let output = absolute_output(output)?;
    if output.try_exists()? {
        return Err("output already exists; choose a new output directory".into());
    }
    let mut resources = Vec::new();
    for resource in &application.resources {
        resources.push((
            manifest::source(&root, &resource.source)?,
            resource.destination.clone(),
        ));
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
    let mut files = Vec::new();
    let mut staged = Vec::new();
    let mut written = 0_u64;
    for (path, destination) in payload {
        let target = portable.join(&destination);
        fs::create_dir_all(target.parent().ok_or("payload destination has no parent")?)?;
        let record = copy(&path, &target, destination.clone(), &mut written)?;
        files.push(record);
        staged.push((target, destination));
    }
    let installer = match kind {
        OutputKind::Portable => None,
        OutputKind::Installer => {
            #[cfg(windows)]
            {
                let file = format!("{}.msi", application.id);
                let path = output.join(&file);
                let spec = crate::windows::InstallerSpec {
                    id: &application.id,
                    name: &application.name,
                    version: &application.version,
                    manufacturer: &application.manufacturer,
                    upgrade_code: &application.upgrade_code,
                    entry: &entry,
                    arguments: &application.arguments,
                    files: &staged,
                    icon: icon.as_deref(),
                };
                let product_code = crate::windows::build(&spec, &path)?;
                let (stored_code, stored_files) = crate::windows::inspect(&path)?;
                if stored_code != product_code || stored_files.len() != files.len() {
                    return Err("installer inventory does not match staged application".into());
                }
                Some(InstallerRecord {
                    file,
                    product_code,
                    sha256: digest_file(&path)?,
                })
            }
            #[cfg(not(windows))]
            {
                return Err("this installer format requires a Windows build host".into());
            }
        }
    };
    let inventory = Inventory {
        schema: 1,
        application,
        entry,
        files,
        installer,
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
        "--target=x86_64-pc-windows-msvc",
        "--locked",
        "--message-format=json-render-diagnostics",
        "--manifest-path",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    args.push(cargo_manifest.into_os_string());
    for binary in &application.binaries {
        args.extend([
            OsString::from("--package"),
            OsString::from(&binary.package),
            OsString::from("--bin"),
            OsString::from(&binary.bin),
        ]);
    }
    let messages = process::capture(&cargo, &args, Duration::from_mins(5))?;
    artifacts(&messages, &selected)
}

fn targets(metadata: &[u8], application: &Application) -> Result<BTreeMap<String, String>> {
    let document: serde_json::Value = serde_json::from_slice(metadata)?;
    let packages = document
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or("Cargo metadata omitted packages")?;
    let members = document
        .get("workspace_members")
        .and_then(serde_json::Value::as_array)
        .ok_or("Cargo metadata omitted workspace membership")?;
    let mut selected = BTreeMap::new();
    for binary in &application.binaries {
        let mut candidates = packages.iter().filter(|package| {
            package.get("name").and_then(serde_json::Value::as_str) == Some(&binary.package)
                && package.get("id").is_some_and(|id| members.contains(id))
        });
        let package = candidates
            .next()
            .ok_or("declared binary package is not a workspace member")?;
        if candidates.next().is_some() {
            return Err("declared workspace package is ambiguous".into());
        }
        let targets = package
            .get("targets")
            .and_then(serde_json::Value::as_array)
            .ok_or("Cargo package omitted targets")?;
        if !targets.iter().any(|target| {
            target.get("name").and_then(serde_json::Value::as_str) == Some(&binary.bin)
                && target
                    .get("kind")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("bin")))
        }) {
            return Err("declared binary does not belong to its selected workspace package".into());
        }
        let id = package
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or("Cargo package omitted identity")?;
        selected.insert(binary.bin.clone(), id.to_owned());
    }
    Ok(selected)
}

fn artifacts(
    messages: &[u8],
    selected: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, PathBuf>> {
    let mut artifacts = BTreeMap::new();
    for line in messages
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let message: serde_json::Value = serde_json::from_slice(line)?;
        if message.get("reason").and_then(serde_json::Value::as_str) != Some("compiler-artifact") {
            continue;
        }
        let target = message
            .get("target")
            .ok_or("compiler artifact has no target")?;
        let Some(name) = target.get("name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if !selected.contains_key(name) {
            continue;
        }
        if !target
            .get("kind")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|kinds| kinds.iter().any(|value| value.as_str() == Some("bin")))
        {
            continue;
        }
        if message
            .get("package_id")
            .and_then(serde_json::Value::as_str)
            != selected.get(name).map(String::as_str)
        {
            return Err(
                "compiler artifact package identity differs from the declared binary owner".into(),
            );
        }
        let path = message
            .get("executable")
            .and_then(serde_json::Value::as_str)
            .ok_or("compiler did not produce the selected executable")?;
        if !Path::new(path).is_absolute() {
            return Err("Cargo executable artifact must be absolute".into());
        }
        if artifacts
            .insert(name.to_owned(), PathBuf::from(path))
            .is_some()
        {
            return Err("ambiguous duplicate compiler executable artifact".into());
        }
    }
    for name in selected.keys() {
        if !artifacts.contains_key(name) {
            return Err(format!("Cargo did not report required binary {name}").into());
        }
    }
    Ok(artifacts)
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

fn copy(source: &Path, target: &Path, destination: String, total: &mut u64) -> Result<FileRecord> {
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
