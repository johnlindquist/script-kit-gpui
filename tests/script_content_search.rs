//! Integration tests for script content search feature.
//!
//! Validates:
//! - Body-only queries return results with match_kind=Content and populated content-hit metadata
//! - The body-content tier contributes exactly +5 and does not outrank name/description matches
//! - The best matching content line is stable with correct snippet highlight indices
//! - Content search is case-insensitive
//! - Content search skips when name/description already matched

use std::path::PathBuf;
use std::sync::Arc;

use script_kit_gpui::scripts::{fuzzy_search_scripts, Script, ScriptMatchKind};

/// Helper to create a test Script with body content
fn make_script(name: &str, description: Option<&str>, body: Option<&str>) -> Arc<Script> {
    Arc::new(Script {
        name: name.to_string(),
        path: PathBuf::from(format!("/tmp/test-scripts/{}.ts", name)),
        extension: "ts".to_string(),
        description: description.map(|s| s.to_string()),
        icon: None,
        alias: None,
        shortcut: None,
        typed_metadata: None,
        schema: None,
        plugin_id: String::new(),
        plugin_title: None,
        kit_name: Some("test".to_string()),
        body: body.map(|s| s.into()),
    })
}

// ── match_kind = Content and populated metadata ────────────────────────

#[test]
fn body_only_query_returns_content_match_kind() {
    let scripts = vec![make_script(
        "alpha",
        None,
        Some("import { uniqueTokenXyz } from 'lib';\n"),
    )];

    let results = fuzzy_search_scripts(&scripts, "uniqueTokenXyz");
    assert_eq!(results.len(), 1, "should find one match");
    assert_eq!(results[0].match_kind, ScriptMatchKind::Content);
}

#[test]
fn body_only_query_populates_content_match_metadata() {
    let scripts = vec![make_script(
        "alpha",
        None,
        Some("line one\nimport { uniqueTokenXyz } from 'lib';\nline three\n"),
    )];

    let results = fuzzy_search_scripts(&scripts, "uniqueTokenXyz");
    assert_eq!(results.len(), 1);

    let cm = results[0]
        .content_match
        .as_ref()
        .expect("content_match must be populated");
    assert_eq!(cm.line_number, 2, "match is on line 2");
    assert!(
        cm.line_text.contains("uniqueTokenXyz"),
        "snippet must contain the matched text"
    );
    assert!(
        !cm.line_match_indices.is_empty(),
        "highlight indices must be populated"
    );
}

// ── +5 scoring tier ────────────────────────────────────────────────────

#[test]
fn content_tier_contributes_exactly_five() {
    let scripts = vec![make_script(
        "unrelated-name",
        Some("unrelated description"),
        Some("console.log('xyzSpecialToken');\n"),
    )];

    let results = fuzzy_search_scripts(&scripts, "xyzSpecialToken");
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].score, 5,
        "content-only match must score exactly 5"
    );
    assert_eq!(results[0].match_kind, ScriptMatchKind::Content);
}

#[test]
fn content_match_does_not_outrank_name_match() {
    let scripts = vec![
        make_script("finder", None, Some("unrelated body\n")),
        make_script("other-script", None, Some("use finder module here\n")),
    ];

    let results = fuzzy_search_scripts(&scripts, "finder");
    assert!(results.len() >= 1);
    // The name match must come first
    assert_eq!(results[0].script.name, "finder");
    assert_ne!(results[0].match_kind, ScriptMatchKind::Content);
    // If the body-only match appears it must rank below
    if results.len() > 1 {
        assert!(
            results[1].score < results[0].score,
            "content match score ({}) must be less than name match score ({})",
            results[1].score,
            results[0].score
        );
    }
}

#[test]
fn content_match_does_not_outrank_description_match() {
    let scripts = vec![
        make_script(
            "abc",
            Some("searches for the magicWord"),
            Some("nothing special here\n"),
        ),
        make_script("def", None, Some("the magicWord is in this body only\n")),
    ];

    let results = fuzzy_search_scripts(&scripts, "magicWord");
    assert!(results.len() >= 1);
    // Description match should outrank content match
    let desc_hit = results.iter().find(|r| r.script.name == "abc");
    let body_hit = results.iter().find(|r| r.script.name == "def");
    if let (Some(d), Some(b)) = (desc_hit, body_hit) {
        assert!(
            d.score > b.score,
            "description score ({}) must exceed content score ({})",
            d.score,
            b.score
        );
    }
}

// ── Snippet stability and highlight indices ────────────────────────────

