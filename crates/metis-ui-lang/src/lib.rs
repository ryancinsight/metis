#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod dom;
pub mod layout;
pub mod parser;
pub mod style;

pub use dom::{DomDocument, DomElement, DomNode};
pub use layout::{DisplayCommand, DisplayList, Rect, compute_layout};
pub use parser::parse_markup;
pub use style::{
    AlignItems, Color, ComputedStyle, Display, EdgeValues, FlexDirection, FontWeight,
    JustifyContent, Size,
};
