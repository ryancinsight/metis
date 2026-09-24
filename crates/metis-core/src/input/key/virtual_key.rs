//! Windows virtual-key resolution.
//!
//! The codes are the documented `VK_*` constants; the table is pure data, so
//! it is compiled and tested on every target.

use super::{Digit, FunctionKey, Key, Letter};

const VK_BACK: u32 = 0x08;
const VK_TAB: u32 = 0x09;
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_SPACE: u32 = 0x20;
const VK_PRIOR: u32 = 0x21;
const VK_NEXT: u32 = 0x22;
const VK_END: u32 = 0x23;
const VK_HOME: u32 = 0x24;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;
const VK_INSERT: u32 = 0x2D;
const VK_DELETE: u32 = 0x2E;
const VK_F1: u32 = 0x70;
const VK_OEM_PLUS: u32 = 0xBB;
const VK_OEM_COMMA: u32 = 0xBC;
const VK_OEM_MINUS: u32 = 0xBD;
const VK_OEM_PERIOD: u32 = 0xBE;
const VK_OEM_2: u32 = 0xBF;

/// Every named key with its virtual-key code; the one table both directions
/// read.
const NAMED: [(u32, Key); 20] = [
    (VK_BACK, Key::Backspace),
    (VK_TAB, Key::Tab),
    (VK_RETURN, Key::Enter),
    (VK_ESCAPE, Key::Escape),
    (VK_SPACE, Key::Space),
    (VK_PRIOR, Key::PageUp),
    (VK_NEXT, Key::PageDown),
    (VK_END, Key::End),
    (VK_HOME, Key::Home),
    (VK_LEFT, Key::ArrowLeft),
    (VK_UP, Key::ArrowUp),
    (VK_RIGHT, Key::ArrowRight),
    (VK_DOWN, Key::ArrowDown),
    (VK_INSERT, Key::Insert),
    (VK_DELETE, Key::Delete),
    (VK_OEM_PLUS, Key::Equal),
    (VK_OEM_COMMA, Key::Comma),
    (VK_OEM_MINUS, Key::Minus),
    (VK_OEM_PERIOD, Key::Period),
    (VK_OEM_2, Key::Slash),
];

impl Key {
    /// Resolves a Windows virtual-key code as `WM_KEYDOWN` reports it.
    #[must_use]
    pub fn from_windows_virtual_key(code: u32) -> Option<Self> {
        let byte = u8::try_from(code).ok()?;
        match code {
            0x41..=0x5A => Letter::new(byte).map(Self::Letter),
            0x30..=0x39 => Digit::new(byte - b'0').map(Self::Digit),
            VK_F1..=0x87 => FunctionKey::new(byte - 0x6F).map(Self::Function),
            _ => NAMED
                .iter()
                .find(|(named, _)| *named == code)
                .map(|(_, key)| *key),
        }
    }

    /// The Windows virtual-key code that produces this key, the inverse of
    /// [`Self::from_windows_virtual_key`]; `None` for a key Windows has no
    /// code for.
    #[must_use]
    pub fn windows_virtual_key(self) -> Option<u32> {
        match self {
            Self::Letter(letter) => Some(u32::from(letter.byte())),
            Self::Digit(digit) => Some(u32::from(b'0' + digit.value())),
            Self::Function(function) => Some(VK_F1 + u32::from(function.number()) - 1),
            named => NAMED
                .iter()
                .find(|(_, key)| *key == named)
                .map(|(code, _)| *code),
        }
    }
}
