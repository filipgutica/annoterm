//! Markdown parsing and a terminal-friendly, source-positioned render model.

mod model;
mod parser;

pub use model::{BlockKind, RenderBlock, RenderSpan, RenderStyle, RenderedDocument};
pub use parser::parse;
