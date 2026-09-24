//! A window's restorable geometry and its bounded text encoding.
//!
//! This is the counterpart of Tauri's window-state plugin. The host saves a
//! [`WindowState`] when a window closes and restores it when the window next
//! opens. The value is platform-neutral: native providers convert it to their
//! own placement type, and the text form is what a host writes to disk. The
//! decoder accepts only the exact form the encoder writes, so a truncated,
//! edited or foreign file is rejected rather than half-applied.

use std::{error::Error, fmt};

/// Largest coordinate magnitude accepted for a restored window's corner.
pub const MAX_WINDOW_STATE_COORDINATE: i32 = 1 << 16;
/// Largest restored window width or height.
pub const MAX_WINDOW_STATE_DIMENSION: u32 = 16_384;
/// Largest encoded window state accepted by [`WindowState::decode`].
pub const MAX_WINDOW_STATE_BYTES: usize = 256;

const HEADER: &str = "metis-window-state 1";
const KEYS: [&str; 5] = ["left", "top", "width", "height", "maximized"];

/// Why a window state was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WindowStateError {
    /// A coordinate or dimension is outside its bound.
    OutOfRange,
    /// The encoded text is not the form [`WindowState::encode`] writes.
    Malformed,
    /// The encoded text exceeds [`MAX_WINDOW_STATE_BYTES`].
    TooLarge,
}

impl fmt::Display for WindowStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OutOfRange => "window state is outside its bounds",
            Self::Malformed => "window state encoding is malformed",
            Self::TooLarge => "window state encoding exceeds its bound",
        })
    }
}

impl Error for WindowStateError {}

/// A window's restored outer rectangle and maximized flag.
///
/// The rectangle is the window's size and position when not maximized, so a
/// maximized window still restores to where the user last placed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowState {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    maximized: bool,
}

impl WindowState {
    /// Validates a restored rectangle and maximized flag.
    ///
    /// # Errors
    /// Returns [`WindowStateError::OutOfRange`] when a corner exceeds
    /// [`MAX_WINDOW_STATE_COORDINATE`] in magnitude or a dimension is zero or
    /// exceeds [`MAX_WINDOW_STATE_DIMENSION`].
    pub const fn new(
        left: i32,
        top: i32,
        width: u32,
        height: u32,
        maximized: bool,
    ) -> Result<Self, WindowStateError> {
        let bound = MAX_WINDOW_STATE_COORDINATE.unsigned_abs();
        if left.unsigned_abs() > bound
            || top.unsigned_abs() > bound
            || width == 0
            || height == 0
            || width > MAX_WINDOW_STATE_DIMENSION
            || height > MAX_WINDOW_STATE_DIMENSION
        {
            return Err(WindowStateError::OutOfRange);
        }
        Ok(Self {
            left,
            top,
            width,
            height,
            maximized,
        })
    }

    /// Left edge of the restored rectangle.
    #[must_use]
    pub const fn left(&self) -> i32 {
        self.left
    }

    /// Top edge of the restored rectangle.
    #[must_use]
    pub const fn top(&self) -> i32 {
        self.top
    }

    /// Width of the restored rectangle.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Height of the restored rectangle.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Whether the window reopens maximized.
    #[must_use]
    pub const fn maximized(&self) -> bool {
        self.maximized
    }

    /// Encodes the state as a short line-oriented text record.
    #[must_use]
    pub fn encode(&self) -> String {
        format!(
            "{HEADER}\nleft={}\ntop={}\nwidth={}\nheight={}\nmaximized={}\n",
            self.left,
            self.top,
            self.width,
            self.height,
            u8::from(self.maximized),
        )
    }

    /// Decodes a record written by [`Self::encode`].
    ///
    /// # Errors
    /// Returns [`WindowStateError::TooLarge`] for text over
    /// [`MAX_WINDOW_STATE_BYTES`], [`WindowStateError::Malformed`] for any
    /// other form, and [`WindowStateError::OutOfRange`] for decoded values
    /// outside the bounds of [`Self::new`].
    pub fn decode(text: &str) -> Result<Self, WindowStateError> {
        if text.len() > MAX_WINDOW_STATE_BYTES {
            return Err(WindowStateError::TooLarge);
        }
        let body = text.strip_suffix('\n').ok_or(WindowStateError::Malformed)?;
        let mut lines = body.split('\n');
        if lines.next() != Some(HEADER) {
            return Err(WindowStateError::Malformed);
        }
        let mut values = [""; KEYS.len()];
        for (key, value) in KEYS.iter().zip(&mut values) {
            *value = lines
                .next()
                .and_then(|line| line.strip_prefix(key))
                .and_then(|line| line.strip_prefix('='))
                .ok_or(WindowStateError::Malformed)?;
        }
        if lines.next().is_some() {
            return Err(WindowStateError::Malformed);
        }
        let [left, top, width, height, maximized] = values;
        let maximized = match maximized {
            "0" => false,
            "1" => true,
            _ => return Err(WindowStateError::Malformed),
        };
        Self::new(
            decimal(left)?,
            decimal(top)?,
            decimal(width)?,
            decimal(height)?,
            maximized,
        )
    }
}

/// Parses a canonical decimal: no sign on zero, no leading `+` or zeros.
fn decimal<T: std::str::FromStr>(text: &str) -> Result<T, WindowStateError> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    let canonical = !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
        && text != "-0";
    if !canonical {
        return Err(WindowStateError::Malformed);
    }
    text.parse().map_err(|_| WindowStateError::OutOfRange)
}

#[cfg(test)]
mod tests;
