use super::{AnchorState, Annotation, SourceRange, capture_anchor};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReanchorOutcome {
    Unchanged,
    Reanchored,
    Orphaned,
}

/// Re-anchor only when an exact quote has one safe location. Ambiguous matches are preserved as
/// orphans so a stale comment can never attach to the wrong source.
pub fn reanchor_annotation(
    annotation: &mut Annotation,
    source: &str,
    document_fingerprint: &str,
) -> ReanchorOutcome {
    let old_range = &annotation.anchor.source_range;
    if source_range_matches(source, old_range, &annotation.anchor.quote) {
        annotation.anchor_state = AnchorState::Anchored;
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

    let Some(range) = selected else {
        annotation.anchor_state = AnchorState::Orphaned;
        return ReanchorOutcome::Orphaned;
    };

    let anchor = capture_anchor(
        source,
        range,
        document_fingerprint,
        annotation.anchor.block_kind.clone(),
    )
    .expect("quote candidates always identify valid source ranges");
    annotation.anchor = anchor;
    annotation.anchor_state = AnchorState::Anchored;
    annotation.touch();
    ReanchorOutcome::Reanchored
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
