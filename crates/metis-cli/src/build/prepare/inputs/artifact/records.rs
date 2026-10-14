//! Raw dependency records retain their compiler-unit owner and native IO source.
use super::{Prefix, Unit, logical_stem};
use crate::Result;
use metis_core::embedded_assets::{FILE_LIMIT, canonical_directory, canonical_file};
use std::{
    collections::BTreeMap,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

pub(in crate::build::prepare::inputs) struct Record<'unit> {
    pub(in crate::build::prepare::inputs) path: PathBuf,
    artifact: &'unit Path,
}

/// Only the selected prepared root uses its explicit rustc raw output. All
/// other units use reported raw primary representations, never checked keys
/// or an uplifted-only dylib's exported dependency declaration.
pub(in crate::build::prepare::inputs) fn collect<'unit>(
    unit: &'unit Unit,
    package: &str,
    root: Option<&Path>,
) -> Result<Vec<Record<'unit>>> {
    let mut records = BTreeMap::new();
    if unit.dylib && root.is_none() && !unit.primary.iter().any(|primary| primary.metadata) {
        return Err(format!(
            "compiler package {package} target {} has an uplifted-only dylib without a reported raw metadata record",
            unit.target
        ).into());
    }
    for primary in &unit.primary {
        context(
            unit,
            package,
            &primary.path,
            &primary.path,
            "admit primary output",
            || canonical_file(&primary.path).map_err(Into::into),
        )?;
        let path = if let Some(root) = root {
            admit(unit, package, &primary.path, root)?
        } else if unit.custom_build {
            custom(unit, package, &primary.path)?
        } else if unit.dylib && !primary.metadata {
            // The reported raw metadata belongs to this whole compiler unit.
            // Cargo may uplift dependency dylibs without writing an exported
            // .d beside them; their raw metadata remains the authoritative set.
            continue;
        } else {
            admit(unit, package, &primary.path, &primary.dependency)?
        };
        records.entry(path).or_insert(primary.path.as_path());
    }
    Ok(records
        .into_iter()
        .map(|(path, artifact)| Record { path, artifact })
        .collect())
}

pub(in crate::build::prepare::inputs) fn read(
    unit: &Unit,
    package: &str,
    record: &Record<'_>,
) -> Result<Vec<u8>> {
    context(
        unit,
        package,
        record.artifact,
        &record.path,
        "read raw dep-info",
        || {
            crate::bounded_read::read_file(
                &record.path,
                metis_core::embedded_assets::MANIFEST_LIMIT,
                "compiler dep-info exceeds 1 MiB",
            )
        },
    )
}

fn admit(unit: &Unit, package: &str, artifact: &Path, path: &Path) -> Result<PathBuf> {
    context(unit, package, artifact, path, "admit raw dep-info", || {
        canonical_file(path).map_err(Into::into)
    })
}

fn custom(unit: &Unit, package: &str, artifact: &Path) -> Result<PathBuf> {
    let parent = artifact
        .parent()
        .ok_or("custom-build primary has no instance directory")?;
    let parent = context(
        unit,
        package,
        artifact,
        parent,
        "admit custom-build instance",
        || canonical_directory(parent).map_err(Into::into),
    )?;
    context(
        unit,
        package,
        artifact,
        &parent,
        "locate custom-build raw dep-info",
        || custom_in(unit, package, artifact, &parent),
    )
}

fn custom_in(unit: &Unit, package: &str, artifact: &Path, parent: &Path) -> Result<PathBuf> {
    let entries = context(
        unit,
        package,
        artifact,
        parent,
        "read custom-build instance",
        || fs::read_dir(parent).map_err(Into::into),
    )?;
    let mut selected = None;
    // The dedicated compiler instance is streamed. At most one candidate path
    // is retained, and the existing input-record budget also bounds the walk.
    for (index, entry) in entries.enumerate() {
        let entry = context(
            unit,
            package,
            artifact,
            parent,
            "read custom-build entry",
            || entry.map_err(Into::into),
        )?;
        if index >= FILE_LIMIT {
            return Err("custom-build instance exceeds the canonical input-entry budget".into());
        }
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "d") {
            if selected.is_some() {
                return Err("custom-build instance contains ambiguous raw dep-info".into());
            }
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or("custom-build raw dep-info basename is not Unicode")?;
            logical_stem(stem, &unit.target, Prefix::Absent, true)?;
            selected = Some(admit(unit, package, artifact, &path)?);
        }
    }
    selected.ok_or_else(|| "custom-build instance contains no raw dep-info".into())
}

fn context<T>(
    unit: &Unit,
    package: &str,
    artifact: &Path,
    path: &Path,
    operation: &'static str,
    action: impl FnOnce() -> Result<T>,
) -> Result<T> {
    action().map_err(|source| {
        DependencyError {
            package: package.into(),
            target: unit.target.clone(),
            artifact: artifact.into(),
            path: path.into(),
            operation,
            source,
        }
        .into()
    })
}

#[derive(Debug)]
struct DependencyError {
    package: String,
    target: String,
    artifact: PathBuf,
    path: PathBuf,
    operation: &'static str,
    // Condition 1: the cold boundary retains native IO and bounded-reader
    // errors whose concrete type is unknown to the shared CLI Result owner.
    source: Box<dyn Error>,
}
impl fmt::Display for DependencyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: package {:?}, target {:?}, artifact ",
            self.operation, self.package, self.target,
        )?;
        fmt::Debug::fmt(self.artifact.as_os_str(), formatter)?;
        formatter.write_str(", dependency ")?;
        fmt::Debug::fmt(self.path.as_os_str(), formatter)?;
        write!(formatter, ": {}", self.source)
    }
}
impl Error for DependencyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        // Condition 1: Error's standard source API preserves the original
        // concrete native error behind the CLI's heterogeneous failure boundary.
        Some(self.source.as_ref())
    }
}

#[cfg(test)]
mod tests;
