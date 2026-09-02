//! Integration tests for content-match snippet rendering in the search list.
//!
//! Validates:
//! - Content matches produce a snippet description in "line_number: line_text" format
//! - Highlight indices are correctly offset by the line-number prefix
//! - Non-content matches preserve the original description behavior
//! - Content snippets carry through the full fuzzy_search_scripts pipeline

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
        body: body.map(Into::into),
    })
}

// ── Snippet format ───────────────────────────────────────────────────────

#[test]
fn content_match_produces_snippet_description() {
    let scripts = vec![make_script(
        "unrelated-name",
        Some("unrelated description"),
        Some("line one\nimport { superRareToken } from 'lib';\nline three\n"),
    )];

    let results = fuzzy_search_scripts(&scripts, "superRareToken");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].match_kind, ScriptMatchKind::Content);

    let cm = results[0]
        .content_match
        .as_ref()
        .expect("content_match must be populated");

    // Snippet should be "2: import { superRareToken } from 'lib';"
    let snippet = format!("{}: {}", cm.line_number, cm.line_text);
    assert_eq!(cm.line_number, 2);
    assert!(
        snippet.starts_with("2: "),
        "snippet should start with line number prefix"
    );
    assert!(
        snippet.contains("superRareToken"),
        "snippet should contain the matched text"
    );
}

// ── Highlight index offset ───────────────────────────────────────────────

#[test]
fn content_match_highlight_indices_offset_correctly() {
    let scripts = vec![make_script(
        "offset-test",
        None,
        Some("first line\nconst myUniqueVar = 42;\nthird line\n"),
    )];

    let results = fuzzy_search_scripts(&scripts, "myUniqueVar");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].match_kind, ScriptMatchKind::Content);

    let cm = results[0].content_match.as_ref().unwrap();
    assert_eq!(cm.line_number, 2);

    // The prefix "2: " has length 3
    let prefix_len = format!("{}: ", cm.line_number).len();
    assert_eq!(prefix_len, 3);

    // After offset, all indices should be valid within the full snippet
    let full_snippet = format!("{}: {}", cm.line_number, cm.line_text);
    for &idx in &cm.line_match_indices {
        let offset_idx = idx + prefix_len;
        assert!(
            offset_idx < full_snippet.len(),
            "offset index {} out of bounds for snippet (len {})",
            offset_idx,
            full_snippet.len()
        );
    }
}

// ── Non-content matches unchanged ────────────────────────────────────────

#[test]
fn name_match_preserves_original_description() {
    let scripts = vec![make_script(
        "clipboard-manager",
        Some("Manage your clipboard history"),
        Some("import clipboard from 'module';\n"),
    )];

    let results = fuzzy_search_scripts(&scripts, "clipboard");
    assert_eq!(results.len(), 1);
    // Name matches should NOT be Content
    assert_ne!(results[0].match_kind, ScriptMatchKind::Content);
    assert!(
        results[0].content_match.is_none(),
        "name match should not have content_match"
    );
    // The original description should be used (not a snippet)
    assert_eq!(
        results[0].script.description.as_deref(),
        Some("Manage your clipboard history")
    );
}

#[test]
fn description_match_preserves_original_description() {
    let scripts = vec![make_script(
        "my-tool",
        Some("Searches for the rareDescToken efficiently"),
        Some("nothing matching here\n"),
    )];

    let results = fuzzy_search_scripts(&scripts, "rareDescToken");
    assert_eq!(results.len(), 1);
    assert_ne!(results[0].match_kind, ScriptMatchKind::Content);
    assert_eq!(
        results[0].script.description.as_deref(),
        Some("Searches for the rareDescToken efficiently")
    );
}

// ── Multi-digit line number prefix ───────────────────────────────────────

#[test]
fn content_match_multidigit_line_number_offsets_correctly() {
    // Create a body with many lines so the match is on a high line number
    let mut body = String::new();
    for i in 1..=99 {
        body.push_str(&format!("line {}\n", i));
    }
    body.push_str("the xyzMagicToken lives here\n");

    let scripts = vec![make_script("deep-match", None, Some(&body))];

    let results = fuzzy_search_scripts(&scripts, "xyzMagicToken");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].match_kind, ScriptMatchKind::Content);

    let cm = results[0].content_match.as_ref().unwrap();
    assert_eq!(cm.line_number, 100);

    // "100: " has length 5
    let prefix_len = format!("{}: ", cm.line_number).len();
    assert_eq!(prefix_len, 5);

    // Verify offset indices are all within bounds of the full snippet
    let full_snippet = format!("{}: {}", cm.line_number, cm.line_text);
    for &idx in &cm.line_match_indices {
        let offset_idx = idx + prefix_len;
        assert!(
            offset_idx < full_snippet.len(),
            "offset index {} out of bounds for snippet '{}' (len {})",
            offset_idx,
            full_snippet,
            full_snippet.len()
        );
    }
}
