//! Windows native-window adapter over Moirai's platform provider.
//!
//! [`NativeSurface`] owns the safe boundary between a Metis [`Framebuffer`]
//! and a thread-owned Win32 window. The adapter carries no application state or
//! authority; callers continue to own rendering, command policy and lifecycle
//! decisions while Moirai owns the operating-system handle and message
//! translation.

use crate::Framebuffer;
use moirai_pal::windows::window::NativeWindow;
use std::io;
use std::time::Duration;

pub use moirai_pal::windows::window::{
    CompositionPhase, MAX_COMPOSITION_UNITS, MAX_FRAME_DIMENSION, MAX_FRAME_PIXELS,
    MAX_PUMP_MESSAGES, MAX_TITLE_UNITS, MAX_WAIT_MILLISECONDS, MAX_WINDOW_EVENTS, MouseButton,
    WindowConfig, WindowEvent, WindowVisibility,
};

/// A Metis framebuffer presented by a Moirai-owned native window.
///
/// The value is thread-owned: construct, poll, present and close it on the
/// same thread. Frame dimensions and event storage remain bounded by the
/// provider constants re-exported from this module. Native IME preedit,
/// commit and cancellation arrive as bounded [`WindowEvent::TextComposition`]
/// values; the application decides how committed text changes its state.
pub struct NativeSurface {
    window: NativeWindow,
}

impl NativeSurface {
    /// Creates a native surface from validated window configuration.
    ///
    /// # Errors
    /// Returns the Win32 or configuration error reported by Moirai.
    pub fn new(config: &WindowConfig) -> io::Result<Self> {
        Ok(Self {
            window: NativeWindow::new(config)?,
        })
    }

    /// Translates at most one bounded provider batch of native events.
    ///
    /// # Errors
    /// Returns a queue-overflow or native message-pump error from Moirai.
    pub fn poll_events(&mut self) -> io::Result<Vec<WindowEvent>> {
        self.window.poll_events()
    }

    /// Waits for native input for a finite duration and returns one event batch.
    ///
    /// # Errors
    /// Returns an invalid-duration, native wait, or bounded queue error.
    pub fn wait_events(&mut self, timeout: Duration) -> io::Result<Vec<WindowEvent>> {
        self.window.wait_events(timeout)
    }

    /// Presents a Metis ARGB framebuffer through the native window.
    ///
    /// The provider copies the pixels before returning, so the framebuffer may
    /// be reused immediately. Dimensions and pixel count are checked by the
    /// provider before any repaint is scheduled.
    ///
    /// # Errors
    /// Returns an invalid-frame, allocation or native-window error from Moirai.
    pub fn present(&mut self, framebuffer: &Framebuffer) -> io::Result<()> {
        self.window.present_argb8888(
            framebuffer.width(),
            framebuffer.height(),
            framebuffer.pixels(),
        )
    }

    /// Requests synchronous destruction of the native window.
    ///
    /// # Errors
    /// Returns the native destruction error from Moirai.
    pub fn close(&mut self) -> io::Result<()> {
        self.window.close()
    }

    /// Returns whether the native window has completed destruction.
    #[must_use]
    pub const fn is_destroyed(&self) -> bool {
        self.window.is_destroyed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, Framebuffer};

    #[test]
    fn adapter_presents_framebuffer_and_closes_native_window() {
        let config =
            WindowConfig::with_visibility("Metis adapter test", 320, 240, WindowVisibility::Hidden)
                .expect("bounded native configuration");
        let mut surface = NativeSurface::new(&config).expect("native surface");
        let mut framebuffer = Framebuffer::new(320, 240).expect("bounded framebuffer");
        framebuffer.clear(Color::BLUE);
        surface.present(&framebuffer).expect("native presentation");
        let events = surface
            .wait_events(Duration::ZERO)
            .expect("native event batch");
        assert!(events.iter().any(|event| matches!(
            event,
            WindowEvent::Resized {
                width: 320,
                height: 240
            }
        )));
        assert!(!surface.is_destroyed());
        surface.close().expect("native close");
        assert!(surface.is_destroyed());
    }
}
