//! Rust-owned host thread for the Windows native surface.

use super::events::{Event, event};
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_platform::Framebuffer;
pub(super) use metis_platform::native::MAX_WAIT_MILLISECONDS;
use metis_platform::native::{NativeSurface, WindowConfig, WindowVisibility};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;
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
#[expect(
    clippy::needless_pass_by_value,
    reason = "the host thread owns these values"
)]
fn host(config: WindowConfig, receiver: Receiver<Command>, ready: SyncSender<Result<()>>) {
    let mut surface = match NativeSurface::new(&config) {
        Ok(surface) => {
            let _ = ready.send(Ok(()));
            surface
        }
        Err(error) => {
            let _ = ready.send(Err(io_error(error)));
            return;
        }
    };
    while let Ok(command) = receiver.recv() {
        match command {
            Command::Present(framebuffer, reply) => {
                let _ = reply.send(surface.present(&framebuffer).map_err(io_error));
            }
            Command::Wait(timeout, reply) => {
                let result = surface
                    .wait_events(timeout)
                    .map(|events| events.into_iter().map(event).collect())
                    .map_err(io_error);
                let _ = reply.send(result);
            }
            Command::Close(reply) => {
                let _ = reply.send(surface.close().map_err(io_error));
            }
            Command::Reopen(reply) => {
                let _ = reply.send(surface.reopen().map_err(io_error));
            }
            Command::Shutdown => {
                let _ = surface.close();
                return;
            }
        }
    }
    let _ = surface.close();
}
pub(super) struct Client {
    sender: Option<SyncSender<Command>>,
    join: Option<JoinHandle<()>>,
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
        let join = thread::Builder::new()
            .name("metis-python-native-host".to_owned())
            .spawn(move || host(config, receiver, ready_sender))
            .map_err(io_error)?;
        match ready_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                sender: Some(sender),
                join: Some(join),
                generation: 0,
                closed: false,
            }),
            Ok(Err(error)) => {
                let _ = join.join();
                Err(error)
            }
            Err(_) => {
                let _ = join.join();
                Err(MetisError::transport(
                    ErrorCode::ConnectionClosed,
                    "native host stopped before readiness",
                ))
            }
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
    fn request<T>(&self, command: Command, receiver: Receiver<Result<T>>) -> Result<T> {
        self.sender
            .as_ref()
            .ok_or_else(|| {
                MetisError::transport(ErrorCode::ConnectionClosed, "native host is stopped")
            })?
            .send(command)
            .map_err(|_| {
                MetisError::transport(ErrorCode::ConnectionClosed, "native host stopped")
            })?;
        receiver.recv().map_err(|_| {
            MetisError::transport(ErrorCode::ConnectionClosed, "native host stopped")
        })?
    }

    pub(super) fn present(&self, generation: u64, framebuffer: Framebuffer) -> Result<()> {
        self.check_generation(generation)?;
        let (sender, receiver) = sync_channel(1);
        self.request(Command::Present(framebuffer, sender), receiver)
    }

    pub(super) fn wait(&self, generation: u64, timeout: Duration) -> Result<Vec<Event>> {
        self.check_generation(generation)?;
        let (sender, receiver) = sync_channel(1);
        self.request(Command::Wait(timeout, sender), receiver)
    }

    pub(super) fn close(&mut self, generation: u64) -> Result<()> {
        self.check_generation(generation)?;
        let (sender, receiver) = sync_channel(1);
        self.request(Command::Close(sender), receiver)?;
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
        self.request(Command::Reopen(sender), receiver)?;
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
            let _ = sender.try_send(Command::Shutdown);
            drop(sender);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
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
