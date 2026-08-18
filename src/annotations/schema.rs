use std::{fmt, ops::Range, path::Path};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourcePosition {
    pub byte: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceRange {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    pub source_range: SourceRange,
    pub quote: String,
    pub context_before: String,
    pub context_after: String,
    pub document_fingerprint: String,
    pub block_kind: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationStatus {
    Open,
    Resolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorState {
    Anchored,
    Orphaned,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub id: Uuid,
    pub status: AnnotationStatus,
    pub anchor_state: AnchorState,
    pub anchor: Anchor,
    pub comment: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SidecarDocument {
    pub id: Uuid,
    pub path: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Sidecar {
    pub schema_version: u32,
    pub document: SidecarDocument,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AnchorError;

impl fmt::Display for AnchorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the annotation selection is not a valid UTF-8 source range")
    }
}

impl std::error::Error for AnchorError {}

impl Annotation {
    pub fn new(anchor: Anchor, comment: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::now_v7(),
            status: AnnotationStatus::Open,
            anchor_state: AnchorState::Anchored,
            anchor,
            comment: comment.into(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    pub fn is_open(&self) -> bool {
        self.status == AnnotationStatus::Open
    }
}

impl Sidecar {
    pub fn new(document_path: &Path, fingerprint: impl Into<String>) -> Self {
        Self {
            schema_version: super::store::SIDECAR_SCHEMA_VERSION,
            document: SidecarDocument {
                id: Uuid::now_v7(),
                path: document_path.to_string_lossy().into_owned(),
                fingerprint: fingerprint.into(),
            },
            annotations: Vec::new(),
        }
    }
}

pub fn capture_anchor(
    source: &str,
    range: Range<usize>,
    document_fingerprint: impl Into<String>,
    block_kind: impl Into<String>,
) -> Result<Anchor, AnchorError> {
    if range.start >= range.end
        || range.end > source.len()
        || !source.is_char_boundary(range.start)
        || !source.is_char_boundary(range.end)
    {
        return Err(AnchorError);
    }

    Ok(Anchor {
        source_range: SourceRange {
            start: position_at(source, range.start),
            end: position_at(source, range.end),
        },
        quote: source[range.clone()].to_owned(),
        context_before: context_before(source, range.start),
        context_after: context_after(source, range.end),
        document_fingerprint: document_fingerprint.into(),
        block_kind: block_kind.into(),
    })
}

pub(crate) fn position_at(source: &str, byte: usize) -> SourcePosition {
    let prefix = &source[..byte];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.chars().count() + 1, |(_, line)| {
            line.chars().count() + 1
        });
    SourcePosition { byte, line, column }
}

fn context_before(source: &str, start: usize) -> String {
    let prefix = &source[..start];
    let boundary = nth_line_boundary_from_end(prefix, 2);
    cap_context_end(&prefix[boundary..])
}

fn context_after(source: &str, end: usize) -> String {
    let suffix = &source[end..];
    let mut line_ends = suffix.match_indices('\n').map(|(index, _)| index + 1);
    let boundary = line_ends.nth(1).unwrap_or(suffix.len());
    cap_context_start(&suffix[..boundary])
}

fn nth_line_boundary_from_end(text: &str, lines: usize) -> usize {
    let mut positions = text.match_indices('\n').map(|(index, _)| index + 1).rev();
    positions.nth(lines).unwrap_or(0)
}

fn cap_context_end(text: &str) -> String {
    if text.len() <= 512 {
        return text.to_owned();
    }
    let mut start = text.len() - 512;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    text[start..].to_owned()
}

fn cap_context_start(text: &str) -> String {
    if text.len() <= 512 {
        return text.to_owned();
    }
    let mut end = 512;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}
