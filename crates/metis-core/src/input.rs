//! Host-neutral pointer displacement, keyboard accelerators, and shortcuts.
//!
//! Every presentation host reports keys differently: browsers send
//! `KeyboardEvent.code`/`key` strings and Windows sends virtual-key codes.
//! This module owns one vocabulary for both, so an application declares a
//! shortcut once — in the `CmdOrCtrl+Shift+K` syntax Tauri and Electron
//! menus use — and every host resolves its own events against that
//! declaration. Parsing is bounded and rejects ambiguous text rather than
//! guessing; the registry rejects conflicting bindings when they are made
//! instead of when a key is pressed.
//!
//! Resolution is pure. Hosts decide when a resolved shortcut is consumed,
//! and nothing here grants operating-system authority or registers a global
//! (system-wide) hotkey.
mod accelerator;
mod error;
mod history;
mod key;
mod modifiers;
mod pointer_drag;
mod shortcut_map;

pub use accelerator::{Accelerator, MAX_ACCELERATOR_BYTES};
pub use error::{AcceleratorError, PointerDragError, ShortcutError};
pub use history::HistoryAction;
pub use key::{Digit, FunctionKey, Key, Letter};
pub use modifiers::Modifiers;
pub use pointer_drag::PointerDrag;
pub use shortcut_map::{MAX_SHORTCUTS, ShortcutMap};

#[cfg(test)]
mod tests;
