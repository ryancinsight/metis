//! Format-neutral browser canvas input values and bounded handoff state.

mod queue;
#[cfg(test)]
mod tests;
mod types;

/// Maximum number of input events retained between browser callbacks and an
/// application frame.
///
/// The bound covers a burst of pointer samples without allowing a stalled
/// application loop to grow browser memory indefinitely.
pub const CANVAS_EVENT_CAPACITY: usize = 256;

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) use queue::CanvasEventQueue;
#[cfg(test)]
pub(super) use types::MAX_KEY_NAME_BYTES;
pub use types::{
    CanvasEvent, CanvasEventError, CanvasEventTrust, CanvasKeyboardEvent, CanvasKeyboardPhase,
    CanvasModifiers, CanvasPointerEvent, CanvasPointerPhase, CanvasPointerType, CanvasWheelEvent,
    CanvasWheelUnit,
};
