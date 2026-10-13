//! Signed Apple bundle inventory and ownership-preserving rebuilds.
use super::Target;
use crate::{
    Result,
    build::{copy, digest_file},
    manifest::{self, WebApplication},
    tree,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub(super) fn identifier(application: &WebApplication) -> String {
    // Cargo treats hyphens and underscores as equivalent crate identifiers;
    // Apple's bundle identifier alphabet admits hyphens, not underscores.
    format!(
        "com.metis.{}",
        application.frontend.package.replace('_', "-")
    )
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    schema: u32,
    kind: String,
    target: String,
    name: String,
    files: Vec<crate::build::FileRecord>,
}

pub(super) fn create(
    application: &WebApplication,
    root: &Path,
    served: &Path,
    executable: &Path,
    output: &Path,
    target: Target,
) -> Result<PathBuf> {
    let icon = super::icon::read(application, root, target)?;
    // The provider's bound is authoritative. Reject incompatible assets before
    // replacing any prior output, then recheck the bytes copied from each file.
    let assets = tree::regular_files(served, |_| false)?;
    for path in &assets {
        validate_resource(path, fs::metadata(path)?.len())?;
    }
    let output = output_path(output)?;
    clear(&output, application, target)?;
    fs::create_dir(&output)?;
    let bundle = output.join(format!("{}.app", application.frontend.package));
    let program = program(&bundle, target);
    let (resources, plist) = if target == Target::Desktop {
        let contents = bundle.join("Contents");
        (contents.join("Resources"), contents.join("Info.plist"))
    } else {
        (bundle.clone(), bundle.join("Info.plist"))
    };
    fs::create_dir_all(
        program
            .parent()
            .ok_or("application program has no directory")?,
    )?;
    fs::create_dir_all(resources.join("app"))?;
    if let Some(bytes) = icon {
        fs::write(resources.join(super::icon::FILE_NAME), bytes)?;
    }
    let mut total = 0;
    copy(
        executable,
        &program,
        tree::relative_name(&output, &program)?,
        &mut total,
    )?;
    for path in assets {
        let name = tree::relative_name(served, &path)?;
        let destination = resources.join("app").join(&name);
        fs::create_dir_all(destination.parent().ok_or("resource has no directory")?)?;
        let record = copy(&path, &destination, name, &mut total)?;
        validate_resource(&path, record.bytes)?;
    }
    fs::write(&plist, info(application, target))?;
    Ok(bundle)
}

/// Target layout supplies the executable path to packaging and supervision.
pub(crate) fn program(bundle: &Path, target: Target) -> PathBuf {
    bundle.join(if target == Target::Desktop {
        "Contents/MacOS/metis-host"
    } else {
        "metis-host"
    })
}

fn validate_resource(path: &Path, bytes: u64) -> Result<()> {
    if bytes > moirai_webview::MAX_RESOURCE_BYTES {
        return Err(format!(
            "{} exceeds the {}-byte native resource budget",
            path.display(),
            moirai_webview::MAX_RESOURCE_BYTES
        )
        .into());
    }
    Ok(())
}

/// Record final bytes after codesign modifies the executable and adds files.
pub(super) fn record(application: &WebApplication, bundle: &Path, target: Target) -> Result<()> {
    let output = bundle.parent().ok_or("application bundle has no parent")?;
    let mut files = Vec::new();
    for path in tree::regular_files(bundle, |_| false)? {
        files.push(crate::build::FileRecord {
            destination: tree::relative_name(output, &path)?,
            bytes: fs::metadata(&path)?.len(),
            sha256: digest_file(&path)?,
        });
    }
    files.sort_by(|left, right| left.destination.cmp(&right.destination));
    let inventory = Inventory {
        schema: 1,
        kind: "apple".into(),
        target: target.label().into(),
        name: application.name.clone(),
        files,
    };
    fs::write(
        output.join("inventory.json"),
        serde_json::to_vec_pretty(&inventory)?,
    )?;
    Ok(())
}

/// Rebuild only an unchanged output that the compiler inventory owns in full.
/// A modified, added, missing, linked or special file prevents any deletion.
fn clear(output: &Path, application: &WebApplication, target: Target) -> Result<()> {
    let metadata = match fs::symlink_metadata(output) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if manifest::linked(&metadata) || !metadata.is_dir() {
        return Err("Apple output is not an unlinked directory".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(output.join("inventory.json"))?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err("Apple inventory exceeds the 1 MiB budget".into());
    }
    let inventory: Inventory = serde_json::from_slice(&bytes)?;
    if inventory.schema != 1
        || inventory.kind != "apple"
        || inventory.target != target.label()
        || inventory.name != application.name
    {
        return Err("output is not a previous build of this Apple application and target".into());
    }
    let files = tree::regular_files(output, |_| false)?;
    if files.len() != inventory.files.len() + 1 {
        return Err("Apple output contains missing or unrecorded files".into());
    }
    for file in files {
        let name = tree::relative_name(output, &file)?;
        if name == "inventory.json" {
            continue;
        }
        let record = inventory
            .files
            .iter()
            .find(|record| record.destination == name)
            .ok_or("Apple output contains an unrecorded file")?;
        if record.bytes != fs::metadata(&file)?.len() || record.sha256 != digest_file(&file)? {
            return Err("Apple output contains a modified file".into());
        }
    }
    fs::remove_dir_all(output)?;
    Ok(())
}

fn output_path(output: &Path) -> Result<PathBuf> {
    let absolute = std::env::current_dir()?.join(output);
    // Validate existing ancestors before creating any output directories.
    for path in absolute.ancestors() {
        match fs::symlink_metadata(path) {
            Ok(meta) if manifest::linked(&meta) => {
                return Err("linked Apple output parent is not admitted".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    let parent = absolute.parent().ok_or("Apple output has no parent")?;
    let name = absolute
        .file_name()
        .ok_or("Apple output has no directory name")?;
    fs::create_dir_all(parent)?;
    Ok(parent.canonicalize()?.join(name))
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub(super) fn info(application: &WebApplication, target: Target) -> String {
    let (_, minimum) = target.deployment();
    let platform = if target == Target::Desktop {
        format!(
            "<key>LSMinimumSystemVersion</key><string>{minimum}</string><key>NSHighResolutionCapable</key><true/>"
        )
    } else {
        format!(
            "<key>MinimumOSVersion</key><string>{minimum}</string><key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array><key>UILaunchScreen</key><dict/><key>UIApplicationSceneManifest</key><dict><key>UIApplicationSupportsMultipleScenes</key><false/><key>UISceneConfigurations</key><dict/></dict><key>UISupportedInterfaceOrientations</key><array><string>UIInterfaceOrientationPortrait</string><string>UIInterfaceOrientationLandscapeLeft</string><string>UIInterfaceOrientationLandscapeRight</string></array>"
        )
    };
    let icon = if target == Target::Desktop && application.icon.is_some() {
        format!(
            "<key>CFBundleIconFile</key><string>{}</string>",
            super::icon::FILE_NAME
        )
    } else {
        String::new()
    };
    // Manifest preflight uses the provider's validator. The signed array keeps
    // exact destination identity; the provider revalidates it on host startup.
    let external_links = format!(
        "<key>{}</key><array>{}</array>",
        moirai_webview::EXTERNAL_LINKS_INFO_KEY,
        application
            .external_links
            .iter()
            .fold(String::new(), |mut xml, destination| {
                xml.push_str("<string>");
                xml.push_str(&escape(destination));
                xml.push_str("</string>");
                xml
            })
    );
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>{}</string><key>CFBundleName</key><string>{}</string><key>CFBundleDisplayName</key><string>{}</string><key>CFBundleExecutable</key><string>metis-host</string><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleShortVersionString</key><string>{}</string><key>CFBundleVersion</key><string>1</string>{platform}{icon}{external_links}</dict></plist>\n",
        identifier(application),
        escape(&application.name),
        escape(&application.name),
        env!("CARGO_PKG_VERSION")
    )
}
