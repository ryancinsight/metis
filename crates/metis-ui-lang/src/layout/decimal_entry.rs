//! Bounded decimal text editing for controls that validate on submission.

/// The visible result of typing a character in a [`DecimalEntry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
#[non_exhaustive]
pub enum DecimalEdit {
    /// The character was appended.
    Appended,
    /// The character cannot occur in decimal notation.
    Ignored,
    /// Appending the character would exceed the field's byte budget.
    Full,
}

/// An editable buffer for ASCII decimal notation, including exponents.
///
/// Intermediate text can be incomplete. The caller parses and validates the
/// value when its control submits, and owns any range or unit policy.
///
/// ```
/// use metis_ui_lang::{DecimalEdit, DecimalEntry};
///
/// let mut entry = DecimalEntry::<8>::new();
/// assert_eq!(entry.insert('3'), DecimalEdit::Appended);
/// assert_eq!(entry.insert('.'), DecimalEdit::Appended);
/// assert_eq!(entry.insert('5'), DecimalEdit::Appended);
/// assert_eq!(entry.as_str().parse::<f32>(), Ok(3.5));
/// ```
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DecimalEntry<const MAX_BYTES: usize> {
    text: String,
}

impl<const MAX_BYTES: usize> DecimalEntry<MAX_BYTES> {
    /// Starts an empty field with space for up to `MAX_BYTES` ASCII characters.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            text: String::new(),
        }
    }

    /// Appends one digit, sign, decimal point, or exponent marker.
    pub fn insert(&mut self, character: char) -> DecimalEdit {
        if !character.is_ascii_digit() && !matches!(character, '.' | '+' | '-' | 'e' | 'E') {
            return DecimalEdit::Ignored;
        }
        if self.text.len() == MAX_BYTES {
            return DecimalEdit::Full;
        }
        self.text.push(character);
        DecimalEdit::Appended
    }

    /// Removes the last character and reports whether the text changed.
    pub fn backspace(&mut self) -> bool {
        self.text.pop().is_some()
    }

    /// Borrows the current text for drawing or validation on submission.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

#[cfg(test)]
mod tests {
    use super::{DecimalEdit, DecimalEntry};

    #[test]
    fn entry_preserves_partial_exponents_and_bounds_edits() {
        let mut entry = DecimalEntry::<4>::new();
        assert!(!entry.backspace());
        for character in ['2', 'e', '-', '1'] {
            assert_eq!(entry.insert(character), DecimalEdit::Appended);
        }
        assert_eq!(entry.insert('λ'), DecimalEdit::Ignored);
        assert_eq!(entry.insert('2'), DecimalEdit::Full);
        assert_eq!(entry.as_str(), "2e-1");
        assert!(entry.backspace());
        assert_eq!(entry.as_str(), "2e-");
        assert_eq!(entry.insert('2'), DecimalEdit::Appended);
        assert_eq!(entry.as_str(), "2e-2");
    }
}
