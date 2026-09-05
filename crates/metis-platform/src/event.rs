//! Application-supplied interaction events; no OS event pump is provided.

/// Input or lifecycle event supplied to a virtual surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformEvent {
    /// Pointer button press.
    PointerDown {
        /// Horizontal coordinate.
        x: i32,
        /// Vertical coordinate.
        y: i32,
    },
    /// Pointer button release.
    PointerUp {
        /// Horizontal coordinate.
        x: i32,
        /// Vertical coordinate.
        y: i32,
    },
    /// Pointer movement.
    PointerMove {
        /// Horizontal coordinate.
        x: i32,
        /// Vertical coordinate.
        y: i32,
    },
    /// Unicode character input.
    CharInput(char),
    /// Application-defined key code.
    KeyDown(u32),
    /// Requested surface dimensions; the receiver decides whether to resize.
    Resize {
        /// Horizontal pixel count.
        width: u32,
        /// Vertical pixel count.
        height: u32,
    },
    /// Request to close the application.
    Quit,
}
