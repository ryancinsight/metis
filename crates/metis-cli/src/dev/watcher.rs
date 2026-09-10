//! Windows directory notifications used by the developer lifecycle.

use crate::Result;
use std::path::Path;

#[cfg(windows)]
use std::{
    sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel},
    thread::{self, JoinHandle},
};

#[cfg(windows)]
use std::{
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr::null,
};

#[cfg(windows)]
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

#[cfg(windows)]
type RawHandle = native::Handle;

#[derive(Debug)]
enum WatchEvent {
    Changed,
    Failed(String),
}

#[cfg(windows)]
#[derive(Debug)]
struct ChangeHandle(RawHandle);

#[cfg(windows)]
impl Drop for ChangeHandle {
    fn drop(&mut self) {
        if !native::invalid_handle(self.0) {
            // SAFETY: the handle was returned by FindFirstChangeNotificationW
            // and remains owned until this destructor runs after the worker has
            // joined.
            let _ = unsafe { native::FindCloseChangeNotification(self.0) };
        }
    }
}

#[cfg(windows)]
#[derive(Debug)]
pub(crate) struct Watcher {
    receiver: Receiver<WatchEvent>,
    stop: OwnedHandle,
    _change: ChangeHandle,
    worker: Option<JoinHandle<()>>,
}

#[cfg(windows)]
impl Watcher {
    pub(crate) fn new(directory: &Path) -> Result<Self> {
        let text = directory.to_string_lossy();
        if text.contains('\0') {
            return Err("dev watch directory contains an embedded NUL".into());
        }
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
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
        // SAFETY: null security attributes request the current process default;
        // a manual-reset, initially non-signaled event is owned below.
        let stop_raw = unsafe { native::CreateEventW(null(), 1, 0, null()) };
        if stop_raw.is_null() {
            // SAFETY: `change` is the valid notification handle just created and
            // no worker owns it yet.
            let _ = unsafe { native::FindCloseChangeNotification(change) };
            return Err(std::io::Error::last_os_error().to_string().into());
        }
        // SAFETY: CreateEventW returned a new owned kernel handle.
        let stop = unsafe { OwnedHandle::from_raw_handle(stop_raw) };
        let stop_handle = stop.as_raw_handle() as usize;
        let change_handle = change as usize;
        let (sender, receiver) = sync_channel(1);
        let worker = thread::Builder::new()
            .name("metis-dev-watch".to_owned())
            .spawn(move || watch_loop(change_handle, stop_handle, &sender))?;
        Ok(Self {
            receiver,
            stop,
            _change: ChangeHandle(change),
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

#[cfg(windows)]
impl Drop for Watcher {
    fn drop(&mut self) {
        // SAFETY: the stop event is a live handle owned by this value; setting
        // it wakes the worker's bounded handle wait before the change handle is
        // closed by `ChangeHandle`.
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

#[cfg(windows)]
fn watch_loop(change_value: usize, stop_value: usize, sender: &SyncSender<WatchEvent>) {
    // Windows HANDLEs are pointer-sized opaque values. Passing their integer
    // representation through the thread boundary preserves the exact bits;
    // they are converted back only on the worker that waits on them.
    let change = change_value as RawHandle;
    let stop = stop_value as RawHandle;
    let handles = [change, stop];
    loop {
        // SAFETY: both handles remain owned by Watcher until this worker joins;
        // the array is live for the duration of the call and requests a bounded
        // two-handle wait with no alertable callbacks.
        let result =
            unsafe { native::WaitForMultipleObjects(2, handles.as_ptr(), 0, native::INFINITE) };
        match result {
            native::WAIT_OBJECT_0 => {
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
            value if value == native::WAIT_OBJECT_0 + 1 => return,
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

#[cfg(not(windows))]
#[derive(Debug)]
pub(crate) struct Watcher;

#[cfg(not(windows))]
impl Watcher {
    pub(crate) fn new(_directory: &Path) -> Result<Self> {
        Err("dev --watch currently requires the Windows filesystem notification host".into())
    }

    pub(crate) fn changed(&self) -> Result<bool> {
        Err("dev --watch currently requires the Windows filesystem notification host".into())
    }

    pub(crate) fn wait(&self) -> Result<()> {
        Err("dev --watch currently requires the Windows filesystem notification host".into())
    }
}
