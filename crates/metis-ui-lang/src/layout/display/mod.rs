//! Display list and the primitive commands a host executes in painter order.
//!
//! [`DisplayList`] owns the commands and the sparse element attribution a
//! paint-time transition addresses. [`DisplayCommand`] is the fixed-size
//! primitive both the software renderer and the Iris backend consume; a
//! command whose element declares a transition names that element through
//! [`ElementIndex`], so a host can repaint one element without laying the
//! document out again.

mod command;
mod element;
mod list;
mod render;

pub use command::DisplayCommand;
pub use element::ElementIndex;
pub use list::DisplayList;
