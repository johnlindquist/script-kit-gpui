use super::super::types::{MatchEvidence, MatchEvidenceField};
use super::{find_ignore_ascii_case, is_word_boundary_match, NucleoCtx};
pub(crate) use sk_protocol::search_primitives::{
    byte_range_for_char_indices, char_indices_for_span, match_tier_from_score,
    query_meets_min_query_chars, score_from_tier,
};

pub(crate) const TIER_EXACT_PRIMARY: i32 = 1000;
pub(crate) const TIER_PREFIX_PRIMARY: i32 = 950;
pub(crate) const TIER_WORD_BOUNDARY_PRIMARY: i32 = 900;
pub(crate) const TIER_SUBSTRING_PRIMARY: i32 = 850;
pub(crate) const TIER_ACRONYM_PRIMARY: i32 = 800;
pub(crate) const TIER_COMPACT_FUZZY_PRIMARY: i32 = 700;
pub(crate) const TIER_ALIAS: i32 = 650;
pub(crate) const TIER_KEYWORD: i32 = 550;
pub(crate) const TIER_DESCRIPTION: i32 = 450;
pub(crate) const TIER_FILENAME: i32 = 375;
pub(crate) const TIER_PATH: i32 = 250;
pub(crate) const TIER_BODY: i32 = 150;

pub(crate) const MIN_PRIMARY_FUZZY_QUERY_LEN: usize = 4;
pub(crate) const MIN_BODY_EXACT_QUERY_LEN: usize = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TextMatchKind {
    Exact,
    Prefix,
    WordBoundary,
    Substring,
    Acronym,
    CompactFuzzy,
}

#[derive(Clone, Debug)]
pub(crate) struct TextMatch {
    pub(crate) kind: TextMatchKind,
    pub(crate) tier: i32,
    pub(crate) score: i32,
    pub(crate) indices: Vec<usize>,
}

pub(crate) fn exact_substring_match(
    haystack: &str,
    query_lower: &str,
    tier: i32,
) -> Option<TextMatch> {
    if query_lower.is_empty() {
        return None;
    }

    let indices = substring_indices(haystack, query_lower)?;
    let start = *indices.first()?;
    let kind = if haystack.chars().count() == indices.len() {
        TextMatchKind::Exact
    } else if start == 0 {
        TextMatchKind::Prefix
    } else if char_index_is_word_start(haystack, start) {
        TextMatchKind::WordBoundary
    } else {
        TextMatchKind::Substring
    };
    let adjusted_tier = match kind {
        TextMatchKind::Exact => tier.max(TIER_EXACT_PRIMARY),
        TextMatchKind::Prefix => tier.max(TIER_PREFIX_PRIMARY),
        TextMatchKind::WordBoundary => tier.max(TIER_WORD_BOUNDARY_PRIMARY),
        TextMatchKind::Substring => tier,
        TextMatchKind::Acronym | TextMatchKind::CompactFuzzy => tier,
    };

    Some(TextMatch {
        kind,
        tier: adjusted_tier,
        score: score_from_tier(
            adjusted_tier,
            900usize.saturating_sub(start).min(900) as i32,
        ),
        indices,
    })
}

pub(crate) fn low_tier_substring_match(
    haystack: &str,
    query_lower: &str,
    tier: i32,
) -> Option<TextMatch> {
    let indices = substring_indices(haystack, query_lower)?;
    let start = *indices.first()?;
    Some(TextMatch {
        kind: if start == 0 {
            TextMatchKind::Prefix
        } else {
            TextMatchKind::Substring
        },
        tier,
        score: score_from_tier(tier, 900usize.saturating_sub(start).min(900) as i32),
        indices,
    })
}

pub(crate) fn normalized_substring_match(
    haystack: &str,
    query_lower: &str,
    tier: i32,
) -> Option<TextMatch> {
    if let Some(exact) = low_tier_substring_match(haystack, query_lower, tier) {
        return Some(exact);
    }
    let indices = normalized_indices_for_query(haystack, query_lower)?;
    Some(TextMatch {
        kind: TextMatchKind::Substring,
        tier,
        score: score_from_tier(tier, 900),
        indices,
    })
}

