//! Directory change notifications used by the developer lifecycle.
//!
//! A notification only says that something may have changed: the dev loop
//! confirms every notification against its content snapshot before it
//! rebuilds. Windows uses kernel change notifications; other hosts poll a
//! bounded metadata fingerprint of the same tree the snapshot hashes.

#[cfg(not(windows))]
mod polling;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub(crate) use polling::Watcher;
#[cfg(windows)]
pub(crate) use windows::Watcher;

/// One coalesced notification from a watcher worker.
#[derive(Debug)]
enum WatchEvent {
    Changed,
    Failed(String),
}