#[test]
fn best_matching_line_is_first_occurrence() {
    let body = "line one\nthe SECRET here\nalso SECRET again\nline four\n";
    let scripts = vec![make_script("multi-hit", None, Some(body))];

    let results = fuzzy_search_scripts(&scripts, "SECRET");
    assert_eq!(results.len(), 1);
    let cm = results[0].content_match.as_ref().unwrap();
    assert_eq!(cm.line_number, 2, "should pick the first matching line");
}

#[test]
fn snippet_highlight_indices_align_with_trimmed_text() {
    let body = "  \t  indented findMe here\n";
    let scripts = vec![make_script("indent-test", None, Some(body))];

    let results = fuzzy_search_scripts(&scripts, "findMe");
    assert_eq!(results.len(), 1);
    let cm = results[0].content_match.as_ref().unwrap();
    // line_text is trimmed
    assert_eq!(cm.line_text, "indented findMe here");
    // Verify that the indices point to the right characters in the trimmed text
    for &idx in &cm.line_match_indices {
        assert!(
            idx < cm.line_text.len(),
            "index {} out of bounds for trimmed text (len {})",
            idx,
            cm.line_text.len()
        );
    }
    // The highlighted chars should spell out "findMe"
    let highlighted: String = cm
        .line_match_indices
        .iter()
        .map(|&i| cm.line_text.as_bytes()[i] as char)
        .collect();
    assert_eq!(highlighted, "findMe");
}

// ── Edge cases ─────────────────────────────────────────────────────────

#[test]
fn content_search_is_case_insensitive() {
    let scripts = vec![make_script(
        "case-test",
        None,
        Some("const MySpecialVar = 42;\n"),
    )];

    let results = fuzzy_search_scripts(&scripts, "myspecialvar");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].match_kind, ScriptMatchKind::Content);
}

#[test]
fn no_content_search_when_name_already_matches() {
    let scripts = vec![make_script(
        "hello-world",
        None,
        Some("// hello this is the body\n"),
    )];

    let results = fuzzy_search_scripts(&scripts, "hello");
    assert_eq!(results.len(), 1);
    assert_ne!(results[0].match_kind, ScriptMatchKind::Content);
    assert!(
        results[0].content_match.is_none(),
        "content_match should be None when name matched"
    );
}

#[test]
fn current_app_recipe_headers_do_not_contribute_body_matches() {
    let scripts = vec![make_script(
        "generated-current-app-script",
        None,
        Some(
            "// Current-App-Recipe-Name: summarize all the text on the screen\n\
             // Current-App-Recipe-Base64: ZXdvZ0lDSnlaV05wY0dWVWVYQmxJam9nSW1OMWNuSmxiblJCY0hCRGIyMXRZVzVrSWl3S0lDSmxabVpsWTNScGRtVlJkV1Z5ZVNJNklDSnpkVzF0WVhKcGVtVWdZV3hzSUhSb1pTQjBaWGgwSUc5dUlIUm9aU0J6WTNKbFpXNGlDZ29nSUhCeWIyMXdkRkpqWldsd2RDSTZJSHNLQ0FvPQ==\n",
        ),
    )];

    let results = fuzzy_search_scripts(&scripts, "summarize all the text on the screen");
    assert!(
        results.is_empty(),
        "current-app recipe headers are machine metadata and must not create launcher matches"
    );
}

#[test]
fn natural_language_question_does_not_fuzzy_match_unrelated_script_body() {
    let scripts = vec![make_script(
        "Create GitHub Issue",
        Some(
            "Create a GitHub issue from ;github menu syntax using a saved GitHub device-flow token",
        ),
        Some(include_str!(
            "../scripts/examples/menu-syntax/capture-github-local.ts"
        )),
    )];

    let results = fuzzy_search_scripts(&scripts, "What is the reasoning");

    assert!(
        results.is_empty(),
        "natural-language questions should not surface unrelated scripts through sparse body fuzziness"
    );
}

// ── Fuzzy body matching ──────────────────────────────────────────────

#[test]
fn content_search_uses_fuzzy_matching_for_body_lines() {
    let scripts = vec![make_script(
        "alpha",
        None,
        Some("import { uniqueTokenXyz } from 'lib';\n"),
    )];

    let results = fuzzy_search_scripts(&scripts, "untknxyz");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].match_kind, ScriptMatchKind::Content);
    assert_eq!(results[0].score, 5);
    assert_eq!(results[0].content_match.as_ref().unwrap().line_number, 1);
}

#[test]
fn multi_word_content_search_requires_phrase_match() {
    let scripts = vec![make_script(
        "alpha",
        None,
        Some("line one\nconst message = 'What is the reasoning';\nline three\n"),
    )];

    let results = fuzzy_search_scripts(&scripts, "what is the reasoning");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].match_kind, ScriptMatchKind::Content);
    assert_eq!(results[0].content_match.as_ref().unwrap().line_number, 2);
}

