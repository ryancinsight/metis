#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[cfg(not(target_arch = "wasm32"))]
pub mod asset;
pub mod dom;
pub mod image;
pub mod layout;
pub mod parser;
pub mod semantics;
pub mod style;

pub use dom::{DomDocument, DomElement, DomNode};
pub use image::{AffineTransform, ImagePlacement, ImageSampling, ImageTransform, RasterImage};
pub use layout::{
    DisplayCommand, DisplayList, LayoutViewport, LineCap, LineJoin, Rect, ScrollAlign, StrokeWidth,
    VirtualList, VisibleWindow, compute_layout,
};
pub use parser::parse_markup;
pub use semantics::{
    MAX_SEMANTIC_ID_BYTES, MAX_SEMANTIC_TEXT_BYTES, SemanticAction, SemanticNode, SemanticRole,
    SemanticTree,
};
pub use style::{
    AlignItems, Color, ComputedStyle, Display, EdgeValues, FlexDirection, FontWeight,
    JustifyContent, LinearGradient, Shadow, Size,
};
