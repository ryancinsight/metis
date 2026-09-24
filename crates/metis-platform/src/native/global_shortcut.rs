//! System-wide shortcuts: Metis accelerators registered as Moirai hotkeys.
//!
//! This is the counterpart of Tauri's global-shortcut plugin. An application
//! binds [`Accelerator`] values, the same type its in-window shortcuts use, to
//! commands; each binding becomes a Moirai hotkey on a native surface, and
//! [`GlobalShortcuts::take_commands`] turns the presses the surface queued
//! into commands. Conflicts are refused when bound, and a chord another
//! application already owns is reported as the native registration error.

use metis_core::input::{Accelerator, MAX_SHORTCUTS, Modifiers, ShortcutMap};
use std::io;

use moirai_pal::windows::window::ModifierState;
pub use moirai_pal::windows::window::{
    GlobalHotkey, HotkeyId, MAX_GLOBAL_HOTKEYS, MAX_PENDING_HOTKEY_PRESSES,
};

/// A native surface that can hold system-wide hotkeys.
pub trait HotkeyHost {
    /// Registers `hotkey`, reported as `id` when pressed.
    ///
    /// # Errors
    /// Returns the native registration error.
    fn register_hotkey(&mut self, id: HotkeyId, hotkey: GlobalHotkey) -> io::Result<()>;

    /// Releases `id`; returns whether it was registered.
    ///
    /// # Errors
    /// Returns the native release error.
    fn unregister_hotkey(&mut self, id: HotkeyId) -> io::Result<bool>;

    /// Drains the queued presses, oldest first.
    fn take_hotkey_presses(&mut self) -> Vec<HotkeyId>;
}

/// Converts an accelerator into a system-wide hotkey chord.
///
/// # Errors
/// Returns `InvalidInput` for an accelerator without modifiers or a key
/// Windows has no virtual-key code for.
pub fn hotkey_for(accelerator: Accelerator) -> io::Result<GlobalHotkey> {
    let key = accelerator.key().windows_virtual_key().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "accelerator key has no Windows virtual-key code",
        )
    })?;
    let held = accelerator.modifiers();
    let mut modifiers = ModifierState::NONE;
    for (flag, state) in [
        (Modifiers::CTRL, ModifierState::CONTROL),
        (Modifiers::ALT, ModifierState::ALT),
        (Modifiers::SHIFT, ModifierState::SHIFT),
        (Modifiers::META, ModifierState::META),
    ] {
        if held.contains(flag) {
            modifiers |= state;
        }
    }
    GlobalHotkey::new(modifiers, key)
}

/// System-wide accelerator bindings held on one native surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalShortcuts<C> {
    bindings: ShortcutMap<(HotkeyId, C)>,
}

impl<C> Default for GlobalShortcuts<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> GlobalShortcuts<C> {
    /// An empty set of bindings.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bindings: ShortcutMap::new(),
        }
    }

    /// Number of registered bindings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Whether nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Registers `accelerator` system-wide on `host` and binds it to
    /// `command`.
    ///
    /// # Errors
    /// Returns `AlreadyExists` when the accelerator is already bound here,
    /// `InvalidInput` when it cannot be a hotkey, `OutOfMemory` at
    /// [`MAX_SHORTCUTS`], or the native error (for example when another
    /// application owns the chord). The bindings are unchanged on error.
    pub fn register(
        &mut self,
        host: &mut impl HotkeyHost,
        accelerator: Accelerator,
        command: C,
    ) -> io::Result<()> {
        if self.bindings.resolve(accelerator).is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "accelerator is already bound to a global shortcut",
            ));
        }
        let hotkey = hotkey_for(accelerator)?;
        let id = self.free_id()?;
        host.register_hotkey(id, hotkey)?;
        if let Err(error) = self.bindings.bind(accelerator, (id, command)) {
            let _ = host.unregister_hotkey(id);
            return Err(io::Error::new(io::ErrorKind::OutOfMemory, error));
        }
        Ok(())
    }

    /// Releases `accelerator` on `host`, returning its command.
    ///
    /// # Errors
    /// Returns the native release error; the binding is then kept.
    pub fn unregister(
        &mut self,
        host: &mut impl HotkeyHost,
        accelerator: Accelerator,
    ) -> io::Result<Option<C>> {
        let Some((id, _)) = self.bindings.resolve(accelerator) else {
            return Ok(None);
        };
        host.unregister_hotkey(*id)?;
        Ok(self
            .bindings
            .unbind(accelerator)
            .map(|(_, command)| command))
    }

    /// The commands of the presses `host` queued, oldest first.
    pub fn take_commands(&self, host: &mut impl HotkeyHost) -> Vec<&C> {
        host.take_hotkey_presses()
            .into_iter()
            .filter_map(|pressed| {
                self.bindings
                    .iter()
                    .find_map(|(_, (id, command))| (*id == pressed).then_some(command))
            })
            .collect()
    }

    fn free_id(&self) -> io::Result<HotkeyId> {
        (0..MAX_SHORTCUTS)
            .filter_map(|raw| u16::try_from(raw).ok())
            .find(|raw| self.bindings.iter().all(|(_, (id, _))| id.get() != *raw))
            .map(HotkeyId::new)
            .transpose()?
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::OutOfMemory,
                    "global shortcut capacity exceeded",
                )
            })
    }
}

#[cfg(test)]
mod tests;
