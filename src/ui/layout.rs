/// The responsive pane arrangement for the current terminal dimensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutMode {
    ThreePane,
    DocumentOnly,
    TooSmall,
}

pub fn layout(width: u16, height: u16) -> LayoutMode {
    if width < 30 || height < 6 {
        LayoutMode::TooSmall
    } else if width < 80 || height < 24 {
        LayoutMode::DocumentOnly
    } else {
        LayoutMode::ThreePane
    }
}
