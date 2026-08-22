//! Behavior locks for the shared sentence-query matcher.
//!
//! These tests lock the long-text search contract: word-boundary matching
//! for completed terms, prefix matching for the live last term, the
//! low-information proximity gate, tiered ranking (visible beats hidden),
//! and evidence-based highlighting (ranges are contiguous and start at word
//! boundaries). See Oracle session `sentence-query-search-pivot`.

use super::{
    FieldClass, FieldVisibility, LongTextField, LongTextFieldId, LongTextMatchTier, LongTextMode,
    QueryTermKind, RenderSlot, compile_long_text_query, match_long_text_query,
};

fn title_field(text: &str) -> LongTextField<'_> {
    LongTextField {
        id: LongTextFieldId::Title,
        text,
        class: FieldClass::NaturalText,
        visibility: FieldVisibility::Visible(RenderSlot::Title),
        weight: 6,
    }
}

fn preview_field(text: &str) -> LongTextField<'_> {
    LongTextField {
        id: LongTextFieldId::Preview,
        text,
        class: FieldClass::NaturalText,
        visibility: FieldVisibility::Visible(RenderSlot::Subtitle),
        weight: 4,
    }
}

fn transcript_field(text: &str) -> LongTextField<'_> {
    LongTextField {
        id: LongTextFieldId::Transcript,
        text,
        class: FieldClass::NaturalText,
        visibility: FieldVisibility::Hidden,
        weight: 1,
    }
}

fn timestamp_field(text: &str) -> LongTextField<'_> {
    LongTextField {
        id: LongTextFieldId::Timestamp,
        text,
        class: FieldClass::Metadata,
        visibility: FieldVisibility::Hidden,
        weight: 1,
    }
}

// ── Mode compilation ─────────────────────────────────────────────────

#[test]
fn single_unfinished_token_stays_in_substring_mode() {
    let query = compile_long_text_query("auth").expect("compiles");
    assert_eq!(query.mode, LongTextMode::SingleToken);
}

#[test]
fn trailing_space_promotes_to_sentence_mode() {
    let query = compile_long_text_query("auth ").expect("compiles");
    assert_eq!(query.mode, LongTextMode::Sentence);
    assert_eq!(query.terms[0].kind, QueryTermKind::ExactWord);
}

#[test]
fn multi_word_query_is_sentence_mode_with_prefix_last_term() {
    let query = compile_long_text_query("what are th").expect("compiles");
    assert_eq!(query.mode, LongTextMode::Sentence);
    assert_eq!(query.terms.len(), 3);
    assert_eq!(query.terms[0].kind, QueryTermKind::ExactWord);
    assert_eq!(query.terms[1].kind, QueryTermKind::ExactWord);
    assert_eq!(query.terms[2].kind, QueryTermKind::WordPrefix);
}

#[test]
fn all_stopword_query_is_low_information() {
    assert!(
        compile_long_text_query("what are the")
            .expect("compiles")
            .low_information
    );
    assert!(
        !compile_long_text_query("oauth redirect failure")
            .expect("compiles")
            .low_information
    );
}

// ── Word-boundary qualification ──────────────────────────────────────

#[test]
fn sentence_query_does_not_match_terms_inside_words() {
    // "are" must not match "shared", "the" must not match "other"/"these"
    // scattered mid-word — this is the exact screenshot failure.
    let query = compile_long_text_query("what are the").expect("compiles");
    let fields = [title_field(
        "Somewhat shared generated themes and other reports",
    )];
    assert!(match_long_text_query(&query, &fields).is_none());
}

#[test]
fn completed_terms_are_exact_words() {
    // "are" completed must not match "arena".
    let query = compile_long_text_query("what are th").expect("compiles");
    let fields = [title_field("what arena thing")];
    assert!(match_long_text_query(&query, &fields).is_none());
}

#[test]
fn last_term_is_a_word_prefix() {
    let query = compile_long_text_query("what are the").expect("compiles");
    let positive = [title_field("what are these options")];
    assert!(match_long_text_query(&query, &positive).is_some());

    let negative = [title_field("what are other options")];
    assert!(match_long_text_query(&query, &negative).is_none());
}

#[test]
fn trailing_space_completes_last_term() {
    let query = compile_long_text_query("what are the ").expect("compiles");
    let fields = [title_field("what are these options")];
    assert!(
        match_long_text_query(&query, &fields).is_none(),
        "after the trailing space, 'the' must be a whole word and not match 'these'"
    );

    let exact = [title_field("what are the options")];
    assert!(match_long_text_query(&query, &exact).is_some());
}

// ── Low-information proximity gate ───────────────────────────────────

#[test]
fn low_information_query_requires_ordered_proximity() {
    let query = compile_long_text_query("what are the").expect("compiles");

    // Terms scattered far apart in a transcript must not qualify.
    let scattered = [transcript_field(
        "what happened yesterday with the build is unclear and many words come between before we ask more questions but nothing here matches are until now and finally the end",
    )];
    assert!(
        match_long_text_query(&query, &scattered).is_none(),
        "distant scattered stopwords must not qualify"
    );

    // Ordered and near (within the slop window) qualifies.
    let near = [preview_field("what exactly are the next steps")];
    assert!(match_long_text_query(&query, &near).is_some());
}

#[test]
fn content_bearing_query_may_distribute_across_visible_fields() {
    let query = compile_long_text_query("oauth redirect").expect("compiles");
    let fields = [
        title_field("fix the oauth flow"),
        preview_field("the redirect URI expired"),
    ];
    let matched = match_long_text_query(&query, &fields).expect("qualifies");
    assert_eq!(matched.tier, LongTextMatchTier::VisibleDistributed);
}

