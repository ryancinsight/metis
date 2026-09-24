//! Build, run and reload an application without selecting stale artifacts.

use crate::{
    Result,
    manifest::{self, Application, DEFAULT_MANIFEST, Manifest},
    process::{self, Containment},
    tree,
};
use moirai_transport::process::ProcessOutcome;
use std::{
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

mod browser;
#[cfg(windows)]
#[expect(
    unsafe_code,
    reason = "ReadDirectoryChangesW-equivalent notification handles are isolated in the watcher"
)]
mod watcher;
#[cfg(not(windows))]
mod watcher;

const PROCESS_OBSERVATION: Duration = Duration::from_millis(100);
const SINGLE_RUN_DEADLINE: Duration = Duration::from_mins(5);
const WATCH_BYTES_LIMIT: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy)]
pub(super) enum DevMode {
    Once,
    Watch,
}

enum RunOutcome {
    Exited(ProcessOutcome),
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Snapshot([u8; 32]);

/// Runs the manifest entry once or reloads it after source/resource changes.
pub(crate) fn run(args: &[OsString]) -> Result<()> {
    let (manifest_path, mode) = arguments(args.get(1..).unwrap_or_default())?;
    if matches!(mode, DevMode::Watch) && !cfg!(windows) {
        return Err(
            "dev --watch currently requires the Windows filesystem notification host".into(),
        );
    }
    let root = match Manifest::read(&manifest_path)? {
        (Manifest::Web(application), root) => return browser::run(&application, &root, mode),
        (Manifest::Native(_), root) => root,
    };
    let cargo = process::tool("cargo")?;
    let mut baseline = snapshot(&root)?;
    let watcher = matches!(mode, DevMode::Watch)
        .then(|| watcher::Watcher::new(&root))
        .transpose()?;
    if watcher.is_some() {
        eprintln!("metis dev: watching for source or resource changes");
    }
    loop {
        let (application, root) = match Application::read(&manifest_path) {
            Ok(value) => value,
            Err(error) if matches!(mode, DevMode::Watch) => {
                eprintln!("metis dev: build not started: {error}");
                wait_for_source_change(
                    watcher.as_ref().ok_or("watcher unavailable")?,
                    &root,
                    &mut baseline,
                )?;
                continue;
            }
            Err(error) => return Err(error),
        };
        let cargo_manifest = match manifest::source(&root, &application.cargo_manifest) {
            Ok(path) => path,
            Err(error) if matches!(mode, DevMode::Watch) => {
                eprintln!("metis dev: build not started: {error}");
                wait_for_source_change(
                    watcher.as_ref().ok_or("watcher unavailable")?,
                    &root,
                    &mut baseline,
                )?;
                continue;
            }
            Err(error) => return Err(error),
        };
        let outcome = launch(
            &cargo,
            &cargo_manifest,
            &application,
            watcher.as_ref(),
            &root,
            &mut baseline,
        )?;
        match outcome {
            RunOutcome::Exited(ProcessOutcome::Succeeded) if matches!(mode, DevMode::Once) => {
                return Ok(());
            }
            RunOutcome::Exited(ProcessOutcome::Failed) if matches!(mode, DevMode::Once) => {
                return Err("dev application exited with a failure status".into());
            }
            RunOutcome::Exited(ProcessOutcome::Succeeded | ProcessOutcome::Failed) => {
                eprintln!("metis dev: application exited; waiting for source or resource change");
                wait_for_source_change(
                    watcher.as_ref().ok_or("watcher unavailable")?,
                    &root,
                    &mut baseline,
                )?;
            }
            RunOutcome::Changed => {
                eprintln!("metis dev: source or resource change detected; reloading");
            }
        }
    }
}

/// `dev [MANIFEST] [--once|--watch]`: `./metis.json` and `--once` when omitted.
fn arguments(args: &[OsString]) -> Result<(PathBuf, DevMode)> {
    let mut manifest_path = None;
    let mut mode = None;
    for argument in args {
        let flag = match argument.to_str() {
            Some("--once") => Some(DevMode::Once),
            Some("--watch") => Some(DevMode::Watch),
            _ => None,
        };
        if let Some(flag) = flag {
            if mode.replace(flag).is_some() {
                return Err("dev accepts exactly one of --once or --watch".into());
            }
        } else if manifest_path.replace(PathBuf::from(argument)).is_some() {
            return Err("dev accepts one MANIFEST and one of --once or --watch".into());
        }
    }
    Ok((
        manifest_path.unwrap_or_else(|| PathBuf::from(DEFAULT_MANIFEST)),
        mode.unwrap_or(DevMode::Once),
    ))
}

fn launch(
    cargo: &Path,
    manifest: &Path,
    application: &Application,
    watcher: Option<&watcher::Watcher>,
    root: &Path,
    baseline: &mut Snapshot,
) -> Result<RunOutcome> {
    let binary = application
        .binaries
        .iter()
        .find(|binary| binary.bin == application.entry)
        .ok_or("entry binary is not declared")?;
    let mut arguments = vec![
        OsString::from("run"),
        OsString::from("--locked"),
        OsString::from("--manifest-path"),
        manifest.as_os_str().to_owned(),
        OsString::from("--package"),
        OsString::from(&binary.package),
        OsString::from("--bin"),
        OsString::from(&binary.bin),
        OsString::from("--"),
    ];
    arguments.extend(application.arguments.iter().map(OsString::from));
    let containment = if cfg!(windows) {
        Containment::Required
    } else {
        Containment::Uncontained
    };
    let mut child = process::spawn(cargo, &arguments, containment)?;
    if watcher.is_none() {
        let status = child
            .wait_timeout(SINGLE_RUN_DEADLINE)?
            .ok_or("dev application exceeded its 300-second deadline")?;
        if status.outcome == ProcessOutcome::Failed {
            eprintln!("metis dev: cargo run exited with status {:?}", status.code);
        }
        return Ok(RunOutcome::Exited(status.outcome));
    }
    loop {
        if let Some(status) = child.wait_timeout(PROCESS_OBSERVATION)? {
            if status.outcome == ProcessOutcome::Failed {
                eprintln!("metis dev: cargo run exited with status {:?}", status.code);
            }
            return Ok(RunOutcome::Exited(status.outcome));
        }
        let Some(watcher) = watcher else {
            return Err("bounded dev observation ended without a completion status".into());
        };
        if watcher.changed()? {
            let current = snapshot(root)?;
            if current != *baseline {
                *baseline = current;
                child.terminate_timeout(Duration::from_secs(1))?;
                return Ok(RunOutcome::Changed);
            }
        }
    }
}

fn wait_for_source_change(
    watcher: &watcher::Watcher,
    root: &Path,
    baseline: &mut Snapshot,
) -> Result<()> {
    loop {
        watcher.wait()?;
        let current = snapshot(root)?;
        if current != *baseline {
            *baseline = current;
            eprintln!("metis dev: source or resource change detected; reloading");
            return Ok(());
        }
    }
}

pub(super) fn snapshot(root: &Path) -> Result<Snapshot> {
    let files = tree::regular_files(root, ignored_directory)?;
    let mut total = 0_u64;
    let mut hash = moirai_crypto::Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    for path in files {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "dev watch path escaped its root")?;
        hash.update(relative.to_string_lossy().as_bytes());
        hash.update(&[0]);
        let mut file = fs::File::open(&path)?;
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            total = total
                .checked_add(u64::try_from(count).map_err(|_| "dev watch byte count overflow")?)
                .ok_or("dev watch byte count overflow")?;
            if total > WATCH_BYTES_LIMIT {
                return Err("dev watch tree exceeds the 512 MiB byte budget".into());
            }
            hash.update(&buffer[..count]);
        }
        hash.update(&[0xff]);
    }
    Ok(Snapshot(hash.finalize()))
}

