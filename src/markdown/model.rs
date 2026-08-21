use std::ops::{Range, RangeInclusive};

/// A top-level Markdown construct that can be selected in rendered mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockKind {
    Heading,
    Paragraph,
    List,
    Table,
    Blockquote,
    Code,
    Footnote,
    Html,
    Math,
    ThematicBreak,
    Definition,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderStyle {
    pub bold: bool,
    pub italic: bool,
    pub crossed_out: bool,
    pub code: bool,
    pub link: bool,
    pub dim: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderSpan {
    pub text: String,
    pub style: RenderStyle,
}

impl RenderSpan {
    pub(crate) fn table_cell_boundary() -> Self {
        Self {
            text: String::new(),
            style: RenderStyle {
                dim: true,
                ..RenderStyle::default()
            },
        }
    }

    pub(crate) fn is_table_cell_boundary(&self) -> bool {
        self.text.is_empty()
            && self.style
                == RenderStyle {
                    dim: true,
                    ..RenderStyle::default()
                }
    }
}

/// A rendered block and its original UTF-8 source location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderBlock {
    pub kind: BlockKind,
    /// Markdown heading depth from 1 through 6, when this is a heading.
    pub heading_level: Option<u8>,
    /// Fence language used for syntax highlighting, when this is a code block.
    pub language: Option<String>,
    /// Half-open UTF-8 byte range in the source document.
    pub source_range: Range<usize>,
    /// Inclusive, one-based source lines.
    pub line_range: RangeInclusive<usize>,
    /// Terminal-safe text, one item per logical display line before wrapping.
    pub lines: Vec<String>,
    /// Inline semantic spans corresponding one-to-one with `lines`.
    pub styled_lines: Vec<Vec<RenderSpan>>,
}

/// The source-positioned terminal representation of a Markdown document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedDocument {
    pub blocks: Vec<RenderBlock>,
}

impl RenderedDocument {
    pub fn plain_text(&self) -> String {
        self.blocks
            .iter()
            .flat_map(|block| block.lines.iter())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn block_at_byte(&self, byte: usize) -> Option<&RenderBlock> {
        self.blocks
            .iter()
            .find(|block| block.source_range.start <= byte && byte < block.source_range.end)
    }
}
