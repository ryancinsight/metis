#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[cfg(not(target_arch = "wasm32"))]
pub mod asset;
pub mod catalog;
pub mod dom;
pub mod image;
pub mod layout;
pub mod parser;
pub mod semantics;
pub mod style;
pub mod transition;

pub use catalog::{
    Catalog, Locale, MAX_KEY_BYTES, MAX_LOCALE_TAG_BYTES, MAX_MESSAGE_SLOTS, MAX_VALUE_BYTES,
    Resolved, format_message,
};
pub use dom::{DomDocument, DomElement, DomNode, Edit};
pub use image::{AffineTransform, ImagePlacement, ImageSampling, ImageTransform, RasterImage};
pub use layout::{
    DecimalEdit, DecimalEntry, DisplayCommand, DisplayList, ElementIndex, LayoutViewport, LineCap,
    LineJoin, PathCapture, PathPush, PathTopology, PolylineError, Rect, ScrollAlign, ScrollbarAxis,
    ScrollbarDrag, ScrollbarGeometry, SegmentHit, StrokeWidth, VertexHit, VirtualList,
    VisibleWindow, compute_layout, compute_layout_into, nearest_polyline_segment,
    nearest_polyline_vertex, polyline_distance,
};
pub use parser::parse_markup;
pub use semantics::{
    MAX_SEMANTIC_ID_BYTES, MAX_SEMANTIC_TEXT_BYTES, SemanticAction, SemanticNode, SemanticRole,
    SemanticTree,
};
pub use style::{
    AlignItems, Color, ComputedStyle, Display, EdgeValues, FlexDirection, FlexGrow, FontWeight,
    JustifyContent, LinearGradient, Shadow, Size,
};
pub use transition::{Easing, MAX_TRANSITION_MILLIS, Transition, TransitionProperty};
