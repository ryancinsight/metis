//! Rust-owned host thread for the Windows native surface.

use super::events::{Event, event};
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_platform::Framebuffer;
pub(super) use metis_platform::native::MAX_WAIT_MILLISECONDS;
use metis_platform::native::{NativeSurface, WindowConfig, WindowVisibility};
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, SendError, SyncSender, TrySendError, sync_channel,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Longest the host thread may take to create its window.
const READY_DEADLINE: Duration = Duration::from_secs(10);
/// Longest the host thread may take beyond a wait's own timeout to answer.
const REPLY_MARGIN: Duration = Duration::from_secs(5);
/// Longest a drop waits for the host thread to close its window and exit.
const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(5);

enum Command {
    Present(Framebuffer, SyncSender<Result<()>>),
    Wait(Duration, SyncSender<Result<Vec<Event>>>),
    Close(SyncSender<Result<()>>),
    Reopen(SyncSender<Result<()>>),
    Shutdown,
}
fn io_error(error: std::io::Error) -> MetisError {
    MetisError::from(error)
}

/// Sends `value` to a requester that may already have given up at its deadline.
fn reply<T>(sender: &SyncSender<T>, value: T) {
    match sender.send(value) {
        Ok(()) => {}
        // The requester drops its receiver when its deadline passes, so the
        // late result has no reader left.
        Err(SendError(_abandoned)) => {}
    }
}
#[expect(
    clippy::needless_pass_by_value,
    reason = "the host thread owns these values"
)]
fn host(
    config: WindowConfig,
    receiver: Receiver<Command>,
    ready: SyncSender<Result<()>>,
    _exited: SyncSender<()>,
) {
    let mut surface = match NativeSurface::new(&config) {
        Ok(surface) => {
            reply(&ready, Ok(()));
            surface
        }
        Err(error) => {
            reply(&ready, Err(io_error(error)));
            return;
        }
    };
    while let Ok(command) = receiver.recv() {
        match command {
            Command::Present(framebuffer, sender) => {
                reply(&sender, surface.present(&framebuffer).map_err(io_error));
            }
            Command::Wait(timeout, sender) => {
                let result = surface
                    .wait_events(timeout)
                    .map(|events| events.into_iter().map(event).collect())
                    .map_err(io_error);
                reply(&sender, result);
            }
            Command::Close(sender) => reply(&sender, surface.close().map_err(io_error)),
            Command::Reopen(sender) => reply(&sender, surface.reopen().map_err(io_error)),
            Command::Shutdown => break,
        }
    }
    match surface.close() {
        Ok(()) => {}
        // No requester remains to receive a failure, and the window is
        // destroyed with this thread either way.
        Err(_unreportable) => {}
    }
}
pub(super) struct Client {
    sender: Option<SyncSender<Command>>,
    join: Option<JoinHandle<()>>,
    /// Disconnects when the host thread exits; nothing is ever sent on it.
    exited: Receiver<()>,
    pub(super) generation: u64,
    closed: bool,
}

impl Client {
    pub(super) fn new(title: &str, width: u32, height: u32, visibility: &str) -> Result<Self> {
        let visibility = match visibility {
            "hidden" => WindowVisibility::Hidden,
            "visible" => WindowVisibility::Visible,
            _ => {
                return Err(MetisError::ui(
                    ErrorCode::MalformedPayload,
                    "visibility must be 'visible' or 'hidden'",
                ));
            }
        };
        let config =
            WindowConfig::with_visibility(title, width, height, visibility).map_err(io_error)?;
        let (sender, receiver) = sync_channel(8);
        let (ready_sender, ready_receiver) = sync_channel(1);
        let (exited_sender, exited) = sync_channel(1);
        let join = thread::Builder::new()
            .name("metis-python-native-host".to_owned())
            .spawn(move || host(config, receiver, ready_sender, exited_sender))
            .map_err(io_error)?;
        match ready_receiver.recv_timeout(READY_DEADLINE) {
            Ok(Ok(())) => Ok(Self {
                sender: Some(sender),
                join: Some(join),
                exited,
                generation: 0,
                closed: false,
            }),
            Ok(Err(error)) => {
                join_exited(join)?;
                Err(error)
            }
            Err(RecvTimeoutError::Disconnected) => {
                join_exited(join)?;
                Err(MetisError::transport(
                    ErrorCode::ConnectionClosed,
                    "native host stopped before readiness",
                ))
            }
            // Dropping `sender` ends the host loop once window creation
            // returns; the thread is detached because that may never happen.
            Err(RecvTimeoutError::Timeout) => Err(MetisError::transport(
                ErrorCode::Timeout,
                "native host did not create its window before the deadline",
            )),
        }
    }

