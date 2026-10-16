//! Owned neutral invocation directory and explicit Cargo configuration inputs.
#[cfg(test)]
mod tests;
use crate::Result;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) struct Context {
    pub(crate) directory: PathBuf,
    pub(crate) workspace: PathBuf,
    pub(crate) environment: Vec<(&'static str, OsString)>,
    pub(crate) configurations: Vec<PathBuf>,
}
impl Context {
    pub(crate) fn new(manifest: &Path) -> Result<Self> {
        let package_root = manifest.parent().ok_or("Cargo manifest has no parent")?;
        let mut root = package_root;
        for ancestor in package_root.ancestors() {
            let manifest = ancestor.join("Cargo.toml");
            if manifest.is_file() && read_toml(&manifest)?.get("workspace").is_some() {
                root = ancestor;
                break;
            }
        }
        // Fallible source admission precedes allocation. After allocation the
        // Context owns every subsequent failure path through its Drop cleanup.
        let workspace = metis_core::embedded_assets::canonical_directory(root)?;
        let mut context = Self {
            directory: fresh_directory()?,
            workspace,
            environment: Vec::new(),
            configurations: Vec::new(),
        };
        let locations = metis_core::embedded_assets::CargoLocations::current(root)?;
        context
            .environment
            .push(("CARGO_HOME", locations.cargo_home().as_os_str().to_owned()));
        if let Some(target) = locations.target_override() {
            context
                .environment
                .push(("CARGO_TARGET_DIR", target.as_os_str().to_owned()));
        }
        let ancestors = root.ancestors().collect::<Vec<_>>();
        {
            if let Some(pin) = ancestors
                .iter()
                .map(|ancestor| ancestor.join("rust-toolchain.toml"))
                .find(|path| path.is_file())
            {
                let document = read_toml(&pin)?;
                let channel = document
                    .get("toolchain")
                    .and_then(|toolchain| toolchain.get("channel"))
                    .and_then(toml::Value::as_str)
                    .ok_or("Rust toolchain pin omits channel")?;
                if std::env::var_os("RUSTUP_TOOLCHAIN").is_none() {
                    context
                        .environment
                        .push(("RUSTUP_TOOLCHAIN", channel.into()));
                }
                context
                    .configurations
                    .push(metis_core::embedded_assets::canonical_file(&pin)?);
            }
        }
        // Explicit project configuration survives neutral cwd; ancestor stack
        // patches do not enter the lock-authoritative module invocation.
        if let Some(config) = metis_core::embedded_assets::configuration(&root.join(".cargo"))? {
            // Loading through Cargo's normal include keeps environment
            // precedence and the included file's original relative path base.
            // This operational file derives only from the declared config path.
            let settings = context.directory.join(".cargo");
            fs::create_dir(&settings)?;
            fs::write(
                settings.join("config.toml"),
                format!(
                    "include = [{:?}]\n",
                    config.to_str().ok_or("Cargo config path is not UTF-8")?
                ),
            )?;
            context
                .configurations
                .extend(metis_core::embedded_assets::configuration_inputs(&config)?);
        }
        if let Some(config) = metis_core::embedded_assets::configuration(locations.cargo_home())? {
            context
                .configurations
                .extend(metis_core::embedded_assets::configuration_inputs(&config)?);
        }
        context.configurations.sort();
        context.configurations.dedup();
        Ok(context)
    }
}
impl Drop for Context {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.directory) {
            tracing::warn!(%error, directory = %self.directory.display(), "neutral build-directory cleanup failed");
        }
    }
}
pub(crate) fn cargo_home(workspace: &Path) -> Result<PathBuf> {
    Ok(
        metis_core::embedded_assets::CargoLocations::current(workspace)?
            .cargo_home()
            .to_path_buf(),
    )
}
fn fresh_directory() -> Result<PathBuf> {
    let epoch = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    for attempt in 0..8 {
        let directory = std::env::temp_dir().join(format!(
            "metis-module-{}-{epoch}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&directory) {
            Ok(()) => match directory.canonicalize() {
                Ok(directory) => return Ok(directory),
                Err(primary) => {
                    if let Err(cleanup) = fs::remove_dir_all(&directory) {
                        return Err(format!(
                            "{primary}; neutral directory cleanup failed: {cleanup}"
                        )
                        .into());
                    }
                    return Err(primary.into());
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err("cannot acquire neutral build-directory ownership after eight attempts".into())
}
fn read_toml(path: &Path) -> Result<toml::Value> {
    let bytes = crate::bounded_read::read_file(
        path,
        metis_core::embedded_assets::MANIFEST_LIMIT,
        "Cargo configuration exceeds 1 MiB",
    )?;
    Ok(toml::from_str(std::str::from_utf8(&bytes)?)?)
}