// ── Tiering and ranking ──────────────────────────────────────────────

#[test]
fn visible_phrase_outranks_hidden_phrase() {
    let query = compile_long_text_query("what are the release criteria").expect("compiles");

    let visible = [
        title_field("What are the release criteria?"),
        transcript_field("unrelated transcript"),
    ];
    let hidden = [
        title_field("Ordinary planning chat"),
        transcript_field("we asked what are the release criteria for launch"),
    ];

    let visible_match = match_long_text_query(&query, &visible).expect("visible qualifies");
    let hidden_match = match_long_text_query(&query, &hidden).expect("hidden qualifies");
    assert!(
        visible_match.rank_score() > hidden_match.rank_score(),
        "visible phrase must outrank hidden transcript phrase regardless of recency"
    );
    assert_eq!(visible_match.tier, LongTextMatchTier::VisibleTitlePhrase);
}

#[test]
fn hidden_only_match_requires_single_hidden_field() {
    // Terms stitched across disjoint hidden fields must not qualify.
    let query = compile_long_text_query("oauth redirect").expect("compiles");
    let fields = [
        title_field("unrelated"),
        transcript_field("the oauth flow"),
        transcript_field("a redirect happened"),
    ];
    assert!(match_long_text_query(&query, &fields).is_none());
}

#[test]
fn natural_language_terms_cannot_match_metadata() {
    // "are" must not be satisfied by a timestamp string.
    let query = compile_long_text_query("what are").expect("compiles");
    let fields = [
        title_field("what happened"),
        timestamp_field("2026-04-01T10:00:00Z are-not-real"),
    ];
    assert!(match_long_text_query(&query, &fields).is_none());
}

#[test]
fn numeric_terms_may_match_metadata() {
    let query = compile_long_text_query("what 2026").expect("compiles");
    let fields = [
        title_field("what happened"),
        timestamp_field("2026-04-01T10:00:00Z"),
    ];
    assert!(match_long_text_query(&query, &fields).is_some());
}

// ── Evidence / highlighting ──────────────────────────────────────────

#[test]
fn highlight_ranges_are_contiguous_and_start_at_word_boundaries() {
    let query = compile_long_text_query("what are the").expect("compiles");
    let title = "Explain what are the options";
    let fields = [title_field(title)];
    let matched = match_long_text_query(&query, &fields).expect("qualifies");
    let evidence = matched.evidence;

    assert!(!evidence.title_indices.is_empty());
    // Every highlighted index begins a run that starts at a word boundary.
    let chars: Vec<char> = title.chars().collect();
    for window in evidence.title_indices.windows(2) {
        assert!(
            window[1] == window[0] + 1 || {
                let idx = window[1];
                idx == 0 || !chars[idx - 1].is_alphanumeric()
            },
            "non-contiguous highlight runs must start at word boundaries: {:?}",
            evidence.title_indices
        );
    }
    let first = evidence.title_indices[0];
    assert!(first == 0 || !chars[first - 1].is_alphanumeric());
}

#[test]
fn prefix_highlight_covers_only_the_typed_prefix() {
    let query = compile_long_text_query("what are the").expect("compiles");
    let title = "what are these options";
    let fields = [title_field(title)];
    let matched = match_long_text_query(&query, &fields).expect("qualifies");

    // "the" typed → only the first 3 chars of "these" highlight.
    let these_start = title.find("these").expect("fixture");
    let evidence = matched.evidence;
    assert!(evidence.title_indices.contains(&these_start));
    assert!(evidence.title_indices.contains(&(these_start + 2)));
    assert!(
        !evidence.title_indices.contains(&(these_start + 3)),
        "prefix highlight must stop at the typed prefix"
    );
}

#[test]
fn hidden_match_returns_excerpt_and_no_unrelated_title_ranges() {
    let query = compile_long_text_query("migration constraints").expect("compiles");
    let fields = [
        title_field("Ordinary visible title"),
        transcript_field(
            "early context words here and then what are the migration constraints for launch day",
        ),
    ];
    let matched = match_long_text_query(&query, &fields).expect("qualifies");
    let evidence = matched.evidence;

    assert!(evidence.title_indices.is_empty(), "no fake title highlight");
    let excerpt = evidence.hidden_excerpt.expect("hidden excerpt present");
    assert!(excerpt.text.contains("migration constraints"));
}

#[test]
fn unicode_and_apostrophe_tokenization() {
    let query = compile_long_text_query("don't panic").expect("compiles");
    let fields = [title_field("Don\u{2019}t panic about the tests")];
    assert!(
        match_long_text_query(&query, &fields).is_some(),
        "apostrophe words must survive tokenization (straight vs curly)"
    );

    let accented = compile_long_text_query("caf\u{e9} menu").expect("compiles");
    let accented_fields = [title_field("the caf\u{e9} menu changed")];
    assert!(match_long_text_query(&accented, &accented_fields).is_some());
}

// ── Single-token compatibility ───────────────────────────────────────

#[test]
fn single_token_keeps_interior_substring_recall_but_prefers_boundaries() {
    let query = compile_long_text_query("auth").expect("compiles");

    let interior = [title_field("the oauth handshake")];
    let interior_match = match_long_text_query(&query, &interior).expect("substring recall kept");

    let boundary = [title_field("the auth handshake")];
    let boundary_match = match_long_text_query(&query, &boundary).expect("boundary qualifies");

    assert!(
        boundary_match.rank_score() > interior_match.rank_score(),
        "word-boundary match must rank above interior substring"
    );
}
