use annoterm::markdown::{BlockKind, parse};

#[test]
fn render_blocks_retain_half_open_utf8_source_ranges() {
    let source = "# Hi\n\nText with 🦀\n";
    let document = parse(source).expect("source parses");
    let heading = document
        .blocks
        .iter()
        .find(|block| block.kind == BlockKind::Heading)
        .expect("heading is mapped");
    let paragraph = document
        .blocks
        .iter()
        .find(|block| block.kind == BlockKind::Paragraph)
        .expect("paragraph is mapped");

    assert_eq!(&source[heading.source_range.clone()], "# Hi");
    assert_eq!(&source[paragraph.source_range.clone()], "Text with 🦀");
    assert_eq!(paragraph.line_range, 3..=3);
    assert_eq!(
        document
            .block_at_byte(paragraph.source_range.start)
            .unwrap()
            .kind,
        BlockKind::Paragraph
    );
}
