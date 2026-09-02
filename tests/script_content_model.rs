//! Integration tests for script content model and content search.
//!
//! Validates that:
//! - Scripts loaded from disk retain their body text
//! - ScriptMatch can encode content-hit metadata (line number, snippet, match indices)
//! - Content search produces correct match_kind and content_match data
//! - Content matches score below name/description matches

use std::path::PathBuf;
use std::sync::Arc;

use script_kit_gpui::scripts::{
    MatchIndices, Script, ScriptContentMatch, ScriptMatch, ScriptMatchKind,
};

/// Helper to create a test Script with body content
fn make_script(name: &str, body: Option<&str>) -> Arc<Script> {
    Arc::new(Script {
        name: name.to_string(),
        path: PathBuf::from(format!("/tmp/test-scripts/{}.ts", name)),
        extension: "ts".to_string(),
        description: Some(format!("Test script: {}", name)),
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

#[test]
fn script_retains_body_text() {
    let body = "import { readFile } from 'fs';\nconsole.log('hello world');\n";
    let script = make_script("read-file", Some(body));
    assert_eq!(script.body.as_deref(), Some(body));
}

#[test]
fn script_without_body_has_none() {
    let script = make_script("no-body", None);
    assert!(script.body.is_none());
}

#[test]
fn script_metadata_preserved_with_body() {
    let script = make_script("my-script", Some("let x = 1;"));
    assert_eq!(script.name, "my-script");
    assert_eq!(script.extension, "ts");
    assert_eq!(
        script.description.as_deref(),
        Some("Test script: my-script")
    );
    assert_eq!(script.kit_name.as_deref(), Some("test"));
    assert!(script.body.is_some());
}

#[test]
fn script_match_kind_defaults_to_name() {
    assert_eq!(ScriptMatchKind::default(), ScriptMatchKind::Name);
}

#[test]
fn script_match_encodes_content_hit() {
    let script = make_script("searcher", Some("line1\nfoo bar baz\nline3"));
    let sm = ScriptMatch {
        script,
        score: 5,
        filename: "searcher.ts".to_string(),
        match_indices: MatchIndices::default(),
        match_kind: ScriptMatchKind::Content,
        content_match: Some(ScriptContentMatch {
            line_number: 2,
            line_text: "foo bar baz".to_string(),
            line_match_indices: vec![4, 5, 6],
            byte_range: 10..13,
        }),
        match_evidence: None,
    };

    assert_eq!(sm.match_kind, ScriptMatchKind::Content);
    let cm = sm
        .content_match
        .as_ref()
        .expect("should have content match");
    assert_eq!(cm.line_number, 2);
    assert_eq!(cm.line_text, "foo bar baz");
    assert_eq!(cm.line_match_indices, vec![4, 5, 6]);
}

#[test]
fn script_match_without_content_hit() {
    let script = make_script("hello", None);
    let sm = ScriptMatch {
        script,
        score: 100,
        filename: "hello.ts".to_string(),
        match_indices: MatchIndices::default(),
        match_kind: ScriptMatchKind::Name,
        content_match: None,
        match_evidence: None,
    };

    assert_eq!(sm.match_kind, ScriptMatchKind::Name);
    assert!(sm.content_match.is_none());
}

#[test]
fn content_search_finds_body_only_match() {
    use script_kit_gpui::scripts::fuzzy_search_scripts;

    let scripts = vec![
        make_script("alpha", Some("import { uniqueTokenXyz } from 'lib';\n")),
        make_script("beta", Some("console.log('nothing here');\n")),
    ];

    let results = fuzzy_search_scripts(&scripts, "uniqueTokenXyz");
    assert_eq!(
        results.len(),
        1,
        "Only one script should match body content"
    );
    assert_eq!(results[0].script.name, "alpha");
    assert_eq!(results[0].match_kind, ScriptMatchKind::Content);

    let cm = results[0]
        .content_match
        .as_ref()
        .expect("should have content match");
    assert_eq!(cm.line_number, 1);
    assert!(cm.line_text.contains("uniqueTokenXyz"));
}

#[test]
fn content_match_scores_below_name_match() {
    use script_kit_gpui::scripts::fuzzy_search_scripts;

    // "finder" in name vs "finder" only in body
    let scripts = vec![
        make_script("finder", Some("unrelated body content\n")),
        make_script("other-script", Some("use finder module here\n")),
    ];

    let results = fuzzy_search_scripts(&scripts, "finder");
    assert!(results.len() >= 1);
    // Name match should rank first
    assert_eq!(results[0].script.name, "finder");
    assert_ne!(results[0].match_kind, ScriptMatchKind::Content);

    // If body match appears, it should be ranked lower
    if results.len() > 1 {
        assert!(results[1].score < results[0].score);
    }
}

#[test]
fn content_search_returns_correct_line_number() {
    use script_kit_gpui::scripts::fuzzy_search_scripts;

    let body = "line one\nline two\nthe secret token here\nline four\n";
    let scripts = vec![make_script("multi-line", Some(body))];

    let results = fuzzy_search_scripts(&scripts, "secret token");
    assert_eq!(results.len(), 1);
    let cm = results[0]
        .content_match
        .as_ref()
        .expect("should have content match");
    assert_eq!(cm.line_number, 3);
    assert!(cm.line_text.contains("secret token"));
}

#[test]
fn content_search_is_case_insensitive() {
    use script_kit_gpui::scripts::fuzzy_search_scripts;

    let scripts = vec![make_script("case-test", Some("const MySpecialVar = 42;\n"))];

    let results = fuzzy_search_scripts(&scripts, "myspecialvar");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].match_kind, ScriptMatchKind::Content);
}

#[test]
fn no_content_search_when_name_already_matches() {
    use script_kit_gpui::scripts::fuzzy_search_scripts;

    // Name contains "hello" and body also contains "hello" — should match on name, not content
    let scripts = vec![make_script(
        "hello-world",
        Some("// hello this is the body\n"),
    )];

    let results = fuzzy_search_scripts(&scripts, "hello");
    assert_eq!(results.len(), 1);
    // Should NOT be a content match since name matched
    assert_ne!(results[0].match_kind, ScriptMatchKind::Content);
    assert!(results[0].content_match.is_none());
}
