//! Tray icons and notifications on the native surfaces.
//!
//! This is the counterpart of Tauri's tray and notification plugins. An
//! application draws its icon into a 16 or 32 pixel [`Framebuffer`] with the
//! same rasterizer it uses for windows, shows it through a [`TrayHost`], and
//! raises notifications from it. Icon activity arrives as [`TrayEvent`]
//! values drained after each event wait; a context request carries the
//! screen position at which [`TrayHost::show_popup_menu`] opens a native
//! menu whose chosen index the application maps to its own command.

use crate::Framebuffer;
use std::io;

pub use moirai_pal::windows::window::{
    MAX_NOTIFICATION_BODY_UNITS, MAX_NOTIFICATION_TITLE_UNITS, MAX_PENDING_TRAY_EVENTS,
    MAX_POPUP_MENU_ITEMS, MAX_POPUP_MENU_LABEL_UNITS, MAX_TRAY_TOOLTIP_UNITS, PopupMenu,
    PopupMenuItem, TRAY_ICON_SIZES, TrayEvent, TrayIconImage,
};

/// A native surface that can show a tray icon and notifications.
pub trait TrayHost {
    /// Shows or replaces the tray icon and its tooltip.
    ///
    /// # Errors
    /// Returns the validation or shell error.
    fn show_tray_icon(&mut self, image: &TrayIconImage, tooltip: &str) -> io::Result<()>;

    /// Shows a notification from the tray icon.
    ///
    /// # Errors
    /// Returns `InvalidInput` without an icon or for invalid text, or the
    /// shell error.
    fn show_notification(&mut self, title: &str, body: &str) -> io::Result<()>;

    /// Removes the tray icon; returns whether one was shown.
    ///
    /// # Errors
    /// Returns the shell error.
    fn remove_tray_icon(&mut self) -> io::Result<bool>;

    /// Drains queued icon activity, oldest first.
    fn take_tray_events(&mut self) -> Vec<TrayEvent>;

    /// Shows a context menu at a screen position, such as a
    /// [`TrayEvent::ContextRequested`] anchor, and waits for the choice.
    ///
    /// Returns the chosen item's index, or `None` when dismissed.
    ///
    /// # Errors
    /// Returns an error for a closed surface or a menu the system refuses.
    fn show_popup_menu(&mut self, menu: &PopupMenu, x: i32, y: i32) -> io::Result<Option<usize>>;
}

/// Converts a square framebuffer drawn by the application into a tray image.
///
/// # Errors
/// Returns `InvalidInput` unless the framebuffer is a square whose edge is
/// one of [`TRAY_ICON_SIZES`].
pub fn tray_image(framebuffer: &Framebuffer) -> io::Result<TrayIconImage> {
    if framebuffer.width() != framebuffer.height() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "tray icon framebuffer must be square",
        ));
    }
    TrayIconImage::new(framebuffer.width(), framebuffer.pixels())
}

#[cfg(test)]
mod tests;