pub(crate) fn primary_text_match(
    haystack: &str,
    query_lower: &str,
    nucleo: &mut NucleoCtx,
) -> Option<TextMatch> {
    if let Some(exact) = exact_substring_match(haystack, query_lower, TIER_SUBSTRING_PRIMARY) {
        return Some(exact);
    }

    if query_lower.chars().count() < MIN_PRIMARY_FUZZY_QUERY_LEN {
        return None;
    }

    let score = nucleo.score(haystack)?;
    let indices = nucleo.indices(haystack)?;
    if indices.is_empty() {
        return None;
    }

    let query_len = query_lower.chars().count();
    let first = *indices.first()?;
    let last = *indices.last()?;
    let span = last.saturating_sub(first).saturating_add(1);
    if span <= query_len.saturating_add(1) {
        let tier = TIER_COMPACT_FUZZY_PRIMARY;
        return Some(TextMatch {
            kind: TextMatchKind::CompactFuzzy,
            tier,
            score: score_from_tier(tier, (score / 20).min(999) as i32),
            indices,
        });
    }

    if fuzzy_indices_are_structured_abbreviation(haystack, &indices) {
        let tier = TIER_ACRONYM_PRIMARY;
        return Some(TextMatch {
            kind: TextMatchKind::Acronym,
            tier,
            score: score_from_tier(tier, (score / 20).min(999) as i32),
            indices,
        });
    }

    None
}

pub(crate) fn better_match(current: &mut Option<TextMatch>, candidate: Option<TextMatch>) {
    let Some(candidate) = candidate else {
        return;
    };
    let replace = match current {
        None => true,
        Some(existing) => {
            candidate.tier > existing.tier
                || (candidate.tier == existing.tier && candidate.score > existing.score)
        }
    };
    if replace {
        *current = Some(candidate);
    }
}

pub(crate) fn match_evidence(
    field: MatchEvidenceField,
    text: &str,
    candidate: Option<TextMatch>,
) -> Option<MatchEvidence> {
    candidate.map(|candidate| MatchEvidence {
        field,
        text: text.to_string(),
        indices: candidate.indices,
        tier: candidate.tier,
        score: candidate.score,
    })
}

pub(crate) fn better_match_evidence(
    current: &mut Option<MatchEvidence>,
    candidate: Option<MatchEvidence>,
) {
    let Some(candidate) = candidate else {
        return;
    };
    let replace = match current {
        None => true,
        Some(existing) => {
            candidate.tier > existing.tier
                || (candidate.tier == existing.tier && candidate.score > existing.score)
        }
    };
    if replace {
        *current = Some(candidate);
    }
}

fn substring_indices(haystack: &str, query_lower: &str) -> Option<Vec<usize>> {
    if haystack.is_ascii() && query_lower.is_ascii() {
        let start = find_ignore_ascii_case(haystack, query_lower)?;
        return char_indices_for_span(haystack, start, query_lower.chars().count());
    }

    normalized_indices_for_query(haystack, query_lower)
}

fn normalized_indices_for_query(haystack: &str, query_lower: &str) -> Option<Vec<usize>> {
    let haystack_norm = normalized_chars_with_original_indices(haystack);
    // Search folding never *shrinks* a string — every char folds to one or more
    // chars (`fold_search_char`) — so a query with more characters than the
    // folded haystack can never be a substring of it. Bail out before folding the
    // whole query, counting only up to the haystack length (`nth` stops early), so
    // a pathologically long query is not re-folded for every candidate line/field.
    // Without this, a single non-ASCII char forces every candidate through the
    // normalized path and re-folds the entire query O(scripts × lines) times,
    // which made long non-ASCII launcher queries stall for seconds.
    if query_lower.chars().nth(haystack_norm.len()).is_some() {
        return None;
    }
    let query_norm = normalized_query_chars(query_lower);
    if query_norm.is_empty() || query_norm.len() > haystack_norm.len() {
        return None;
    }

    for start in 0..=(haystack_norm.len() - query_norm.len()) {
        if haystack_norm[start..start + query_norm.len()]
            .iter()
            .map(|(ch, _)| *ch)
            .eq(query_norm.iter().copied())
        {
            let mut indices = haystack_norm[start..start + query_norm.len()]
                .iter()
                .map(|(_, original_index)| *original_index)
                .collect::<Vec<_>>();
            indices.sort_unstable();
            indices.dedup();
            return Some(indices);
        }
    }

    None
}

