//! Bounded virtual surface and ANSI preview; no native window or OS event integration.

use crate::event::PlatformEvent;
use crate::framebuffer::{Framebuffer, allocation_error};
use metis_core::error::{ErrorCode, MetisError, Result};
use std::collections::VecDeque;
use std::fmt::Write;

/// Maximum pending application-supplied events.
pub const MAX_EVENTS: usize = 1024;
/// Maximum preview size, bounding terminal string allocation to about 2 MiB.
pub const MAX_TERMINAL_CELLS: u64 = 65_536;

/// Software surface with application-supplied events; does not create an OS window.
pub struct PlatformSurface {
    /// Pixel storage used for software rendering.
    pub framebuffer: Framebuffer,
    events: VecDeque<PlatformEvent>,
}

impl PlatformSurface {
    /// Allocates a framebuffer and an empty event queue.
    ///
    /// # Errors
    /// Returns an allocation error when framebuffer limits or reservation fail.
    pub fn new(width: u32, height: u32) -> Result<Self> {
        Ok(Self {
            framebuffer: Framebuffer::new(width, height)?,
            events: VecDeque::new(),
        })
    }

    /// Enqueues an application-supplied event.
    ///
    /// # Errors
    /// Returns `RenderFailure` at [`MAX_EVENTS`], or an allocation error.
    pub fn push_event(&mut self, event: PlatformEvent) -> Result<()> {
        if self.events.len() >= MAX_EVENTS {
            return Err(MetisError::ui(
                ErrorCode::RenderFailure,
                "Event queue capacity exceeded",
            ));
        }
        self.events.try_reserve(1).map_err(|_| allocation_error())?;
        self.events.push_back(event);
        Ok(())
    }

    /// Removes the next queued event in FIFO order.
    pub fn poll_event(&mut self) -> Option<PlatformEvent> {
        self.events.pop_front()
    }

    /// Produces a nearest-sample, 24-bit ANSI color preview.
    ///
    /// Zero downsampling factors mean one. Alpha below 128 produces a blank cell.
    ///
    /// # Errors
    /// Rejects previews exceeding [`MAX_TERMINAL_CELLS`] or failed string reservations.
    pub fn to_ansi_terminal(&self, downsample_x: u32, downsample_y: u32) -> Result<String> {
        let dx = downsample_x.max(1);
        let dy = downsample_y.max(1);
        let rows = self.framebuffer.height() / dy;
        let columns = self.framebuffer.width() / dx;
        let cells = u64::from(rows) * u64::from(columns);
        if cells > MAX_TERMINAL_CELLS || u64::from(rows) > MAX_TERMINAL_CELLS {
            return Err(MetisError::ui(
                ErrorCode::RenderFailure,
                "Terminal preview exceeds cell limit; increase downsampling",
            ));
        }
        // Each cell uses at most 24 bytes: escape + three RGB channels + reset.
        let bytes =
            usize::try_from(cells * 24 + u64::from(rows)).map_err(|_| allocation_error())?;
        let mut output = String::new();
        output
            .try_reserve_exact(bytes)
            .map_err(|_| allocation_error())?;
        for row in 0..rows {
            for column in 0..columns {
                let x = i32::try_from(column * dx).map_err(|_| allocation_error())?;
                let y = i32::try_from(row * dy).map_err(|_| allocation_error())?;
                let color = self.framebuffer.get_pixel(x, y);
                if color.a < 128 {
                    output.push(' ');
                } else {
                    write!(
                        output,
                        "\x1b[48;2;{};{};{}m \x1b[0m",
                        color.r, color.g, color.b
                    )
                    .map_err(|_| {
                        MetisError::ui(ErrorCode::RenderFailure, "Terminal formatting failed")
                    })?;
                }
            }
            output.push('\n');
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_capacity_and_fifo_are_explicit() {
        let mut surface = PlatformSurface::new(1, 1).expect("small surface");
        for key in 0..MAX_EVENTS {
            surface
                .push_event(PlatformEvent::KeyDown(
                    u32::try_from(key).expect("bounded key"),
                ))
                .expect("queue capacity");
        }
        assert_eq!(
            surface
                .push_event(PlatformEvent::Quit)
                .expect_err("full queue")
                .code,
            ErrorCode::RenderFailure
        );
        assert_eq!(surface.poll_event(), Some(PlatformEvent::KeyDown(0)));
        assert_eq!(surface.poll_event(), Some(PlatformEvent::KeyDown(1)));
    }

    #[test]
    fn ansi_preview_encodes_real_pixels() {
        let mut surface = PlatformSurface::new(1, 1).expect("small surface");
        surface.framebuffer.clear(crate::Color::rgb(1, 2, 3));
        assert_eq!(
            surface.to_ansi_terminal(1, 1).expect("small preview"),
            "\x1b[48;2;1;2;3m \x1b[0m\n"
        );
    }
}
