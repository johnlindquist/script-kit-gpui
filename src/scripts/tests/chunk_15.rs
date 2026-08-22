// ============================================
// SEARCH RESULT HIGHLIGHT TESTS
// ============================================

#[test]
fn test_compute_match_indices_for_script_result() {
    let script_match = ScriptMatch {
        script: Arc::new(Script {
            name: "OpenFile".to_string(),
            path: PathBuf::from("/openfile.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        }),
        score: 100,
        filename: "openfile.ts".to_string(),
        match_indices: MatchIndices::default(),
        match_kind: ScriptMatchKind::default(),
        content_match: None,
        match_evidence: None,
    };
    let result = SearchResult::Script(script_match);

    let indices = compute_match_indices_for_result(&result, "of");
    assert!(!indices.name_indices.is_empty());
    assert_eq!(indices.name_indices, vec![0, 4]); // 'O' at 0, 'F' at 4
}

#[test]
fn test_compute_match_indices_for_scriptlet_result() {
    let scriptlet_match = ScriptletMatch {
        scriptlet: Arc::new(test_scriptlet("Copy Text", "ts", "copy()")),
        score: 100,
        display_file_path: Some("copy.md#copy-text".to_string()),
        match_indices: MatchIndices::default(),
        match_evidence: None,
    };
    let result = SearchResult::Scriptlet(scriptlet_match);

    let indices = compute_match_indices_for_result(&result, "ct");
    assert!(!indices.name_indices.is_empty());
    assert_eq!(indices.name_indices, vec![0, 5]); // 'C' at 0, 'T' at 5
}

#[test]
fn test_compute_match_indices_empty_query() {
    let script_match = ScriptMatch {
        script: Arc::new(Script {
            name: "Test".to_string(),
            path: PathBuf::from("/test.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        }),
        score: 0,
        filename: "test.ts".to_string(),
        match_indices: MatchIndices::default(),
        match_kind: ScriptMatchKind::default(),
        content_match: None,
        match_evidence: None,
    };
    let result = SearchResult::Script(script_match);

    let indices = compute_match_indices_for_result(&result, "");
    assert!(indices.name_indices.is_empty());
    assert!(indices.filename_indices.is_empty());
}

#[test]
fn test_scriptlet_code_search_gated_by_length() {
    // The search relevance contract restricts scriptlet matching to visible
    // or exact low-tier fields (name, file path, description, keyword, alias,
    // shortcut, group, tool). Scriptlet source/code is intentionally NOT
    // searched anymore, so a query that only occurs in the code body must
    // never match — regardless of query length.
    let scriptlets = wrap_scriptlets(vec![test_scriptlet(
        "Utility",
        "ts",
        "contains_xyz_function()",
    )]);

    let results = fuzzy_search_scriptlets(&scriptlets, "xyz");
    assert!(results.is_empty());

    let results = fuzzy_search_scriptlets(&scriptlets, "xyz_f");
    assert!(
        results.is_empty(),
        "scriptlet code/body is intentionally not searched"
    );
}

#[test]
fn test_scriptlet_code_search_skipped_when_name_matches() {
    // If name matches, code search is skipped (score > 0)
    let scriptlets = wrap_scriptlets(vec![test_scriptlet(
        "special_snippet",
        "ts",
        "unrelated_code()",
    )]);

    // Should match on name, not search code
    let results = fuzzy_search_scriptlets(&scriptlets, "special");
    assert_eq!(results.len(), 1);
    // Score should be from name match, not code match
    assert!(results[0].score > 5);
}
