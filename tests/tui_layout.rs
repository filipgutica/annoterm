use annoterm::ui::{LayoutMode, layout};

#[test]
fn layout_collapses_comments_panel_in_small_terminals() {
    assert_eq!(layout(120, 40), LayoutMode::ThreePane);
    assert_eq!(layout(70, 20), LayoutMode::DocumentOnly);
    assert_eq!(layout(20, 4), LayoutMode::TooSmall);
}
