use std::{collections::BTreeMap, ops::Range};

use similar::{DiffTag, TextDiff};

use super::{
    AnchorState, Annotation, NavigationHint, SourceRange, capture_anchor, schema::position_at,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReanchorOutcome {
    Unchanged,
    Reanchored,
    Outdated,
    Orphaned,
}

/// Re-anchor only when an exact quote has one safe location. Ambiguous matches are preserved as
/// orphans so a stale comment can never attach to the wrong source.
pub fn reanchor_annotation(
    annotation: &mut Annotation,
    source: &str,
    document_fingerprint: &str,
) -> ReanchorOutcome {
    reanchor_annotation_with_snapshots(annotation, source, document_fingerprint, &BTreeMap::new())
}

pub fn reanchor_annotation_with_snapshots(
    annotation: &mut Annotation,
    source: &str,
    document_fingerprint: &str,
    snapshots: &BTreeMap<String, String>,
) -> ReanchorOutcome {
    let old_range = &annotation.anchor.source_range;
    if source_range_matches(source, old_range, &annotation.anchor.quote) {
        annotation.anchor_state = AnchorState::Anchored;
        annotation.navigation_hint = None;
        annotation.anchor.document_fingerprint = document_fingerprint.to_owned();
        return ReanchorOutcome::Unchanged;
    }

    let candidates = quote_candidates(source, &annotation.anchor.quote);
    let selected = match candidates.as_slice() {
        [] => None,
        [only] => Some(only.clone()),
        many => select_context_match(many, source, annotation),
    }
    .or_else(|| changed_quote_candidate(source, annotation));

    if let Some(range) = selected {
        let anchor = capture_anchor(
            source,
            range,
            document_fingerprint,
            annotation.anchor.block_kind.clone(),
        )
        .expect("quote candidates always identify valid source ranges");
        annotation.anchor = anchor;
        annotation.anchor_state = AnchorState::Anchored;
        annotation.navigation_hint = None;
        annotation.touch();
        return ReanchorOutcome::Reanchored;
    }

    let (base_range, snapshot) = annotation
        .navigation_hint
        .as_ref()
        .and_then(|hint| {
            snapshots
                .get(&hint.document_fingerprint)
                .map(|snapshot| (&hint.source_range, snapshot.as_str()))
        })
        .or_else(|| {
            snapshots
                .get(&annotation.anchor.document_fingerprint)
                .map(|snapshot| (&annotation.anchor.source_range, snapshot.as_str()))
        })
        .map_or(
            (&annotation.anchor.source_range, None),
            |(range, snapshot)| (range, Some(snapshot)),
        );
    let navigation_range = match snapshot {
        Some(snapshot) => map_source_range(snapshot, source, base_range),
        None => approximate_source_range(source, base_range),
    };

    let Some(source_range) = navigation_range else {
        annotation.anchor_state = AnchorState::Orphaned;
        annotation.navigation_hint = None;
        return ReanchorOutcome::Orphaned;
    };
    let hint = NavigationHint {
        source_range,
        document_fingerprint: document_fingerprint.to_owned(),
    };
    if annotation.anchor_state != AnchorState::Outdated
        || annotation.navigation_hint.as_ref() != Some(&hint)
    {
        annotation.touch();
    }
    annotation.anchor_state = AnchorState::Outdated;
    annotation.navigation_hint = Some(hint);
    ReanchorOutcome::Outdated
}

fn map_source_range(
    previous_source: &str,
    source: &str,
    previous_range: &SourceRange,
) -> Option<SourceRange> {
    let source_lines = source_line_ranges(source);
    if source_lines.is_empty() {
        return None;
    }
    let previous_start = previous_range.start.line.saturating_sub(1);
    let previous_end = final_included_line(previous_range).max(previous_start);
    let diff = TextDiff::from_lines(previous_source, source);
    let start_line = map_line(&diff, previous_start, false)?.min(source_lines.len() - 1);
    let end_line = map_line(&diff, previous_end, true)?
        .max(start_line)
        .min(source_lines.len() - 1);
    Some(source_range_for_lines(
        source,
        &source_lines,
        start_line..end_line + 1,
    ))
}

fn map_line(diff: &TextDiff<'_, '_, str>, line: usize, prefer_end: bool) -> Option<usize> {
    for (index, operation) in diff.ops().iter().enumerate() {
        let (tag, old, new) = operation.as_tag_tuple();
        if old.start <= line && line < old.end {
            if tag == DiffTag::Equal {
                return Some(new.start + line - old.start);
            }
            if !has_unchanged_boundary(diff, index, &old) {
                return None;
            }
            if new.is_empty() {
                return Some(new.start.saturating_sub(usize::from(prefer_end)));
            }
            return Some(if prefer_end { new.end - 1 } else { new.start });
        }
    }
    None
}

fn has_unchanged_boundary(
    diff: &TextDiff<'_, '_, str>,
    operation_index: usize,
    old: &Range<usize>,
) -> bool {
    diff.ops().iter().enumerate().any(|(index, operation)| {
        if index == operation_index || operation.tag() != DiffTag::Equal {
            return false;
        }
        let unchanged = operation.old_range();
        unchanged.end == old.start || unchanged.start == old.end
    })
}

fn approximate_source_range(source: &str, previous_range: &SourceRange) -> Option<SourceRange> {
    let lines = source_line_ranges(source);
    if lines.is_empty() {
        return None;
    }
    let start = previous_range
        .start
        .line
        .saturating_sub(1)
        .min(lines.len() - 1);
    let end = final_included_line(previous_range)
        .max(start)
        .min(lines.len() - 1);
    Some(source_range_for_lines(source, &lines, start..end + 1))
}

fn final_included_line(range: &SourceRange) -> usize {
    let end_line = range.end.line.saturating_sub(1);
    if range.end.byte > range.start.byte && range.end.column == 1 {
        end_line.saturating_sub(1)
    } else {
        end_line
    }
}

fn source_line_ranges(source: &str) -> Vec<Range<usize>> {
    let mut start = 0;
    source
        .split_inclusive('\n')
        .map(|line| {
            let range = start..start + line.len();
            start = range.end;
            range
        })
        .collect()
}

fn source_range_for_lines(
    source: &str,
    lines: &[Range<usize>],
    line_range: Range<usize>,
) -> SourceRange {
    let bytes = lines[line_range.start].start..lines[line_range.end - 1].end;
    SourceRange {
        start: position_at(source, bytes.start),
        end: position_at(source, bytes.end),
    }
}

fn source_range_matches(source: &str, range: &SourceRange, quote: &str) -> bool {
    range.start.byte <= range.end.byte
        && range.end.byte <= source.len()
        && source.is_char_boundary(range.start.byte)
        && source.is_char_boundary(range.end.byte)
        && source[range.start.byte..range.end.byte] == *quote
}

fn quote_candidates(source: &str, quote: &str) -> Vec<std::ops::Range<usize>> {
    if quote.is_empty() {
        return Vec::new();
    }
    source
        .match_indices(quote)
        .map(|(start, _)| start..start + quote.len())
        .collect()
}

fn select_context_match(
    candidates: &[std::ops::Range<usize>],
    source: &str,
    annotation: &Annotation,
) -> Option<std::ops::Range<usize>> {
    let mut scored = candidates
        .iter()
        .map(|candidate| {
            let before = &source[..candidate.start];
            let after = &source[candidate.end..];
            let score = usize::from(
                !annotation.anchor.context_before.is_empty()
                    && before.ends_with(&annotation.anchor.context_before),
            ) + usize::from(
                !annotation.anchor.context_after.is_empty()
                    && after.starts_with(&annotation.anchor.context_after),
            );
            (score, candidate.clone())
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.start.cmp(&right.1.start))
    });

    let (best_score, best) = scored.first()?;
    let runner_up_score = scored.get(1).map(|(score, _)| *score).unwrap_or_default();
    (*best_score > runner_up_score).then(|| best.clone())
}