#[test]
fn content_search_prefers_best_fuzzy_line_not_first_line() {
    let scripts = vec![make_script(
        "alpha",
        None,
        Some("token partial\nveryUniqueTokenHere\nanother token\n"),
    )];

    let results = fuzzy_search_scripts(&scripts, "vutoknhr");
    assert_eq!(results.len(), 1);
    let cm = results[0].content_match.as_ref().unwrap();
    assert_eq!(cm.line_number, 2);
    assert!(cm.line_text.contains("veryUniqueTokenHere"));
}

#[test]
fn content_match_byte_range_tracks_the_matched_span() {
    let scripts = vec![make_script("gamma", None, Some("const alpha = beta;\n"))];

    let alpha_results = fuzzy_search_scripts(&scripts, "alpha");
    let beta_results = fuzzy_search_scripts(&scripts, "beta");

    let alpha_match = alpha_results[0]
        .content_match
        .as_ref()
        .expect("alpha query should produce a content match");
    let beta_match = beta_results[0]
        .content_match
        .as_ref()
        .expect("beta query should produce a content match");

    assert_eq!(alpha_match.byte_range, 6..11);
    assert_eq!(beta_match.byte_range, 14..18);
    assert_ne!(alpha_match.byte_range, beta_match.byte_range);
}

#[test]
fn content_bonus_stacks_with_name_match() {
    // Script name matches AND body matches — score should include both
    let scripts = vec![make_script(
        "finder",
        None,
        Some("use finder module here\n"),
    )];

    let results = fuzzy_search_scripts(&scripts, "finder");
    assert_eq!(results.len(), 1);
    // Name match contributes > 0, content adds +5
    assert!(
        results[0].score > 5,
        "score ({}) must include name match contribution",
        results[0].score
    );
    // match_kind stays Name since name is the stronger tier
    assert_ne!(results[0].match_kind, ScriptMatchKind::Content);
    // content_match is None because primary_text_match suppresses the snippet
    assert!(results[0].content_match.is_none());
}

#[test]
fn script_without_body_does_not_match_on_content() {
    let scripts = vec![make_script("nobody", None, None)];

    let results = fuzzy_search_scripts(&scripts, "xyzNonExistent");
    assert!(
        results.is_empty(),
        "script with no body should not match arbitrary query"
    );
}

#[test]
fn empty_query_returns_all_scripts_without_content_match() {
    let scripts = vec![
        make_script("a", None, Some("body a\n")),
        make_script("b", None, Some("body b\n")),
    ];

    let results = fuzzy_search_scripts(&scripts, "");
    assert_eq!(results.len(), 2);
    for r in &results {
        assert!(r.content_match.is_none());
        assert_eq!(r.match_kind, ScriptMatchKind::Name);
    }
}

// ── Dominant primary-text field regression ─────────────────────────────

#[test]
fn alias_match_does_not_block_content_snippet_when_name_and_description_do_not_match() {
    let scripts = vec![Arc::new(Script {
        name: "utility".to_string(),
        path: PathBuf::from("/tmp/test-scripts/utility.ts"),
        extension: "ts".to_string(),
        description: None,
        icon: None,
        alias: Some("tok".to_string()),
        shortcut: None,
        typed_metadata: None,
        schema: None,
        plugin_id: String::new(),
        plugin_title: None,
        kit_name: Some("test".to_string()),
        body: Some("const tok = 1;\n".into()),
    })];

    let results = fuzzy_search_scripts(&scripts, "tok");
    assert_eq!(results.len(), 1);
    // Alias contributes 80 + content contributes 5 = 85
    assert_eq!(results[0].score, 85);
    // No primary text field (name/filename/description) matched, so match_kind is Content
    assert_eq!(results[0].match_kind, ScriptMatchKind::Content);
    assert_eq!(
        results[0].content_match.as_ref().map(|cm| cm.line_number),
        Some(1)
    );
}

#[test]
fn filename_match_remains_primary_when_body_also_matches() {
    let scripts = vec![Arc::new(Script {
        name: "launcher".to_string(),
        path: PathBuf::from("/tmp/test-scripts/utility.ts"),
        extension: "ts".to_string(),
        description: None,
        icon: None,
        alias: None,
        shortcut: None,
        typed_metadata: None,
        schema: None,
        plugin_id: String::new(),
        plugin_title: None,
        kit_name: Some("test".to_string()),
        body: Some("import './utility.ts';\n".into()),
    })];

    let results = fuzzy_search_scripts(&scripts, "utility.ts");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].match_kind, ScriptMatchKind::Filename);
    assert!(results[0].content_match.is_none());
}
