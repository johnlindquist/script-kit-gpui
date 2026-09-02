// ============================================
// EXISTING SCRIPTLET PARSING TESTS
// ============================================

#[test]
fn test_parse_scriptlet_basic() {
    let section = "## Test Snippet\n\n```ts\nconst x = 1;\n```";
    let scriptlet = parse_scriptlet_section(section, None);
    assert!(scriptlet.is_some());
    let s = scriptlet.unwrap();
    assert_eq!(s.name, "Test Snippet");
    assert_eq!(s.tool, "ts");
    assert_eq!(s.code, "const x = 1;");
    assert_eq!(s.shortcut, None);
}

#[test]
fn test_parse_scriptlet_with_metadata() {
    let section = "## Open File\n\n<!-- \nshortcut: cmd o\n-->\n\n```ts\nawait exec('open')\n```";
    let scriptlet = parse_scriptlet_section(section, None);
    assert!(scriptlet.is_some());
    let s = scriptlet.unwrap();
    assert_eq!(s.name, "Open File");
    assert_eq!(s.tool, "ts");
    assert_eq!(s.shortcut, Some("cmd o".to_string()));
}

#[test]
fn test_parse_scriptlet_with_description() {
    let section = "## Test\n\n<!-- \ndescription: Test description\n-->\n\n```bash\necho test\n```";
    let scriptlet = parse_scriptlet_section(section, None);
    assert!(scriptlet.is_some());
    let s = scriptlet.unwrap();
    assert_eq!(s.description, Some("Test description".to_string()));
}

#[test]
fn test_parse_scriptlet_with_keyword() {
    let section = "## Execute Plan\n\n<!-- \nkeyword: plan,,\n-->\n\n```paste\nPlease execute\n```";
    let scriptlet = parse_scriptlet_section(section, None);
    assert!(scriptlet.is_some());
    let s = scriptlet.unwrap();
    assert_eq!(s.keyword, Some("plan,,".to_string()));
    assert_eq!(s.tool, "paste");
}

#[test]
fn test_parse_scriptlet_ignores_commented_codefence_keyword_and_paste_block() {
    let section = r#"## Execute Active Plan

```metadata
keyword: active,,
```

```paste
active paste
```

<!--
```metadata
keyword: commented,,
```

```paste
commented paste
```
-->"#;

    let scriptlet = parse_scriptlet_section(section, None).unwrap();

    assert_eq!(scriptlet.keyword, Some("active,,".to_string()));
    assert_eq!(scriptlet.tool, "paste");
    assert_eq!(scriptlet.code, "active paste");
}

#[test]
fn test_parse_scriptlet_does_not_fallback_to_commented_codefence_keyword() {
    let section = r#"## Execute Active Plan

<!--
```metadata
keyword: commented,,
```
-->

```paste
active paste
```"#;

    let scriptlet = parse_scriptlet_section(section, None).unwrap();

    assert_eq!(scriptlet.keyword, None);
    assert_eq!(scriptlet.tool, "paste");
    assert_eq!(scriptlet.code, "active paste");
}

#[test]
fn test_extract_code_block_ts() {
    let text = "Some text\n```ts\nconst x = 1;\n```\nMore text";
    let result = extract_code_block(text);
    assert!(result.is_some());
    let (lang, code) = result.unwrap();
    assert_eq!(lang, "ts");
    assert_eq!(code, "const x = 1;");
}

#[test]
fn test_extract_code_block_bash() {
    let text = "```bash\necho hello\necho world\n```";
    let result = extract_code_block(text);
    assert!(result.is_some());
    let (lang, code) = result.unwrap();
    assert_eq!(lang, "bash");
    assert_eq!(code, "echo hello\necho world");
}

#[test]
fn test_extract_html_metadata_shortcut() {
    let text = "<!-- \nshortcut: opt s\n-->";
    let metadata = extract_html_comment_metadata(text);
    assert_eq!(metadata.get("shortcut"), Some(&"opt s".to_string()));
}

#[test]
fn test_extract_html_metadata_multiple() {
    let text = "<!-- \nshortcut: cmd k\nkeyword: foo,,\ndescription: Test\n-->";
    let metadata = extract_html_comment_metadata(text);
    assert_eq!(metadata.get("shortcut"), Some(&"cmd k".to_string()));
    assert_eq!(metadata.get("keyword"), Some(&"foo,,".to_string()));
    assert_eq!(metadata.get("description"), Some(&"Test".to_string()));
}

#[test]
fn test_extract_html_metadata_ignores_commented_codefence_body() {
    let text =
        "<!--\nshortcut: cmd k\n```metadata\nkeyword: ignored,,\n```\ndescription: Test\n-->";
    let metadata = extract_html_comment_metadata(text);
    assert_eq!(metadata.get("shortcut"), Some(&"cmd k".to_string()));
    assert_eq!(metadata.get("keyword"), None);
    assert_eq!(metadata.get("description"), Some(&"Test".to_string()));
}