// Test-only, thread-local count of how many times the *query* is folded. Lets a
// test assert deterministically (no wall-clock) that a pathologically long query
// is not re-folded per candidate — thread-local so parallel tests never perturb
// it. Compiled out entirely in non-test builds.
#[cfg(test)]
thread_local! {
    pub(crate) static QUERY_FOLD_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn normalized_query_chars(value: &str) -> Vec<char> {
    #[cfg(test)]
    QUERY_FOLD_CALLS.with(|c| c.set(c.get().saturating_add(1)));
    value
        .chars()
        .flat_map(|ch| fold_search_char(ch).into_iter())
        .collect()
}

fn normalized_chars_with_original_indices(value: &str) -> Vec<(char, usize)> {
    value
        .chars()
        .enumerate()
        .flat_map(|(index, ch)| {
            fold_search_char(ch)
                .into_iter()
                .map(move |folded| (folded, index))
        })
        .collect()
}

fn fold_search_char(ch: char) -> Vec<char> {
    let folded = match ch {
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => "a",
        'Ç' | 'ç' => "c",
        'È' | 'É' | 'Ê' | 'Ë' | 'è' | 'é' | 'ê' | 'ë' => "e",
        'Ì' | 'Í' | 'Î' | 'Ï' | 'ì' | 'í' | 'î' | 'ï' | 'İ' => "i",
        'Ñ' | 'ñ' => "n",
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => "o",
        'Ù' | 'Ú' | 'Û' | 'Ü' | 'ù' | 'ú' | 'û' | 'ü' => "u",
        'Ý' | 'Ÿ' | 'ý' | 'ÿ' => "y",
        'Æ' | 'æ' => "ae",
        'Œ' | 'œ' => "oe",
        'ß' => "ss",
        _ => return ch.to_lowercase().collect(),
    };
    folded.chars().collect()
}

fn byte_pos_is_word_boundary(haystack: &str, byte_pos: usize) -> bool {
    if haystack.is_ascii() {
        return is_word_boundary_match(haystack, byte_pos);
    }

    if byte_pos == 0 {
        return true;
    }
    let mut previous: Option<char> = None;
    for (idx, current) in haystack.char_indices() {
        if idx == byte_pos {
            let Some(previous) = previous else {
                return false;
            };
            return !char::is_alphanumeric(previous)
                || (char::is_lowercase(previous) && char::is_uppercase(current));
        }
        previous = Some(current);
    }
    false
}

fn fuzzy_indices_are_structured_abbreviation(haystack: &str, indices: &[usize]) -> bool {
    let Some(first) = indices.first().copied() else {
        return false;
    };

    if !char_index_is_word_start(haystack, first) {
        return false;
    }

    let mut previous = first;
    let mut run_count = 1;
    for current in indices.iter().copied().skip(1) {
        if current == previous.saturating_add(1) {
            previous = current;
            continue;
        }
        if !char_index_is_word_start(haystack, current) {
            return false;
        }
        run_count += 1;
        previous = current;
    }

    run_count >= 2
}

fn char_index_is_word_start(haystack: &str, char_index: usize) -> bool {
    if char_index == 0 {
        return true;
    }

    let mut previous: Option<char> = None;
    for (index, current) in haystack.chars().enumerate() {
        if index == char_index {
            let Some(previous) = previous else {
                return false;
            };
            return !previous.is_alphanumeric()
                || (previous.is_lowercase() && current.is_uppercase());
        }
        previous = Some(current);
    }

    false
}

#[cfg(test)]
mod normalized_substring_perf_tests {
    use super::{normalized_indices_for_query, substring_indices};

    #[test]
    fn normalized_match_still_finds_non_ascii_substrings() {
        // Fold path preserved: an accented query still substring-matches an
        // accented haystack, and diacritics fold to their ASCII base.
        assert!(substring_indices("café société", "société").is_some());
        assert!(substring_indices("naïve", "naive").is_some()); // ï → i
        assert!(substring_indices("straße", "strasse").is_some()); // ß → ss
                                                                   // A genuine non-match still returns None.
        assert!(substring_indices("café", "zzz").is_none());
    }

    #[test]
    fn query_longer_than_haystack_returns_none() {
        // Behavior preserved by the early-out: a query with more chars than the
        // haystack can never be a substring.
        let long = "é".repeat(50);
        assert!(normalized_indices_for_query("café", &long).is_none());
        assert!(substring_indices("café", &long).is_none());
    }

    #[test]
    fn huge_non_ascii_query_is_not_re_folded_per_candidate() {
        // Regression (F1): a long non-ASCII query used to be re-folded for every
        // candidate line/field (per-char Unicode lowercasing), so launcher search
        // stalled for seconds. Deterministic guard — no wall-clock: the early-out
        // must reject an over-long query WITHOUT folding it. Pre-fix, this loop
        // folded the query 1600 times (→ QUERY_FOLD_CALLS == 1600); the fix makes
        // it 0. Thread-local, so parallel tests can't perturb the count.
        use super::QUERY_FOLD_CALLS;
        let huge = "é".repeat(200_000);
        let haystacks = ["run notes", "clipboard history", "open terminal", "café"];
        QUERY_FOLD_CALLS.with(|c| c.set(0));
        let mut hits = 0usize;
        for _ in 0..400 {
            for h in haystacks {
                if normalized_indices_for_query(h, &huge).is_some() {
                    hits += 1;
                }
            }
        }
        assert_eq!(
            hits, 0,
            "a 200k-char query cannot substring-match short labels"
        );
        assert_eq!(
            QUERY_FOLD_CALLS.with(|c| c.get()),
            0,
            "an over-long query must never be folded per candidate (F1 perf regression)"
        );
    }
}
