// ============================================
// EDGE CASES: Missing Files, Malformed Data
// ============================================

#[test]
fn test_extract_code_block_no_fence() {
    let text = "No code block here, just text";
    let result = extract_code_block(text);
    assert!(result.is_none());
}

#[test]
fn test_extract_code_block_incomplete_fence() {
    let text = "```ts\ncode here\nno closing fence";
    let result = extract_code_block(text);
    assert!(result.is_none());
}

#[test]
fn test_extract_code_block_empty() {
    let text = "```ts\n```";
    let result = extract_code_block(text);
    assert!(result.is_some());
    let (lang, code) = result.unwrap();
    assert_eq!(lang, "ts");
    assert!(code.is_empty());
}

#[test]
fn test_extract_code_block_no_language() {
    let text = "```\ncode here\n```";
    let result = extract_code_block(text);
    assert!(result.is_some());
    let (lang, code) = result.unwrap();
    assert!(lang.is_empty());
    assert_eq!(code, "code here");
}

#[test]
fn test_extract_code_block_with_multiple_fences() {
    let text = "```ts\nfirst\n```\n\n```bash\nsecond\n```";
    let result = extract_code_block(text);
    assert!(result.is_some());
    let (lang, code) = result.unwrap();
    assert_eq!(lang, "ts");
    assert_eq!(code, "first");
}

#[test]
fn test_parse_scriptlet_empty_heading() {
    let section = "## \n\n```ts\ncode\n```";
    let scriptlet = parse_scriptlet_section(section, None);
    assert!(scriptlet.is_none());
}

#[test]
fn test_parse_scriptlet_whitespace_only_heading() {
    let section = "##   \n\n```ts\ncode\n```";
    let scriptlet = parse_scriptlet_section(section, None);
    assert!(scriptlet.is_none());
}

#[test]
fn test_extract_html_metadata_empty_comment() {
    let text = "<!-- -->";
    let metadata = extract_html_comment_metadata(text);
    assert!(metadata.is_empty());
}

#[test]
fn test_extract_html_metadata_no_comments() {
    let text = "Some text without HTML comments";
    let metadata = extract_html_comment_metadata(text);
    assert!(metadata.is_empty());
}

#[test]
fn test_extract_html_metadata_malformed_colon() {
    let text = "<!-- \nkey_without_colon value\n-->";
    let metadata = extract_html_comment_metadata(text);
    assert!(metadata.is_empty());
}

#[test]
fn test_extract_html_metadata_unclosed_comment() {
    let text = "<!-- metadata here";
    let metadata = extract_html_comment_metadata(text);
    assert!(metadata.is_empty());
}

#[test]
fn test_extract_html_metadata_with_colons_in_value() {
    let text = "<!-- \ndescription: Full URL: https://example.com\n-->";
    let metadata = extract_html_comment_metadata(text);
    assert_eq!(
        metadata.get("description"),
        Some(&"Full URL: https://example.com".to_string())
    );
}

