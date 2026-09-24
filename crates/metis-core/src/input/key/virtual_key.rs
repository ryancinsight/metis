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

impl Key {
    /// Resolves a Windows virtual-key code as `WM_KEYDOWN` reports it.
    #[must_use]
    pub fn from_windows_virtual_key(code: u32) -> Option<Self> {
        let byte = u8::try_from(code).ok()?;
        Some(match code {
            0x41..=0x5A => Self::Letter(Letter::new(byte)?),
            0x30..=0x39 => Self::Digit(Digit::new(byte - b'0')?),
            VK_F1..=0x87 => Self::Function(FunctionKey::new(byte - 0x6F)?),
            VK_BACK => Self::Backspace,
            VK_TAB => Self::Tab,
            VK_RETURN => Self::Enter,
            VK_ESCAPE => Self::Escape,
            VK_SPACE => Self::Space,
            VK_PRIOR => Self::PageUp,
            VK_NEXT => Self::PageDown,
            VK_END => Self::End,
            VK_HOME => Self::Home,
            VK_LEFT => Self::ArrowLeft,
            VK_UP => Self::ArrowUp,
            VK_RIGHT => Self::ArrowRight,
            VK_DOWN => Self::ArrowDown,
            VK_INSERT => Self::Insert,
            VK_DELETE => Self::Delete,
            VK_OEM_PLUS => Self::Equal,
            VK_OEM_COMMA => Self::Comma,
            VK_OEM_MINUS => Self::Minus,
            VK_OEM_PERIOD => Self::Period,
            VK_OEM_2 => Self::Slash,
            _ => return None,
        })
    }
}
