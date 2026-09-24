//! Browser `KeyboardEvent` resolution.

use super::{Digit, FunctionKey, Key, Letter};

impl Key {
    /// Resolves a W3C `KeyboardEvent.code` physical-key value, such as `KeyK`,
    /// `Digit1`, `F5` or `ArrowUp`. Numeric-keypad Enter resolves to [`Key::Enter`].
    #[must_use]
    pub fn from_browser_code(code: &str) -> Option<Self> {
        if let Some(rest) = code.strip_prefix("Key") {
            return match rest.as_bytes() {
                [byte] if byte.is_ascii_uppercase() => Letter::new(*byte).map(Self::Letter),
                _ => None,
            };
        }
        if let Some(rest) = code.strip_prefix("Digit") {
            return match rest.as_bytes() {
                [byte] if byte.is_ascii_digit() => Digit::new(byte - b'0').map(Self::Digit),
                _ => None,
            };
        }
        if let Some(number) = code
            .strip_prefix('F')
            .filter(|digits| !digits.starts_with('0'))
            .and_then(|digits| digits.parse::<u8>().ok())
        {
            return FunctionKey::new(number).map(Self::Function);
        }
        Some(match code {
            "Enter" | "NumpadEnter" => Self::Enter,
            "Escape" => Self::Escape,
            "Tab" => Self::Tab,
            "Space" => Self::Space,
            "Backspace" => Self::Backspace,
            "Delete" => Self::Delete,
            "Insert" => Self::Insert,
            "Home" => Self::Home,
            "End" => Self::End,
            "PageUp" => Self::PageUp,
            "PageDown" => Self::PageDown,
            "ArrowUp" => Self::ArrowUp,
            "ArrowDown" => Self::ArrowDown,
            "ArrowLeft" => Self::ArrowLeft,
            "ArrowRight" => Self::ArrowRight,
            "Minus" => Self::Minus,
            "Equal" => Self::Equal,
            "Comma" => Self::Comma,
            "Period" => Self::Period,
            "Slash" => Self::Slash,
            _ => return None,
        })
    }

    /// Resolves a W3C `KeyboardEvent.key` value for hosts that report no
    /// physical code. Shifted punctuation is not reversed, because the
    /// produced character depends on the keyboard layout.
    #[must_use]
    pub fn from_browser_key(key: &str) -> Option<Self> {
        match key {
            " " => Some(Self::Space),
            "Esc" => Some(Self::Escape),
            // One produced character: a letter of either case, a digit or
            // unshifted punctuation.
            _ if key.len() == 1 => Self::from_token(key),
            // Named keys arrive in their canonical spelling only.
            _ => Self::from_token(key).filter(|resolved| resolved.name() == key),
        }
    }
}
