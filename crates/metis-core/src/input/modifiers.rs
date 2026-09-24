//! Modifier-key sets.

use std::fmt;

/// A set of held modifier keys.
///
/// The set is a closed four-bit value, so equality is exact: `Ctrl+K` does
/// not match an event that also holds Shift.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(u8);

impl Modifiers {
    /// No modifier held.
    pub const NONE: Self = Self(0);
    /// The Control key.
    pub const CTRL: Self = Self(1 << 0);
    /// The Alt key (Option on macOS).
    pub const ALT: Self = Self(1 << 1);
    /// The Shift key.
    pub const SHIFT: Self = Self(1 << 2);
    /// The Meta key (Command on macOS, Windows/Super elsewhere).
    pub const META: Self = Self(1 << 3);

    /// Canonical display order paired with each flag's display and ARIA names.
    pub(super) const ORDERED: [(Self, &'static str, &'static str); 4] = [
        (Self::CTRL, "Ctrl", "Control"),
        (Self::ALT, "Alt", "Alt"),
        (Self::SHIFT, "Shift", "Shift"),
        (Self::META, "Meta", "Meta"),
    ];

    /// The platform's primary command modifier: Command on Apple targets and
    /// Control elsewhere. `CmdOrCtrl` in accelerator text resolves to this.
    #[must_use]
    pub const fn host_primary() -> Self {
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            Self::META
        } else {
            Self::CTRL
        }
    }

    /// Adds `flag` when `held`, for building a set from the individual key
    /// states hosts report.
    #[must_use]
    pub const fn with(self, flag: Self, held: bool) -> Self {
        if held { self.union(flag) } else { self }
    }

    /// Whether every flag of `other` is held.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The union of both sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether no modifier is held.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether this is exactly one modifier flag.
    pub(super) const fn is_single(self) -> bool {
        self.0.is_power_of_two()
    }
}

impl fmt::Debug for Modifiers {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut list = formatter.debug_set();
        for (flag, name, _) in Self::ORDERED {
            if self.contains(flag) {
                list.entry(&format_args!("{name}"));
            }
        }
        list.finish()
    }
}
