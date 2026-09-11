//! Format-neutral raster presentation at the browser host boundary.

mod events;
mod frame;

#[cfg(target_arch = "wasm32")]
mod surface;

pub use events::{
    CANVAS_EVENT_CAPACITY, CanvasEvent, CanvasEventError, CanvasModifiers, CanvasPointerEvent,
    CanvasPointerPhase, CanvasPointerType, CanvasWheelEvent, CanvasWheelUnit,
};
pub use frame::CanvasFrame;

#[cfg(target_arch = "wasm32")]
pub use surface::CanvasSurface;
