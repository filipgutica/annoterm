use annoterm::feedback::{Clipboard, ClipboardStatus};

#[test]
fn clipboard_reports_unavailable_when_no_backend_can_run() {
    let clipboard = Clipboard::with_commands([]);

    assert_eq!(clipboard.copy("feedback"), ClipboardStatus::Unavailable);
}
