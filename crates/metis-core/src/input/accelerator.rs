//! Accelerator values and their text form.

use super::{AcceleratorError, Key, Modifiers};
use std::fmt;

/// Upper bound on accelerator text, far above the longest valid form
/// (`CommandOrControl+Shift+Alt+ArrowRight` is 37 bytes).
pub const MAX_ACCELERATOR_BYTES: usize = 64;

/// One key pressed with an exact set of modifiers.
///
/// ```
/// use metis_core::input::{Accelerator, Key, Letter, Modifiers};
///
/// let save = Accelerator::parse_with_primary("CmdOrCtrl+Shift+S", Modifiers::CTRL)?;
/// assert_eq!(save.to_string(), "Ctrl+Shift+S");
/// assert_eq!(save.aria_keyshortcuts(), "Control+Shift+S");
/// let pressed = Accelerator::new(
///     Modifiers::NONE.with(Modifiers::CTRL, true).with(Modifiers::SHIFT, true),
///     Key::Letter(Letter::new(b's').expect("letter")),
/// );
/// assert_eq!(save, pressed);
/// # Ok::<(), metis_core::input::AcceleratorError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Accelerator {
    modifiers: Modifiers,
    key: Key,
}

impl Accelerator {
    /// Pairs a key with the modifiers that must be held.
    #[must_use]
    pub const fn new(modifiers: Modifiers, key: Key) -> Self {
        Self { modifiers, key }
    }

    /// The modifiers that must be held.
    #[must_use]
    pub const fn modifiers(self) -> Modifiers {
        self.modifiers
    }

    /// The key that must be pressed.
    #[must_use]
    pub const fn key(self) -> Key {
        self.key
    }

    /// Parses `+`-separated accelerator text, resolving `CmdOrCtrl` to
    /// [`Modifiers::host_primary`].
    ///
    /// # Errors
    /// Returns an [`AcceleratorError`] for empty, oversized, ambiguous or
    /// unknown text.
    pub fn parse(text: &str) -> Result<Self, AcceleratorError> {
        Self::parse_with_primary(text, Modifiers::host_primary())
    }

    /// Parses accelerator text with an explicit primary modifier, for hosts
    /// such as browsers that learn the platform only at run time.
    ///
    /// Modifier tokens are `Ctrl`/`Control`, `Alt`/`Option`, `Shift`,
    /// `Meta`/`Super`/`Cmd`/`Command` and `CmdOrCtrl`/`CommandOrControl`/
    /// `Primary`, all case-insensitive. Exactly one key follows them.
    ///
    /// # Errors
    /// Returns an [`AcceleratorError`] for empty, oversized, ambiguous or
    /// unknown text, or when `primary` is not exactly one modifier.
    pub fn parse_with_primary(text: &str, primary: Modifiers) -> Result<Self, AcceleratorError> {
        if text.is_empty() {
            return Err(AcceleratorError::Empty);
        }
        if text.len() > MAX_ACCELERATOR_BYTES {
            return Err(AcceleratorError::TooLong);
        }
        if !primary.is_single() {
            return Err(AcceleratorError::InvalidPrimary);
        }
        let mut modifiers = Modifiers::NONE;
        let mut key = None;
        for token in text.split('+') {
            if token.is_empty() {
                return Err(AcceleratorError::EmptyToken);
            }
            if let Some(flag) = modifier_token(token, primary) {
                if key.is_some() {
                    return Err(AcceleratorError::MultipleKeys);
                }
                if modifiers.contains(flag) {
                    return Err(AcceleratorError::DuplicateModifier);
                }
                modifiers = modifiers.union(flag);
            } else if key.is_some() {
                return Err(AcceleratorError::MultipleKeys);
            } else {
                key = Some(
                    Key::from_token(token)
                        .ok_or_else(|| AcceleratorError::UnknownToken(token.to_owned()))?,
                );
            }
        }
        key.map(|key| Self::new(modifiers, key))
            .ok_or(AcceleratorError::MissingKey)
    }

    /// The WAI-ARIA `aria-keyshortcuts` value announcing this accelerator.
    #[must_use]
    pub fn aria_keyshortcuts(self) -> String {
        let mut text = String::with_capacity(MAX_ACCELERATOR_BYTES / 2);
        // Writing into a `String` cannot fail.
        let _ = self.write_names(&mut text, Spelling::Aria);
        text
    }

    fn write_names(self, out: &mut impl fmt::Write, spelling: Spelling) -> fmt::Result {
        for (flag, display, aria) in Modifiers::ORDERED {
            if self.modifiers.contains(flag) {
                out.write_str(match spelling {
                    Spelling::Display => display,
                    Spelling::Aria => aria,
                })?;
                out.write_char('+')?;
            }
        }
        out.write_str(self.key.name())
    }
}

#[derive(Clone, Copy)]
enum Spelling {
    Display,
    Aria,
}

impl fmt::Display for Accelerator {
    /// Writes the canonical text form, which [`Accelerator::parse`] accepts.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_names(formatter, Spelling::Display)
    }
}

fn modifier_token(token: &str, primary: Modifiers) -> Option<Modifiers> {
    const TOKENS: [(&str, Option<Modifiers>); 11] = [
        ("Ctrl", Some(Modifiers::CTRL)),
        ("Control", Some(Modifiers::CTRL)),
        ("Alt", Some(Modifiers::ALT)),
        ("Option", Some(Modifiers::ALT)),
        ("Shift", Some(Modifiers::SHIFT)),
        ("Meta", Some(Modifiers::META)),
        ("Super", Some(Modifiers::META)),
        ("Cmd", Some(Modifiers::META)),
        ("Command", Some(Modifiers::META)),
        ("CmdOrCtrl", None),
        ("CommandOrControl", None),
    ];
    if token.eq_ignore_ascii_case("Primary") {
        return Some(primary);
    }
    TOKENS
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(token))
        .map(|(_, flag)| flag.unwrap_or(primary))
}
