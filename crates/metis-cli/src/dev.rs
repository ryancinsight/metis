//! Build, run and reload an application without selecting stale artifacts.

use crate::{
    Result,
    manifest::{self, Application},
    process::{self, Containment},
};
use moirai_transport::process::ProcessOutcome;
use std::{
    collections::VecDeque,
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

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
const WATCH_FILE_LIMIT: usize = 4096;
const WATCH_BYTES_LIMIT: u64 = 512 * 1024 * 1024;
const WATCH_DEPTH_LIMIT: usize = 64;

#[derive(Clone, Copy)]
enum DevMode {
    Once,
    Watch,
}

enum RunOutcome {
    Exited(ProcessOutcome),
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Snapshot([u8; 32]);

/// Runs the manifest entry once or reloads it after source/resource changes.
pub(crate) fn run(args: &[OsString]) -> Result<()> {
    let manifest_path = args
        .get(1)
        .ok_or("dev requires a manifest path")?
        .as_os_str();
    let mut mode = None;
    for argument in args.iter().skip(2) {
        match argument.to_str() {
            Some("--once") => {
                if mode.replace(DevMode::Once).is_some() {
                    return Err("dev accepts exactly one of --once or --watch".into());
                }
            }
            Some("--watch") => {
                if mode.replace(DevMode::Watch).is_some() {
                    return Err("dev accepts exactly one of --once or --watch".into());
                }
            }
            _ => return Err("dev accepts MANIFEST followed by --once or --watch".into()),
        }
    }
    let mode = mode.unwrap_or(DevMode::Once);
    if matches!(mode, DevMode::Watch) && !cfg!(windows) {
        return Err(
            "dev --watch currently requires the Windows filesystem notification host".into(),
        );
    }
    let manifest_path = PathBuf::from(manifest_path);
    let (_, root) = Application::read(&manifest_path)?;
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

fn snapshot(root: &Path) -> Result<Snapshot> {
    let mut files = Vec::new();
    let mut pending = VecDeque::from([(root.to_path_buf(), 0_usize)]);
    while let Some((directory, depth)) = pending.pop_front() {
        if depth > WATCH_DEPTH_LIMIT {
            return Err("dev watch tree exceeds the 64-level depth budget".into());
        }
        let mut entries = fs::read_dir(&directory)?.collect::<std::result::Result<Vec<_>, _>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if manifest::linked(&metadata) {
                return Err("dev watch input contains a linked path".into());
            }
            if metadata.is_dir() {
                if !ignored_directory(entry.file_name().as_os_str()) {
                    pending.push_back((path, depth + 1));
                }
            } else if metadata.is_file() {
                files.push(path);
                if files.len() > WATCH_FILE_LIMIT {
                    return Err("dev watch tree exceeds the 4096-file budget".into());
                }
            } else {
                return Err("dev watch input contains a non-regular file".into());
            }
        }
    }
    files.sort();
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
        Some(".git" | "target" | "output" | "node_modules")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
