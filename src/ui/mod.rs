//! Ratatui rendering and keyboard event handling.

mod layout;
mod terminal;

pub use layout::{LayoutMode, layout};
pub use terminal::run;
