use std::fmt::Write;

use crate::annotations::{AnchorState, Sidecar};

pub fn generate_prompt(sidecar: &Sidecar) -> String {
    let mut annotations = sidecar
        .annotations
        .iter()
        .filter(|annotation| annotation.is_open())
        .collect::<Vec<_>>();
    annotations.sort_by(
        |left, right| match (left.anchor_state, right.anchor_state) {
            (AnchorState::Anchored, AnchorState::Anchored) => left
                .anchor
                .source_range
                .start
                .byte
                .cmp(&right.anchor.source_range.start.byte)
                .then_with(|| left.id.cmp(&right.id)),
            (AnchorState::Anchored, AnchorState::Orphaned) => std::cmp::Ordering::Less,
            (AnchorState::Orphaned, AnchorState::Anchored) => std::cmp::Ordering::Greater,
            (AnchorState::Orphaned, AnchorState::Orphaned) => left
                .created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id)),
        },
    );

    let mut prompt = format!(
        "# Annoterm feedback\n\nDocument: `{}`\nFingerprint: `{}`\n\nUpdate the document to address every open annotation. Preserve unrelated content.\nKeep annotation identifiers in your response so the changes can be reviewed.\n",
        sidecar.document.path, sidecar.document.fingerprint,
    );
    if annotations.is_empty() {
        prompt.push_str("\nThere are no open annotations.\n");
        return prompt;
    }

    for annotation in annotations {
        let range = &annotation.anchor.source_range;
        let _ = write!(prompt, "\n## Annotation `{}`\n\n", annotation.id);
        match annotation.anchor_state {
            AnchorState::Anchored => {
                let _ = writeln!(
                    prompt,
                    "Location: lines {}–{}",
                    range.start.line, range.end.line
                );
            }
            AnchorState::Orphaned => {
                let _ = writeln!(
                    prompt,
                    "Location: orphaned; last known lines {}–{}",
                    range.start.line, range.end.line
                );
            }
        }
        prompt.push_str("State: ");
        prompt.push_str(match annotation.anchor_state {
            AnchorState::Anchored => "anchored",
            AnchorState::Orphaned => "orphaned",
        });
        prompt.push_str("\n\nSelected text:\n\n");
        for line in annotation.anchor.quote.lines() {
            let _ = writeln!(prompt, "> {line}");
        }
        if annotation.anchor.quote.ends_with('\n') {
            prompt.push_str(">\n");
        }
        let _ = write!(prompt, "\nComment:\n\n{}\n", annotation.comment);
    }
    prompt
}
