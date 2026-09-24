//! Value-semantic tests for the accelerator vocabulary.

mod accelerator;
mod key;
mod shortcut_map;

use super::{Accelerator, Key, Letter, Modifiers};

fn letter(byte: u8) -> Key {
    Key::Letter(Letter::new(byte).expect("ASCII letter"))
}

fn chord(modifiers: Modifiers, byte: u8) -> Accelerator {
    Accelerator::new(modifiers, letter(byte))
}
