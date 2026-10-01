//! Windows directory notifications.
//!
//! `FindFirstChangeNotificationW` signals a subtree change; a worker thread
//! waits on it together with a stop event and forwards one coalesced event.

use super::WatchEvent;
use crate::Result;
use std::path::Path;
use std::{
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    ptr::null,
    sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel},
    thread::{self, JoinHandle},
};

mod native {
    use core::ffi::c_void;

    pub type Handle = *mut c_void;

    pub const FILE_NOTIFY_CHANGE_FILE_NAME: u32 = 0x0000_0001;
    pub const FILE_NOTIFY_CHANGE_DIR_NAME: u32 = 0x0000_0002;
    pub const FILE_NOTIFY_CHANGE_SIZE: u32 = 0x0000_0008;
    pub const FILE_NOTIFY_CHANGE_LAST_WRITE: u32 = 0x0000_0010;
    pub const WAIT_OBJECT_0: u32 = 0;
    pub const WAIT_FAILED: u32 = u32::MAX;
    pub const INFINITE: u32 = u32::MAX;

    pub fn invalid_handle(handle: Handle) -> bool {
        handle == std::ptr::with_exposed_provenance_mut(usize::MAX)
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn FindFirstChangeNotificationW(
            path: *const u16,
            watch_subtree: i32,
            notify_filter: u32,
        ) -> Handle;
        pub fn FindNextChangeNotification(handle: Handle) -> i32;
        pub fn FindCloseChangeNotification(handle: Handle) -> i32;
        pub fn CreateEventW(
            attributes: *const c_void,
            manual_reset: i32,
            initial_state: i32,
            name: *const u16,
        ) -> Handle;
        pub fn SetEvent(handle: Handle) -> i32;
        pub fn WaitForMultipleObjects(
            count: u32,
            handles: *const Handle,
            wait_all: i32,
            timeout: u32,
        ) -> u32;
    }
}

type RawHandle = native::Handle;

#[derive(Debug)]
struct ChangeHandle(RawHandle);

impl Drop for ChangeHandle {
    fn drop(&mut self) {
        // SAFETY: the handle is the valid value FindFirstChangeNotificationW
        // returned, and this is its only owner. No thread waits on it now:
        // either no worker was started, or `Watcher::drop` joined it first.
        if unsafe { native::FindCloseChangeNotification(self.0) } == 0 {
            std::process::abort();
        }
    }
}

#[derive(Debug)]
pub(crate) struct Watcher {
    receiver: Receiver<WatchEvent>,
    stop: OwnedHandle,
    _change: ChangeHandle,
    worker: Option<JoinHandle<()>>,
}

