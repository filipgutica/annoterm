use anyhow::{Result, anyhow};
use markdown::mdast::Node;
use unicode_width::UnicodeWidthStr;

use super::{BlockKind, RenderBlock, RenderSpan, RenderStyle, RenderedDocument};

/// Parse CommonMark plus GitHub Flavored Markdown and retain parser positions.
pub fn parse(source: &str) -> Result<RenderedDocument> {
    let mut options = markdown::ParseOptions::gfm();
    options.constructs.math_flow = true;
    options.constructs.math_text = true;
    let root = markdown::to_mdast(source, &options).map_err(|error| anyhow!(error.to_string()))?;
    let children = root
        .children()
        .ok_or_else(|| anyhow!("Markdown parser did not return a document root"))?;

    Ok(RenderedDocument {
        blocks: children.iter().filter_map(render_block).collect(),
    })
}

fn render_block(node: &Node) -> Option<RenderBlock> {
    let position = node.position()?;
    let language = match node {
        Node::Code(code) => code.lang.clone(),
        _ => None,
    };
    let heading_level = match node {
        Node::Heading(heading) => Some(heading.depth),
        _ => None,
    };
    let (kind, fallback_lines) = match node {
        Node::Heading(heading) => (BlockKind::Heading, vec![inline_children(&heading.children)]),
        Node::Paragraph(paragraph) => (
            BlockKind::Paragraph,
            lines(inline_children(&paragraph.children)),
        ),
        Node::List(list) => (BlockKind::List, render_list(list)),
        Node::Table(table) => (BlockKind::Table, render_table(table)),
        Node::Blockquote(quote) => (
            BlockKind::Blockquote,
            quote
                .children
                .iter()
                .flat_map(render_nested)
                .map(|line| format!("│ {line}"))
                .collect(),
        ),
        Node::Code(code) => render_code(code),
        Node::FootnoteDefinition(footnote) => (
            BlockKind::Footnote,
            vec![format!(
                "[^{0}] {1}",
                escape_terminal(&footnote.identifier),
                inline_children(&footnote.children)
            )],
        ),
        Node::Html(html) => (BlockKind::Html, lines(escape_terminal(&html.value))),
        Node::Math(math) => (
            BlockKind::Math,
            lines(format!("[math] {}", escape_terminal(&math.value))),
        ),
        Node::ThematicBreak(_) => (BlockKind::ThematicBreak, vec!["────────".into()]),
        Node::Definition(definition) => (
            BlockKind::Definition,
            vec![format!(
                "[link definition: {} — {}]",
                escape_terminal(&definition.identifier),
                escape_terminal(&definition.url)
            )],
        ),
        _ => return None,
    };
    let styled_lines = styled_block(node, &fallback_lines);
    let lines = styled_lines
        .iter()
        .map(|line| line.iter().map(|span| span.text.as_str()).collect())
        .collect();

    Some(RenderBlock {
        kind,
        heading_level,
        language,
        source_range: position.start.offset..position.end.offset,
        line_range: position.start.line..=position.end.line,
        lines,
        styled_lines,
    })
}

fn styled_block(node: &Node, fallback: &[String]) -> Vec<Vec<RenderSpan>> {
    match node {
        Node::Heading(heading) => styled_inline(&heading.children),
        Node::Paragraph(paragraph) => styled_inline(&paragraph.children),
        Node::List(list) => styled_list(list),
        Node::Table(table) => styled_table(table),
        Node::Blockquote(quote) => quote
            .children
            .iter()
            .flat_map(styled_nested)
            .map(|mut line| {
                line.insert(0, plain_span("│ "));
                line
            })
            .collect(),
        Node::FootnoteDefinition(footnote) => {
            let mut lines = footnote
                .children
                .iter()
                .flat_map(styled_nested)
                .collect::<Vec<_>>();
            if lines.is_empty() {
                lines.push(Vec::new());
            }
            lines[0].insert(
                0,
                plain_span(format!("[^{}] ", escape_terminal(&footnote.identifier))),
            );
            for line in lines.iter_mut().skip(1) {
                line.insert(0, plain_span("  "));
            }
            lines
        }
        _ => fallback
            .iter()
            .map(|line| vec![plain_span(line.clone())])
            .collect(),
    }
}

fn styled_nested(node: &Node) -> Vec<Vec<RenderSpan>> {
    match node {
        Node::Paragraph(paragraph) => styled_inline(&paragraph.children),
        Node::List(list) => styled_list(list),
        Node::Code(code) => render_code(code)
            .1
            .into_iter()
            .map(|line| vec![plain_span(line)])
            .collect(),
        _ => vec![vec![plain_span(inline(node))]],
    }
}