fn changed_quote_candidate(
    source: &str,
    annotation: &Annotation,
) -> Option<std::ops::Range<usize>> {
    let before = &annotation.anchor.context_before;
    let after = &annotation.anchor.context_after;
    if before.is_empty() || after.is_empty() {
        return None;
    }

    let max_gap_bytes = annotation
        .anchor
        .quote
        .len()
        .saturating_mul(2)
        .saturating_add(256);
    let candidates = source
        .match_indices(before)
        .flat_map(|(before_start, _)| {
            let gap_start = before_start + before.len();
            source[gap_start..]
                .match_indices(after)
                .map(move |(relative_after_start, _)| gap_start..gap_start + relative_after_start)
        })
        .filter(|gap| !gap.is_empty() && gap.len() <= max_gap_bytes)
        .filter(|gap| quote_similarity(&source[gap.clone()], &annotation.anchor.quote) >= 0.85)
        .collect::<Vec<_>>();

    (candidates.len() == 1).then(|| candidates.into_iter().next().expect("length checked"))
}

fn quote_similarity(left: &str, right: &str) -> f64 {
    let left = left.chars().collect::<Vec<_>>();
    let right = right.chars().collect::<Vec<_>>();
    let maximum = left.len().max(right.len());
    if maximum == 0 {
        return 1.0;
    }

    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    for (left_index, left_character) in left.iter().enumerate() {
        let mut current = vec![left_index + 1];
        for (right_index, right_character) in right.iter().enumerate() {
            let substitution =
                previous[right_index] + usize::from(left_character != right_character);
            let insertion = current[right_index] + 1;
            let deletion = previous[right_index + 1] + 1;
            current.push(substitution.min(insertion).min(deletion));
        }
        previous = current;
    }
    1.0 - previous[right.len()] as f64 / maximum as f64
}