impl Watcher {
    pub(crate) fn new(directory: &Path) -> Result<Self> {
        // The exact UTF-16 units of the path, including any unpaired surrogate:
        // a lossy conversion would watch a different directory.
        let mut wide: Vec<u16> = directory.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return Err("dev watch directory contains an embedded NUL".into());
        }
        wide.push(0);
        let filter = native::FILE_NOTIFY_CHANGE_FILE_NAME
            | native::FILE_NOTIFY_CHANGE_DIR_NAME
            | native::FILE_NOTIFY_CHANGE_LAST_WRITE
            | native::FILE_NOTIFY_CHANGE_SIZE;
        // SAFETY: `wide` is a live, NUL-terminated UTF-16 path for the duration
        // of the call; the filter is a documented FILE_NOTIFY_CHANGE mask.
        let change = unsafe { native::FindFirstChangeNotificationW(wide.as_ptr(), 1, filter) };
        if native::invalid_handle(change) {
            return Err(std::io::Error::last_os_error().to_string().into());
        }
        // Owned from here on, so every later failure closes it.
        let change = ChangeHandle(change);
        // SAFETY: null security attributes request the current process default;
        // a manual-reset, initially non-signaled event is owned below.
        let stop_raw = unsafe { native::CreateEventW(null(), 1, 0, null()) };
        if stop_raw.is_null() {
            return Err(std::io::Error::last_os_error().to_string().into());
        }
        // SAFETY: CreateEventW returned a new owned kernel handle.
        let stop = unsafe { OwnedHandle::from_raw_handle(stop_raw) };
        let stop_handle = stop.as_raw_handle() as usize;
        let change_handle = change.0 as usize;
        let (sender, receiver) = sync_channel(1);
        let worker = thread::Builder::new()
            .name("metis-dev-watch".to_owned())
            .spawn(move || watch_loop(change_handle, stop_handle, &sender))?;
        Ok(Self {
            receiver,
            stop,
            _change: change,
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
        // SAFETY: the stop event is a live handle owned by this value; setting
        // it wakes the worker's wait, which has no timeout, before the change
        // handle is closed by `ChangeHandle`.
        let signaled = unsafe { native::SetEvent(self.stop.as_raw_handle()) };
        if signaled == 0 {
            std::process::abort();
        }
        if self
            .worker
            .take()
            .is_some_and(|worker| worker.join().is_err())
        {
            std::process::abort();
        }
    }
}

fn watch_loop(change_value: usize, stop_value: usize, sender: &SyncSender<WatchEvent>) {
    // Windows HANDLEs are pointer-sized opaque values. Passing their integer
    // representation through the thread boundary preserves the exact bits;
    // they are converted back only on the worker that waits on them.
    let change = change_value as RawHandle;
    let stop = stop_value as RawHandle;
    // The stop event comes first: a wait that finds several handles signaled
    // reports the lowest index, so a steady stream of changes cannot starve it.
    let handles = [stop, change];
    loop {
        // SAFETY: both handles remain owned by Watcher until this worker joins;
        // the array is live for the duration of the call. The wait has no
        // timeout but always ends: `Watcher::drop` signals the stop handle,
        // which is entry zero. It is not alertable.
        let result =
            unsafe { native::WaitForMultipleObjects(2, handles.as_ptr(), 0, native::INFINITE) };
        match result {
            native::WAIT_OBJECT_0 => return,
            value if value == native::WAIT_OBJECT_0 + 1 => {
                match sender.try_send(WatchEvent::Changed) {
                    Ok(()) | Err(TrySendError::Full(_)) => {}
                    Err(TrySendError::Disconnected(_)) => return,
                }
                // SAFETY: the change handle remains valid and signaled until it
                // is re-armed; the worker is its sole notifier consumer.
                if unsafe { native::FindNextChangeNotification(change) } == 0 {
                    let _ = sender.try_send(WatchEvent::Failed(
                        std::io::Error::last_os_error().to_string(),
                    ));
                    return;
                }
            }
            native::WAIT_FAILED => {
                let _ = sender.try_send(WatchEvent::Failed(
                    std::io::Error::last_os_error().to_string(),
                ));
                return;
            }
            _ => {
                let _ = sender.try_send(WatchEvent::Failed(
                    "dev watcher returned an unknown wait result".to_owned(),
                ));
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{WatchEvent, Watcher};
    use std::{ffi::OsString, fs, os::windows::ffi::OsStringExt, path::PathBuf, time::Duration};

    /// The Win32 code for a watched directory that does not exist.
    const ERROR_FILE_NOT_FOUND: i32 = 2;

    /// Bound on a wait that only a lost notification can prolong: the kernel
    /// queues the change before the write returns. Half the 60 s termination
    /// budget of the nextest profile.
    const NOTIFICATION_DEADLINE: Duration = Duration::from_secs(30);

    #[test]
    fn a_path_with_an_unpaired_surrogate_is_watched_exactly() {
        let mut name: Vec<u16> = "metis-watch-".encode_utf16().collect();
        name.push(0xD800);
        name.extend(std::process::id().to_string().encode_utf16());
        let directory: PathBuf = std::env::temp_dir().join(OsString::from_wide(&name));
        fs::create_dir(&directory).expect("directory named with an unpaired surrogate");
        let watcher = Watcher::new(&directory).expect("watch the exact directory");
        fs::write(directory.join("changed"), b"x").expect("write inside the directory");
        // A lossy conversion would watch a different, absent directory and
        // never report this write.
        let event = watcher.receiver.recv_timeout(NOTIFICATION_DEADLINE);
        drop(watcher);
        fs::remove_dir_all(&directory).expect("cleanup");
        assert_eq!(event, Ok(WatchEvent::Changed));
    }

    #[test]
    fn a_dropped_watcher_joins_its_worker() {
        let directory = std::env::temp_dir().join(format!("metis-stop-{}", std::process::id()));
        fs::create_dir(&directory).expect("directory");
        let watcher = Watcher::new(&directory).expect("watch");
        drop(watcher);
        fs::remove_dir(&directory).expect("the directory is removable after the drop");
    }

    #[test]
    fn a_missing_directory_is_an_error() {
        let directory = std::env::temp_dir().join(format!("metis-absent-{}", std::process::id()));
        let error = Watcher::new(&directory).expect_err("the directory does not exist");
        assert_eq!(
            error.to_string(),
            std::io::Error::from_raw_os_error(ERROR_FILE_NOT_FOUND).to_string()
        );
    }
}
