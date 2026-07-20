use super::super::types::{MatchEvidence, MatchEvidenceField};
use super::{find_ignore_ascii_case, is_word_boundary_match, NucleoCtx};

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

/// Shared `min_query_chars` gate for every root passive source.
///
/// Measured in BYTES, not chars: the gate exists to skip noisy 1–2 keystroke
/// ASCII prefixes, but a single emoji (4 bytes) or CJK character (3 bytes)
/// already carries full recall intent — counting chars made "🚀" and
/// one-character CJK queries unsearchable (audit F12). Byte length only ever
/// admits multibyte queries EARLIER than char count, never later.
///
/// Callers pass the already-trimmed query; sources with additional rules
/// (newline rejection, empty-query browse) keep those checks locally.
pub(crate) fn query_meets_min_query_chars(trimmed_query: &str, min_query_chars: usize) -> bool {
    trimmed_query.len() >= min_query_chars
}

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

pub(crate) fn score_from_tier(tier: i32, bonus: i32) -> i32 {
    tier.saturating_mul(1000)
        .saturating_add(bonus.clamp(0, 999))
}

pub(crate) fn match_tier_from_score(score: i32) -> i32 {
    if score <= 0 {
        0
    } else {
        score / 1000
    }
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

pub(crate) fn char_indices_for_span(
    haystack: &str,
    byte_start: usize,
    char_len: usize,
) -> Option<Vec<usize>> {
    let start_char = haystack[..byte_start].chars().count();
    Some((start_char..start_char + char_len).collect())
}

pub(crate) fn byte_range_for_char_indices(
    haystack: &str,
    indices: &[usize],
) -> Option<std::ops::Range<usize>> {
    let first = *indices.first()?;
    let last = *indices.last()?;
    let mut offsets: Vec<usize> = haystack.char_indices().map(|(idx, _)| idx).collect();
    offsets.push(haystack.len());
    Some(*offsets.get(first)?..*offsets.get(last + 1)?)
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
mod min_query_len_tests {
    use super::query_meets_min_query_chars;

    #[test]
    fn ascii_queries_gate_on_length() {
        assert!(!query_meets_min_query_chars("ab", 3));
        assert!(query_meets_min_query_chars("abc", 3));
    }

    #[test]
    fn single_cjk_char_and_emoji_meet_a_min_of_three() {
        // 3-byte CJK char and 4-byte emoji carry full recall intent; byte
        // semantics admit them where char counting locked them out (F12).
        assert!(query_meets_min_query_chars("日", 3));
        assert!(query_meets_min_query_chars("🚀", 3));
        assert!(query_meets_min_query_chars("日本", 4));
    }
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

/// Chaos-monkey fuzz of the long-text sentence matcher (`sentence.rs`), driven
/// through its public API so the whole pipeline (query compile → tokenize →
/// per-field scan → occurrence math → excerpt build) is exercised on adversarial
/// input. Two invariants are asserted:
///   1. No panic on ANY (query, field-set) combination — the byte/char/grapheme
///      confusion family that produced most chaos-monkey finds this session.
///   2. Every returned highlight index is IN BOUNDS for the text it indexes
///      (`title_indices` < `title_text.chars().count()`, same for subtitle) — the
///      contract renderers rely on (`LongTextMatchEvidence` doc: "char indices
///      into title_text"). An out-of-bounds index would panic a highlighter.
#[cfg(test)]
mod sentence_matcher_fuzz_tests {
    use crate::scripts::search::sentence::{
        compile_long_text_query, match_long_text, FieldClass, FieldVisibility, LongTextField,
        LongTextFieldId, RenderSlot,
    };

    /// Hostile query strings: multibyte, combining, RTL override, ZWJ, control,
    /// quoted phrases, whitespace-only, huge, and mixed-script.
    fn hostile_queries() -> Vec<String> {
        vec![
            String::new(),
            " ".to_string(),
            "\t\n".to_string(),
            "é".to_string(),
            "café".to_string(),
            "\"café latte\"".to_string(),
            "a\u{0301}".to_string(), // combining acute
            "a".to_string() + &"\u{0301}".repeat(50),
            "👩‍👩‍👧‍👦".to_string(),               // ZWJ family
            "\u{202E}reversed".to_string(), // RTL override
            "الله اكبر".to_string(),
            "日本語 テスト".to_string(),
            "\0null".to_string(),
            "line1\nline2".to_string(),
            "AaÉé ÄÖÜ".to_string(),
            "é ".repeat(500), // long multibyte, many terms
            "\"".to_string(), // lone quote
            "\"unterminated phrase".to_string(),
            "a".repeat(100_000), // huge single term
            "(){}[]<>|&;".to_string(),
        ]
    }

    /// Hostile field texts, including ones that CONTAIN query terms at multibyte
    /// offsets so occurrence/excerpt math actually fires.
    fn hostile_field_texts() -> Vec<String> {
        vec![
            String::new(),
            "café".to_string(),
            "a café at the end é".to_string(),
            "prefix é café suffix".to_string(),
            "日本語 café テスト reversed 👩‍👩‍👧‍👦".to_string(),
            "a\u{0301}bc a\u{0301}bc".to_string(),
            "\u{202E}café latte reversed\u{202C}".to_string(),
            format!("{} café {}", "x".repeat(4000), "y".repeat(4000)),
            "café\ncafé\ncafé".to_string(),
            "the quick brown fox café jumps over the lazy é dog".to_string(),
            "\0café\0latte\0".to_string(),
            "é".repeat(3000),
            "AaÉé café ÄÖÜ latte".to_string(),
        ]
    }

    fn field<'a>(
        id: LongTextFieldId,
        text: &'a str,
        vis: FieldVisibility,
        class: FieldClass,
    ) -> LongTextField<'a> {
        LongTextField {
            id,
            text,
            class,
            visibility: vis,
            weight: 10,
        }
    }

    #[test]
    fn matcher_never_panics_and_never_emits_out_of_bounds_indices() {
        let queries = hostile_queries();
        let texts = hostile_field_texts();
        let mut compiled_count = 0usize;
        let mut match_count = 0usize;

        for raw in &queries {
            let Some(query) = compile_long_text_query(raw) else {
                continue;
            };
            compiled_count += 1;

            // Cross every field text into title (visible) + a hidden transcript
            // (drives build_excerpt) + a metadata url field.
            for t_title in &texts {
                for t_hidden in &texts {
                    let fields = vec![
                        field(
                            LongTextFieldId::Title,
                            t_title,
                            FieldVisibility::Visible(RenderSlot::Title),
                            FieldClass::NaturalText,
                        ),
                        field(
                            LongTextFieldId::Preview,
                            t_hidden,
                            FieldVisibility::Visible(RenderSlot::Subtitle),
                            FieldClass::NaturalText,
                        ),
                        field(
                            LongTextFieldId::Transcript,
                            t_hidden,
                            FieldVisibility::Hidden,
                            FieldClass::NaturalText,
                        ),
                        field(
                            LongTextFieldId::Url,
                            t_title,
                            FieldVisibility::Hidden,
                            FieldClass::Metadata,
                        ),
                    ];

                    if let Some(m) = match_long_text(&query, &fields) {
                        match_count += 1;
                        let ev = &m.evidence;
                        let title_chars = ev.title_text.chars().count();
                        let sub_chars = ev.subtitle_text.chars().count();
                        for &i in &ev.title_indices {
                            assert!(
                                i < title_chars,
                                "title highlight index {i} out of bounds for {title_chars}-char \
                                 title_text (query {raw:?}) — would panic a highlighter",
                            );
                        }
                        for &i in &ev.subtitle_indices {
                            assert!(
                                i < sub_chars,
                                "subtitle highlight index {i} out of bounds for {sub_chars}-char \
                                 subtitle_text (query {raw:?}) — would panic a highlighter",
                            );
                        }
                        // Indices must be sorted+deduped per the evidence contract.
                        assert!(
                            ev.title_indices.windows(2).all(|w| w[0] < w[1]),
                            "title_indices must be strictly ascending (query {raw:?})",
                        );
                    }
                }
            }
        }

        // Sanity: the fuzz actually exercised the pipeline, not a no-op.
        assert!(
            compiled_count >= 5,
            "expected several hostile queries to compile, got {compiled_count}",
        );
        assert!(
            match_count > 0,
            "expected at least one adversarial match to build evidence, got {match_count}",
        );
    }
}