fn styled_list(list: &markdown::mdast::List) -> Vec<Vec<RenderSpan>> {
    list.children
        .iter()
        .flat_map(|node| {
            let Node::ListItem(item) = node else {
                return vec![vec![plain_span(inline(node))]];
            };
            let marker = match item.checked {
                Some(true) => "[x]",
                Some(false) => "[ ]",
                None if list.ordered => "1.",
                None => "-",
            };
            let mut content = item
                .children
                .iter()
                .flat_map(styled_nested)
                .collect::<Vec<_>>();
            if content.is_empty() {
                content.push(Vec::new());
            }
            for (index, line) in content.iter_mut().enumerate() {
                line.insert(
                    0,
                    plain_span(if index == 0 {
                        format!("{marker} ")
                    } else {
                        "  ".into()
                    }),
                );
            }
            content
        })
        .collect()
}

fn styled_table(table: &markdown::mdast::Table) -> Vec<Vec<RenderSpan>> {
    let rows = styled_table_cells(table);
    let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths = (0..column_count)
        .map(|column| {
            rows.iter()
                .filter_map(|row| row.get(column))
                .map(|cell| UnicodeWidthStr::width(cell_text(cell).as_str()))
                .max()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();

    rows.iter()
        .cloned()
        .map(|row| {
            let mut output = Vec::new();
            for (column, cell) in row.into_iter().enumerate() {
                let width = UnicodeWidthStr::width(cell_text(&cell).as_str());
                output.extend(cell);
                if column + 1 < column_count {
                    output.push(plain_span(
                        " ".repeat(widths[column].saturating_sub(width) + 2),
                    ));
                    output.push(RenderSpan::table_cell_boundary());
                }
            }
            output
        })
        .collect()
}

fn styled_table_cells(table: &markdown::mdast::Table) -> Vec<Vec<Vec<RenderSpan>>> {
    let mut rows = table
        .children
        .iter()
        .map(|row| match row {
            Node::TableRow(row) => row
                .children
                .iter()
                .map(|cell| match cell {
                    Node::TableCell(cell) => styled_inline(&cell.children)
                        .into_iter()
                        .flatten()
                        .collect(),
                    _ => vec![plain_span(inline(cell))],
                })
                .collect::<Vec<Vec<RenderSpan>>>(),
            _ => vec![vec![plain_span(inline(row))]],
        })
        .collect::<Vec<_>>();
    if let Some(header) = rows.first_mut() {
        for cell in header {
            for span in cell {
                span.style.bold = true;
            }
        }
    }
    rows
}

fn cell_text(cell: &[RenderSpan]) -> String {
    cell.iter().map(|span| span.text.as_str()).collect()
}

fn styled_inline(children: &[Node]) -> Vec<Vec<RenderSpan>> {
    let mut output = vec![Vec::new()];
    for child in children {
        append_inline(child, RenderStyle::default(), &mut output);
    }
    output
}

fn append_inline(node: &Node, inherited: RenderStyle, output: &mut Vec<Vec<RenderSpan>>) {
    match node {
        Node::Text(text) => push_styled(output, escape_terminal(&text.value), inherited),
        Node::InlineCode(code) => {
            let mut style = inherited;
            style.code = true;
            push_styled(output, escape_terminal(&code.value), style);
        }
        Node::InlineMath(math) => {
            let mut style = inherited;
            style.code = true;
            push_styled(
                output,
                format!("[math] {}", escape_terminal(&math.value)),
                style,
            );
        }
        Node::Emphasis(node) => {
            let mut style = inherited;
            style.italic = true;
            for child in &node.children {
                append_inline(child, style, output);
            }
        }
        Node::Strong(node) => {
            let mut style = inherited;
            style.bold = true;
            for child in &node.children {
                append_inline(child, style, output);
            }
        }
        Node::Delete(node) => {
            let mut style = inherited;
            style.crossed_out = true;
            for child in &node.children {
                append_inline(child, style, output);
            }
        }
        Node::Link(link) => {
            let mut link_style = inherited;
            link_style.link = true;
            for child in &link.children {
                append_inline(child, link_style, output);
            }
            let mut destination_style = link_style;
            destination_style.dim = true;
            push_styled(
                output,
                format!(" <{}>", escape_terminal(&link.url)),
                destination_style,
            );
        }
        Node::LinkReference(link) => {
            let mut link_style = inherited;
            link_style.link = true;
            for child in &link.children {
                append_inline(child, link_style, output);
            }
            let mut destination_style = link_style;
            destination_style.dim = true;
            push_styled(
                output,
                format!(" [{}]", escape_terminal(&link.identifier)),
                destination_style,
            );
        }
        Node::Break(_) => push_styled(output, "\n", inherited),
        _ => push_styled(output, inline(node), inherited),
    }
}

fn push_styled(output: &mut Vec<Vec<RenderSpan>>, text: impl AsRef<str>, style: RenderStyle) {
    for (index, part) in text.as_ref().split('\n').enumerate() {
        if index > 0 {
            output.push(Vec::new());
        }
        if part.is_empty() {
            continue;
        }
        let line = output.last_mut().expect("styled output always has a line");
        if let Some(previous) = line.last_mut()
            && previous.style == style
        {
            previous.text.push_str(part);
        } else {
            line.push(RenderSpan {
                text: part.into(),
                style,
            });
        }
    }
}

fn plain_span(text: impl Into<String>) -> RenderSpan {
    RenderSpan {
        text: text.into(),
        style: RenderStyle::default(),
    }
}

fn render_code(code: &markdown::mdast::Code) -> (BlockKind, Vec<String>) {
    let language = escape_terminal(code.lang.as_deref().unwrap_or("text"));
    let label = if language.eq_ignore_ascii_case("mermaid") {
        "Mermaid diagram source".into()
    } else {
        format!("code · {language}")
    };
    let mut lines = vec![label];
    lines.extend(code.value.lines().map(escape_terminal));
    (BlockKind::Code, lines)
}

fn render_nested(node: &Node) -> Vec<String> {
    match node {
        Node::Paragraph(paragraph) => lines(inline_children(&paragraph.children)),
        Node::List(list) => render_list(list),
        Node::Code(code) => render_code(code).1,
        _ => vec![inline(node)],
    }
}

fn render_list(list: &markdown::mdast::List) -> Vec<String> {
    list.children
        .iter()
        .flat_map(|node| {
            let Node::ListItem(item) = node else {
                return vec![inline(node)];
            };
            let marker = match item.checked {
                Some(true) => "[x]",
                Some(false) => "[ ]",
                None if list.ordered => "1.",
                None => "-",
            };
            let content = item
                .children
                .iter()
                .flat_map(render_nested)
                .collect::<Vec<_>>();
            content
                .into_iter()
                .enumerate()
                .map(|(index, line)| {
                    if index == 0 {
                        format!("{marker} {line}")
                    } else {
                        format!("  {line}")
                    }
                })
                .collect()
        })
        .collect()
}

fn render_table(table: &markdown::mdast::Table) -> Vec<String> {
    table
        .children
        .iter()
        .map(|row| match row {
            Node::TableRow(row) => row
                .children
                .iter()
                .map(|cell| match cell {
                    Node::TableCell(cell) => inline_children(&cell.children),
                    _ => inline(cell),
                })
                .collect::<Vec<_>>()
                .join(" │ "),
            _ => inline(row),
        })
        .collect()
}

fn inline_children(children: &[Node]) -> String {
    children.iter().map(inline).collect()
}

fn inline(node: &Node) -> String {
    match node {
        Node::Text(text) => escape_terminal(&text.value),
        Node::InlineCode(code) => escape_terminal(&code.value),
        Node::InlineMath(math) => format!("[math] {}", escape_terminal(&math.value)),
        Node::Emphasis(node) => inline_children(&node.children),
        Node::Strong(node) => inline_children(&node.children),
        Node::Delete(node) => inline_children(&node.children),
        Node::Link(link) => format!(
            "{} <{}>",
            inline_children(&link.children),
            escape_terminal(&link.url)
        ),
        Node::Image(image) => format!(
            "[image: {} — {}]",
            escape_terminal(&image.alt),
            escape_terminal(&image.url)
        ),
        Node::LinkReference(link) => format!(
            "{} [{}]",
            inline_children(&link.children),
            escape_terminal(&link.identifier)
        ),
        Node::ImageReference(image) => format!(
            "[image: {} — {}]",
            escape_terminal(&image.alt),
            escape_terminal(&image.identifier)
        ),
        Node::FootnoteReference(footnote) => {
            format!("[^{}]", escape_terminal(&footnote.identifier))
        }
        Node::Html(html) => escape_terminal(&html.value),
        Node::Break(_) => "\n".into(),
        _ => escape_terminal(&node.to_string()),
    }
}

fn lines(value: String) -> Vec<String> {
    let trailing_newline = value.ends_with('\n');
    value
        .lines()
        .map(str::to_owned)
        .chain(trailing_newline.then(String::new))
        .collect()
}

fn escape_terminal(value: &str) -> String {
    value
        .chars()
        .filter_map(|character| match character {
            '\u{1b}' => Some("␛".into()),
            '<' => Some("&lt;".into()),
            '>' => Some("&gt;".into()),
            character if character.is_control() && character != '\n' && character != '\t' => None,
            character => Some(character.to_string()),
        })
        .collect()
}
