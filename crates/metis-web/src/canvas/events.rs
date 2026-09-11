//! Format-neutral browser canvas input values and bounded handoff state.

#[cfg(any(target_arch = "wasm32", test))]
use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

/// Maximum number of input events retained between browser callbacks and an
/// application frame.
///
/// The bound covers a burst of pointer samples without allowing a stalled
/// application loop to grow browser memory indefinitely.
pub const CANVAS_EVENT_CAPACITY: usize = 256;

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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanvasPointerEvent {
    pub(crate) phase: CanvasPointerPhase,
    pub(crate) pointer_id: i32,
    pub(crate) pointer_type: CanvasPointerType,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) button: i16,
    pub(crate) buttons: u16,
    pub(crate) modifiers: CanvasModifiers,
    pub(crate) primary: bool,
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
    pub const fn x(self) -> i32 {
        self.x
    }

    /// Returns the vertical target-local CSS-pixel coordinate.
    #[must_use]
    pub const fn y(self) -> i32 {
        self.y
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
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) modifiers: CanvasModifiers,
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
    pub const fn x(self) -> i32 {
        self.x
    }

    /// Returns the vertical target-local CSS-pixel coordinate.
    #[must_use]
    pub const fn y(self) -> i32 {
        self.y
    }

    /// Returns the modifier-key snapshot.
    #[must_use]
    pub const fn modifiers(self) -> CanvasModifiers {
        self.modifiers
    }
}

/// One format-neutral event captured from a canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CanvasEvent {
    /// A pointer lifecycle event.
    Pointer(CanvasPointerEvent),
    /// A wheel event.
    Wheel(CanvasWheelEvent),
}

/// Failure reported by a bounded canvas event handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CanvasEventError {
    /// The callback queue reached its fixed capacity and was cleared.
    QueueOverflow,
    /// The browser delivered an event without the expected metadata.
    InvalidMetadata,
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
            Self::PointerCapture => "canvas pointer capture failed",
            Self::PointerLimit => "canvas active-pointer limit reached",
        };
        formatter.write_str(message)
    }
}

impl Error for CanvasEventError {}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) struct CanvasEventQueue {
    events: VecDeque<CanvasEvent>,
    error: Option<CanvasEventError>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl CanvasEventQueue {
    pub(crate) fn new() -> Self {
        Self {
            events: VecDeque::with_capacity(CANVAS_EVENT_CAPACITY),
            error: None,
        }
    }

    pub(crate) fn push(&mut self, event: CanvasEvent) -> bool {
        if self.error.is_some() {
            return false;
        }
        if self.events.len() == CANVAS_EVENT_CAPACITY {
            self.events.clear();
            self.error = Some(CanvasEventError::QueueOverflow);
            return false;
        }
        self.events.push_back(event);
        true
    }

    pub(crate) fn fail(&mut self, error: CanvasEventError) {
        self.events.clear();
        self.error.get_or_insert(error);
    }

    pub(crate) fn take(&mut self) -> Result<Box<[CanvasEvent]>, CanvasEventError> {
        let events = self.events.drain(..).collect::<Vec<_>>().into_boxed_slice();
        match self.error.take() {
            Some(error) => {
                drop(events);
                Err(error)
            }
            None => Ok(events),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CANVAS_EVENT_CAPACITY, CanvasEvent, CanvasEventError, CanvasEventQueue};

    #[test]
    fn queue_preserves_order_and_capacity() {
        let mut queue = CanvasEventQueue::new();
        for _ in 0..CANVAS_EVENT_CAPACITY {
            assert!(queue.push(CanvasEvent::Wheel(super::CanvasWheelEvent {
                delta_x: 0.0,
                delta_y: 1.0,
                delta_z: 0.0,
                unit: super::CanvasWheelUnit::Pixel,
                x: 2,
                y: 3,
                modifiers: super::CanvasModifiers::default(),
            })));
        }
        let events = queue.take().expect("queue capacity is valid");
        assert_eq!(events.len(), CANVAS_EVENT_CAPACITY);
        assert_eq!(events[0], events[CANVAS_EVENT_CAPACITY - 1]);
    }

    #[test]
    fn overflow_discards_stale_events_and_reports_once() {
        let mut queue = CanvasEventQueue::new();
        for _ in 0..=CANVAS_EVENT_CAPACITY {
            queue.push(CanvasEvent::Wheel(super::CanvasWheelEvent {
                delta_x: 0.0,
                delta_y: 1.0,
                delta_z: 0.0,
                unit: super::CanvasWheelUnit::Pixel,
                x: 0,
                y: 0,
                modifiers: super::CanvasModifiers::default(),
            }));
        }
        assert_eq!(queue.take(), Err(CanvasEventError::QueueOverflow));
        assert!(
            queue
                .take()
                .expect("queue recovers after reporting")
                .is_empty()
        );
    }

    #[test]
    fn failure_discards_pending_events_and_recovers() {
        let mut queue = CanvasEventQueue::new();
        queue.push(CanvasEvent::Wheel(super::CanvasWheelEvent {
            delta_x: 0.0,
            delta_y: 1.0,
            delta_z: 0.0,
            unit: super::CanvasWheelUnit::Pixel,
            x: 0,
            y: 0,
            modifiers: super::CanvasModifiers::default(),
        }));
        queue.fail(CanvasEventError::InvalidMetadata);
        assert_eq!(queue.take(), Err(CanvasEventError::InvalidMetadata));
        assert!(
            queue
                .take()
                .expect("queue recovers after failure")
                .is_empty()
        );
    }
}
