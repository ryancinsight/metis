//! Format-neutral browser canvas input values and bounded handoff state.

use std::error::Error;
use std::fmt;

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const MAX_KEY_NAME_BYTES: usize = 64;

/// Pointer device classification carried by a canvas event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CanvasPointerType {
    /// A mouse or mouse-like pointing device.
    Mouse,
    /// A pen or stylus device.
    Pen,
    /// A direct-touch device.
    Touch,
    /// A browser-defined device type.
    Other,
}

/// Pointer lifecycle phase carried by a canvas event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasPointerPhase {
    /// A pointer became active on the canvas.
    Down,
    /// An active pointer moved.
    Move,
    /// An active pointer was released.
    Up,
    /// The browser cancelled an active pointer.
    Cancel,
}

/// Keyboard lifecycle phase carried by a canvas event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasKeyboardPhase {
    /// A key became active.
    Down,
    /// A key was released.
    Up,
}

/// Browser provenance captured with a canvas event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CanvasEventTrust {
    /// The browser marked the event as trusted.
    Trusted,
    /// The browser marked the event as synthetic or otherwise untrusted.
    Untrusted,
}

impl From<bool> for CanvasEventTrust {
    fn from(value: bool) -> Self {
        if value {
            Self::Trusted
        } else {
            Self::Untrusted
        }
    }
}

impl CanvasEventTrust {
    /// Returns whether the browser marked the event as trusted.
    #[must_use]
    pub const fn is_trusted(self) -> bool {
        matches!(self, Self::Trusted)
    }
}

/// Modifier-key state captured with a canvas event.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CanvasModifiers {
    bits: u8,
}

impl CanvasModifiers {
    #[cfg(target_arch = "wasm32")]
    pub(crate) const CTRL: u8 = 1;
    #[cfg(target_arch = "wasm32")]
    pub(crate) const SHIFT: u8 = 2;
    #[cfg(target_arch = "wasm32")]
    pub(crate) const ALT: u8 = 4;
    #[cfg(target_arch = "wasm32")]
    pub(crate) const META: u8 = 8;

    #[cfg(target_arch = "wasm32")]
    pub(crate) const fn from_bits(bits: u8) -> Self {
        Self { bits }
    }

    /// Returns whether Control was held.
    #[must_use]
    pub const fn ctrl(self) -> bool {
        self.bits & 1 != 0
    }

    /// Returns whether Shift was held.
    #[must_use]
    pub const fn shift(self) -> bool {
        self.bits & 2 != 0
    }

    /// Returns whether Alt was held.
    #[must_use]
    pub const fn alt(self) -> bool {
        self.bits & 4 != 0
    }

    /// Returns whether Meta was held.
    #[must_use]
    pub const fn meta(self) -> bool {
        self.bits & 8 != 0
    }
}

/// One pointer event routed to a canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasPointerEvent {
    pub(crate) phase: CanvasPointerPhase,
    pub(crate) pointer_id: i32,
    pub(crate) pointer_type: CanvasPointerType,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) content_width: f64,
    pub(crate) content_height: f64,
    pub(crate) button: i16,
    pub(crate) buttons: u16,
    pub(crate) modifiers: CanvasModifiers,
    pub(crate) primary: bool,
    pub(crate) trust: CanvasEventTrust,
}

impl CanvasPointerEvent {
    /// Returns the pointer lifecycle phase.
    #[must_use]
    pub const fn phase(self) -> CanvasPointerPhase {
        self.phase
    }

    /// Returns the browser-assigned pointer identifier.
    #[must_use]
    pub const fn pointer_id(self) -> i32 {
        self.pointer_id
    }

    /// Returns the pointer device classification.
    #[must_use]
    pub const fn pointer_type(self) -> CanvasPointerType {
        self.pointer_type
    }

    /// Returns the horizontal target-local CSS-pixel coordinate.
    #[must_use]
    pub const fn x(self) -> f64 {
        self.x
    }

    /// Returns the vertical target-local CSS-pixel coordinate.
    #[must_use]
    pub const fn y(self) -> f64 {
        self.y
    }

    /// Returns the event-time untransformed content width in CSS pixels.
    #[must_use]
    pub const fn content_width(self) -> f64 {
        self.content_width
    }

    /// Returns the event-time untransformed content height in CSS pixels.
    #[must_use]
    pub const fn content_height(self) -> f64 {
        self.content_height
    }

    /// Returns the button changed by the event (`-1` when unavailable).
    #[must_use]
    pub const fn button(self) -> i16 {
        self.button
    }

    /// Returns the bitmask of buttons currently held down.
    #[must_use]
    pub const fn buttons(self) -> u16 {
        self.buttons
    }

    /// Returns the modifier-key snapshot.
    #[must_use]
    pub const fn modifiers(self) -> CanvasModifiers {
        self.modifiers
    }

    /// Returns whether this is the primary pointer for its device.
    #[must_use]
    pub const fn is_primary(self) -> bool {
        self.primary
    }

    /// Returns the browser provenance captured with this event.
    #[must_use]
    pub const fn trust(self) -> CanvasEventTrust {
        self.trust
    }

    /// Returns the browser trust snapshot captured with this event.
    #[must_use]
    pub const fn is_trusted(self) -> bool {
        self.trust.is_trusted()
    }
}

/// Unit used by a wheel event's deltas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CanvasWheelUnit {
    /// CSS pixels.
    Pixel,
    /// Browser content lines.
    Line,
    /// Browser content pages.
    Page,
    /// A browser-defined unit.
    Other,
}

