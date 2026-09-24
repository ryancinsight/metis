//! Conversions between Metis window state and Moirai window placement.
//!
//! [`WindowState`] is the platform-neutral value a host saves; Moirai's
//! [`WindowPlacement`] is what a native window accepts. Both bound the same
//! coordinates and dimensions, and a value outside the other side's bounds is
//! refused rather than clamped, so a restored window is exactly the saved one.

use metis_core::window_state::WindowState;
use std::io;

pub use moirai_pal::windows::window::{MAX_PLACEMENT_COORDINATE, WindowPlacement};

/// Converts a saved window state into a native placement.
///
/// # Errors
/// Returns `InvalidInput` when the state is outside the native bounds.
pub fn placement_from_state(state: &WindowState) -> io::Result<WindowPlacement> {
    WindowPlacement::new(
        state.left(),
        state.top(),
        state.width(),
        state.height(),
        state.maximized(),
    )
}

/// Converts a native placement into the state a host saves.
///
/// # Errors
/// Returns `InvalidData` when the placement is outside the saved bounds.
pub fn state_from_placement(placement: &WindowPlacement) -> io::Result<WindowState> {
    WindowState::new(
        placement.left(),
        placement.top(),
        placement.width(),
        placement.height(),
        placement.maximized(),
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::{NativeSurface, WindowConfig, WindowVisibility};
    use crate::window_state_file::WindowStateFile;

    #[test]
    fn saved_state_restores_the_native_placement() {
        let saved = WindowState::new(72, 88, 520, 380, true).expect("state");
        let config =
            WindowConfig::with_visibility("Metis window state", 320, 240, WindowVisibility::Hidden)
                .expect("bounded native configuration")
                .with_placement(placement_from_state(&saved).expect("placement"));
        let mut surface = NativeSurface::new(&config).expect("native surface");
        let current =
            state_from_placement(&surface.placement().expect("placement")).expect("state");
        assert_eq!(current, saved);

        let root = std::env::temp_dir().join(format!("metis-placement-{}", std::process::id()));
        let file = WindowStateFile::new(root.join("window.state")).expect("store");
        file.save(&current).expect("save");
        assert_eq!(file.load().expect("load"), Some(saved));
        std::fs::remove_dir_all(root).expect("cleanup");
        surface.close().expect("native close");
        assert!(surface.placement().is_err());
    }
}