    fn check_generation(&self, generation: u64) -> Result<()> {
        if self.generation != generation {
            return Err(MetisError::ui(
                ErrorCode::RenderFailure,
                "NativeApplication generation is stale",
            ));
        }
        if self.closed {
            return Err(MetisError::ui(
                ErrorCode::RenderFailure,
                "NativeApplication is closed",
            ));
        }
        Ok(())
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "the response receiver is consumed by this request"
    )]
    fn request<T>(
        &self,
        command: Command,
        receiver: Receiver<Result<T>>,
        deadline: Duration,
    ) -> Result<T> {
        self.sender
            .as_ref()
            .ok_or_else(|| {
                MetisError::transport(ErrorCode::ConnectionClosed, "native host is stopped")
            })?
            .try_send(command)
            .map_err(|error| match error {
                TrySendError::Full(_) => {
                    MetisError::transport(ErrorCode::QueueFull, "native host command queue is full")
                }
                TrySendError::Disconnected(_) => {
                    MetisError::transport(ErrorCode::ConnectionClosed, "native host stopped")
                }
            })?;
        receiver
            .recv_timeout(deadline)
            .map_err(|error| match error {
                RecvTimeoutError::Timeout => MetisError::transport(
                    ErrorCode::Timeout,
                    "native host did not answer before the deadline",
                ),
                RecvTimeoutError::Disconnected => {
                    MetisError::transport(ErrorCode::ConnectionClosed, "native host stopped")
                }
            })?
    }

    pub(super) fn present(&self, generation: u64, framebuffer: Framebuffer) -> Result<()> {
        self.check_generation(generation)?;
        let (sender, receiver) = sync_channel(1);
        self.request(
            Command::Present(framebuffer, sender),
            receiver,
            REPLY_MARGIN,
        )
    }

    pub(super) fn wait(&self, generation: u64, timeout: Duration) -> Result<Vec<Event>> {
        self.check_generation(generation)?;
        let (sender, receiver) = sync_channel(1);
        self.request(
            Command::Wait(timeout, sender),
            receiver,
            timeout + REPLY_MARGIN,
        )
    }

    pub(super) fn close(&mut self, generation: u64) -> Result<()> {
        self.check_generation(generation)?;
        let (sender, receiver) = sync_channel(1);
        self.request(Command::Close(sender), receiver, REPLY_MARGIN)?;
        self.closed = true;
        Ok(())
    }

    pub(super) fn reopen(&mut self) -> Result<u64> {
        if !self.closed {
            return Err(MetisError::ui(
                ErrorCode::RenderFailure,
                "NativeApplication must be closed before reopen",
            ));
        }
        let (sender, receiver) = sync_channel(1);
        // Creating the window takes as long as at first start.
        if let Err(error) = self.request(Command::Reopen(sender), receiver, READY_DEADLINE) {
            if error.code == ErrorCode::Timeout {
                // The command was queued but unanswered, so the host may still
                // create the window later, which this client could no longer
                // tell apart from a closed one. Dropping the sender retires the
                // client: the host exits after its queued command, and a later
                // reopen reports a stopped host instead of reaching a window
                // nobody tracks. A full queue never queued the command and
                // leaves the client usable.
                self.sender = None;
            }
            return Err(error);
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| MetisError::ui(ErrorCode::RenderFailure, "generation exhausted"))?;
        self.closed = false;
        Ok(self.generation)
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Some(sender) = self.sender.take() {
            match sender.try_send(Command::Shutdown) {
                // A full queue means the host is busy, and a closed one has
                // already stopped. Dropping the sender ends the host loop
                // after its queued commands either way.
                Ok(()) | Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {}
            }
        }
        // A host stuck past its deadline is detached rather than joined, so
        // the interpreter thread running this drop never blocks without bound.
        if matches!(
            self.exited.recv_timeout(SHUTDOWN_DEADLINE),
            Err(RecvTimeoutError::Disconnected)
        ) && let Some(join) = self.join.take()
        {
            match join.join() {
                Ok(()) => {}
                // A panicked host thread has no requester left to tell.
                Err(_panic) => {}
            }
        }
    }
}

/// Joins a host thread that has stopped, reporting a panic.
fn join_exited(join: JoinHandle<()>) -> Result<()> {
    join.join().map_err(|_| {
        MetisError::transport(ErrorCode::ConnectionClosed, "native host thread panicked")
    })
}
pub(super) fn frame_from_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Framebuffer> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|value| value.checked_mul(4))
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| MetisError::ui(ErrorCode::SurfaceAllocationError, "frame size overflow"))?;
    if rgba.len() != pixels {
        return Err(MetisError::ui(
            ErrorCode::RenderFailure,
            "RGBA frame length does not match the native window",
        ));
    }
    let mut framebuffer = Framebuffer::new(width, height)?;
    for (index, channels) in rgba.chunks_exact(4).enumerate() {
        let index = u64::try_from(index).map_err(|_| {
            MetisError::ui(ErrorCode::SurfaceAllocationError, "frame index overflow")
        })?;
        let x = u32::try_from(index % u64::from(width)).map_err(|_| {
            MetisError::ui(
                ErrorCode::SurfaceAllocationError,
                "frame coordinate overflow",
            )
        })?;
        let y = u32::try_from(index / u64::from(width)).map_err(|_| {
            MetisError::ui(
                ErrorCode::SurfaceAllocationError,
                "frame coordinate overflow",
            )
        })?;
        let color = metis_platform::Color::rgba(channels[0], channels[1], channels[2], channels[3]);
        let x = i32::try_from(x).map_err(|_| {
            MetisError::ui(
                ErrorCode::SurfaceAllocationError,
                "frame coordinate exceeds i32",
            )
        })?;
        let y = i32::try_from(y).map_err(|_| {
            MetisError::ui(
                ErrorCode::SurfaceAllocationError,
                "frame coordinate exceeds i32",
            )
        })?;
        framebuffer.set_pixel(x, y, color);
    }
    Ok(framebuffer)
}
