//! Windows native-window adapter over Moirai's platform provider.
//!
//! [`NativeSurface`] owns the safe boundary between a Metis [`Framebuffer`]
//! and a thread-owned Win32 window. The adapter carries no application state or
//! authority; callers continue to own rendering, command policy and lifecycle
//! decisions while Moirai owns the operating-system handle and message
//! translation.

use crate::Framebuffer;
use moirai_pal::windows::window::{NativeWindow, WindowPlacement};
use std::io;
use std::time::Duration;

pub use moirai_pal::windows::window::{
    AccessibilityAction, AccessibilityActionRequest, AccessibilityNode, AccessibilityRole,
    AccessibilityTree, CompositionPhase, MAX_ACCESSIBILITY_ACTIONS, MAX_ACCESSIBILITY_NODES,
    MAX_ACCESSIBILITY_TEXT_BYTES, MAX_COMPOSITION_UNITS, MAX_FRAME_DIMENSION, MAX_FRAME_PIXELS,
    MAX_PUMP_MESSAGES, MAX_TITLE_UNITS, MAX_WAIT_MILLISECONDS, MAX_WINDOW_EVENTS, ModifierState,
    MouseButton, WindowConfig, WindowEvent, WindowVisibility,
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
    config: WindowConfig,
    accessibility_tree: Option<AccessibilityTree>,
}

impl NativeSurface {
    /// Creates a native surface from validated window configuration.
    ///
    /// # Errors
    /// Returns the Win32 or configuration error reported by Moirai.
    pub fn new(config: &WindowConfig) -> io::Result<Self> {
        Self::new_with_accessibility(config, None)
    }

    /// Creates a native surface and installs an initial accessibility tree before visibility.
    ///
    /// The tree is retained so updates and [`Self::reopen`] preserve the same
    /// host contract. A visible configuration is shown only after the adapter
    /// has been installed on the hidden HWND.
    ///
    /// # Errors
    /// Returns the Win32, configuration or accessibility validation error
    /// reported by Moirai.
    pub fn new_with_accessibility(
        config: &WindowConfig,
        accessibility_tree: Option<AccessibilityTree>,
    ) -> io::Result<Self> {
        let window = if let Some(tree) = accessibility_tree.clone() {
            let hidden_config = WindowConfig::with_visibility(
                config.title(),
                config.width(),
                config.height(),
                WindowVisibility::Hidden,
            )?;
            let mut window = NativeWindow::new(&hidden_config)?;
            window.install_accessibility(tree)?;
            if config.visibility() == WindowVisibility::Visible {
                window.show()?;
            }
            window
        } else {
            NativeWindow::new(config)?
        };
        Ok(Self {
            window,
            config: config.clone(),
            accessibility_tree,
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

    /// Replaces the native accessibility tree after the HWND is installed.
    ///
    /// # Errors
    /// Returns the validation or native adapter error reported by Moirai.
    pub fn update_accessibility(&mut self, tree: AccessibilityTree) -> io::Result<()> {
        self.window.update_accessibility(tree.clone())?;
        self.accessibility_tree = Some(tree);
        Ok(())
    }

    /// Recreates a window after [`Self::close`] using its validated configuration.
    ///
    /// A closed surface has no live event stream; pending terminal events are
    /// discarded when the old provider is replaced. The new surface starts with
    /// the same title, client dimensions and visibility policy.
    ///
    /// # Errors
    /// Returns an invalid-state error when the current window is still live, or
    /// the native/configuration error reported while creating the new window.
    pub fn reopen(&mut self) -> io::Result<()> {
        if !self.window.is_destroyed() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "native window must be closed before reopening",
            ));
        }
        let replacement =
            Self::new_with_accessibility(&self.config, self.accessibility_tree.clone())?;
        self.window = replacement.window;
        Ok(())
    }

    /// Reads the window's restored rectangle and maximized state for saving.
    ///
    /// # Errors
    /// Returns the native error, or `InvalidInput` once the window is closed.
    pub fn placement(&self) -> io::Result<WindowPlacement> {
        self.window.placement()
    }

    /// Returns whether the native window has completed destruction.
    #[must_use]
    pub const fn is_destroyed(&self) -> bool {
        self.window.is_destroyed()
    }
}

impl crate::native::TrayHost for NativeSurface {
    fn show_tray_icon(
        &mut self,
        image: &crate::native::TrayIconImage,
        tooltip: &str,
    ) -> io::Result<()> {
        self.window.show_tray_icon(image, tooltip)
    }

    fn show_notification(&mut self, title: &str, body: &str) -> io::Result<()> {
        self.window.show_notification(title, body)
    }

