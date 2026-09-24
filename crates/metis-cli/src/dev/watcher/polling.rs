//! Portable polling notifications for hosts without a native watcher.
//!
//! A worker thread fingerprints the watched tree's paths, sizes and
//! modification times at a fixed interval and reports a change when the
//! fingerprint moves. Reading metadata, not contents, keeps each poll cheap;
//! the dev loop's content snapshot remains the authority on whether a
//! rebuild is due, so a coarse filesystem clock cannot cause a missed or
//! spurious rebuild — only a delayed or redundant check.

use super::WatchEvent;
use crate::{Result, tree};
use std::{
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::Path,
    sync::mpsc::{
        Receiver, RecvTimeoutError, Sender, SyncSender, TryRecvError, TrySendError, channel,
        sync_channel,
    },
    thread::{self, JoinHandle},
    time::{Duration, UNIX_EPOCH},
};

/// Interval between metadata polls; short enough to feel immediate after a
/// save, long enough that an idle poll of a bounded tree costs little.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Polling watcher over one directory tree.
#[derive(Debug)]
pub(crate) struct Watcher {
    receiver: Receiver<WatchEvent>,
    // Dropping the sender disconnects the worker's stop receiver.
    stop: Option<Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl Watcher {
    pub(crate) fn new(directory: &Path) -> Result<Self> {
        Self::with_interval(directory, POLL_INTERVAL)
    }

    fn with_interval(directory: &Path, interval: Duration) -> Result<Self> {
        let root = directory.to_path_buf();
        // The initial fingerprint is taken before returning so a change made
        // immediately after construction is observed, and an unreadable tree
        // fails here rather than on the worker.
        let baseline = fingerprint(&root)?;
        let (sender, receiver) = sync_channel(1);
        let (stop, stopped) = channel();
        let worker = thread::Builder::new()
            .name("metis-dev-watch".to_owned())
            .spawn(move || watch_loop(&root, baseline, interval, &stopped, &sender))?;
        Ok(Self {
            receiver,
            stop: Some(stop),
            worker: Some(worker),
        })
    }

    pub(crate) fn changed(&self) -> Result<bool> {
        match self.receiver.try_recv() {
            Ok(WatchEvent::Changed) => Ok(true),
            Ok(WatchEvent::Failed(error)) => Err(error.into()),
            Err(TryRecvError::Empty) => Ok(false),
            Err(TryRecvError::Disconnected) => Err("dev watcher stopped unexpectedly".into()),
        }
    }

    pub(crate) fn wait(&self) -> Result<()> {
        match self.receiver.recv() {
            Ok(WatchEvent::Changed) => Ok(()),
            Ok(WatchEvent::Failed(error)) => Err(error.into()),
            Err(_) => Err("dev watcher stopped unexpectedly".into()),
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        drop(self.stop.take());
        if self
            .worker
            .take()
            .is_some_and(|worker| worker.join().is_err())
        {
            std::process::abort();
        }
    }
}

fn watch_loop(
    root: &Path,
    mut baseline: u64,
    interval: Duration,
    stopped: &Receiver<()>,
    sender: &SyncSender<WatchEvent>,
) {
    loop {
        match stopped.recv_timeout(interval) {
            Err(RecvTimeoutError::Timeout) => {}
            Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
        }
        let current = match fingerprint(root) {
            Ok(current) => current,
            Err(error) => {
                let _ = sender.try_send(WatchEvent::Failed(error.to_string()));
                return;
            }
        };
        if current == baseline {
            continue;
        }
        baseline = current;
        // A full channel already holds an unread change; coalesce into it.
        match sender.try_send(WatchEvent::Changed) {
            Ok(()) | Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => return,
        }
    }
}

/// Hashes the relative path, size and modification time of every file the
/// content snapshot would read, under the same ignore rules and bounds.
fn fingerprint(root: &Path) -> Result<u64> {
    let mut hasher = DefaultHasher::new();
    for path in tree::regular_files(root, super::super::ignored_directory)? {
        let metadata = fs::metadata(&path)?;
        path.strip_prefix(root)
            .map_err(|_| "dev watch path escaped its root")?
            .hash(&mut hasher);
        metadata.len().hash(&mut hasher);
        metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .hash(&mut hasher);
    }
    Ok(hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::{Watcher, fingerprint};
    use std::{fs, path::PathBuf, time::Duration};

    fn scratch(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("metis-dev-watch-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).expect("scratch tree");
        fs::write(root.join("src").join("main.rs"), b"fn main() {}").expect("source");
        root
    }

    #[test]
    fn reports_source_edits_and_ignores_build_output() {
        let root = scratch("edit");
        let watcher =
            Watcher::with_interval(&root, Duration::from_millis(10)).expect("polling watcher");
        // Build output is outside the fingerprint, so it cannot trigger a poll.
        let before = fingerprint(&root).expect("fingerprint");
        fs::create_dir_all(root.join("target")).expect("build directory");
        fs::write(root.join("target").join("artifact"), b"ignored").expect("artifact");
        assert_eq!(fingerprint(&root).expect("fingerprint"), before);

        fs::write(root.join("src").join("lib.rs"), b"pub fn edited() {}").expect("edit");
        watcher.wait().expect("change notification");
        drop(watcher);
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn drop_stops_the_worker_promptly() {
        let root = scratch("drop");
        let watcher = Watcher::with_interval(&root, Duration::from_hours(1)).expect("watcher");
        let started = std::time::Instant::now();
        drop(watcher);
        assert!(started.elapsed() < Duration::from_secs(5));
        fs::remove_dir_all(&root).expect("cleanup");
    }
}