#[test]
fn test_fuzzy_search_no_results() {
    let scripts = wrap_scripts(vec![Script {
        name: "test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }]);

    let results = fuzzy_search_scripts(&scripts, "nonexistent");
    assert_eq!(results.len(), 0);
}

#[test]
fn test_fuzzy_search_all_match() {
    let scripts = wrap_scripts(vec![
        Script {
            name: "test1".to_string(),
            path: PathBuf::from("/test1.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "test2".to_string(),
            path: PathBuf::from("/test2.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ]);

    let results = fuzzy_search_scripts(&scripts, "test");
    assert_eq!(results.len(), 2);
}

#[test]
fn test_fuzzy_search_by_description() {
    let scripts = wrap_scripts(vec![
        Script {
            name: "foo".to_string(),
            path: PathBuf::from("/foo.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("database connection helper".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "bar".to_string(),
            path: PathBuf::from("/bar.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("ui component".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ]);

    let results = fuzzy_search_scripts(&scripts, "database");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].script.name, "foo");
}

#[test]
fn test_fuzzy_search_by_path() {
    let scripts = wrap_scripts(vec![
        Script {
            name: "foo".to_string(),
            path: PathBuf::from("/home/user/.scriptplugins/main/scripts/open.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "bar".to_string(),
            path: PathBuf::from("/home/user/.other/bar.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ]);

    // The search relevance contract restricts script matching to visible or
    // exact low-tier fields (name, filename, alias, description, body, …).
    // Full path/directory components are intentionally NOT searched anymore,
    // so a directory-only query must not match any script.
    let results = fuzzy_search_scripts(&scripts, ".scriptplugins");
    assert!(
        results.is_empty(),
        "directory components of the path must not be searchable"
    );

    // The filename portion of the path is still searchable.
    let results = fuzzy_search_scripts(&scripts, "open.ts");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].script.name, "foo");
}

#[test]
fn test_fuzzy_search_score_ordering() {
    let scripts = wrap_scripts(vec![
        Script {
            name: "exactmatch".to_string(),
            path: PathBuf::from("/test.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "other".to_string(),
            path: PathBuf::from("/test.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("exactmatch in description".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ]);

    let results = fuzzy_search_scripts(&scripts, "exactmatch");
    // Name match should score higher than description match
    assert_eq!(results[0].script.name, "exactmatch");
    assert!(results[0].score > results[1].score);
}

#[test]
fn test_fuzzy_search_scriptlets_by_tool() {
    let scriptlets = wrap_scriptlets(vec![
        test_scriptlet("Snippet1", "bash", "code"),
        test_scriptlet("Snippet2", "ts", "code"),
    ]);

    let results = fuzzy_search_scriptlets(&scriptlets, "bash");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scriptlet.name, "Snippet1");
}

#[test]
fn test_fuzzy_search_scriptlets_no_results() {
    let scriptlets = wrap_scriptlets(vec![test_scriptlet_with_desc(
        "Copy Text",
        "ts",
        "copy()",
        "Copy current selection",
    )]);

    let results = fuzzy_search_scriptlets(&scriptlets, "paste");
    assert_eq!(results.len(), 0);
}

#[test]
fn test_fuzzy_search_unified_empty_query() {
    let scripts = wrap_scripts(vec![Script {
        name: "script1".to_string(),
        path: PathBuf::from("/script1.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }]);

    let scriptlets = wrap_scriptlets(vec![test_scriptlet("Snippet1", "ts", "code")]);

    let results = fuzzy_search_unified(&scripts, &scriptlets, "");
    assert_eq!(results.len(), 2);
}

#[test]
fn test_fuzzy_search_unified_scripts_first() {
    let scripts = wrap_scripts(vec![Script {
        name: "test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: Some("test script".to_string()),
        alias: None,
        shortcut: None,
        ..Default::default()
    }]);

    let scriptlets = wrap_scriptlets(vec![test_scriptlet_with_desc(
        "test",
        "ts",
        "test()",
        "test snippet",
    )]);

    let results = fuzzy_search_unified(&scripts, &scriptlets, "test");
    // When scores are equal, scripts should come first
    match &results[0] {
        SearchResult::Script(_) => {} // Correct
        SearchResult::Scriptlet(_) => panic!("Script should be first"),
        SearchResult::BuiltIn(_) => panic!("Script should be first"),
        SearchResult::App(_) => panic!("Script should be first"),
        SearchResult::Window(_) => panic!("Script should be first"),
        SearchResult::Agent(_) => panic!("Script should be first"),
        SearchResult::Skill(_) => panic!("Script should be first"),
        SearchResult::Fallback(_) => panic!("Script should be first"),
        SearchResult::ScriptIssue(_) => panic!("Script should be first"),
        _ => panic!("Script should be first"),
    }
}

#[test]
fn test_search_result_properties() {
    let script_match = ScriptMatch {
        script: Arc::new(Script {
            name: "TestScript".to_string(),
            path: PathBuf::from("/test.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("A test script".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        }),
        score: 100,
        filename: "test.ts".to_string(),
        match_indices: MatchIndices::default(),
        match_kind: ScriptMatchKind::default(),
        content_match: None,
        match_evidence: None,
    };

    let result = SearchResult::Script(script_match);

    assert_eq!(result.name(), "TestScript");
    assert_eq!(result.description(), Some("A test script"));
    assert_eq!(result.score(), 100);
    assert_eq!(result.type_label(), "Script");
}

#[test]
fn test_scriptlet_with_all_metadata() {
    let scriptlet = Scriptlet {
        icon: None,
        name: "Full Scriptlet".to_string(),
        description: Some("Complete metadata".to_string()),
        code: "code here".to_string(),
        tool: "bash".to_string(),
        shortcut: Some("cmd k".to_string()),
        keyword: Some("prompt,,".to_string()),
        group: None,
        file_path: None,
        command: None,
        alias: None,
        plugin_id: String::new(),
        plugin_title: None,
    };

    assert_eq!(scriptlet.name, "Full Scriptlet");
    assert_eq!(scriptlet.description, Some("Complete metadata".to_string()));
    assert_eq!(scriptlet.shortcut, Some("cmd k".to_string()));
    assert_eq!(scriptlet.keyword, Some("prompt,,".to_string()));
}

#[test]
fn test_parse_scriptlet_preserves_whitespace_in_code() {
    let section = "## WhitespaceTest\n\n```ts\n  const x = 1;\n    const y = 2;\n```";
    let scriptlet = parse_scriptlet_section(section, None);
    assert!(scriptlet.is_some());
    let s = scriptlet.unwrap();
    // Code should preserve relative indentation
    assert!(s.code.contains("const x"));
    assert!(s.code.contains("const y"));
}

#[test]
fn test_parse_scriptlet_multiline_code() {
    let section = "## MultiLine\n\n```ts\nconst obj = {\n  key: value,\n  other: thing\n};\nconsole.log(obj);\n```";
    let scriptlet = parse_scriptlet_section(section, None);
    assert!(scriptlet.is_some());
    let s = scriptlet.unwrap();
    assert!(s.code.contains("obj"));
    assert!(s.code.contains("console.log"));
}

#[test]
fn test_extract_metadata_case_insensitive_description() {
    // Metadata extraction is case-sensitive (looks for "// Description:")
    // Verify this behavior
    let script = Script {
        name: "test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None, // Would be extracted from file if existed
        alias: None,
        shortcut: None,
        ..Default::default()
    };
    assert_eq!(script.name, "test");
}
