//! The closed set of keys an accelerator may name.

mod browser;
mod virtual_key;

/// An ASCII letter key, `A` through `Z`, independent of case and layout
/// shift state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Letter(u8);

impl Letter {
    /// Admits an ASCII letter of either case.
    #[must_use]
    pub const fn new(byte: u8) -> Option<Self> {
        if byte.is_ascii_alphabetic() {
            Some(Self(byte.to_ascii_uppercase()))
        } else {
            None
        }
    }

    /// The uppercase ASCII byte.
    #[must_use]
    pub const fn byte(self) -> u8 {
        self.0
    }
}

/// A top-row digit key, `0` through `9`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Digit(u8);

impl Digit {
    /// Admits a value from 0 through 9.
    #[must_use]
    pub const fn new(value: u8) -> Option<Self> {
        if value <= 9 { Some(Self(value)) } else { None }
    }

    /// The numeric value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

/// A function key, `F1` through `F24`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionKey(u8);

impl FunctionKey {
    /// The highest function key number hosts report.
    pub const MAX: u8 = 24;

    /// Admits a function key number from 1 through [`Self::MAX`].
    #[must_use]
    pub const fn new(number: u8) -> Option<Self> {
        if number >= 1 && number <= Self::MAX {
            Some(Self(number))
        } else {
            None
        }
    }

    /// The function key number.
    #[must_use]
    pub const fn number(self) -> u8 {
        self.0
    }
}

/// A non-modifier key.
///
/// Letters, digits and punctuation name the physical US-layout position, as
/// `KeyboardEvent.code` and Windows virtual keys do, so a shortcut does not
/// change meaning when Shift or Option alters the produced character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Key {
    /// A letter key.
    Letter(Letter),
    /// A top-row digit key.
    Digit(Digit),
    /// A function key.
    Function(FunctionKey),
    /// Enter or Return.
    Enter,
    /// Escape.
    Escape,
    /// Tab.
    Tab,
    /// The space bar.
    Space,
    /// Backspace.
    Backspace,
    /// Forward delete.
    Delete,
    /// Insert.
    Insert,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Up arrow.
    ArrowUp,
    /// Down arrow.
    ArrowDown,
    /// Left arrow.
    ArrowLeft,
    /// Right arrow.
    ArrowRight,
    /// `-` and `_`.
    Minus,
    /// `=` and `+`.
    Equal,
    /// `,` and `<`.
    Comma,
    /// `.` and `>`.
    Period,
    /// `/` and `?`.
    Slash,
}

/// Named keys with their canonical names, which are also their W3C
/// `KeyboardEvent.key` values; accelerator text accepts each case-insensitively.
const NAMED: [(Key, &str); 20] = [
    (Key::Enter, "Enter"),
    (Key::Escape, "Escape"),
    (Key::Tab, "Tab"),
    (Key::Space, "Space"),
    (Key::Backspace, "Backspace"),
    (Key::Delete, "Delete"),
    (Key::Insert, "Insert"),
    (Key::Home, "Home"),
    (Key::End, "End"),
    (Key::PageUp, "PageUp"),
    (Key::PageDown, "PageDown"),
    (Key::ArrowUp, "ArrowUp"),
    (Key::ArrowDown, "ArrowDown"),
    (Key::ArrowLeft, "ArrowLeft"),
    (Key::ArrowRight, "ArrowRight"),
    (Key::Minus, "-"),
    (Key::Equal, "="),
    (Key::Comma, ","),
    (Key::Period, "."),
    (Key::Slash, "/"),
];

/// Common alternative spellings accepted by accelerator text.
const ALIASES: [(Key, &str); 12] = [
    (Key::Enter, "Return"),
    (Key::Escape, "Esc"),
    (Key::Delete, "Del"),
    (Key::ArrowUp, "Up"),
    (Key::ArrowDown, "Down"),
    (Key::ArrowLeft, "Left"),
    (Key::ArrowRight, "Right"),
    (Key::Minus, "Minus"),
    (Key::Equal, "Equal"),
    (Key::Comma, "Comma"),
    (Key::Period, "Period"),
    (Key::Slash, "Slash"),
];

const LETTER_NAMES: [&str; 26] = [
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];
const DIGIT_NAMES: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
const FUNCTION_NAMES: [&str; FunctionKey::MAX as usize] = [
    "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13", "F14", "F15",
    "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24",
];

impl Key {
    /// The canonical name used in accelerator text and `aria-keyshortcuts`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Letter(letter) => LETTER_NAMES[usize::from(letter.0 - b'A')],
            Self::Digit(digit) => DIGIT_NAMES[usize::from(digit.0)],
            Self::Function(function) => FUNCTION_NAMES[usize::from(function.0 - 1)],
            named => NAMED
                .iter()
                .find_map(|(key, name)| (*key == named).then_some(*name))
                .unwrap_or_default(),
        }
    }

    /// Resolves one accelerator token, ignoring ASCII case.
    pub(super) fn from_token(token: &str) -> Option<Self> {
        if let [byte] = token.as_bytes() {
            if let Some(letter) = Letter::new(*byte) {
                return Some(Self::Letter(letter));
            }
            if byte.is_ascii_digit() {
                return Digit::new(byte - b'0').map(Self::Digit);
            }
        }
        if let Some(number) = token
            .strip_prefix(['F', 'f'])
            .filter(|digits| !digits.starts_with('0'))
            .and_then(|digits| digits.parse::<u8>().ok())
        {
            return FunctionKey::new(number).map(Self::Function);
        }
        NAMED
            .iter()
            .chain(ALIASES.iter())
            .find_map(|(key, name)| name.eq_ignore_ascii_case(token).then_some(*key))
    }
}