/// One wheel event routed to a canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasWheelEvent {
    pub(crate) delta_x: f64,
    pub(crate) delta_y: f64,
    pub(crate) delta_z: f64,
    pub(crate) unit: CanvasWheelUnit,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) content_width: f64,
    pub(crate) content_height: f64,
    pub(crate) modifiers: CanvasModifiers,
    pub(crate) trust: CanvasEventTrust,
}

impl CanvasWheelEvent {
    /// Returns the horizontal wheel delta in [`Self::unit`] units.
    #[must_use]
    pub const fn delta_x(self) -> f64 {
        self.delta_x
    }

    /// Returns the vertical wheel delta in [`Self::unit`] units.
    #[must_use]
    pub const fn delta_y(self) -> f64 {
        self.delta_y
    }

    /// Returns the depth wheel delta in [`Self::unit`] units.
    #[must_use]
    pub const fn delta_z(self) -> f64 {
        self.delta_z
    }

    /// Returns the browser unit for the deltas.
    #[must_use]
    pub const fn unit(self) -> CanvasWheelUnit {
        self.unit
    }

    /// Returns the horizontal target-local CSS-pixel coordinate.
    #[must_use]
    pub const fn x(self) -> f64 {
        self.x
    }

    /// Returns the vertical target-local CSS-pixel coordinate.
    #[must_use]
    pub const fn y(self) -> f64 {
        self.y
    }

    /// Returns the event-time untransformed content width in CSS pixels.
    #[must_use]
    pub const fn content_width(self) -> f64 {
        self.content_width
    }

    /// Returns the event-time untransformed content height in CSS pixels.
    #[must_use]
    pub const fn content_height(self) -> f64 {
        self.content_height
    }

    /// Returns the modifier-key snapshot.
    #[must_use]
    pub const fn modifiers(self) -> CanvasModifiers {
        self.modifiers
    }

    /// Returns the browser provenance captured with this event.
    #[must_use]
    pub const fn trust(self) -> CanvasEventTrust {
        self.trust
    }

    /// Returns the browser trust snapshot captured with this event.
    #[must_use]
    pub const fn is_trusted(self) -> bool {
        self.trust.is_trusted()
    }
}

/// One keyboard event routed to a canvas.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasKeyboardEvent {
    phase: CanvasKeyboardPhase,
    key: Box<str>,
    code: Box<str>,
    repeated: bool,
    modifiers: CanvasModifiers,
    trust: CanvasEventTrust,
}

impl CanvasKeyboardEvent {
    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn try_new(
        phase: CanvasKeyboardPhase,
        key: String,
        code: String,
        repeated: bool,
        modifiers: CanvasModifiers,
        trust: CanvasEventTrust,
    ) -> Result<Self, CanvasEventError> {
        if key.len() > MAX_KEY_NAME_BYTES || code.len() > MAX_KEY_NAME_BYTES {
            return Err(CanvasEventError::InvalidMetadata);
        }
        Ok(Self {
            phase,
            key: key.into_boxed_str(),
            code: code.into_boxed_str(),
            repeated,
            modifiers,
            trust,
        })
    }

    /// Returns the keyboard lifecycle phase.
    #[must_use]
    pub const fn phase(&self) -> CanvasKeyboardPhase {
        self.phase
    }

    /// Returns the bounded browser key value.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the bounded physical key code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns whether the browser marked this keydown as an auto-repeat.
    #[must_use]
    pub const fn is_repeated(&self) -> bool {
        self.repeated
    }

    /// Returns the modifier-key snapshot.
    #[must_use]
    pub const fn modifiers(&self) -> CanvasModifiers {
        self.modifiers
    }

    /// Returns the browser provenance captured with this event.
    #[must_use]
    pub const fn trust(&self) -> CanvasEventTrust {
        self.trust
    }

    /// Returns the browser trust snapshot captured with this event.
    #[must_use]
    pub const fn is_trusted(&self) -> bool {
        self.trust.is_trusted()
    }
}

/// One format-neutral event captured from a canvas.
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasEvent {
    /// A pointer lifecycle event.
    Pointer(CanvasPointerEvent),
    /// A wheel event.
    Wheel(CanvasWheelEvent),
    /// A keyboard lifecycle event.
    Keyboard(CanvasKeyboardEvent),
}

impl CanvasEvent {
    /// Returns the browser provenance captured with this event.
    #[must_use]
    pub const fn trust(&self) -> CanvasEventTrust {
        match self {
            Self::Pointer(event) => event.trust(),
            Self::Wheel(event) => event.trust(),
            Self::Keyboard(event) => event.trust(),
        }
    }

    /// Returns the browser trust snapshot captured with this event.
    #[must_use]
    pub const fn is_trusted(&self) -> bool {
        self.trust().is_trusted()
    }
}

/// Failure reported by a bounded canvas event handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CanvasEventError {
    /// The callback queue reached its fixed capacity and was cleared.
    QueueOverflow,
    /// The browser delivered an event without the expected metadata.
    InvalidMetadata,
    /// The browser could not map viewport input into local content coordinates.
    LocalCoordinates,
    /// Pointer capture could not be established or released.
    PointerCapture,
    /// The bounded active-pointer table is full.
    PointerLimit,
}

impl fmt::Display for CanvasEventError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::QueueOverflow => "canvas input queue overflowed",
            Self::InvalidMetadata => "canvas event metadata was unavailable",
            Self::LocalCoordinates => "canvas local content coordinates were unavailable",
            Self::PointerCapture => "canvas pointer capture failed",
            Self::PointerLimit => "canvas active-pointer limit reached",
        };
        formatter.write_str(message)
    }
}

impl Error for CanvasEventError {}
