//! Native menu bars on the Windows surfaces.
//!
//! The counterpart of the menus `muda` gives Tauri and Dioxus. An
//! application builds a [`MenuBar`] of titled [`PopupMenu`](super::PopupMenu)s,
//! attaches it through a [`MenuBarHost`] and reads each chosen item as a
//! [`MenuCommand`] after its event wait. [`menu_label`] shows an item's
//! keyboard shortcut from the same [`Accelerator`] the application's
//! `ShortcutMap` binds, so the menu and the key agree.

use metis_core::input::Accelerator;
use std::io;

pub use moirai_pal::windows::window::{
    MAX_MENU_BAR_MENUS, MAX_PENDING_MENU_COMMANDS, MenuBar, MenuCommand,
};

/// A native surface that can show a menu bar.
pub trait MenuBarHost {
    /// Attaches, replaces or removes (`None`) the menu bar.
    ///
    /// # Errors
    /// Returns an error for a closed surface or a menu the system refuses.
    fn set_menu_bar(&mut self, bar: Option<&MenuBar>) -> io::Result<()>;

    /// Drains the chosen menu-bar items, oldest first.
    fn take_menu_commands(&mut self) -> Vec<MenuCommand>;
}

/// An item label with its shortcut right-aligned, as Windows menus show it.
#[must_use]
pub fn menu_label(label: &str, shortcut: Option<Accelerator>) -> String {
    match shortcut {
        Some(accelerator) => format!("{label}\t{accelerator}"),
        None => label.to_owned(),
    }
}

#[cfg(test)]
mod tests;
