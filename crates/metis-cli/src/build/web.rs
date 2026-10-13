//! Browser applications: a WebAssembly module, its generated loader and the page.
use super::{FileRecord, absolute_output, copy, digest_file};
use crate::{Result, manifest::WebApplication, tree};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Output directory beside the manifest when none is named, as in Trunk.
const DEFAULT_OUTPUT: &str = "dist";
/// The served tree, parallel to a native build's `app/`.
const SERVED: &str = "app";
const INVENTORY: &str = "inventory.json";
const INVENTORY_KIND: &str = "browser";
#[derive(Serialize)]
struct Inventory<'application> {
    schema: u32,
    kind: &'static str,
    name: &'application str,
    files: Vec<FileRecord>,
}

/// The fields of a previous inventory that identify a replaceable build.
#[derive(Deserialize)]
struct Marker {
    schema: u32,
    kind: String,
}

/// Builds `application` and returns the directory to serve.
///
/// `output` defaults to `dist` beside the manifest. An existing output is
/// replaced only when its inventory records a previous browser build, so a
/// build never erases a directory it did not create.
pub(crate) fn application(
    application: &WebApplication,
    root: &Path,
    output: Option<&Path>,
) -> Result<PathBuf> {
    let cargo_manifest = metis_core::embedded_assets::source(root, &application.cargo_manifest)?;
    let pages = page_files(&root.join(&application.frontend.directory))?;
    let output = match output {
        Some(path) => absolute_output(path)?,
        None => root.join(DEFAULT_OUTPUT),
    };
    let compilation = super::module::resolve(&cargo_manifest, &application.frontend.package)?
        .compile(&cargo_manifest)?;
    clear(&output)?;
    fs::create_dir(&output)?;
    let served = output.join(SERVED);
    fs::create_dir(&served)?;
    compilation.generate(&served)?;
    let mut files = Vec::new();
    let mut total = 0_u64;
    for path in tree::regular_files(&served, |_| false)? {
        let record = FileRecord {
            destination: tree::relative_name(&served, &path)?,
            bytes: fs::metadata(&path)?.len(),
            sha256: digest_file(&path)?,
        };
        total = total
            .checked_add(record.bytes)
            .ok_or("payload size overflow")?;
        files.push(record);
    }
    for (source, destination) in pages {
        let staged = served.join(&destination);
        if staged.try_exists()? {
            return Err(
                format!("frontend file {destination} collides with the generated loader").into(),
            );
        }
        fs::create_dir_all(staged.parent().ok_or("page destination has no parent")?)?;
        files.push(copy(&source, &staged, destination, &mut total)?);
    }
    files.sort_by(|left, right| left.destination.cmp(&right.destination));
    let inventory = Inventory {
        schema: 1,
        kind: INVENTORY_KIND,
        name: &application.name,
        files,
    };
    let mut report = fs::File::options()
        .write(true)
        .create_new(true)
        .open(output.join(INVENTORY))?;
    serde_json::to_writer_pretty(&mut report, &inventory)?;
    report.write_all(b"\n")?;
    report.sync_all()?;
    Ok(served)
}

/// The page's files with their served names; the page must have an `index.html`.
fn page_files(directory: &Path) -> Result<Vec<(PathBuf, String)>> {
    let metadata = fs::symlink_metadata(directory)?;
    if !metadata.is_dir() || metis_core::embedded_assets::linked(&metadata) {
        return Err("frontend directory must be a directory, not a link".into());
    }
    let mut pages = Vec::new();
    for path in tree::regular_files(directory, |_| false)? {
        let name = tree::relative_name(directory, &path)?;
        metis_core::embedded_assets::relative(&name)?;
        pages.push((path, name));
    }
    if !pages.iter().any(|(_, name)| name == "index.html") {
        return Err("frontend directory has no index.html".into());
    }
    Ok(pages)
}

/// Removes a previous browser build at `output`; any other existing entry is refused.
fn clear(output: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(output) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let previous = metadata.is_dir() && !metis_core::embedded_assets::linked(&metadata) && {
        let inventory = fs::read(output.join(INVENTORY)).ok();
        inventory
            .and_then(|bytes| serde_json::from_slice::<Marker>(&bytes).ok())
            .is_some_and(|marker| marker.schema == 1 && marker.kind == INVENTORY_KIND)
    };
    if !previous {
        return Err(format!(
            "{} exists and is not a previous browser build; choose a new output directory",
            output.display()
        )
        .into());
    }
    fs::remove_dir_all(output)?;
    Ok(())
}