fn ignored_directory(name: &std::ffi::OsStr) -> bool {
    matches!(
        name.to_str(),
        Some(".git" | "target" | "output" | "node_modules" | "dist")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_default_the_manifest_and_mode() {
        let parse = |args: &[&str]| {
            arguments(&args.iter().map(OsString::from).collect::<Vec<_>>())
                .map(|(path, mode)| (path, matches!(mode, DevMode::Watch)))
        };
        assert_eq!(
            parse(&[]).expect("defaults"),
            (PathBuf::from(DEFAULT_MANIFEST), false)
        );
        assert_eq!(
            parse(&["--watch"]).expect("default manifest"),
            (PathBuf::from(DEFAULT_MANIFEST), true)
        );
        assert_eq!(
            parse(&["app/metis.json", "--once"]).expect("manifest and mode"),
            (PathBuf::from("app/metis.json"), false)
        );
        assert_eq!(
            parse(&["--watch", "app/metis.json"]).expect("mode first"),
            (PathBuf::from("app/metis.json"), true)
        );
        for rejected in [&["--once", "--watch"][..], &["a.json", "b.json"]] {
            assert!(parse(rejected).is_err(), "accepted {rejected:?}");
        }
    }

    #[test]
    fn snapshot_changes_for_source_and_resource_bytes() {
        let root = std::env::temp_dir().join(format!(
            "metis-dev-snapshot-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("test clock")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).expect("test root");
        fs::create_dir(root.join("target")).expect("ignored target");
        fs::write(root.join("src/main.rs"), b"one").expect("source");
        fs::write(root.join("target/generated"), b"one").expect("generated");
        let first = snapshot(&root).expect("first snapshot");
        fs::write(root.join("src/main.rs"), b"two").expect("source update");
        assert_ne!(first, snapshot(&root).expect("second snapshot"));
        fs::write(root.join("target/generated"), b"two").expect("generated update");
        assert_eq!(
            snapshot(&root).expect("target ignored"),
            snapshot(&root).expect("stable")
        );
        fs::remove_dir_all(root).expect("test cleanup");
    }

    #[test]
    fn snapshot_rejects_a_linked_input() {
        let root = std::env::temp_dir().join(format!(
            "metis-dev-linked-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("test clock")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root");
        #[cfg(windows)]
        if let Err(error) =
            std::os::windows::fs::symlink_file(root.join("missing"), root.join("link"))
        {
            // Developer mode must still be testable on hosts where Windows
            // Developer Mode or SeCreateSymbolicLinkPrivilege is disabled.
            assert_eq!(error.raw_os_error(), Some(1314));
            fs::remove_dir_all(root).expect("test cleanup");
            return;
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("missing"), root.join("link")).expect("test link");
        assert!(snapshot(&root).is_err());
        fs::remove_dir_all(root).expect("test cleanup");
    }
}