#[test]
fn test_parse_scriptlet_none_without_heading() {
    let section = "Some text without heading\n```ts\ncode\n```";
    let scriptlet = parse_scriptlet_section(section, None);
    assert!(scriptlet.is_none());
}

#[test]
fn test_parse_scriptlet_none_without_code_block() {
    let section = "## Name\nNo code block here";
    let scriptlet = parse_scriptlet_section(section, None);
    assert!(scriptlet.is_none());
}

#[test]
fn test_script_struct_has_required_fields() {
    let script = Script {
        name: "test".to_string(),
        path: PathBuf::from("/test/path"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    };
    assert_eq!(script.name, "test");
    assert_eq!(script.extension, "ts");
}

#[test]
fn test_fuzzy_search_by_name() {
    let scripts = wrap_scripts(vec![
        Script {
            name: "openfile".to_string(),
            path: PathBuf::from("/test/openfile.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("Open a file dialog".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "savefile".to_string(),
            path: PathBuf::from("/test/savefile.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ]);

    let results = fuzzy_search_scripts(&scripts, "open");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].script.name, "openfile");
    assert!(results[0].score > 0);
}

#[test]
fn test_fuzzy_search_empty_query() {
    let scripts = wrap_scripts(vec![Script {
        name: "test1".to_string(),
        path: PathBuf::from("/test/test1.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }]);

    let results = fuzzy_search_scripts(&scripts, "");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].score, 0);
}

#[test]
fn test_fuzzy_search_ranking() {
    let scripts = wrap_scripts(vec![
        Script {
            name: "openfile".to_string(),
            path: PathBuf::from("/test/openfile.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("Open a file dialog".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "open".to_string(),
            path: PathBuf::from("/test/open.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("Basic open function".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "reopen".to_string(),
            path: PathBuf::from("/test/reopen.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ]);

    let results = fuzzy_search_scripts(&scripts, "open");
    // Should have all three results
    assert_eq!(results.len(), 3);
    // "open" should be first (exact match at start: 100 + fuzzy match: 50 = 150)
    assert_eq!(results[0].script.name, "open");
    // "openfile" should be second (substring at start: 100 + fuzzy match: 50 = 150, but "open" comes first alphabetically in tie)
    assert_eq!(results[1].script.name, "openfile");
    // "reopen" should be third (substring not at start: 75 + fuzzy match: 50 = 125)
    assert_eq!(results[2].script.name, "reopen");
}

#[test]
fn test_fuzzy_search_scriptlets() {
    let scriptlets = wrap_scriptlets(vec![
        test_scriptlet_with_desc("Copy Text", "ts", "copy()", "Copy current selection"),
        test_scriptlet("Paste Code", "ts", "paste()"),
    ]);

    let results = fuzzy_search_scriptlets(&scriptlets, "copy");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scriptlet.name, "Copy Text");
    assert!(results[0].score > 0);
}

#[test]
fn test_fuzzy_search_unified() {
    let scripts = wrap_scripts(vec![Script {
        name: "open".to_string(),
        path: PathBuf::from("/test/open.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: Some("Open a file".to_string()),
        alias: None,
        shortcut: None,
        ..Default::default()
    }]);

    let scriptlets = wrap_scriptlets(vec![test_scriptlet_with_desc(
        "Open Browser",
        "ts",
        "open()",
        "Open in browser",
    )]);

    let results = fuzzy_search_unified(&scripts, &scriptlets, "open");
    assert_eq!(results.len(), 2);

    // First result should be the script (same score but scripts come first)
    match &results[0] {
        SearchResult::Script(sm) => assert_eq!(sm.script.name, "open"),
        _ => panic!("Expected script"),
    }

    // Second result should be the scriptlet
    match &results[1] {
        SearchResult::Scriptlet(sm) => assert_eq!(sm.scriptlet.name, "Open Browser"),
        _ => panic!("Expected scriptlet"),
    }
}

#[test]
fn test_search_result_type_label() {
    let script = SearchResult::Script(ScriptMatch {
        script: Arc::new(Script {
            name: "test".to_string(),
            path: PathBuf::from("/test.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
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
    });

    let scriptlet = SearchResult::Scriptlet(ScriptletMatch {
        scriptlet: Arc::new(test_scriptlet("snippet", "ts", "code")),
        score: 50,
        display_file_path: None,
        match_indices: MatchIndices::default(),
        match_evidence: None,
    });

    assert_eq!(script.type_label(), "Script");
    assert_eq!(scriptlet.type_label(), "Snippet");
}
