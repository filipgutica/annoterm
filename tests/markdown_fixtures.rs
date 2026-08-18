use annoterm::markdown::{BlockKind, parse};

#[test]
fn renders_commonmark_and_gfm_blocks() {
    let document = parse(
        "# Title\n\n- [x] done\n- [ ] next\n\n> Read [the guide](guide.md).\n\n| Name | Value |\n| --- | --- |\n| A | B |\n\n```rust\nfn main() {}\n```\n\n[^note]: Footnote\n",
    )
    .expect("fixture parses");

    assert!(
        document
            .blocks
            .iter()
            .any(|block| block.kind == BlockKind::Heading)
    );
    assert!(
        document
            .blocks
            .iter()
            .any(|block| block.kind == BlockKind::List)
    );
    assert!(
        document
            .blocks
            .iter()
            .any(|block| block.kind == BlockKind::Table)
    );
    assert!(
        document
            .blocks
            .iter()
            .any(|block| block.kind == BlockKind::Footnote)
    );
    assert!(document.blocks.iter().any(|block| {
        block.kind == BlockKind::Code && block.language.as_deref() == Some("rust")
    }));
    assert!(document.plain_text().contains("[x] done"));
    assert!(document.plain_text().contains("Read the guide <guide.md>."));
}

#[test]
fn renders_explicit_terminal_fallbacks() {
    let document = parse(
        "<details>unsafe</details>\n\n![diagram](diagram.png)\n\n$x^2$\n\n```mermaid\ngraph TD\n```\n",
    )
    .expect("fixture parses");
    let rendered = document.plain_text();

    assert!(rendered.contains("&lt;details&gt;unsafe&lt;/details&gt;"));
    assert!(rendered.contains("[image: diagram — diagram.png]"));
    assert!(rendered.contains("[math] x^2"));
    assert!(rendered.contains("Mermaid diagram source"));
}

#[test]
fn rendered_metadata_cannot_inject_terminal_escapes() {
    let document = parse("~~~ru\u{1b}st\ncode\n~~~\n\n$bad\u{1b}[2J$\n").unwrap();

    assert!(!document.plain_text().contains('\u{1b}'));
    assert!(document.plain_text().contains('␛'));
}

#[test]
fn rendered_text_hides_markdown_formatting_delimiters() {
    let document = parse("# Title\n\nThis is **bold**, *italic*, and `code`.\n").unwrap();
    let rendered = document.plain_text();

    assert!(rendered.contains("Title"));
    assert!(rendered.contains("This is bold, italic, and code."));
    assert!(!rendered.contains("# Title"));
    assert!(!rendered.contains("**bold**"));
    assert!(!rendered.contains("`code`"));
}

#[test]
fn heading_blocks_retain_their_markdown_depth() {
    let document = parse("# One\n\n### Three\n\n###### Six\n").unwrap();
    let levels = document
        .blocks
        .iter()
        .filter(|block| block.kind == BlockKind::Heading)
        .map(|block| block.heading_level)
        .collect::<Vec<_>>();

    assert_eq!(levels, vec![Some(1), Some(3), Some(6)]);
}

#[test]
fn terminal_tables_and_code_blocks_use_clean_layouts() {
    let document =
        parse("| Key | Action |\n| --- | --- |\n| Ctrl+R | Render |\n\n```sh\ncargo build\n```\n")
            .unwrap();
    let rendered = document.plain_text();

    assert!(rendered.contains("Key     Action"));
    assert!(!rendered.contains('│'));
    assert!(rendered.contains("code · sh\ncargo build"));
    assert!(!rendered.contains('┌'));
    assert!(!rendered.contains('└'));
}
