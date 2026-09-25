//! Format-neutral native application lifecycle over a Moirai window.

use super::{AccessibilityTree, NativeSurface, WindowConfig, WindowEvent};
use crate::{Damage, Framebuffer, Rect};
use std::{error::Error, fmt, io, time::Duration};

/// Result of one application event-batch transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NativeFlow {
    /// Keep the host alive and optionally present the current framebuffer.
    Continue {
        /// Whether the application changed pixels and needs presentation.
        repaint: bool,
    },
    /// End the application session after the current batch.
    Exit,
}

/// Application-owned state consumed by the native host loop.
pub trait NativeApplication {
    /// Typed application failure returned while applying native events.
    type Error: Error + 'static;

    /// Returns the complete frame for the next native presentation.
    fn framebuffer(&self) -> &Framebuffer;

    /// Takes the part of the framebuffer changed since the last take, so the
    /// host presents only that part.
    ///
    /// The default reports the whole frame, which is correct for any
    /// application; one that tracks its repaints reports less. The host takes
    /// damage before the first presentation and at each requested repaint,
    /// and presents nothing for [`Damage::Unchanged`].
    fn take_damage(&mut self) -> Damage {
        Damage::Full
    }

    /// Returns the current native accessibility tree, when the application
    /// opted into the native provider at startup.
    ///
    /// An application that returns `Some` during startup must continue to
    /// return a validated tree after each event batch. Browser hosts retain
    /// their DOM accessibility path and return `None`.
    ///
    /// # Errors
    /// Returns the application's typed semantic projection error.
    fn accessibility_tree(&self) -> Result<Option<AccessibilityTree>, Self::Error> {
        Ok(None)
    }

    /// Applies one bounded native event batch and reports the next host action.
    ///
    /// The batch can be empty when the finite wait expires. Applications use
    /// that case for bounded timer or animation ticks without introducing a
    /// second event loop. A `Resized` event must update the returned framebuffer
    /// before requesting a repaint.
    ///
    /// # Errors
    /// Returns the application's typed transition error without changing host
    /// ownership of the native surface.
    fn handle_events(&mut self, events: &[WindowEvent]) -> Result<NativeFlow, Self::Error>;
}

/// Failure from either the native surface or the application transition.
#[derive(Debug)]
#[non_exhaustive]
pub enum NativeHostError<E> {
    /// The window provider rejected creation, waiting, presentation or close.
    Surface(io::Error),
    /// The application rejected a native event batch.
    Application(E),
}

trait NativeSurfaceDriver {
    fn wait_events(&mut self, timeout: Duration) -> io::Result<Vec<WindowEvent>>;

    fn present(&mut self, framebuffer: &Framebuffer) -> io::Result<()>;

    fn present_region(&mut self, framebuffer: &Framebuffer, region: Rect) -> io::Result<()>;

    fn update_accessibility(&mut self, _tree: AccessibilityTree) -> io::Result<()> {
        Ok(())
    }

    fn close(&mut self) -> io::Result<()>;
}

impl NativeSurfaceDriver for NativeSurface {
    fn wait_events(&mut self, timeout: Duration) -> io::Result<Vec<WindowEvent>> {
        NativeSurface::wait_events(self, timeout)
    }

    fn present(&mut self, framebuffer: &Framebuffer) -> io::Result<()> {
        NativeSurface::present(self, framebuffer)
    }

    fn present_region(&mut self, framebuffer: &Framebuffer, region: Rect) -> io::Result<()> {
        NativeSurface::present_region(self, framebuffer, region)
    }

    fn update_accessibility(&mut self, tree: AccessibilityTree) -> io::Result<()> {
        NativeSurface::update_accessibility(self, tree)
    }

    fn close(&mut self) -> io::Result<()> {
        NativeSurface::close(self)
    }
}

impl<E> fmt::Display for NativeHostError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Surface(error) => write!(formatter, "native surface error: {error}"),
            Self::Application(error) => write!(formatter, "native application error: {error}"),
        }
    }
}

impl<E> Error for NativeHostError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Surface(error) => Some(error),
            Self::Application(error) => Some(error),
        }
    }
}

/// Runs a format-neutral application over a thread-owned native surface.
///
/// The host owns only window creation, finite waiting, retained-frame
/// presentation and terminal-window cleanup. The application owns all state,
/// event policy and format-specific interpretation. This keeps application
/// domain protocols outside the platform crate.
///
/// # Errors
/// Returns [`NativeHostError::Surface`] when the provider rejects an operation
/// or [`NativeHostError::Application`] when the application rejects an event
/// batch.
pub fn run_native_application<A>(
    config: &WindowConfig,
    application: A,
    wait: Duration,
) -> Result<(), NativeHostError<A::Error>>
where
    A: NativeApplication,
{
    let initial_accessibility = application
        .accessibility_tree()
        .map_err(NativeHostError::Application)?;
    let surface = NativeSurface::new_with_accessibility(config, initial_accessibility)
        .map_err(NativeHostError::Surface)?;
    run_application_loop(surface, application, wait)
}

fn run_application_loop<S, A>(
    mut surface: S,
    application: A,
    wait: Duration,
) -> Result<(), NativeHostError<A::Error>>
where
    S: NativeSurfaceDriver,
    A: NativeApplication,
{
    let mut application = application;
    // The first presentation is whole whatever the application reports;
    // taking its damage starts the accounting from this frame.
    let initial = application.take_damage().merge(Damage::Full);
    present_damage(&mut surface, application.framebuffer(), initial)
        .map_err(NativeHostError::Surface)?;

    loop {
        let events = surface
            .wait_events(wait)
            .map_err(NativeHostError::Surface)?;
        let destroyed = events.iter().any(is_destroyed);
        let terminal = events.iter().any(is_terminal);
        let flow = application
            .handle_events(&events)
            .map_err(NativeHostError::Application)?;

        if destroyed {
            return Ok(());
        }
        if terminal || matches!(flow, NativeFlow::Exit) {
            surface.close().map_err(NativeHostError::Surface)?;
            return Ok(());
        }
        if let Some(tree) = application
            .accessibility_tree()
            .map_err(NativeHostError::Application)?
        {
            surface
                .update_accessibility(tree)
                .map_err(NativeHostError::Surface)?;
        }
        if matches!(flow, NativeFlow::Continue { repaint: true }) {
            let damage = application.take_damage();
            present_damage(&mut surface, application.framebuffer(), damage)
                .map_err(NativeHostError::Surface)?;
        }
    }
}

fn present_damage<S: NativeSurfaceDriver>(
    surface: &mut S,
    framebuffer: &Framebuffer,
    damage: Damage,
) -> io::Result<()> {
    match damage {
        Damage::Unchanged => Ok(()),
        Damage::Region(region) => surface.present_region(framebuffer, region),
        Damage::Full => surface.present(framebuffer),
    }
}

fn is_destroyed(event: &WindowEvent) -> bool {
    matches!(event, WindowEvent::Destroyed)
}

fn is_terminal(event: &WindowEvent) -> bool {
    matches!(event, WindowEvent::CloseRequested | WindowEvent::Destroyed)
}

#[cfg(test)]
mod tests;