    fn remove_tray_icon(&mut self) -> io::Result<bool> {
        self.window.remove_tray_icon()
    }

    fn take_tray_events(&mut self) -> Vec<crate::native::TrayEvent> {
        self.window.take_tray_events()
    }

    fn show_popup_menu(
        &mut self,
        menu: &crate::native::PopupMenu,
        x: i32,
        y: i32,
    ) -> io::Result<Option<usize>> {
        self.window.show_popup_menu(menu, x, y)
    }
}

impl crate::native::HotkeyHost for NativeSurface {
    fn register_hotkey(
        &mut self,
        id: crate::native::HotkeyId,
        hotkey: crate::native::GlobalHotkey,
    ) -> io::Result<()> {
        self.window.register_hotkey(id, hotkey)
    }

    fn unregister_hotkey(&mut self, id: crate::native::HotkeyId) -> io::Result<bool> {
        self.window.unregister_hotkey(id)
    }

    fn take_hotkey_presses(&mut self) -> Vec<crate::native::HotkeyId> {
        self.window.take_hotkey_presses()
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

    #[test]
    fn adapter_reopens_only_after_close_and_reuses_validated_configuration() {
        let config =
            WindowConfig::with_visibility("Metis reopen test", 320, 240, WindowVisibility::Hidden)
                .expect("bounded native configuration");
        let mut surface = NativeSurface::new(&config).expect("native surface");
        let error = surface.reopen().expect_err("live window cannot reopen");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

        surface.close().expect("native close");
        assert!(surface.is_destroyed());
        surface.reopen().expect("native reopen");
        assert!(!surface.is_destroyed());
        surface.close().expect("reopened native close");
        assert!(surface.is_destroyed());
    }

    #[test]
    fn adapter_keeps_two_hidden_windows_independent() {
        let first_config =
            WindowConfig::with_visibility("Metis first window", 320, 240, WindowVisibility::Hidden)
                .expect("first bounded native configuration");
        let second_config = WindowConfig::with_visibility(
            "Metis second window",
            400,
            300,
            WindowVisibility::Hidden,
        )
        .expect("second bounded native configuration");
        let mut first = NativeSurface::new(&first_config).expect("first native surface");
        let mut second = NativeSurface::new(&second_config).expect("second native surface");
        let mut first_frame = Framebuffer::new(320, 240).expect("first framebuffer");
        let mut second_frame = Framebuffer::new(400, 300).expect("second framebuffer");
        first_frame.clear(Color::BLUE);
        second_frame.clear(Color::DARK_BLUE);
        first.present(&first_frame).expect("first presentation");
        second.present(&second_frame).expect("second presentation");

        let first_events = first
            .wait_events(Duration::ZERO)
            .expect("first native event batch");
        let second_events = second
            .wait_events(Duration::ZERO)
            .expect("second native event batch");
        assert!(first_events.iter().any(|event| matches!(
            event,
            WindowEvent::Resized {
                width: 320,
                height: 240
            }
        )));
        assert!(second_events.iter().any(|event| matches!(
            event,
            WindowEvent::Resized {
                width: 400,
                height: 300
            }
        )));

        first.close().expect("first native close");
        assert!(first.is_destroyed());
        assert!(!second.is_destroyed());
        second.close().expect("second native close");
        assert!(second.is_destroyed());
    }

    #[test]
    fn adapter_installs_updates_and_reopens_accessibility_before_visibility() {
        let config = WindowConfig::with_visibility(
            "Metis accessibility test",
            320,
            240,
            WindowVisibility::Hidden,
        )
        .expect("bounded native configuration");
        let mut button = AccessibilityNode::new(2, AccessibilityRole::Button, "Submit")
            .expect("accessibility button");
        button.set_focusable(true);
        button.add_action(AccessibilityAction::Activate);
        let mut root = AccessibilityNode::new(1, AccessibilityRole::Application, "Metis")
            .expect("accessibility root");
        root.set_children(vec![2]).expect("root children");
        let tree = AccessibilityTree::from_nodes(1, 2, vec![root, button])
            .expect("validated accessibility tree");

        let mut surface = NativeSurface::new_with_accessibility(&config, Some(tree.clone()))
            .expect("native surface with accessibility");
        surface
            .update_accessibility(tree.clone())
            .expect("accessibility update");
        surface.close().expect("native close");
        surface.reopen().expect("native reopen with accessibility");
        surface
            .update_accessibility(tree)
            .expect("reopened accessibility update");
        surface.close().expect("reopened native close");
        assert!(surface.is_destroyed());
    }
}
