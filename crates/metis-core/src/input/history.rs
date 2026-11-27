//! Standard undo and redo chords for hosts with application-owned edit history.
use super::{Accelerator, Key, Modifiers};

/// History action requested by a primary-modifier keyboard chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryAction {
    /// Restore the previous edit.
    Undo,
    /// Reapply an undone edit.
    Redo,
}

impl HistoryAction {
    /// Decode exact primary-Z and primary-Shift-Z chords using the supplied
    /// host primary modifier. Extra modifiers or unrelated keys are ignored.
    #[must_use]
    pub fn from_accelerator(chord: Accelerator, primary: Modifiers) -> Option<Self> {
        if !matches!(primary, Modifiers::CTRL | Modifiers::META)
            || !matches!(chord.key(),Key::Letter(letter) if letter.byte()==b'Z')
        {
            return None;
        }
        if chord.modifiers() == primary {
            Some(Self::Undo)
        } else if chord.modifiers() == primary.union(Modifiers::SHIFT) {
            Some(Self::Redo)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_and_apple_history_require_exact_modifiers() {
        for primary in [Modifiers::CTRL, Modifiers::META] {
            let key = Key::from_windows_virtual_key(0x5a).expect("Z");
            for (mods, action) in [
                (primary, Some(HistoryAction::Undo)),
                (primary.union(Modifiers::SHIFT), Some(HistoryAction::Redo)),
                (primary.union(Modifiers::ALT), None),
                (Modifiers::NONE, None),
            ] {
                assert_eq!(
                    HistoryAction::from_accelerator(Accelerator::new(mods, key), primary),
                    action
                );
            }
            assert_eq!(
                HistoryAction::from_accelerator(Accelerator::new(primary, Key::ArrowUp), primary),
                None
            );
        }
    }
}
