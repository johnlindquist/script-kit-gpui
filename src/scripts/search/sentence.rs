//! Shared "long text" sentence-query matcher for launcher passive sources
//! that search long-form natural language (Agent Chat conversations,
//! dictation transcripts, clipboard previews).
//!
//! Contract (Oracle session `sentence-query-search-pivot`):
//! - A query with 2+ tokens (or one completed token followed by whitespace)
//!   enters *sentence mode*: every completed token must match a whole word,
//!   and only the unfinished final token may match a word prefix. Terms never
//!   match from the middle of a word ("are" must not match "shared").
//! - A single unfinished token keeps contiguous-substring recall, ranking
//!   word-boundary matches above interior ones.
//! - Stopwords are kept but down-weighted; a query made only of
//!   low-information words ("what are the") additionally requires the terms
//!   to appear as a phrase or ordered-near window inside one field.
//! - Qualification produces [`LongTextMatchEvidence`] — the word-level ranges
//!   the renderer highlights — so a row is highlighted for the same reason it
//!   qualified. Metadata fields (timestamps, durations) only satisfy numeric
//!   or temporal-looking terms.

use std::ops::Range;

/// Maximum intervening word positions allowed for an "ordered near" window.
const NEAR_WINDOW_SLOP: usize = 10;

// ── Query compilation ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LongTextMode {
    SingleToken,
    Sentence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryTermKind {
    ExactWord,
    WordPrefix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryTerm {
    pub folded: String,
    pub kind: QueryTermKind,
    pub information_weight: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LongTextQuery {
    pub mode: LongTextMode,
    pub terms: Vec<QueryTerm>,
    /// True when every term is a stopword-tier word; such queries require
    /// phrase or ordered-near proximity within a single field to qualify.
    pub low_information: bool,
}

impl LongTextQuery {
    pub fn is_sentence(&self) -> bool {
        self.mode == LongTextMode::Sentence
    }
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric()
}

fn is_query_apostrophe(ch: char) -> bool {
    matches!(ch, '\'' | '\u{2019}')
}

/// Lowercased words with their char-offset ranges in the original text.
/// Apostrophes stay inside a word only when flanked by word chars, so
/// "don't" tokenizes as one word.
fn tokenize_words(text: &str) -> Vec<(Range<usize>, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut words = Vec::new();
    let mut idx = 0usize;
    while idx < chars.len() {
        if !is_word_char(chars[idx]) {
            idx += 1;
            continue;
        }
        let start = idx;
        let mut word = String::new();
        while idx < chars.len() {
            let ch = chars[idx];
            if is_word_char(ch) {
                word.extend(ch.to_lowercase());
                idx += 1;
            } else if is_query_apostrophe(ch)
                && idx + 1 < chars.len()
                && is_word_char(chars[idx + 1])
            {
                word.push('\'');
                idx += 1;
            } else {
                break;
            }
        }
        words.push((start..idx, word));
    }
    words
}

fn information_weight(word: &str) -> u8 {
    const WEIGHT_1: &[&str] = &[
        "the", "a", "an", "is", "are", "was", "were", "to", "of", "in", "on", "for", "and", "or",
        "with",
    ];
    const WEIGHT_2: &[&str] = &[
        "what", "when", "where", "who", "why", "how", "this", "that", "these", "those", "it", "i",
        "we", "you", "do", "does", "can",
    ];
    if WEIGHT_1.contains(&word) {
        1
    } else if WEIGHT_2.contains(&word) {
        2
    } else if word.chars().count() >= 8 {
        5
    } else {
        4
    }
}

/// Compile a raw filter string into a long-text query. Returns `None` when
/// the query has no word content.
pub fn compile_long_text_query(raw: &str) -> Option<LongTextQuery> {
    let input = raw.trim_start();
    if input.is_empty() {
        return None;
    }
    // The final term is "completed" once the user types any non-word
    // character after it (space, punctuation); only then does it require a
    // whole-word match.
    let ends_at_boundary = input
        .chars()
        .last()
        .is_some_and(|ch| !is_word_char(ch) && !is_query_apostrophe(ch));

    let mut terms: Vec<QueryTerm> = tokenize_words(input)
        .into_iter()
        .map(|(_, word)| QueryTerm {
            information_weight: information_weight(&word),
            folded: word,
            kind: QueryTermKind::ExactWord,
        })
        .collect();
    if terms.is_empty() {
        return None;
    }

    let mode = if terms.len() >= 2 || ends_at_boundary {
        LongTextMode::Sentence
    } else {
        LongTextMode::SingleToken
    };

    if mode == LongTextMode::Sentence && !ends_at_boundary {
        if let Some(last) = terms.last_mut() {
            last.kind = QueryTermKind::WordPrefix;
        }
    }

    let low_information = terms.iter().all(|term| term.information_weight <= 2);

    Some(LongTextQuery {
        mode,
        terms,
        low_information,
    })
}

/// True when a term is allowed to match metadata fields (timestamps,
/// durations): numeric or temporal-looking words only. Ordinary language
/// ("the", "are") must never be satisfied by formatted metadata.
fn term_is_metadata_applicable(term: &str) -> bool {
    const TEMPORAL: &[&str] = &[
        "ms",
        "sec",
        "secs",
        "second",
        "seconds",
        "min",
        "mins",
        "minute",
        "minutes",
        "hour",
        "hours",
        "am",
        "pm",
        "today",
        "yesterday",
        "jan",
        "feb",
        "mar",
        "apr",
        "may",
        "jun",
        "jul",
        "aug",
        "sep",
        "oct",
        "nov",
        "dec",
        "january",
        "february",
        "march",
        "april",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
        "monday",
        "tuesday",
        "wednesday",
        "thursday",
        "friday",
        "saturday",
        "sunday",
    ];
    term.chars().any(|ch| ch.is_ascii_digit()) || TEMPORAL.contains(&term)
}

// ── Fields ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LongTextFieldId {
    Title,
    Preview,
    Transcript,
    Ocr,
    Target,
    Url,
    Timestamp,
    Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldClass {
    NaturalText,
    Metadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSlot {
    Title,
    Subtitle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldVisibility {
    Visible(RenderSlot),
    Hidden,
}

pub struct LongTextField<'a> {
    pub id: LongTextFieldId,
    pub text: &'a str,
    pub class: FieldClass,
    pub visibility: FieldVisibility,
    pub weight: i32,
}

// ── Match output ─────────────────────────────────────────────────────

/// Relevance tiers, weakest to strongest so `Ord` sorts naturally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LongTextMatchTier {
    HiddenAllTerms,
    HiddenNear,
    MixedVisibleHidden,
    VisibleDistributed,
    VisibleNear,
    VisiblePreviewPhrase,
    VisibleTitlePhrase,
}

/// Excerpt of a hidden field centered on the matched window, for honest
/// "why did this row qualify" rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedExcerpt {
    pub field: LongTextFieldId,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LongTextMatchEvidence {
    pub tier: LongTextMatchTier,
    pub primary_field: LongTextFieldId,
    /// Sorted, deduped char indices into `title_text`.
    pub title_indices: Vec<usize>,
    /// Sorted, deduped char indices into `subtitle_text`.
    pub subtitle_indices: Vec<usize>,
    /// The text `title_indices` refers to. Renderers must verify the
    /// rendered title starts with this before applying the indices.
    pub title_text: String,
    /// The text `subtitle_indices` refers to (composed subtitles are only
    /// safe when they start with this text).
    pub subtitle_text: String,
    pub hidden_excerpt: Option<MatchedExcerpt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LongTextMatch {
    pub tier: LongTextMatchTier,
    pub score: u32,
    pub evidence: LongTextMatchEvidence,
}

impl LongTextMatch {
    /// Single ordering key: tier dominates, in-tier score refines. Callers
    /// break remaining ties by recency.
    pub fn rank_score(&self) -> u32 {
        (self.tier as u32) * 1_000_000 + self.score.min(999_999)
    }
}

// ── Matching ─────────────────────────────────────────────────────────

/// One term's best occurrence inside one field.
#[derive(Debug, Clone)]
struct TermOccurrence {
    /// Index into the field's word list (for proximity/ordering).
    word_index: usize,
    /// Char range of the matched portion (prefix length for WordPrefix).
    char_range: Range<usize>,
}

struct FieldScan {
    /// Per query term: occurrences within this field.
    occurrences: Vec<Vec<TermOccurrence>>,
    words: Vec<(Range<usize>, String)>,
}

impl FieldScan {
    fn term_matched(&self, term_idx: usize) -> bool {
        !self.occurrences[term_idx].is_empty()
    }

    fn all_terms_matched(&self, term_count: usize) -> bool {
        (0..term_count).all(|idx| self.term_matched(idx))
    }
}

fn scan_field(query: &LongTextQuery, field: &LongTextField<'_>) -> FieldScan {
    let words = tokenize_words(field.text);
    let mut occurrences = vec![Vec::new(); query.terms.len()];

    for (term_idx, term) in query.terms.iter().enumerate() {
        if field.class == FieldClass::Metadata && !term_is_metadata_applicable(&term.folded) {
            continue;
        }
        for (word_index, (range, word)) in words.iter().enumerate() {
            let matched_len = match term.kind {
                QueryTermKind::ExactWord => (word == &term.folded).then(|| word.chars().count()),
                QueryTermKind::WordPrefix => word
                    .starts_with(term.folded.as_str())
                    .then(|| term.folded.chars().count()),
            };
            // Metadata fields additionally allow interior substrings for
            // applicable terms ("2" inside "12 sec") since formatted
            // metadata is not natural language.
            let matched_len = matched_len.or_else(|| {
                (field.class == FieldClass::Metadata && word.contains(term.folded.as_str()))
                    .then(|| term.folded.chars().count())
            });
            if let Some(len) = matched_len {
                occurrences[term_idx].push(TermOccurrence {
                    word_index,
                    char_range: range.start..(range.start + len).min(range.end),
                });
            }
        }
    }

    FieldScan { occurrences, words }
}

/// Find the tightest window of word positions covering all terms in query
/// order. Returns (start_word, end_word, chosen occurrences) when the terms
/// appear in order.
fn ordered_window(scan: &FieldScan, term_count: usize) -> Option<(usize, usize, Vec<usize>)> {
    if term_count == 0 || !scan.all_terms_matched(term_count) {
        return None;
    }
    let mut best: Option<(usize, usize, Vec<usize>)> = None;
    // Anchor on each occurrence of the first term, then greedily take the
    // next term's earliest occurrence after the previous one.
    for first in &scan.occurrences[0] {
        let mut chosen = vec![first.word_index];
        let mut prev = first.word_index;
        let mut ok = true;
        for term_idx in 1..term_count {
            match scan.occurrences[term_idx]
                .iter()
                .find(|occ| occ.word_index > prev)
            {
                Some(occ) => {
                    prev = occ.word_index;
                    chosen.push(occ.word_index);
                }
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }
        let span = (first.word_index, prev);
        if best
            .as_ref()
            .is_none_or(|(bs, be, _)| (span.1 - span.0) < (be - bs))
        {
            best = Some((span.0, span.1, chosen));
        }
    }
    best
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldProximity {
    Phrase,
    Near,
    Scattered,
    None,
}

fn field_proximity(scan: &FieldScan, term_count: usize) -> (FieldProximity, Option<Vec<usize>>) {
    match ordered_window(scan, term_count) {
        Some((start, end, chosen)) => {
            let span = end - start;
            if span + 1 == term_count {
                (FieldProximity::Phrase, Some(chosen))
            } else if span < term_count + NEAR_WINDOW_SLOP {
                (FieldProximity::Near, Some(chosen))
            } else {
                (FieldProximity::Scattered, Some(chosen))
            }
        }
        None => {
            if scan.all_terms_matched(term_count) {
                (FieldProximity::Scattered, None)
            } else {
                (FieldProximity::None, None)
            }
        }
    }
}

fn char_indices_for_occurrence(range: &Range<usize>, out: &mut Vec<usize>) {
    out.extend(range.clone());
}

/// Evidence indices for one field: the chosen window occurrences when a
/// window exists, otherwise each term's first occurrence.
fn field_evidence_indices(
    scan: &FieldScan,
    term_count: usize,
    chosen: Option<&[usize]>,
) -> Vec<usize> {
    let mut indices = Vec::new();
    match chosen {
        Some(word_positions) => {
            for (term_idx, word_index) in word_positions.iter().enumerate() {
                if let Some(occ) = scan.occurrences[term_idx]
                    .iter()
                    .find(|occ| occ.word_index == *word_index)
                {
                    char_indices_for_occurrence(&occ.char_range, &mut indices);
                }
            }
        }
        None => {
            for term_idx in 0..term_count {
                if let Some(occ) = scan.occurrences[term_idx].first() {
                    char_indices_for_occurrence(&occ.char_range, &mut indices);
                }
            }
        }
    }
    indices.sort_unstable();
    indices.dedup();
    indices
}

/// Excerpt of `field_text` centered on the matched window (word positions),
/// bounded to ~12 words of context each side.
fn build_excerpt(scan: &FieldScan, chosen: Option<&[usize]>, field_text: &str) -> String {
    const CONTEXT_WORDS: usize = 6;
    let (first, last) = match chosen {
        Some(positions) if !positions.is_empty() => (
            *positions.iter().min().unwrap_or(&0),
            *positions.iter().max().unwrap_or(&0),
        ),
        _ => {
            let firsts: Vec<usize> = scan
                .occurrences
                .iter()
                .filter_map(|occs| occs.first().map(|occ| occ.word_index))
                .collect();
            match (firsts.iter().min(), firsts.iter().max()) {
                (Some(min), Some(max)) => (*min, *max),
                _ => return String::new(),
            }
        }
    };
    let start_word = first.saturating_sub(CONTEXT_WORDS);
    let end_word = (last + CONTEXT_WORDS).min(scan.words.len().saturating_sub(1));
    let chars: Vec<char> = field_text.chars().collect();
    let start_char = scan.words[start_word].0.start;
    let end_char = scan.words[end_word].0.end;
    let mut excerpt = String::new();
    if start_char > 0 {
        excerpt.push('\u{2026}');
    }
    excerpt.extend(chars[start_char..end_char.min(chars.len())].iter());
    if end_char < chars.len() {
        excerpt.push('\u{2026}');
    }
    excerpt
}

/// Match a compiled query against a record's fields.
///
/// Field order matters for primary-field attribution: list visible fields
/// before hidden ones so redundant hidden blobs (which often contain the
/// title/preview text again) don't claim the match.
pub fn match_long_text(
    query: &LongTextQuery,
    fields: &[LongTextField<'_>],
) -> Option<LongTextMatch> {
    let term_count = query.terms.len();
    if term_count == 0 {
        return None;
    }

    let scans: Vec<FieldScan> = fields
        .iter()
        .map(|field| scan_field(query, field))
        .collect();

    // Union coverage: every term must match somewhere.
    for term_idx in 0..term_count {
        if !scans.iter().any(|scan| scan.term_matched(term_idx)) {
            return None;
        }
    }

    let visible =
        |field: &LongTextField<'_>| matches!(field.visibility, FieldVisibility::Visible(_));
    let hidden_natural = |field: &LongTextField<'_>| {
        field.visibility == FieldVisibility::Hidden && field.class == FieldClass::NaturalText
    };

    // Visible coverage counts metadata fields too (e.g. "ai 2 sec" mixing a
    // visible target with duration metadata), but phrase/near tiers only
    // come from natural-text fields.
    let visible_all = (0..term_count).all(|term_idx| {
        scans
            .iter()
            .zip(fields)
            .any(|(scan, field)| visible(field) && scan.term_matched(term_idx))
    });

    // Best proximity among visible natural-text fields.
    let mut best_visible: Option<(usize, FieldProximity, Option<Vec<usize>>)> = None;
    let mut best_hidden: Option<(usize, FieldProximity, Option<Vec<usize>>)> = None;
    for (field_idx, (scan, field)) in scans.iter().zip(fields).enumerate() {
        if field.class != FieldClass::NaturalText || !scan.all_terms_matched(term_count) {
            continue;
        }
        let (proximity, chosen) = field_proximity(scan, term_count);
        if proximity == FieldProximity::None {
            continue;
        }
        let slot = if visible(field) {
            &mut best_visible
        } else {
            &mut best_hidden
        };
        let better = match slot {
            Some((_, existing, _)) => proximity_rank(proximity) > proximity_rank(*existing),
            None => true,
        };
        if better {
            *slot = Some((field_idx, proximity, chosen));
        }
    }

    let hidden_any = (0..term_count).any(|term_idx| {
        scans
            .iter()
            .zip(fields)
            .any(|(scan, field)| hidden_natural(field) && scan.term_matched(term_idx))
    });

    // Tier decision.
    let tier = if let Some((field_idx, proximity, _)) = &best_visible {
        match proximity {
            FieldProximity::Phrase => match fields[*field_idx].visibility {
                FieldVisibility::Visible(RenderSlot::Title) => {
                    LongTextMatchTier::VisibleTitlePhrase
                }
                _ => LongTextMatchTier::VisiblePreviewPhrase,
            },
            FieldProximity::Near => LongTextMatchTier::VisibleNear,
            FieldProximity::Scattered | FieldProximity::None => {
                if visible_all {
                    LongTextMatchTier::VisibleDistributed
                } else {
                    LongTextMatchTier::MixedVisibleHidden
                }
            }
        }
    } else if visible_all {
        LongTextMatchTier::VisibleDistributed
    } else if let Some((_, proximity, _)) = &best_hidden {
        match proximity {
            FieldProximity::Phrase | FieldProximity::Near => LongTextMatchTier::HiddenNear,
            _ => LongTextMatchTier::HiddenAllTerms,
        }
    } else if hidden_any && visible_partial_coverage(&scans, fields, term_count) {
        LongTextMatchTier::MixedVisibleHidden
    } else if (0..term_count).all(|term_idx| {
        scans.iter().zip(fields).any(|(scan, field)| {
            scan.term_matched(term_idx) && (visible(field) || field.class == FieldClass::Metadata)
        })
    }) {
        // Metadata terms (dates, durations) may complete coverage; this is
        // a weak tier because metadata is not ordinary text.
        LongTextMatchTier::MixedVisibleHidden
    } else {
        // Terms are only covered by stitching disjoint hidden fields
        // together — not a real match.
        return None;
    };

    // Low-information queries must show phrase or near proximity within a
    // single natural-text field.
    if query.low_information && query.is_sentence() {
        let near_ok = |slot: &Option<(usize, FieldProximity, Option<Vec<usize>>)>| {
            slot.as_ref().is_some_and(|(_, proximity, _)| {
                matches!(proximity, FieldProximity::Phrase | FieldProximity::Near)
            })
        };
        if !near_ok(&best_visible) && !near_ok(&best_hidden) {
            return None;
        }
    }

    // Evidence: word-level char indices per rendered slot.
    let mut title_indices = Vec::new();
    let mut subtitle_indices = Vec::new();
    let mut primary_field: Option<(i32, LongTextFieldId)> = None;
    for (scan_idx, (scan, field)) in scans.iter().zip(fields).enumerate() {
        let matched_terms = (0..term_count)
            .filter(|idx| scan.term_matched(*idx))
            .count();
        if matched_terms == 0 {
            continue;
        }
        // Metadata fields are often rendered inside composed subtitles at
        // unknown offsets, so they never emit highlight indices.
        if field.class == FieldClass::NaturalText {
            if let FieldVisibility::Visible(slot) = field.visibility {
                let chosen = match &best_visible {
                    Some((field_idx, _, chosen)) if *field_idx == scan_idx => chosen.as_deref(),
                    _ => None,
                };
                let indices = field_evidence_indices(scan, term_count, chosen);
                match slot {
                    RenderSlot::Title => title_indices.extend(indices),
                    RenderSlot::Subtitle => subtitle_indices.extend(indices),
                }
            }
        }
        let strength = field.weight * matched_terms as i32;
        if primary_field.is_none_or(|(best_strength, _)| strength > best_strength) {
            primary_field = Some((strength, field.id));
        }
    }
    title_indices.sort_unstable();
    title_indices.dedup();
    subtitle_indices.sort_unstable();
    subtitle_indices.dedup();

    // Hidden excerpt for hidden-led tiers.
    let hidden_excerpt = match tier {
        LongTextMatchTier::HiddenAllTerms
        | LongTextMatchTier::HiddenNear
        | LongTextMatchTier::MixedVisibleHidden => {
            best_hidden
                .as_ref()
                .map(|(field_idx, _, chosen)| MatchedExcerpt {
                    field: fields[*field_idx].id,
                    text: build_excerpt(
                        &scans[*field_idx],
                        chosen.as_deref(),
                        fields[*field_idx].text,
                    ),
                })
                .or_else(|| {
                    // Mixed tier without a single hidden field holding all
                    // terms: excerpt around whichever hidden field matched
                    // the most terms.
                    scans
                        .iter()
                        .zip(fields)
                        .filter(|(scan, field)| {
                            hidden_natural(field)
                                && (0..term_count).any(|idx| scan.term_matched(idx))
                        })
                        .max_by_key(|(scan, _)| {
                            (0..term_count)
                                .filter(|idx| scan.term_matched(*idx))
                                .count()
                        })
                        .map(|(scan, field)| MatchedExcerpt {
                            field: field.id,
                            text: build_excerpt(scan, None, field.text),
                        })
                })
        }
        _ => None,
    };

    // Score within the tier.
    let mut score: i64 = 0;
    for (term_idx, term) in query.terms.iter().enumerate() {
        let factor = match term.kind {
            QueryTermKind::ExactWord => 4,
            QueryTermKind::WordPrefix => 3,
        };
        let best_field_weight = scans
            .iter()
            .zip(fields)
            .filter(|(scan, _)| scan.term_matched(term_idx))
            .map(|(_, field)| field.weight)
            .max()
            .unwrap_or(0);
        score += i64::from(term.information_weight) * i64::from(best_field_weight) * factor;
    }
    if let Some((_, proximity, chosen)) = &best_visible {
        score += proximity_bonus(*proximity, chosen.as_deref(), term_count);
    } else if let Some((_, proximity, chosen)) = &best_hidden {
        score += proximity_bonus(*proximity, chosen.as_deref(), term_count) / 2;
    }

    let (_, primary_field) = primary_field?;
    let (title_text, subtitle_text) = slot_texts(fields);

    Some(LongTextMatch {
        tier,
        score: score.clamp(0, u32::MAX as i64) as u32,
        evidence: LongTextMatchEvidence {
            tier,
            primary_field,
            title_indices,
            subtitle_indices,
            title_text,
            subtitle_text,
            hidden_excerpt,
        },
    })
}

/// The natural-text source strings behind each rendered slot.
fn slot_texts(fields: &[LongTextField<'_>]) -> (String, String) {
    let text_for = |slot: RenderSlot| {
        fields
            .iter()
            .find(|field| {
                field.class == FieldClass::NaturalText
                    && field.visibility == FieldVisibility::Visible(slot)
            })
            .map(|field| field.text.to_string())
            .unwrap_or_default()
    };
    (text_for(RenderSlot::Title), text_for(RenderSlot::Subtitle))
}

fn visible_partial_coverage(
    scans: &[FieldScan],
    fields: &[LongTextField<'_>],
    term_count: usize,
) -> bool {
    // Mixed tier requires at least one term matched in a visible field and
    // the remaining terms covered by a single hidden natural-text field.
    let any_visible = (0..term_count).any(|term_idx| {
        scans.iter().zip(fields).any(|(scan, field)| {
            matches!(field.visibility, FieldVisibility::Visible(_)) && scan.term_matched(term_idx)
        })
    });
    if !any_visible {
        return false;
    }
    scans.iter().zip(fields).any(|(scan, field)| {
        field.visibility == FieldVisibility::Hidden
            && field.class == FieldClass::NaturalText
            && (0..term_count).all(|term_idx| {
                scan.term_matched(term_idx)
                    || scans.iter().zip(fields).any(|(other_scan, other_field)| {
                        matches!(other_field.visibility, FieldVisibility::Visible(_))
                            && other_scan.term_matched(term_idx)
                    })
            })
    })
}

fn proximity_rank(proximity: FieldProximity) -> u8 {
    match proximity {
        FieldProximity::Phrase => 3,
        FieldProximity::Near => 2,
        FieldProximity::Scattered => 1,
        FieldProximity::None => 0,
    }
}

fn proximity_bonus(proximity: FieldProximity, chosen: Option<&[usize]>, term_count: usize) -> i64 {
    match proximity {
        FieldProximity::Phrase => 400,
        FieldProximity::Near => {
            let span = chosen
                .and_then(|positions| {
                    let min = positions.iter().min()?;
                    let max = positions.iter().max()?;
                    Some(max - min)
                })
                .unwrap_or(term_count + NEAR_WINDOW_SLOP);
            200_i64.saturating_sub(10 * span.saturating_sub(term_count.saturating_sub(1)) as i64)
        }
        FieldProximity::Scattered | FieldProximity::None => 0,
    }
}

// ── Single-token compatibility mode ──────────────────────────────────

/// Contiguous substring match for single-token queries, keeping today's
/// recall ("auth" still finds "oauth") while ranking word-boundary matches
/// higher and returning honest contiguous evidence.
fn single_token_field_match(term: &str, field_text: &str) -> Option<(bool, Range<usize>)> {
    let haystack: Vec<char> = field_text.chars().flat_map(|c| c.to_lowercase()).collect();
    // NOTE: to_lowercase can change char counts for exotic scripts; accept
    // the approximation (same as the existing highlight ASCII fast path).
    let needle: Vec<char> = term.chars().collect();
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    let mut interior: Option<Range<usize>> = None;
    for start in 0..=(haystack.len() - needle.len()) {
        if haystack[start..start + needle.len()] == needle[..] {
            let at_boundary = start == 0 || !is_word_char(haystack[start - 1]);
            let range = start..start + needle.len();
            if at_boundary {
                return Some((true, range));
            }
            if interior.is_none() {
                interior = Some(range);
            }
        }
    }
    interior.map(|range| (false, range))
}

fn match_single_token(
    query: &LongTextQuery,
    fields: &[LongTextField<'_>],
) -> Option<LongTextMatch> {
    let term = query.terms.first()?;
    let mut best: Option<(i64, bool, usize, Range<usize>)> = None;
    for (field_idx, field) in fields.iter().enumerate() {
        if field.class == FieldClass::Metadata && !term_is_metadata_applicable(&term.folded) {
            continue;
        }
        if let Some((boundary, range)) = single_token_field_match(&term.folded, field.text) {
            let strength = i64::from(field.weight)
                * if boundary { 4 } else { 2 }
                * i64::from(term.information_weight);
            if best
                .as_ref()
                .is_none_or(|(existing, _, _, _)| strength > *existing)
            {
                best = Some((strength, boundary, field_idx, range));
            }
        }
    }
    let (strength, boundary, field_idx, range) = best?;
    let field = &fields[field_idx];
    let tier = match field.visibility {
        FieldVisibility::Visible(RenderSlot::Title) if boundary => {
            LongTextMatchTier::VisibleTitlePhrase
        }
        FieldVisibility::Visible(RenderSlot::Subtitle) if boundary => {
            LongTextMatchTier::VisiblePreviewPhrase
        }
        FieldVisibility::Visible(_) => LongTextMatchTier::VisibleDistributed,
        FieldVisibility::Hidden => {
            if boundary {
                LongTextMatchTier::HiddenNear
            } else {
                LongTextMatchTier::HiddenAllTerms
            }
        }
    };
    let mut title_indices = Vec::new();
    let mut subtitle_indices = Vec::new();
    let mut hidden_excerpt = None;
    match field.visibility {
        FieldVisibility::Visible(RenderSlot::Title) if field.class == FieldClass::NaturalText => {
            title_indices.extend(range.clone())
        }
        FieldVisibility::Visible(RenderSlot::Subtitle)
            if field.class == FieldClass::NaturalText =>
        {
            subtitle_indices.extend(range.clone())
        }
        FieldVisibility::Visible(_) => {}
        FieldVisibility::Hidden => {
            if field.class == FieldClass::NaturalText {
                let scan = scan_field(query, field);
                let text = build_excerpt(&scan, None, field.text);
                if !text.is_empty() {
                    hidden_excerpt = Some(MatchedExcerpt {
                        field: field.id,
                        text,
                    });
                }
            }
        }
    }
    // Visible fields other than the winner may also contain the token; add
    // their evidence so highlights stay complete.
    for other in fields.iter() {
        if std::ptr::eq(other, field) {
            continue;
        }
        if let FieldVisibility::Visible(slot) = other.visibility {
            if other.class != FieldClass::NaturalText {
                continue;
            }
            if let Some((_, other_range)) = single_token_field_match(&term.folded, other.text) {
                match slot {
                    RenderSlot::Title => title_indices.extend(other_range),
                    RenderSlot::Subtitle => subtitle_indices.extend(other_range),
                }
            }
        }
    }
    title_indices.sort_unstable();
    title_indices.dedup();
    subtitle_indices.sort_unstable();
    subtitle_indices.dedup();
    let (title_text, subtitle_text) = slot_texts(fields);

    Some(LongTextMatch {
        tier,
        score: strength.clamp(0, u32::MAX as i64) as u32,
        evidence: LongTextMatchEvidence {
            tier,
            primary_field: field.id,
            title_indices,
            subtitle_indices,
            title_text,
            subtitle_text,
            hidden_excerpt,
        },
    })
}

/// Entry point: match a raw query against a record's fields using the
/// mode-appropriate semantics. Returns `None` when the record does not
/// qualify.
pub fn match_long_text_query(
    query: &LongTextQuery,
    fields: &[LongTextField<'_>],
) -> Option<LongTextMatch> {
    match query.mode {
        LongTextMode::SingleToken => match_single_token(query, fields),
        LongTextMode::Sentence => match_long_text(query, fields),
    }
}
