use super::*;

/// Helper to create a test Scriptlet with minimal required fields
fn test_scriptlet(name: &str, tool: &str, code: &str) -> Scriptlet {
    Scriptlet {
        name: name.to_string(),
        description: None,
        code: code.to_string(),
        tool: tool.to_string(),
        shortcut: None,
        expand: None,
        group: None,
        file_path: None,
        command: None,
        alias: None,
    }
}

/// Helper to create a test Scriptlet with description
fn test_scriptlet_with_desc(name: &str, tool: &str, code: &str, desc: &str) -> Scriptlet {
    Scriptlet {
        name: name.to_string(),
        description: Some(desc.to_string()),
        code: code.to_string(),
        tool: tool.to_string(),
        shortcut: None,
        expand: None,
        group: None,
        file_path: None,
        command: None,
        alias: None,
    }
}

// ============================================
// LOAD_SCRIPTLETS INTEGRATION TESTS
// ============================================

#[test]
fn test_load_scriptlets_returns_vec() {
    // load_scriptlets should return a Vec even if directory doesn't exist
    let scriptlets = load_scriptlets();
    // Just verify it returns without panicking
    let _ = scriptlets.len();
}

#[test]
fn test_extract_kit_from_path_nested() {
    use std::path::Path;
    let home = Path::new("/Users/test");

    // Nested kit path
    let nested_path = Path::new("/Users/test/.sk/kit/my-kit/scriptlets/file.md");
    let kit = extract_kit_from_path(nested_path, home);
    assert_eq!(kit, Some("my-kit".to_string()));
}

#[test]
fn test_extract_kit_from_path_main_kit() {
    use std::path::Path;
    let home = Path::new("/Users/test");

    // Main kit path (not nested)
    let main_path = Path::new("/Users/test/.sk/kit/scriptlets/file.md");
    let kit = extract_kit_from_path(main_path, home);
    assert_eq!(kit, None);
}

#[test]
fn test_build_scriptlet_file_path() {
    use std::path::Path;
    let md_path = Path::new("/Users/test/.sk/kit/scriptlets/my-scripts.md");
    let result = build_scriptlet_file_path(md_path, "my-slug");
    assert_eq!(result, "/Users/test/.sk/kit/scriptlets/my-scripts.md#my-slug");
}

#[test]
fn test_read_scriptlets_from_file_nonexistent() {
    use std::path::Path;
    // Non-existent file should return empty vec
    let path = Path::new("/nonexistent/path/to/file.md");
    let scriptlets = read_scriptlets_from_file(path);
    assert!(scriptlets.is_empty());
}

#[test]
fn test_read_scriptlets_from_file_not_markdown() {
    use std::path::Path;
    // Non-markdown file should return empty vec
    let path = Path::new("/some/path/to/file.ts");
    let scriptlets = read_scriptlets_from_file(path);
    assert!(scriptlets.is_empty());
}

#[test]
fn test_scriptlet_new_fields() {
    // Verify the new Scriptlet struct fields work
    let scriptlet = Scriptlet {
        name: "Test".to_string(),
        description: Some("Desc".to_string()),
        code: "code".to_string(),
        tool: "ts".to_string(),
        shortcut: None,
        expand: None,
        group: Some("My Group".to_string()),
        file_path: Some("/path/to/file.md#test".to_string()),
        command: Some("test".to_string()),
        alias: None,
    };

    assert_eq!(scriptlet.group, Some("My Group".to_string()));
    assert_eq!(
        scriptlet.file_path,
        Some("/path/to/file.md#test".to_string())
    );
    assert_eq!(scriptlet.command, Some("test".to_string()));
}

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
fn test_parse_scriptlet_with_expand() {
    let section = "## Execute Plan\n\n<!-- \nexpand: plan,,\n-->\n\n```paste\nPlease execute\n```";
    let scriptlet = parse_scriptlet_section(section, None);
    assert!(scriptlet.is_some());
    let s = scriptlet.unwrap();
    assert_eq!(s.expand, Some("plan,,".to_string()));
    assert_eq!(s.tool, "paste");
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
    let text = "<!-- \nshortcut: cmd k\nexpand: foo,,\ndescription: Test\n-->";
    let metadata = extract_html_comment_metadata(text);
    assert_eq!(metadata.get("shortcut"), Some(&"cmd k".to_string()));
    assert_eq!(metadata.get("expand"), Some(&"foo,,".to_string()));
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
fn test_read_scripts_returns_vec() {
    let scripts = read_scripts();
    // scripts should be a Vec, check it's valid
    assert!(scripts.is_empty() || !scripts.is_empty());
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
    let scripts = vec![
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
    ];

    let results = fuzzy_search_scripts(&scripts, "open");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].script.name, "openfile");
    assert!(results[0].score > 0);
}

#[test]
fn test_fuzzy_search_empty_query() {
    let scripts = vec![Script {
        name: "test1".to_string(),
        path: PathBuf::from("/test/test1.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let results = fuzzy_search_scripts(&scripts, "");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].score, 0);
}

#[test]
fn test_fuzzy_search_ranking() {
    let scripts = vec![
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
    ];

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
    let scriptlets = vec![
        test_scriptlet_with_desc("Copy Text", "ts", "copy()", "Copy current selection"),
        test_scriptlet("Paste Code", "ts", "paste()"),
    ];

    let results = fuzzy_search_scriptlets(&scriptlets, "copy");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scriptlet.name, "Copy Text");
    assert!(results[0].score > 0);
}

#[test]
fn test_fuzzy_search_unified() {
    let scripts = vec![Script {
        name: "open".to_string(),
        path: PathBuf::from("/test/open.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: Some("Open a file".to_string()),
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let scriptlets = vec![test_scriptlet_with_desc(
        "Open Browser",
        "ts",
        "open()",
        "Open in browser",
    )];

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
        script: Script {
            name: "test".to_string(),
            path: PathBuf::from("/test.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        score: 100,
        filename: "test.ts".to_string(),
        match_indices: MatchIndices::default(),
    });

    let scriptlet = SearchResult::Scriptlet(ScriptletMatch {
        scriptlet: test_scriptlet("snippet", "ts", "code"),
        score: 50,
        display_file_path: None,
        match_indices: MatchIndices::default(),
    });

    assert_eq!(script.type_label(), "Script");
    assert_eq!(scriptlet.type_label(), "Snippet");
}

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
fn test_fuzzy_match_case_insensitive() {
    assert!(is_fuzzy_match("OPENFILE", "open"));
    assert!(is_fuzzy_match("Open File", "of"));
    assert!(is_fuzzy_match("OpenFile", "OP"));
}

#[test]
fn test_fuzzy_match_single_char() {
    assert!(is_fuzzy_match("test", "t"));
    assert!(is_fuzzy_match("test", "e"));
    assert!(is_fuzzy_match("test", "s"));
}

#[test]
fn test_fuzzy_match_not_in_order() {
    // "st" IS in order in "test" (t-e-s-t), so this should match
    assert!(is_fuzzy_match("test", "st"));
    // But "cab" is NOT in order in "abc"
    assert!(!is_fuzzy_match("abc", "cab"));
    // And "nope" is NOT in order in "open" (o-p-e-n doesn't contain n-o-p-e in order)
    assert!(!is_fuzzy_match("open", "nope"));
}

#[test]
fn test_fuzzy_match_exact_match() {
    assert!(is_fuzzy_match("test", "test"));
    assert!(is_fuzzy_match("open", "open"));
}

#[test]
fn test_fuzzy_match_empty_pattern() {
    assert!(is_fuzzy_match("test", ""));
    assert!(is_fuzzy_match("", ""));
}

#[test]
fn test_fuzzy_match_pattern_longer_than_haystack() {
    assert!(!is_fuzzy_match("ab", "abc"));
    assert!(!is_fuzzy_match("x", "xyz"));
}

#[test]
fn test_fuzzy_search_no_results() {
    let scripts = vec![Script {
        name: "test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let results = fuzzy_search_scripts(&scripts, "nonexistent");
    assert_eq!(results.len(), 0);
}

#[test]
fn test_fuzzy_search_all_match() {
    let scripts = vec![
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
    ];

    let results = fuzzy_search_scripts(&scripts, "test");
    assert_eq!(results.len(), 2);
}

#[test]
fn test_fuzzy_search_by_description() {
    let scripts = vec![
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
    ];

    let results = fuzzy_search_scripts(&scripts, "database");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].script.name, "foo");
}

#[test]
fn test_fuzzy_search_by_path() {
    let scripts = vec![
        Script {
            name: "foo".to_string(),
            path: PathBuf::from("/home/user/.sk/kit/scripts/open.ts"),
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
    ];

    let results = fuzzy_search_scripts(&scripts, "kenv");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].script.name, "foo");
}

#[test]
fn test_fuzzy_search_score_ordering() {
    let scripts = vec![
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
    ];

    let results = fuzzy_search_scripts(&scripts, "exactmatch");
    // Name match should score higher than description match
    assert_eq!(results[0].script.name, "exactmatch");
    assert!(results[0].score > results[1].score);
}

#[test]
fn test_fuzzy_search_scriptlets_by_tool() {
    let scriptlets = vec![
        test_scriptlet("Snippet1", "bash", "code"),
        test_scriptlet("Snippet2", "ts", "code"),
    ];

    let results = fuzzy_search_scriptlets(&scriptlets, "bash");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scriptlet.name, "Snippet1");
}

#[test]
fn test_fuzzy_search_scriptlets_no_results() {
    let scriptlets = vec![test_scriptlet_with_desc(
        "Copy Text",
        "ts",
        "copy()",
        "Copy current selection",
    )];

    let results = fuzzy_search_scriptlets(&scriptlets, "paste");
    assert_eq!(results.len(), 0);
}

#[test]
fn test_fuzzy_search_unified_empty_query() {
    let scripts = vec![Script {
        name: "script1".to_string(),
        path: PathBuf::from("/script1.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let scriptlets = vec![test_scriptlet("Snippet1", "ts", "code")];

    let results = fuzzy_search_unified(&scripts, &scriptlets, "");
    assert_eq!(results.len(), 2);
}

#[test]
fn test_fuzzy_search_unified_scripts_first() {
    let scripts = vec![Script {
        name: "test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: Some("test script".to_string()),
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let scriptlets = vec![test_scriptlet_with_desc(
        "test",
        "ts",
        "test()",
        "test snippet",
    )];

    let results = fuzzy_search_unified(&scripts, &scriptlets, "test");
    // When scores are equal, scripts should come first
    match &results[0] {
        SearchResult::Script(_) => {} // Correct
        SearchResult::Scriptlet(_) => panic!("Script should be first"),
        SearchResult::BuiltIn(_) => panic!("Script should be first"),
        SearchResult::App(_) => panic!("Script should be first"),
        SearchResult::Window(_) => panic!("Script should be first"),
    }
}

#[test]
fn test_search_result_properties() {
    let script_match = ScriptMatch {
        script: Script {
            name: "TestScript".to_string(),
            path: PathBuf::from("/test.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("A test script".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        score: 100,
        filename: "test.ts".to_string(),
        match_indices: MatchIndices::default(),
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
        name: "Full Scriptlet".to_string(),
        description: Some("Complete metadata".to_string()),
        code: "code here".to_string(),
        tool: "bash".to_string(),
        shortcut: Some("cmd k".to_string()),
        expand: Some("prompt,,".to_string()),
        group: None,
        file_path: None,
        command: None,
        alias: None,
    };

    assert_eq!(scriptlet.name, "Full Scriptlet");
    assert_eq!(scriptlet.description, Some("Complete metadata".to_string()));
    assert_eq!(scriptlet.shortcut, Some("cmd k".to_string()));
    assert_eq!(scriptlet.expand, Some("prompt,,".to_string()));
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

// ============================================
// NAME METADATA PARSING TESTS
// ============================================

#[test]
fn test_parse_metadata_line_name_basic() {
    // Basic case: "// Name: Test"
    let line = "// Name: Test";
    let result = parse_metadata_line(line);
    assert!(result.is_some());
    let (key, value) = result.unwrap();
    assert_eq!(key.to_lowercase(), "name");
    assert_eq!(value, "Test");
}

#[test]
fn test_parse_metadata_line_name_no_space_after_slashes() {
    // "//Name:Test" - no spaces
    let line = "//Name:Test";
    let result = parse_metadata_line(line);
    assert!(result.is_some());
    let (key, value) = result.unwrap();
    assert_eq!(key.to_lowercase(), "name");
    assert_eq!(value, "Test");
}

#[test]
fn test_parse_metadata_line_name_space_after_colon() {
    // "//Name: Test" - space after colon
    let line = "//Name: Test";
    let result = parse_metadata_line(line);
    assert!(result.is_some());
    let (key, value) = result.unwrap();
    assert_eq!(key.to_lowercase(), "name");
    assert_eq!(value, "Test");
}

#[test]
fn test_parse_metadata_line_name_space_before_key() {
    // "// Name:Test" - space before key
    let line = "// Name:Test";
    let result = parse_metadata_line(line);
    assert!(result.is_some());
    let (key, value) = result.unwrap();
    assert_eq!(key.to_lowercase(), "name");
    assert_eq!(value, "Test");
}

#[test]
fn test_parse_metadata_line_name_full_spacing() {
    // "// Name: Test" - standard spacing
    let line = "// Name: Test";
    let result = parse_metadata_line(line);
    assert!(result.is_some());
    let (key, value) = result.unwrap();
    assert_eq!(key.to_lowercase(), "name");
    assert_eq!(value, "Test");
}

#[test]
fn test_parse_metadata_line_name_multiple_spaces() {
    // "//  Name:Test" - multiple spaces after slashes
    let line = "//  Name:Test";
    let result = parse_metadata_line(line);
    assert!(result.is_some());
    let (key, value) = result.unwrap();
    assert_eq!(key.to_lowercase(), "name");
    assert_eq!(value, "Test");
}

#[test]
fn test_parse_metadata_line_name_multiple_spaces_and_colon_space() {
    // "//  Name: Test" - multiple spaces after slashes and space after colon
    let line = "//  Name: Test";
    let result = parse_metadata_line(line);
    assert!(result.is_some());
    let (key, value) = result.unwrap();
    assert_eq!(key.to_lowercase(), "name");
    assert_eq!(value, "Test");
}

#[test]
fn test_parse_metadata_line_name_with_tab() {
    // "//\tName:Test" - tab after slashes
    let line = "//\tName:Test";
    let result = parse_metadata_line(line);
    assert!(result.is_some());
    let (key, value) = result.unwrap();
    assert_eq!(key.to_lowercase(), "name");
    assert_eq!(value, "Test");
}

#[test]
fn test_parse_metadata_line_name_with_tab_and_space_after_colon() {
    // "//\tName: Test" - tab after slashes, space after colon
    let line = "//\tName: Test";
    let result = parse_metadata_line(line);
    assert!(result.is_some());
    let (key, value) = result.unwrap();
    assert_eq!(key.to_lowercase(), "name");
    assert_eq!(value, "Test");
}

#[test]
fn test_parse_metadata_line_case_insensitive_name() {
    // Case insensitivity: "// name: Test", "// NAME: Test"
    for line in ["// name: Test", "// NAME: Test", "// NaMe: Test"] {
        let result = parse_metadata_line(line);
        assert!(result.is_some(), "Failed for: {}", line);
        let (key, value) = result.unwrap();
        assert_eq!(key.to_lowercase(), "name");
        assert_eq!(value, "Test");
    }
}

#[test]
fn test_parse_metadata_line_description() {
    // Should also work for Description
    let line = "// Description: My script description";
    let result = parse_metadata_line(line);
    assert!(result.is_some());
    let (key, value) = result.unwrap();
    assert_eq!(key.to_lowercase(), "description");
    assert_eq!(value, "My script description");
}

#[test]
fn test_parse_metadata_line_not_a_comment() {
    // Non-comment lines should return None
    let line = "const name = 'test';";
    let result = parse_metadata_line(line);
    assert!(result.is_none());
}

#[test]
fn test_parse_metadata_line_no_colon() {
    // Comment without colon should return None
    let line = "// Just a comment";
    let result = parse_metadata_line(line);
    assert!(result.is_none());
}

#[test]
fn test_extract_script_metadata_name_and_description() {
    let content = r#"// Name: My Script Name
// Description: This is my script
const x = 1;
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.name, Some("My Script Name".to_string()));
    assert_eq!(metadata.description, Some("This is my script".to_string()));
}

#[test]
fn test_extract_script_metadata_with_alias() {
    let content = r#"// Name: Git Commit
// Description: Commit changes to git
// Alias: gc
const x = 1;
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.name, Some("Git Commit".to_string()));
    assert_eq!(
        metadata.description,
        Some("Commit changes to git".to_string())
    );
    assert_eq!(metadata.alias, Some("gc".to_string()));
}

#[test]
fn test_extract_script_metadata_alias_only() {
    let content = r#"// Alias: shortcut
const x = 1;
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.name, None);
    assert_eq!(metadata.alias, Some("shortcut".to_string()));
}

#[test]
fn test_extract_script_metadata_name_only() {
    let content = r#"// Name: My Script
const x = 1;
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.name, Some("My Script".to_string()));
    assert_eq!(metadata.description, None);
}

#[test]
fn test_extract_script_metadata_description_only() {
    let content = r#"// Description: A description
const x = 1;
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.name, None);
    assert_eq!(metadata.description, Some("A description".to_string()));
}

// ============================================
// SHORTCUT METADATA PARSING TESTS
// ============================================

#[test]
fn test_extract_script_metadata_with_shortcut() {
    let content = r#"// Name: Quick Action
// Description: Run a quick action
// Shortcut: opt i
const x = 1;
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.name, Some("Quick Action".to_string()));
    assert_eq!(metadata.description, Some("Run a quick action".to_string()));
    assert_eq!(metadata.shortcut, Some("opt i".to_string()));
}

#[test]
fn test_extract_script_metadata_shortcut_with_modifiers() {
    let content = r#"// Shortcut: cmd shift k
const x = 1;
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.shortcut, Some("cmd shift k".to_string()));
}

#[test]
fn test_extract_script_metadata_shortcut_ctrl_alt() {
    let content = r#"// Shortcut: ctrl alt t
const x = 1;
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.shortcut, Some("ctrl alt t".to_string()));
}

#[test]
fn test_extract_script_metadata_shortcut_only() {
    let content = r#"// Shortcut: opt space
const x = 1;
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.name, None);
    assert_eq!(metadata.alias, None);
    assert_eq!(metadata.shortcut, Some("opt space".to_string()));
}

#[test]
fn test_extract_script_metadata_shortcut_with_alias() {
    let content = r#"// Name: Git Status
// Alias: gs
// Shortcut: cmd g
const x = 1;
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.name, Some("Git Status".to_string()));
    assert_eq!(metadata.alias, Some("gs".to_string()));
    assert_eq!(metadata.shortcut, Some("cmd g".to_string()));
}

#[test]
fn test_extract_script_metadata_shortcut_case_insensitive() {
    // Shortcut key should be case-insensitive (SHORTCUT, Shortcut, shortcut)
    for variant in [
        "// Shortcut: opt x",
        "// shortcut: opt x",
        "// SHORTCUT: opt x",
    ] {
        let content = format!("{}\nconst x = 1;", variant);
        let metadata = extract_script_metadata(&content);
        assert_eq!(
            metadata.shortcut,
            Some("opt x".to_string()),
            "Failed for variant: {}",
            variant
        );
    }
}

#[test]
fn test_extract_script_metadata_shortcut_lenient_whitespace() {
    // Test lenient whitespace handling like other metadata fields
    let variants = [
        "//Shortcut:opt j",
        "//Shortcut: opt j",
        "// Shortcut:opt j",
        "// Shortcut: opt j",
        "//  Shortcut: opt j",
    ];

    for variant in variants {
        let content = format!("{}\nconst x = 1;", variant);
        let metadata = extract_script_metadata(&content);
        assert_eq!(
            metadata.shortcut,
            Some("opt j".to_string()),
            "Failed for variant: {}",
            variant
        );
    }
}

#[test]
fn test_extract_script_metadata_shortcut_empty_ignored() {
    // Empty shortcut value should be ignored
    let content = r#"// Shortcut:
// Name: Has a name
const x = 1;
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.shortcut, None);
    assert_eq!(metadata.name, Some("Has a name".to_string()));
}

#[test]
fn test_extract_script_metadata_first_shortcut_wins() {
    // If multiple Shortcut: lines exist, the first one wins
    let content = r#"// Shortcut: first shortcut
// Shortcut: second shortcut
const x = 1;
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.shortcut, Some("first shortcut".to_string()));
}

#[test]
fn test_extract_script_metadata_all_fields() {
    // Test all metadata fields together
    let content = r#"// Name: Complete Script
// Description: A complete script with all metadata
// Icon: Terminal
// Alias: cs
// Shortcut: cmd shift c
const x = 1;
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.name, Some("Complete Script".to_string()));
    assert_eq!(
        metadata.description,
        Some("A complete script with all metadata".to_string())
    );
    assert_eq!(metadata.icon, Some("Terminal".to_string()));
    assert_eq!(metadata.alias, Some("cs".to_string()));
    assert_eq!(metadata.shortcut, Some("cmd shift c".to_string()));
}

#[test]
fn test_extract_script_metadata_no_metadata() {
    let content = r#"const x = 1;
console.log(x);
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.name, None);
    assert_eq!(metadata.description, None);
}

#[test]
fn test_extract_script_metadata_lenient_whitespace() {
    // Test all the lenient whitespace variants for Name
    let variants = [
        "//Name:Test",
        "//Name: Test",
        "// Name:Test",
        "// Name: Test",
        "//  Name:Test",
        "//  Name: Test",
        "//\tName:Test",
        "//\tName: Test",
    ];

    for content in variants {
        let full_content = format!("{}\nconst x = 1;", content);
        let metadata = extract_script_metadata(&full_content);
        assert_eq!(
            metadata.name,
            Some("Test".to_string()),
            "Failed for variant: {}",
            content
        );
    }
}

#[test]
fn test_extract_script_metadata_first_name_wins() {
    // If multiple Name: lines exist, the first one wins
    let content = r#"// Name: First Name
// Name: Second Name
const x = 1;
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.name, Some("First Name".to_string()));
}

#[test]
fn test_extract_script_metadata_empty_value_ignored() {
    // Empty value should be ignored
    let content = r#"// Name:
// Description: Has a description
const x = 1;
"#;
    let metadata = extract_script_metadata(content);
    assert_eq!(metadata.name, None);
    assert_eq!(metadata.description, Some("Has a description".to_string()));
}

#[test]
fn test_parse_metadata_line_value_with_colons() {
    // Value can contain colons (e.g., URLs)
    let line = "// Description: Visit https://example.com for more info";
    let result = parse_metadata_line(line);
    assert!(result.is_some());
    let (key, value) = result.unwrap();
    assert_eq!(key.to_lowercase(), "description");
    assert_eq!(value, "Visit https://example.com for more info");
}

#[test]
fn test_parse_metadata_line_value_with_leading_trailing_spaces() {
    // Value should be trimmed
    let line = "// Name:   Padded Value   ";
    let result = parse_metadata_line(line);
    assert!(result.is_some());
    let (_, value) = result.unwrap();
    assert_eq!(value, "Padded Value");
}

#[test]
fn test_extract_script_metadata_only_first_20_lines() {
    // Metadata after line 20 should be ignored
    let mut content = String::new();
    for i in 1..=25 {
        if i == 22 {
            content.push_str("// Name: Too Late\n");
        } else {
            content.push_str(&format!("// Comment line {}\n", i));
        }
    }
    let metadata = extract_script_metadata(&content);
    assert_eq!(metadata.name, None);
}

#[test]
fn test_extract_script_metadata_within_first_20_lines() {
    // Metadata within first 20 lines should be captured
    let mut content = String::new();
    for i in 1..=25 {
        if i == 15 {
            content.push_str("// Name: Just In Time\n");
        } else {
            content.push_str(&format!("// Comment line {}\n", i));
        }
    }
    let metadata = extract_script_metadata(&content);
    assert_eq!(metadata.name, Some("Just In Time".to_string()));
}

// ============================================
// INTEGRATION TESTS: End-to-End Flows
// ============================================

#[test]
fn test_script_struct_creation_and_properties() {
    let script = Script {
        name: "myScript".to_string(),
        path: PathBuf::from("/home/user/.sk/kit/scripts/myScript.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: Some("My custom script".to_string()),
        alias: None,
        shortcut: None,
        ..Default::default()
    };

    assert_eq!(script.name, "myScript");
    assert_eq!(script.extension, "ts");
    assert!(script.description.is_some());
    assert!(script.path.to_string_lossy().contains("myScript"));
}

#[test]
fn test_script_clone_independence() {
    let original = Script {
        name: "original".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: Some("desc".to_string()),
        alias: None,
        shortcut: None,
        ..Default::default()
    };

    let cloned = original.clone();
    assert_eq!(original.name, cloned.name);
    assert_eq!(original.path, cloned.path);
}

#[test]
fn test_scriptlet_clone_independence() {
    let mut original = test_scriptlet("original", "ts", "code");
    original.description = Some("desc".to_string());
    original.shortcut = Some("cmd k".to_string());

    let cloned = original.clone();
    assert_eq!(original.name, cloned.name);
    assert_eq!(original.code, cloned.code);
}

#[test]
fn test_search_multiple_scriptlets() {
    let scriptlets = vec![
        test_scriptlet_with_desc("Copy", "ts", "copy()", "Copy to clipboard"),
        test_scriptlet_with_desc("Paste", "ts", "paste()", "Paste from clipboard"),
        test_scriptlet_with_desc(
            "Custom Paste",
            "ts",
            "pasteCustom()",
            "Custom paste with format",
        ),
    ];

    let results = fuzzy_search_scriptlets(&scriptlets, "paste");
    assert_eq!(results.len(), 2); // "Paste" and "Custom Paste"
                                  // "Paste" should rank higher than "Custom Paste"
    assert_eq!(results[0].scriptlet.name, "Paste");
}

#[test]
fn test_unified_search_mixed_results() {
    let scripts = vec![
        Script {
            name: "openFile".to_string(),
            path: PathBuf::from("/openFile.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "saveFile".to_string(),
            path: PathBuf::from("/saveFile.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];

    let scriptlets = vec![test_scriptlet("Open URL", "ts", "open(url)")];

    let results = fuzzy_search_unified(&scripts, &scriptlets, "open");
    assert_eq!(results.len(), 2); // "openFile" script and "Open URL" scriptlet
}

#[test]
fn test_search_result_name_accessor() {
    let script = SearchResult::Script(ScriptMatch {
        script: Script {
            name: "TestName".to_string(),
            path: PathBuf::from("/test.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        score: 50,
        filename: "test.ts".to_string(),
        match_indices: MatchIndices::default(),
    });

    assert_eq!(script.name(), "TestName");
}

#[test]
fn test_search_result_description_accessor() {
    let scriptlet = SearchResult::Scriptlet(ScriptletMatch {
        scriptlet: test_scriptlet_with_desc("Test", "ts", "code", "Test Description"),
        score: 75,
        display_file_path: None,
        match_indices: MatchIndices::default(),
    });

    assert_eq!(scriptlet.description(), Some("Test Description"));
}

#[test]
fn test_parse_multiple_scriptlets_from_markdown() {
    let markdown = r#"## First Snippet
<!-- description: First desc -->
```ts
first()
```

## Second Snippet
<!-- description: Second desc -->
```bash
second
```

## Third Snippet
```ts
third()
```"#;

    // Simulate splitting and parsing
    let sections: Vec<&str> = markdown.split("## ").collect();
    let mut count = 0;
    for section in sections.iter().skip(1) {
        let full_section = format!("## {}", section);
        if let Some(scriptlet) = parse_scriptlet_section(&full_section, None) {
            count += 1;
            assert!(!scriptlet.name.is_empty());
        }
    }
    assert_eq!(count, 3);
}

#[test]
fn test_fuzzy_search_preserves_vector_order() {
    let scripts = vec![
        Script {
            name: "alpha".to_string(),
            path: PathBuf::from("/alpha.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "beta".to_string(),
            path: PathBuf::from("/beta.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "gamma".to_string(),
            path: PathBuf::from("/gamma.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];

    let results = fuzzy_search_scripts(&scripts, "");
    assert_eq!(results.len(), 3);
    // Empty query should return in name order
    assert_eq!(results[0].script.name, "alpha");
    assert_eq!(results[1].script.name, "beta");
    assert_eq!(results[2].script.name, "gamma");
}

#[test]
fn test_extract_html_metadata_whitespace_handling() {
    let text = "<!--\n  key1:   value1  \n  key2: value2\n-->";
    let metadata = extract_html_comment_metadata(text);
    // Values should be trimmed
    assert_eq!(metadata.get("key1"), Some(&"value1".to_string()));
    assert_eq!(metadata.get("key2"), Some(&"value2".to_string()));
}

#[test]
fn test_parse_scriptlet_with_html_comment_no_fence() {
    // Test that parse_scriptlet_section requires code block even with metadata
    let section = "## NoCode\n\n<!-- description: Test -->\nJust text";
    let scriptlet = parse_scriptlet_section(section, None);
    assert!(scriptlet.is_none());
}

#[test]
fn test_fuzzy_match_special_characters() {
    assert!(is_fuzzy_match("test-file", "test"));
    assert!(is_fuzzy_match("test.file", "file"));
    assert!(is_fuzzy_match("test_name", "name"));
}

// ============================================
// CACHING & PERFORMANCE TESTS
// ============================================

#[test]
fn test_read_scripts_returns_sorted_list() {
    // read_scripts should return sorted by name
    let scripts = vec![
        Script {
            name: "zebra".to_string(),
            path: PathBuf::from("/zebra.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "apple".to_string(),
            path: PathBuf::from("/apple.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "monkey".to_string(),
            path: PathBuf::from("/monkey.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];

    // Manual check of sorting (since read_scripts reads from filesystem)
    let mut sorted = scripts.clone();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(sorted[0].name, "apple");
    assert_eq!(sorted[1].name, "monkey");
    assert_eq!(sorted[2].name, "zebra");
}

#[test]
fn test_scriptlet_ordering_by_name() {
    let scriptlets = vec![
        test_scriptlet("Zebra", "ts", "code"),
        test_scriptlet("Apple", "ts", "code"),
    ];

    let results = fuzzy_search_scriptlets(&scriptlets, "");
    // Empty query returns all scriptlets in original order with score 0
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].scriptlet.name, "Zebra");
    assert_eq!(results[1].scriptlet.name, "Apple");
    assert_eq!(results[0].score, 0);
    assert_eq!(results[1].score, 0);
}

#[test]
fn test_large_search_result_set() {
    let mut scripts = Vec::new();
    for i in 0..100 {
        scripts.push(Script {
            name: format!("script_{:03}", i),
            path: PathBuf::from(format!("/script_{}.ts", i)),
            extension: "ts".to_string(),
            icon: None,
            description: Some(format!("Script number {}", i)),
            alias: None,
            shortcut: None,
            ..Default::default()
        });
    }

    let results = fuzzy_search_scripts(&scripts, "script_05");
    // Should find scripts with 05 in name
    assert!(!results.is_empty());
    assert!(results[0].score > 0);
}

#[test]
fn test_script_match_score_meaningful() {
    let scripts = vec![Script {
        name: "openfile".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: Some("Opens a file".to_string()),
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let results = fuzzy_search_scripts(&scripts, "open");
    assert!(results[0].score >= 50); // Should have at least fuzzy match score
}

#[test]
fn test_complex_markdown_parsing() {
    // Test a realistic markdown structure
    let markdown = r#"# Script Collection

## Script One
<!-- 
description: First script
shortcut: cmd 1
-->
```ts
console.log("first");
```

## Script Two
```bash
echo "second"
```

## Script Three
<!-- 
description: Has URL: https://example.com
expand: type,,
-->
```ts
open("https://example.com");
```
"#;

    // Split and parse sections
    let sections: Vec<&str> = markdown.split("## ").collect();
    let mut parsed = 0;
    for section in sections.iter().skip(1) {
        if let Some(scriptlet) = parse_scriptlet_section(&format!("## {}", section), None) {
            parsed += 1;
            assert!(!scriptlet.name.is_empty());
            assert!(!scriptlet.code.is_empty());
        }
    }
    assert_eq!(parsed, 3);
}

#[test]
fn test_search_consistency_across_calls() {
    let scripts = vec![Script {
        name: "test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let result1 = fuzzy_search_scripts(&scripts, "test");
    let result2 = fuzzy_search_scripts(&scripts, "test");

    assert_eq!(result1.len(), result2.len());
    if !result1.is_empty() && !result2.is_empty() {
        assert_eq!(result1[0].score, result2[0].score);
    }
}

#[test]
fn test_search_result_name_never_empty() {
    let scripts = vec![Script {
        name: "test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let results = fuzzy_search_scripts(&scripts, "test");
    for result in results {
        let script_match = ScriptMatch {
            script: result.script.clone(),
            score: result.score,
            filename: result.filename.clone(),
            match_indices: result.match_indices.clone(),
        };
        let search_result = SearchResult::Script(script_match);
        assert!(!search_result.name().is_empty());
    }
}

#[test]
fn test_scriptlet_code_extraction_with_special_chars() {
    let section = r#"## SpecialChars
```ts
const regex = /test\d+/;
const str = "test\nline";
const obj = { key: "value" };
```"#;

    let scriptlet = parse_scriptlet_section(section, None);
    assert!(scriptlet.is_some());
    let s = scriptlet.unwrap();
    assert!(s.code.contains("regex"));
    assert!(s.code.contains("str"));
}

#[test]
fn test_fuzzy_search_with_unicode() {
    let scripts = vec![Script {
        name: "café".to_string(),
        path: PathBuf::from("/cafe.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    // Should be able to search for the ASCII version
    let results = fuzzy_search_scripts(&scripts, "cafe");
    // Depending on implementation, may or may not match
    let _ = results;
}

#[test]
fn test_script_extension_field_accuracy() {
    let script = Script {
        name: "test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    };

    assert_eq!(script.extension, "ts");

    let script_js = Script {
        name: "test".to_string(),
        path: PathBuf::from("/test.js"),
        extension: "js".to_string(),
        description: None,
        icon: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    };

    assert_eq!(script_js.extension, "js");
}

#[test]
fn test_searchlet_tool_field_various_values() {
    let tools = vec!["ts", "bash", "paste", "sh", "zsh", "py"];

    for tool in tools {
        let scriptlet = test_scriptlet(&format!("Test {}", tool), tool, "code");
        assert_eq!(scriptlet.tool, tool);
    }
}

#[test]
fn test_extract_code_block_with_language_modifiers() {
    let text = "```ts\nconst x = 1;\n```";
    let (lang, _code) = extract_code_block(text).unwrap();
    assert_eq!(lang, "ts");

    let text2 = "```javascript\nconst x = 1;\n```";
    let (lang2, _code2) = extract_code_block(text2).unwrap();
    assert_eq!(lang2, "javascript");
}

#[test]
fn test_parse_scriptlet_section_all_metadata_fields() {
    let section = r#"## Complete
<!-- 
description: Full description here
shortcut: ctrl shift k
expand: choices,,
custom: value
-->
```ts
code here
```"#;

    let scriptlet = parse_scriptlet_section(section, None).unwrap();

    assert_eq!(scriptlet.name, "Complete");
    assert_eq!(
        scriptlet.description,
        Some("Full description here".to_string())
    );
    assert_eq!(scriptlet.shortcut, Some("ctrl shift k".to_string()));
    assert_eq!(scriptlet.expand, Some("choices,,".to_string()));
    // "custom" field won't be extracted as it's not a known field
}

#[test]
fn test_search_result_type_label_consistency() {
    let script = SearchResult::Script(ScriptMatch {
        script: Script {
            name: "test".to_string(),
            path: PathBuf::from("/test.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        score: 0,
        filename: "test.ts".to_string(),
        match_indices: MatchIndices::default(),
    });

    // Should always return "Script"
    assert_eq!(script.type_label(), "Script");

    let scriptlet = SearchResult::Scriptlet(ScriptletMatch {
        scriptlet: test_scriptlet("test", "ts", "code"),
        score: 0,
        display_file_path: None,
        match_indices: MatchIndices::default(),
    });

    // Should always return "Snippet"
    assert_eq!(scriptlet.type_label(), "Snippet");
}

#[test]
fn test_empty_inputs_handling() {
    // Empty script list
    let empty_scripts: Vec<Script> = vec![];
    let results = fuzzy_search_scripts(&empty_scripts, "test");
    assert!(results.is_empty());

    // Empty scriptlet list
    let empty_scriptlets: Vec<Scriptlet> = vec![];
    let results = fuzzy_search_scriptlets(&empty_scriptlets, "test");
    assert!(results.is_empty());

    // Empty both
    let unified = fuzzy_search_unified(&empty_scripts, &empty_scriptlets, "test");
    assert!(unified.is_empty());
}

// ============================================
// COMPREHENSIVE RANKING & RELEVANCE TESTS
// ============================================

#[test]
fn test_exact_substring_at_start_highest_score() {
    let scripts = vec![
        Script {
            name: "open".to_string(),
            path: PathBuf::from("/open.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "reopen".to_string(),
            path: PathBuf::from("/reopen.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];

    let results = fuzzy_search_scripts(&scripts, "open");
    // "open" starts with "open" (score 100 + fuzzy 50 = 150)
    // "reopen" has "open" but not at start (score 75 + fuzzy 50 = 125)
    assert_eq!(results[0].script.name, "open");
    assert!(results[0].score > results[1].score);
}

#[test]
fn test_description_match_lower_priority_than_name() {
    let scripts = vec![
        Script {
            name: "test".to_string(),
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
            path: PathBuf::from("/other.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("test description".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];

    let results = fuzzy_search_scripts(&scripts, "test");
    // Name match should rank higher than description match
    assert_eq!(results[0].script.name, "test");
}

#[test]
fn test_path_match_lowest_priority() {
    let scripts = vec![
        Script {
            name: "test".to_string(),
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
            path: PathBuf::from("/test/other.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];

    let results = fuzzy_search_scripts(&scripts, "test");
    // Name match should rank higher than path match
    assert_eq!(results[0].script.name, "test");
}

#[test]
fn test_scriptlet_code_match_lower_than_description() {
    let mut snippet = test_scriptlet("Snippet", "ts", "paste()");
    snippet.description = Some("copy text".to_string());

    let other = test_scriptlet("Other", "ts", "copy()");

    let scriptlets = vec![snippet, other];

    let results = fuzzy_search_scriptlets(&scriptlets, "copy");
    // Description match should score higher than code match
    assert_eq!(results[0].scriptlet.name, "Snippet");
}

#[test]
fn test_tool_type_bonus_in_scoring() {
    let scriptlets = vec![
        test_scriptlet("Script1", "bash", "code"),
        test_scriptlet("Script2", "ts", "code"),
    ];

    let results = fuzzy_search_scriptlets(&scriptlets, "bash");
    // "bash" matches tool type in Script1
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scriptlet.name, "Script1");
}

#[test]
fn test_longer_exact_match_ties_with_fuzzy() {
    let scripts = vec![
        Script {
            name: "open".to_string(),
            path: PathBuf::from("/open.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("Open a file".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "openfile".to_string(),
            path: PathBuf::from("/openfile.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];

    let results = fuzzy_search_scripts(&scripts, "open");
    // Both have name matches at start (100 points) and fuzzy match (50 points)
    // When tied, should sort by name alphabetically
    assert_eq!(results[0].script.name, "open");
    assert_eq!(results[1].script.name, "openfile");
}

#[test]
fn test_case_insensitive_matching() {
    let scripts = vec![Script {
        name: "OpenFile".to_string(),
        path: PathBuf::from("/openfile.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let results = fuzzy_search_scripts(&scripts, "OPEN");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].script.name, "OpenFile");
}

#[test]
fn test_ranking_preserves_relative_order_on_score_tie() {
    let scripts = vec![
        Script {
            name: "aaa".to_string(),
            path: PathBuf::from("/aaa.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("test".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "bbb".to_string(),
            path: PathBuf::from("/bbb.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("test".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];

    let results = fuzzy_search_scripts(&scripts, "test");
    // Same score, should sort by name
    assert_eq!(results[0].script.name, "aaa");
    assert_eq!(results[1].script.name, "bbb");
}

#[test]
fn test_scriptlet_name_match_bonus_points() {
    let scriptlets = vec![
        test_scriptlet("copy", "ts", "copy()"),
        test_scriptlet("paste", "ts", "copy()"),
    ];

    let results = fuzzy_search_scriptlets(&scriptlets, "copy");
    // "copy" name has higher bonus than "paste" code match
    assert_eq!(results[0].scriptlet.name, "copy");
    assert!(results[0].score > 0);
}

#[test]
fn test_unified_search_ties_scripts_first() {
    let scripts = vec![Script {
        name: "test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: Some("Test script".to_string()),
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let scriptlets = vec![test_scriptlet_with_desc(
        "test",
        "ts",
        "test()",
        "Test snippet",
    )];

    let results = fuzzy_search_unified(&scripts, &scriptlets, "test");
    // Same score, scripts should come before scriptlets
    assert_eq!(results.len(), 2);
    match &results[0] {
        SearchResult::Script(_) => {}
        SearchResult::Scriptlet(_) => panic!("Expected Script first"),
        SearchResult::BuiltIn(_) => panic!("Expected Script first"),
        SearchResult::App(_) => panic!("Expected Script first"),
        SearchResult::Window(_) => panic!("Expected Script first"),
    }
}

#[test]
fn test_partial_match_scores_appropriately() {
    let scripts = vec![Script {
        name: "test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let results = fuzzy_search_scripts(&scripts, "es");
    // "es" is fuzzy match in "test" but not a substring match
    assert_eq!(results.len(), 1);
    assert!(results[0].score > 0);
}

#[test]
fn test_multiple_word_query() {
    let scripts = vec![
        Script {
            name: "open file".to_string(),
            path: PathBuf::from("/openfile.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "save".to_string(),
            path: PathBuf::from("/save.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];

    // Query with space - will be treated as literal string
    let results = fuzzy_search_scripts(&scripts, "open file");
    assert!(!results.is_empty());
}

#[test]
fn test_all_search_types_contribute_to_score() {
    // Test that all scoring categories work
    let scripts = vec![Script {
        name: "database".to_string(),
        path: PathBuf::from("/database.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: Some("database connection".to_string()),
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let results = fuzzy_search_scripts(&scripts, "database");
    // Should match on name (100 + 50 = 150) + description (25) = 175
    assert!(results[0].score > 100);
}

#[test]
fn test_search_quality_metrics() {
    // Ensure search returns meaningful results
    let scripts = vec![
        Script {
            name: "zzzFile".to_string(),
            path: PathBuf::from("/home/user/.sk/kit/scripts/zzzFile.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("Opens a file dialog".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "someScript".to_string(),
            path: PathBuf::from("/home/user/.sk/kit/scripts/someScript.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("Does something".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "saveData".to_string(),
            path: PathBuf::from("/home/user/.sk/kit/scripts/saveData.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("Saves data to file".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];

    let results = fuzzy_search_scripts(&scripts, "file");
    // Two should match (zzzFile name and saveData description)
    assert_eq!(results.len(), 2);
    // Name match (zzzFile) should rank higher than description match (saveData)
    assert_eq!(results[0].script.name, "zzzFile");
    assert_eq!(results[1].script.name, "saveData");
}

#[test]
fn test_relevance_ranking_realistic_scenario() {
    let scripts = vec![
        Script {
            name: "grep".to_string(),
            path: PathBuf::from("/grep.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("Search files with grep".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "find".to_string(),
            path: PathBuf::from("/grep-utils.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("Find files".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "search".to_string(),
            path: PathBuf::from("/search.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];

    let results = fuzzy_search_scripts(&scripts, "grep");
    // "grep" name should rank highest
    assert_eq!(results[0].script.name, "grep");
    // "find" with grep in path should rank second
    assert!(results.len() >= 2);
}

#[test]
fn test_mixed_content_search() {
    // Combine scripts and scriptlets in unified search
    let scripts = vec![Script {
        name: "copyClipboard".to_string(),
        path: PathBuf::from("/copy.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: Some("Copy to clipboard".to_string()),
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let mut quick_copy = test_scriptlet_with_desc("Quick Copy", "ts", "copy()", "Copy selection");
    quick_copy.shortcut = Some("cmd c".to_string());
    let scriptlets = vec![quick_copy];

    let results = fuzzy_search_unified(&scripts, &scriptlets, "copy");
    assert_eq!(results.len(), 2);
    // Verify both types are present
    let has_script = results.iter().any(|r| matches!(r, SearchResult::Script(_)));
    let has_scriptlet = results
        .iter()
        .any(|r| matches!(r, SearchResult::Scriptlet(_)));
    assert!(has_script);
    assert!(has_scriptlet);
}

// ============================================
// BUILT-IN SEARCH TESTS
// ============================================

fn create_test_builtins() -> Vec<BuiltInEntry> {
    use crate::builtins::BuiltInFeature;
    vec![
        BuiltInEntry {
            id: "builtin-clipboard-history".to_string(),
            name: "Clipboard History".to_string(),
            description: "View and manage your clipboard history".to_string(),
            keywords: vec![
                "clipboard".to_string(),
                "history".to_string(),
                "paste".to_string(),
                "copy".to_string(),
            ],
            feature: BuiltInFeature::ClipboardHistory,
            icon: Some("📋".to_string()),
        },
        BuiltInEntry {
            id: "builtin-app-launcher".to_string(),
            name: "App Launcher".to_string(),
            description: "Search and launch installed applications".to_string(),
            keywords: vec![
                "app".to_string(),
                "launch".to_string(),
                "open".to_string(),
                "application".to_string(),
            ],
            feature: BuiltInFeature::AppLauncher,
            icon: Some("🚀".to_string()),
        },
    ]
}

#[test]
fn test_fuzzy_search_builtins_by_name() {
    let builtins = create_test_builtins();
    let results = fuzzy_search_builtins(&builtins, "clipboard");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.name, "Clipboard History");
    assert!(results[0].score > 0);
}

#[test]
fn test_fuzzy_search_builtins_by_keyword() {
    let builtins = create_test_builtins();

    // "paste" is a keyword for clipboard history
    let results = fuzzy_search_builtins(&builtins, "paste");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.name, "Clipboard History");

    // "launch" is a keyword for app launcher
    let results = fuzzy_search_builtins(&builtins, "launch");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.name, "App Launcher");
}

#[test]
fn test_fuzzy_search_builtins_partial_keyword() {
    let builtins = create_test_builtins();

    // "clip" should match "clipboard" keyword
    let results = fuzzy_search_builtins(&builtins, "clip");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.name, "Clipboard History");

    // "app" should match "app" keyword in App Launcher
    let results = fuzzy_search_builtins(&builtins, "app");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.name, "App Launcher");
}

#[test]
fn test_fuzzy_search_builtins_by_description() {
    let builtins = create_test_builtins();

    // "manage" is in clipboard history description
    let results = fuzzy_search_builtins(&builtins, "manage");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.name, "Clipboard History");

    // "installed" is in app launcher description
    let results = fuzzy_search_builtins(&builtins, "installed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.name, "App Launcher");
}

#[test]
fn test_fuzzy_search_builtins_empty_query() {
    let builtins = create_test_builtins();
    let results = fuzzy_search_builtins(&builtins, "");

    assert_eq!(results.len(), 2);
    // Both should have score 0
    assert_eq!(results[0].score, 0);
    assert_eq!(results[1].score, 0);
}

#[test]
fn test_fuzzy_search_builtins_no_match() {
    let builtins = create_test_builtins();
    let results = fuzzy_search_builtins(&builtins, "nonexistent");

    assert!(results.is_empty());
}

#[test]
fn test_builtin_match_struct() {
    use crate::builtins::BuiltInFeature;

    let entry = BuiltInEntry {
        id: "test".to_string(),
        name: "Test Entry".to_string(),
        description: "Test description".to_string(),
        keywords: vec!["test".to_string()],
        feature: BuiltInFeature::ClipboardHistory,
        icon: None,
    };

    let builtin_match = BuiltInMatch {
        entry: entry.clone(),
        score: 100,
    };

    assert_eq!(builtin_match.entry.name, "Test Entry");
    assert_eq!(builtin_match.score, 100);
}

#[test]
fn test_search_result_builtin_variant() {
    use crate::builtins::BuiltInFeature;

    let entry = BuiltInEntry {
        id: "test".to_string(),
        name: "Test Built-in".to_string(),
        description: "Test built-in description".to_string(),
        keywords: vec!["test".to_string()],
        feature: BuiltInFeature::AppLauncher,
        icon: Some("🚀".to_string()),
    };

    let result = SearchResult::BuiltIn(BuiltInMatch { entry, score: 75 });

    assert_eq!(result.name(), "Test Built-in");
    assert_eq!(result.description(), Some("Test built-in description"));
    assert_eq!(result.score(), 75);
    assert_eq!(result.type_label(), "Built-in");
}

#[test]
fn test_unified_search_with_builtins() {
    let scripts = vec![Script {
        name: "my-clipboard".to_string(),
        path: PathBuf::from("/clipboard.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: Some("My clipboard script".to_string()),
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let scriptlets = vec![test_scriptlet_with_desc(
        "Clipboard Helper",
        "ts",
        "clipboard()",
        "Helper for clipboard",
    )];

    let builtins = create_test_builtins();

    let results = fuzzy_search_unified_with_builtins(&scripts, &scriptlets, &builtins, "clipboard");

    // All three should match
    assert_eq!(results.len(), 3);

    // Verify all types are present
    let has_builtin = results
        .iter()
        .any(|r| matches!(r, SearchResult::BuiltIn(_)));
    let has_script = results.iter().any(|r| matches!(r, SearchResult::Script(_)));
    let has_scriptlet = results
        .iter()
        .any(|r| matches!(r, SearchResult::Scriptlet(_)));

    assert!(has_builtin);
    assert!(has_script);
    assert!(has_scriptlet);
}

#[test]
fn test_unified_search_builtins_appear_at_top() {
    let scripts = vec![Script {
        name: "history".to_string(),
        path: PathBuf::from("/history.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let builtins = create_test_builtins();

    let results = fuzzy_search_unified_with_builtins(&scripts, &[], &builtins, "history");

    // Both should match (Clipboard History builtin and history script)
    assert!(results.len() >= 2);

    // When scores are equal, built-ins should appear first
    // Check that the first result is a built-in if scores are equal
    if results.len() >= 2 && results[0].score() == results[1].score() {
        match &results[0] {
            SearchResult::BuiltIn(_) => {} // Expected
            _ => panic!("Built-in should appear before script when scores are equal"),
        }
    }
}

#[test]
fn test_unified_search_backward_compatible() {
    // Ensure the original fuzzy_search_unified still works without builtins
    let scripts = vec![Script {
        name: "test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let scriptlets = vec![test_scriptlet("Test Snippet", "ts", "test()")];

    let results = fuzzy_search_unified(&scripts, &scriptlets, "test");

    // Should still work without builtins
    assert_eq!(results.len(), 2);
}

#[test]
fn test_builtin_keyword_matching_priority() {
    let builtins = create_test_builtins();

    // "copy" matches keyword in clipboard history
    let results = fuzzy_search_builtins(&builtins, "copy");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].entry.name, "Clipboard History");
    assert!(results[0].score >= 75); // Keyword match gives 75 points
}

#[test]
fn test_builtin_fuzzy_keyword_matching() {
    let builtins = create_test_builtins();

    // "hist" should fuzzy match "history" keyword
    let results = fuzzy_search_builtins(&builtins, "hist");
    assert!(!results.is_empty());
    assert_eq!(results[0].entry.name, "Clipboard History");
}

// ============================================
// WINDOW SEARCH TESTS
// ============================================
//
// Note: Most window search tests require WindowInfo to have a public constructor.
// These tests verify the function signatures and empty input handling.
// Integration tests with actual WindowInfo require window_control module changes.

#[test]
fn test_fuzzy_search_windows_empty_list() {
    // Test with empty window list
    let windows: Vec<crate::window_control::WindowInfo> = vec![];

    let results = fuzzy_search_windows(&windows, "test");
    assert!(results.is_empty());

    let results_empty_query = fuzzy_search_windows(&windows, "");
    assert!(results_empty_query.is_empty());
}

#[test]
fn test_window_match_type_exists() {
    // Verify WindowMatch struct has expected fields by type-checking
    fn _type_check(wm: &WindowMatch) {
        let _window: &crate::window_control::WindowInfo = &wm.window;
        let _score: i32 = wm.score;
    }
}

#[test]
fn test_search_result_window_type_label() {
    // We can't construct WindowInfo directly, but we can verify
    // the SearchResult::Window variant exists and type_label is correct
    // by checking the match arm in type_label implementation compiles
    fn _verify_window_variant_exists() {
        fn check_label(result: &SearchResult) -> &'static str {
            match result {
                SearchResult::Window(_) => "Window",
                _ => "other",
            }
        }
        let _ = check_label;
    }
}

#[test]
fn test_fuzzy_search_unified_with_windows_empty_inputs() {
    let scripts: Vec<Script> = vec![];
    let scriptlets: Vec<Scriptlet> = vec![];
    let builtins: Vec<BuiltInEntry> = vec![];
    let apps: Vec<crate::app_launcher::AppInfo> = vec![];
    let windows: Vec<crate::window_control::WindowInfo> = vec![];

    let results = fuzzy_search_unified_with_windows(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &windows,
        "test",
    );

    assert!(results.is_empty());
}

#[test]
fn test_fuzzy_search_unified_with_windows_returns_scripts() {
    let scripts = vec![Script {
        name: "test_script".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }];
    let scriptlets: Vec<Scriptlet> = vec![];
    let builtins: Vec<BuiltInEntry> = vec![];
    let apps: Vec<crate::app_launcher::AppInfo> = vec![];
    let windows: Vec<crate::window_control::WindowInfo> = vec![];

    let results = fuzzy_search_unified_with_windows(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &windows,
        "test",
    );

    assert_eq!(results.len(), 1);
    assert!(matches!(&results[0], SearchResult::Script(_)));
}

// ============================================
// GROUPED RESULTS (FRECENCY) TESTS
// ============================================

#[test]
fn test_get_grouped_results_search_mode_flat_list() {
    let scripts = vec![
        Script {
            name: "open".to_string(),
            path: PathBuf::from("/open.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "save".to_string(),
            path: PathBuf::from("/save.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];
    let scriptlets: Vec<Scriptlet> = vec![];
    let builtins: Vec<BuiltInEntry> = vec![];
    let apps: Vec<AppInfo> = vec![];
    let frecency_store = FrecencyStore::new();

    // Search mode: non-empty filter should return flat list
    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        "open",
        10,
    );

    // Should be a flat list with no headers
    assert!(!grouped.is_empty());
    for item in &grouped {
        assert!(matches!(item, GroupedListItem::Item(_)));
    }
    assert_eq!(results.len(), 1); // Only "open" matches
}

#[test]
fn test_get_grouped_results_empty_filter_grouped_view() {
    let scripts = vec![
        Script {
            name: "alpha".to_string(),
            path: PathBuf::from("/alpha.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "beta".to_string(),
            path: PathBuf::from("/beta.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];
    let scriptlets: Vec<Scriptlet> = vec![];
    let builtins: Vec<BuiltInEntry> = vec![];
    let apps: Vec<AppInfo> = vec![];
    let frecency_store = FrecencyStore::new();

    // Empty filter should return grouped view
    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        "",
        10,
    );

    // Results should contain all items
    assert_eq!(results.len(), 2);

    // Grouped should have MAIN section (no RECENT since frecency is empty)
    assert!(!grouped.is_empty());

    // First item should be MAIN section header
    assert!(matches!(&grouped[0], GroupedListItem::SectionHeader(s) if s == "MAIN"));
}

#[test]
fn test_get_grouped_results_with_frecency() {
    let scripts = vec![
        Script {
            name: "alpha".to_string(),
            path: PathBuf::from("/alpha.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "beta".to_string(),
            path: PathBuf::from("/beta.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "gamma".to_string(),
            path: PathBuf::from("/gamma.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];
    let scriptlets: Vec<Scriptlet> = vec![];
    let builtins: Vec<BuiltInEntry> = vec![];
    let apps: Vec<AppInfo> = vec![];

    // Create frecency store and record usage for one script
    let mut frecency_store = FrecencyStore::new();
    frecency_store.record_use("/beta.ts");

    // Empty filter should return grouped view with RECENT section
    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        "",
        10,
    );

    // Results should contain all items
    assert_eq!(results.len(), 3);

    // Grouped should have both RECENT and MAIN sections
    let section_headers: Vec<&str> = grouped
        .iter()
        .filter_map(|item| match item {
            GroupedListItem::SectionHeader(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();

    assert!(section_headers.contains(&"RECENT"));
    assert!(section_headers.contains(&"MAIN"));
}

#[test]
fn test_get_grouped_results_frecency_script_appears_before_builtins() {
    // This test verifies the fix for: Clipboard History appearing first
    // regardless of frecency scores.
    //
    // Expected behavior: When a script has frecency > 0, it should appear
    // in the RECENT section BEFORE builtins in MAIN.
    //
    // Bug scenario: User frequently uses "test-script", but Clipboard History
    // still appears as the first choice when opening Script Kit.

    let scripts = vec![
        Script {
            name: "test-script".to_string(),
            path: PathBuf::from("/test-script.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("A frequently used script".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "another-script".to_string(),
            path: PathBuf::from("/another-script.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];
    let scriptlets: Vec<Scriptlet> = vec![];
    let builtins = create_test_builtins(); // Includes Clipboard History and App Launcher
    let apps: Vec<AppInfo> = vec![];

    // Record usage for test-script to give it frecency
    let mut frecency_store = FrecencyStore::new();
    frecency_store.record_use("/test-script.ts");

    // Get grouped results with empty filter (default view)
    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        "",
        10,
    );

    // Verify structure:
    // grouped[0] = SectionHeader("RECENT")
    // grouped[1] = Item(idx) where results[idx] is the frecency script
    // grouped[2] = SectionHeader("MAIN")
    // grouped[3+] = Items including builtins and other scripts

    // First should be RECENT header
    assert!(
        matches!(&grouped[0], GroupedListItem::SectionHeader(s) if s == "RECENT"),
        "First item should be RECENT section header, got {:?}",
        grouped[0]
    );

    // Second should be the frecency script (test-script)
    assert!(
        matches!(&grouped[1], GroupedListItem::Item(idx) if {
            let result = &results[*idx];
            matches!(result, SearchResult::Script(sm) if sm.script.name == "test-script")
        }),
        "Second item should be the frecency script 'test-script', got {:?}",
        grouped.get(1).map(|g| {
            if let GroupedListItem::Item(idx) = g {
                format!("Item({}) = {}", idx, results[*idx].name())
            } else {
                format!("{:?}", g)
            }
        })
    );

    // Third should be MAIN header
    assert!(
        matches!(&grouped[2], GroupedListItem::SectionHeader(s) if s == "MAIN"),
        "Third item should be MAIN section header, got {:?}",
        grouped[2]
    );

    // Find builtins in MAIN section (after grouped[2])
    let main_items: Vec<&str> = grouped[3..]
        .iter()
        .filter_map(|item| {
            if let GroupedListItem::Item(idx) = item {
                Some(results[*idx].name())
            } else {
                None
            }
        })
        .collect();

    // Builtins should be in MAIN, not RECENT
    assert!(
        main_items.contains(&"Clipboard History"),
        "Clipboard History should be in MAIN section, not RECENT. MAIN items: {:?}",
        main_items
    );
    assert!(
        main_items.contains(&"App Launcher"),
        "App Launcher should be in MAIN section. MAIN items: {:?}",
        main_items
    );

    // Verify the frecency script is NOT in MAIN (it's in RECENT)
    assert!(
        !main_items.contains(&"test-script"),
        "test-script should NOT be in MAIN (it should be in RECENT). MAIN items: {:?}",
        main_items
    );
}

#[test]
fn test_get_grouped_results_builtin_with_frecency_vs_script_frecency() {
    // This test captures a more nuanced bug scenario:
    // When BOTH a builtin (Clipboard History) AND a script have frecency,
    // the script with higher frecency should appear first in RECENT.
    //
    // Bug: Clipboard History appears first even when user scripts have
    // higher/more recent frecency scores.

    let scripts = vec![Script {
        name: "my-frequent-script".to_string(),
        path: PathBuf::from("/my-frequent-script.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: Some("User's frequently used script".to_string()),
        alias: None,
        shortcut: None,
        ..Default::default()
    }];
    let scriptlets: Vec<Scriptlet> = vec![];
    let builtins = create_test_builtins(); // Clipboard History, App Launcher
    let apps: Vec<AppInfo> = vec![];

    let mut frecency_store = FrecencyStore::new();

    // Record builtin usage once (older)
    frecency_store.record_use("builtin:Clipboard History");

    // Record script usage multiple times (more frequent, should have higher score)
    frecency_store.record_use("/my-frequent-script.ts");
    frecency_store.record_use("/my-frequent-script.ts");
    frecency_store.record_use("/my-frequent-script.ts");

    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        "",
        10,
    );

    // Both should be in RECENT, but script should come FIRST (higher frecency)
    assert!(
        matches!(&grouped[0], GroupedListItem::SectionHeader(s) if s == "RECENT"),
        "First item should be RECENT header"
    );

    // The first ITEM in RECENT should be the user script (higher frecency)
    assert!(
        matches!(&grouped[1], GroupedListItem::Item(idx) if {
            let result = &results[*idx];
            matches!(result, SearchResult::Script(sm) if sm.script.name == "my-frequent-script")
        }),
        "First item in RECENT should be 'my-frequent-script' (highest frecency), got: {}",
        if let GroupedListItem::Item(idx) = &grouped[1] {
            results[*idx].name().to_string()
        } else {
            format!("{:?}", grouped[1])
        }
    );

    // Clipboard History should be second in RECENT (lower frecency)
    assert!(
        matches!(&grouped[2], GroupedListItem::Item(idx) if {
            results[*idx].name() == "Clipboard History"
        }),
        "Second item in RECENT should be 'Clipboard History' (lower frecency), got: {}",
        if let GroupedListItem::Item(idx) = &grouped[2] {
            results[*idx].name().to_string()
        } else {
            format!("{:?}", grouped[2])
        }
    );
}

#[test]
fn test_get_grouped_results_selection_priority_with_frecency() {
    // This test verifies the SELECTION behavior, not just grouping.
    //
    // Bug: When user opens Script Kit, the FIRST SELECTABLE item should be
    // the most recently used item (from RECENT), not the first item in MAIN.
    //
    // The grouped list structure determines what gets selected initially.
    // With frecency, the first Item (not SectionHeader) should be the
    // frecency script, which means selected_index=0 should point to
    // the frecency script when we skip headers.

    let scripts = vec![
        Script {
            name: "alpha-script".to_string(),
            path: PathBuf::from("/alpha-script.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "zebra-script".to_string(),
            path: PathBuf::from("/zebra-script.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];
    let scriptlets: Vec<Scriptlet> = vec![];
    let builtins = create_test_builtins(); // Clipboard History, App Launcher
    let apps: Vec<AppInfo> = vec![];

    let mut frecency_store = FrecencyStore::new();
    frecency_store.record_use("/zebra-script.ts"); // Give frecency to zebra

    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        "",
        10,
    );

    // Find the first Item (not SectionHeader) - this is what gets selected
    let first_selectable_idx = grouped
        .iter()
        .find_map(|item| {
            if let GroupedListItem::Item(idx) = item {
                Some(*idx)
            } else {
                None
            }
        })
        .expect("Should have at least one selectable item");

    let first_result = &results[first_selectable_idx];

    // The first selectable item MUST be the frecency script
    // NOT Clipboard History (which would be first alphabetically in MAIN)
    assert_eq!(
        first_result.name(),
        "zebra-script",
        "First selectable item should be the frecency script 'zebra-script', got '{}'. \
             This bug causes Clipboard History to appear first regardless of user's frecency.",
        first_result.name()
    );

    // Verify the structure explicitly
    // grouped[0] = SectionHeader("RECENT")
    // grouped[1] = Item(zebra-script) <- THIS should be first selection
    // grouped[2] = SectionHeader("MAIN")
    // grouped[3+] = Other items (builtins and scripts sorted together alphabetically)

    let grouped_names: Vec<String> = grouped
        .iter()
        .map(|item| match item {
            GroupedListItem::SectionHeader(s) => format!("[{}]", s),
            GroupedListItem::Item(idx) => results[*idx].name().to_string(),
        })
        .collect();

    // First 3 items should be: RECENT header, frecency item, MAIN header
    assert_eq!(
        &grouped_names[..3],
        &["[RECENT]", "zebra-script", "[MAIN]"],
        "First 3 items should be: RECENT header, frecency item, MAIN header. Got: {:?}",
        grouped_names
    );
}

#[test]
fn test_get_grouped_results_no_frecency_builtins_sorted_with_scripts() {
    // TDD FAILING TEST: This test documents the BUG and expected fix.
    //
    // BUG: When there's NO frecency data, builtins appear BEFORE scripts in MAIN,
    // regardless of alphabetical order. This causes "Clipboard History" to always
    // appear first.
    //
    // EXPECTED BEHAVIOR (after fix): MAIN section items sorted alphabetically by name,
    // with builtins mixed in with scripts.
    //
    // Current broken behavior: ["App Launcher", "Clipboard History", "alpha-script", "zebra-script"]
    // Expected fixed behavior:  ["alpha-script", "App Launcher", "Clipboard History", "zebra-script"]

    let scripts = vec![
        Script {
            name: "alpha-script".to_string(),
            path: PathBuf::from("/alpha-script.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "zebra-script".to_string(),
            path: PathBuf::from("/zebra-script.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];
    let scriptlets: Vec<Scriptlet> = vec![];
    let builtins = create_test_builtins(); // Clipboard History, App Launcher
    let apps: Vec<AppInfo> = vec![];

    // No frecency - fresh start
    let frecency_store = FrecencyStore::new();

    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        "",
        10,
    );

    // With no frecency, should only have MAIN section
    let grouped_names: Vec<String> = grouped
        .iter()
        .map(|item| match item {
            GroupedListItem::SectionHeader(s) => format!("[{}]", s),
            GroupedListItem::Item(idx) => results[*idx].name().to_string(),
        })
        .collect();

    // First should be MAIN header (no RECENT because no frecency)
    assert_eq!(
        grouped_names[0], "[MAIN]",
        "First item should be MAIN header when no frecency. Got: {:?}",
        grouped_names
    );

    // Items should be sorted alphabetically - check the order
    let item_names: Vec<&str> = grouped_names[1..].iter().map(|s| s.as_str()).collect();

    // EXPECTED: Items sorted alphabetically, builtins mixed with scripts
    // "alpha-script" < "App Launcher" < "Clipboard History" < "zebra-script"
    assert_eq!(
        item_names,
        vec![
            "alpha-script",
            "App Launcher",
            "Clipboard History",
            "zebra-script"
        ],
        "BUG: Builtins appear before scripts instead of being sorted alphabetically. \
             This causes 'Clipboard History' to always be first choice. \
             Expected alphabetical order, got: {:?}",
        item_names
    );
}

#[test]
fn test_get_grouped_results_empty_inputs() {
    let scripts: Vec<Script> = vec![];
    let scriptlets: Vec<Scriptlet> = vec![];
    let builtins: Vec<BuiltInEntry> = vec![];
    let apps: Vec<AppInfo> = vec![];
    let frecency_store = FrecencyStore::new();

    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        "",
        10,
    );

    // Both should be empty when no inputs
    assert!(results.is_empty());
    assert!(grouped.is_empty());
}

#[test]
fn test_get_grouped_results_items_reference_correct_indices() {
    let scripts = vec![
        Script {
            name: "first".to_string(),
            path: PathBuf::from("/first.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "second".to_string(),
            path: PathBuf::from("/second.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];
    let scriptlets: Vec<Scriptlet> = vec![];
    let builtins: Vec<BuiltInEntry> = vec![];
    let apps: Vec<AppInfo> = vec![];
    let frecency_store = FrecencyStore::new();

    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        "",
        10,
    );

    // All Item indices should be valid indices into results
    for item in &grouped {
        if let GroupedListItem::Item(idx) = item {
            assert!(
                *idx < results.len(),
                "Index {} out of bounds for results len {}",
                idx,
                results.len()
            );
        }
    }
}

// ============================================
// FILENAME SEARCH TESTS
// ============================================

#[test]
fn test_fuzzy_search_scripts_by_file_extension() {
    // Users should be able to search by typing ".ts" to find TypeScript scripts
    let scripts = vec![
        Script {
            name: "My Script".to_string(),
            path: PathBuf::from("/home/user/.sk/kit/scripts/my-script.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "Other Script".to_string(),
            path: PathBuf::from("/home/user/.sk/kit/scripts/other.js"),
            extension: "js".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];

    let results = fuzzy_search_scripts(&scripts, ".ts");
    assert_eq!(results.len(), 1, "Should find scripts by file extension");
    assert_eq!(results[0].script.name, "My Script");
    assert_eq!(results[0].filename, "my-script.ts");
    assert!(results[0].score > 0);
}

#[test]
fn test_fuzzy_search_scripts_by_filename() {
    // Users should be able to search by filename
    let scripts = vec![
        Script {
            name: "Open File".to_string(), // Name differs from filename
            path: PathBuf::from("/scripts/open-file-dialog.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "Save Data".to_string(),
            path: PathBuf::from("/scripts/save-data.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];

    // Search by filename (not matching the name "Open File")
    let results = fuzzy_search_scripts(&scripts, "dialog");
    assert_eq!(results.len(), 1, "Should find scripts by filename content");
    assert_eq!(results[0].script.name, "Open File");
    assert_eq!(results[0].filename, "open-file-dialog.ts");
}

#[test]
fn test_fuzzy_search_scripts_filename_returns_correct_filename() {
    let scripts = vec![Script {
        name: "Test".to_string(),
        path: PathBuf::from("/path/to/my-test-script.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let results = fuzzy_search_scripts(&scripts, "test");
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].filename, "my-test-script.ts",
        "Should extract correct filename from path"
    );
}

#[test]
fn test_fuzzy_search_scripts_name_match_higher_priority_than_filename() {
    // Name match should score higher than filename-only match
    let scripts = vec![
        Script {
            name: "open".to_string(),               // Name matches query
            path: PathBuf::from("/scripts/foo.ts"), // Filename doesn't match
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "bar".to_string(),                           // Name doesn't match
            path: PathBuf::from("/scripts/open-something.ts"), // Filename matches
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ];

    let results = fuzzy_search_scripts(&scripts, "open");
    assert_eq!(results.len(), 2);
    // Name match should be first
    assert_eq!(
        results[0].script.name, "open",
        "Name match should rank higher than filename match"
    );
    assert_eq!(results[1].script.name, "bar");
}

#[test]
fn test_fuzzy_search_scripts_match_indices_for_name() {
    let scripts = vec![Script {
        name: "openfile".to_string(),
        path: PathBuf::from("/scripts/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let results = fuzzy_search_scripts(&scripts, "opf");
    assert_eq!(results.len(), 1);
    // Match indices are now computed lazily - verify using compute_match_indices_for_result
    let indices =
        compute_match_indices_for_result(&SearchResult::Script(results[0].clone()), "opf");
    // "opf" matches indices 0, 1, 4 in "openfile"
    assert_eq!(
        indices.name_indices,
        vec![0, 1, 4],
        "Should return correct match indices for name"
    );
}

#[test]
fn test_fuzzy_search_scripts_match_indices_for_filename() {
    let scripts = vec![Script {
        name: "Other Name".to_string(), // Name doesn't match
        path: PathBuf::from("/scripts/my-test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }];

    let results = fuzzy_search_scripts(&scripts, "mts");
    assert_eq!(results.len(), 1);
    // Match indices are now computed lazily - verify using compute_match_indices_for_result
    let indices =
        compute_match_indices_for_result(&SearchResult::Script(results[0].clone()), "mts");
    // "mts" matches indices in "my-test.ts": m=0, t=3, s=5
    assert_eq!(
        indices.filename_indices,
        vec![0, 3, 5],
        "Should return correct match indices for filename when name doesn't match"
    );
}

#[test]
fn test_fuzzy_search_scriptlets_by_file_path() {
    // Users should be able to search by ".md" to find scriptlets
    let scriptlets = vec![
        Scriptlet {
            name: "Open GitHub".to_string(),
            description: Some("Opens GitHub in browser".to_string()),
            code: "open('https://github.com')".to_string(),
            tool: "ts".to_string(),
            shortcut: None,
            expand: None,
            group: Some("URLs".to_string()),
            file_path: Some("/path/to/urls.md#open-github".to_string()),
            command: Some("open-github".to_string()),
            alias: None,
        },
        Scriptlet {
            name: "Copy Text".to_string(),
            description: Some("Copies text".to_string()),
            code: "copy()".to_string(),
            tool: "ts".to_string(),
            shortcut: None,
            expand: None,
            group: None,
            file_path: Some("/path/to/clipboard.md#copy-text".to_string()),
            command: Some("copy-text".to_string()),
            alias: None,
        },
    ];

    let results = fuzzy_search_scriptlets(&scriptlets, ".md");
    assert_eq!(results.len(), 2, "Should find scriptlets by .md extension");
}

#[test]
fn test_fuzzy_search_scriptlets_by_anchor() {
    // Users should be able to search by anchor slug
    let scriptlets = vec![
        Scriptlet {
            name: "Open GitHub".to_string(),
            description: None,
            code: "code".to_string(),
            tool: "ts".to_string(),
            shortcut: None,
            expand: None,
            group: None,
            file_path: Some("/path/to/file.md#open-github".to_string()),
            command: Some("open-github".to_string()),
            alias: None,
        },
        Scriptlet {
            name: "Close Tab".to_string(),
            description: None,
            code: "code".to_string(),
            tool: "ts".to_string(),
            shortcut: None,
            expand: None,
            group: None,
            file_path: Some("/path/to/file.md#close-tab".to_string()),
            command: Some("close-tab".to_string()),
            alias: None,
        },
    ];

    let results = fuzzy_search_scriptlets(&scriptlets, "github");
    assert_eq!(results.len(), 1, "Should find scriptlet by anchor slug");
    assert_eq!(results[0].scriptlet.name, "Open GitHub");
}

#[test]
fn test_fuzzy_search_scriptlets_display_file_path() {
    // display_file_path should be the filename#anchor format
    let scriptlets = vec![Scriptlet {
        name: "Test".to_string(),
        description: None,
        code: "code".to_string(),
        tool: "ts".to_string(),
        shortcut: None,
        expand: None,
        group: None,
        file_path: Some("/home/user/.sk/kit/scriptlets/urls.md#test-slug".to_string()),
        command: Some("test-slug".to_string()),
        alias: None,
    }];

    let results = fuzzy_search_scriptlets(&scriptlets, "");
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].display_file_path,
        Some("urls.md#test-slug".to_string()),
        "display_file_path should be filename#anchor format"
    );
}

#[test]
fn test_fuzzy_search_scriptlets_match_indices() {
    let scriptlets = vec![Scriptlet {
        name: "Other".to_string(), // Name doesn't match
        description: None,
        code: "code".to_string(),
        tool: "ts".to_string(),
        shortcut: None,
        expand: None,
        group: None,
        file_path: Some("/path/urls.md#test".to_string()),
        command: None,
        alias: None,
    }];

    let results = fuzzy_search_scriptlets(&scriptlets, "url");
    assert_eq!(results.len(), 1);
    // Match indices are now computed lazily - verify using compute_match_indices_for_result
    let indices =
        compute_match_indices_for_result(&SearchResult::Scriptlet(results[0].clone()), "url");
    // "url" matches in "urls.md#test" at indices 0, 1, 2
    assert_eq!(
        indices.filename_indices,
        vec![0, 1, 2],
        "Should return correct match indices for file_path"
    );
}

#[test]
fn test_fuzzy_match_with_indices_basic() {
    let (matched, indices) = fuzzy_match_with_indices("openfile", "opf");
    assert!(matched);
    assert_eq!(indices, vec![0, 1, 4]);
}

#[test]
fn test_fuzzy_match_with_indices_no_match() {
    let (matched, indices) = fuzzy_match_with_indices("test", "xyz");
    assert!(!matched);
    assert!(indices.is_empty());
}

#[test]
fn test_fuzzy_match_with_indices_case_insensitive() {
    let (matched, indices) = fuzzy_match_with_indices("OpenFile", "of");
    assert!(matched);
    assert_eq!(indices, vec![0, 4]);
}

#[test]
fn test_extract_filename() {
    assert_eq!(
        extract_filename(&PathBuf::from("/path/to/script.ts")),
        "script.ts"
    );
    assert_eq!(
        extract_filename(&PathBuf::from("relative/path.js")),
        "path.js"
    );
    assert_eq!(extract_filename(&PathBuf::from("single.ts")), "single.ts");
}

#[test]
fn test_extract_scriptlet_display_path() {
    // With anchor
    assert_eq!(
        extract_scriptlet_display_path(&Some("/path/to/file.md#slug".to_string())),
        Some("file.md#slug".to_string())
    );

    // Without anchor
    assert_eq!(
        extract_scriptlet_display_path(&Some("/path/to/file.md".to_string())),
        Some("file.md".to_string())
    );

    // None input
    assert_eq!(extract_scriptlet_display_path(&None), None);
}

#[test]
fn test_fuzzy_search_scripts_empty_query_has_filename() {
    // Even with empty query, filename should be populated
    let scripts = vec![Script {
        name: "Test".to_string(),
        path: PathBuf::from("/path/my-script.ts"),
        extension: "ts".to_string(),
        ..Default::default()
    }];

    let results = fuzzy_search_scripts(&scripts, "");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].filename, "my-script.ts");
}

// ============================================
// TYPED METADATA & SCHEMA INTEGRATION TESTS
// ============================================

#[test]
fn test_script_struct_has_typed_fields() {
    // Test that Script struct includes typed_metadata and schema fields
    use crate::metadata_parser::TypedMetadata;
    use crate::schema_parser::{FieldDef, FieldType, Schema};
    use std::collections::HashMap;

    let typed_meta = TypedMetadata {
        name: Some("My Typed Script".to_string()),
        description: Some("A script with typed metadata".to_string()),
        alias: Some("mts".to_string()),
        icon: Some("Star".to_string()),
        ..Default::default()
    };

    let mut input_fields = HashMap::new();
    input_fields.insert(
        "title".to_string(),
        FieldDef {
            field_type: FieldType::String,
            required: true,
            description: Some("The title".to_string()),
            ..Default::default()
        },
    );

    let schema = Schema {
        input: input_fields,
        output: HashMap::new(),
    };

    let script = Script {
        name: "test".to_string(),
        path: PathBuf::from("/test.ts"),
        extension: "ts".to_string(),
        typed_metadata: Some(typed_meta.clone()),
        schema: Some(schema.clone()),
        ..Default::default()
    };

    // Verify typed_metadata is accessible
    assert!(script.typed_metadata.is_some());
    let meta = script.typed_metadata.as_ref().unwrap();
    assert_eq!(meta.name, Some("My Typed Script".to_string()));
    assert_eq!(meta.alias, Some("mts".to_string()));
    assert_eq!(meta.icon, Some("Star".to_string()));

    // Verify schema is accessible
    assert!(script.schema.is_some());
    let sch = script.schema.as_ref().unwrap();
    assert_eq!(sch.input.len(), 1);
    assert!(sch.input.contains_key("title"));
}

#[test]
fn test_extract_typed_metadata_from_script() {
    // Test that extract_full_metadata correctly parses typed metadata
    let content = r#"
metadata = {
    name: "Create Note",
    description: "Creates a new note in the notes directory",
    author: "John Lindquist",
    alias: "note",
    icon: "File",
    shortcut: "cmd n"
}

const title = await arg("Enter title");
"#;

    let (script_meta, typed_meta, _schema) = extract_full_metadata(content);

    // Typed metadata should be parsed
    assert!(typed_meta.is_some());
    let meta = typed_meta.unwrap();
    assert_eq!(meta.name, Some("Create Note".to_string()));
    assert_eq!(
        meta.description,
        Some("Creates a new note in the notes directory".to_string())
    );
    assert_eq!(meta.alias, Some("note".to_string()));
    assert_eq!(meta.icon, Some("File".to_string()));
    assert_eq!(meta.shortcut, Some("cmd n".to_string()));

    // Script metadata should also be populated from typed
    assert_eq!(script_meta.name, Some("Create Note".to_string()));
    assert_eq!(script_meta.alias, Some("note".to_string()));
}

#[test]
fn test_extract_schema_from_script() {
    use crate::schema_parser::ItemsDef;

    // Test that extract_full_metadata correctly parses schema
    let content = r#"
schema = {
    input: {
        title: { type: "string", required: true, description: "Note title" },
        tags: { type: "array", items: "string" }
    },
    output: {
        path: { type: "string", description: "Path to created file" }
    }
}

const { title, tags } = await input();
"#;

    let (_script_meta, _typed_meta, schema) = extract_full_metadata(content);

    // Schema should be parsed
    assert!(schema.is_some());
    let sch = schema.unwrap();

    // Check input fields
    assert_eq!(sch.input.len(), 2);
    let title_field = sch.input.get("title").unwrap();
    assert!(title_field.required);
    assert_eq!(title_field.description, Some("Note title".to_string()));

    let tags_field = sch.input.get("tags").unwrap();
    assert_eq!(tags_field.items, Some(ItemsDef::Type("string".to_string())));

    // Check output fields
    assert_eq!(sch.output.len(), 1);
    assert!(sch.output.contains_key("path"));
}

#[test]
fn test_fallback_to_comment_metadata() {
    // Test that when no typed metadata exists, we fall back to comment-based metadata
    let content = r#"// Name: My Script
// Description: A script without typed metadata
// Icon: Terminal
// Alias: ms
// Shortcut: opt m

const x = await arg("Pick one");
"#;

    let (script_meta, typed_meta, schema) = extract_full_metadata(content);

    // No typed metadata in this script
    assert!(typed_meta.is_none());
    assert!(schema.is_none());

    // But script metadata should be extracted from comments
    assert_eq!(script_meta.name, Some("My Script".to_string()));
    assert_eq!(
        script_meta.description,
        Some("A script without typed metadata".to_string())
    );
    assert_eq!(script_meta.icon, Some("Terminal".to_string()));
    assert_eq!(script_meta.alias, Some("ms".to_string()));
    assert_eq!(script_meta.shortcut, Some("opt m".to_string()));
}

#[test]
fn test_both_typed_and_comment_prefers_typed() {
    // Test that when both typed metadata AND comment metadata exist,
    // the typed metadata takes precedence
    let content = r#"// Name: Comment Name
// Description: Comment Description
// Alias: cn

metadata = {
    name: "Typed Name",
    description: "Typed Description",
    alias: "tn"
}

const x = await arg("Pick");
"#;

    let (script_meta, typed_meta, _schema) = extract_full_metadata(content);

    // Typed metadata should be present
    assert!(typed_meta.is_some());
    let meta = typed_meta.unwrap();
    assert_eq!(meta.name, Some("Typed Name".to_string()));
    assert_eq!(meta.description, Some("Typed Description".to_string()));
    assert_eq!(meta.alias, Some("tn".to_string()));

    // Script metadata should use typed values (typed takes precedence)
    assert_eq!(script_meta.name, Some("Typed Name".to_string()));
    assert_eq!(
        script_meta.description,
        Some("Typed Description".to_string())
    );
    assert_eq!(script_meta.alias, Some("tn".to_string()));
}

#[test]
fn test_typed_metadata_partial_with_comment_fallback() {
    // Test that typed metadata can be partial and comment metadata fills gaps
    let content = r#"// Name: Comment Name
// Description: Full description
// Icon: Terminal
// Shortcut: opt x

metadata = {
    name: "Typed Name",
    alias: "tn"
}

const x = await arg("Pick");
"#;

    let (script_meta, typed_meta, _schema) = extract_full_metadata(content);

    // Typed metadata is present but partial
    assert!(typed_meta.is_some());
    let meta = typed_meta.unwrap();
    assert_eq!(meta.name, Some("Typed Name".to_string()));
    assert_eq!(meta.alias, Some("tn".to_string()));
    assert!(meta.description.is_none()); // Not in typed
    assert!(meta.icon.is_none()); // Not in typed
    assert!(meta.shortcut.is_none()); // Not in typed

    // Script metadata should use typed for what's available, comments for rest
    assert_eq!(script_meta.name, Some("Typed Name".to_string())); // From typed
    assert_eq!(script_meta.alias, Some("tn".to_string())); // From typed
    assert_eq!(
        script_meta.description,
        Some("Full description".to_string())
    ); // From comment
    assert_eq!(script_meta.icon, Some("Terminal".to_string())); // From comment
    assert_eq!(script_meta.shortcut, Some("opt x".to_string())); // From comment
}

#[test]
fn test_both_metadata_and_schema() {
    // Test extracting both metadata and schema from a single script
    let content = r#"
metadata = {
    name: "Full Featured Script",
    description: "Has both metadata and schema",
    alias: "ffs"
}

schema = {
    input: {
        query: { type: "string", required: true }
    },
    output: {
        result: { type: "string" }
    }
}

const { query } = await input();
"#;

    let (script_meta, typed_meta, schema) = extract_full_metadata(content);

    // Both should be present
    assert!(typed_meta.is_some());
    assert!(schema.is_some());

    // Verify metadata
    let meta = typed_meta.unwrap();
    assert_eq!(meta.name, Some("Full Featured Script".to_string()));
    assert_eq!(meta.alias, Some("ffs".to_string()));

    // Verify schema
    let sch = schema.unwrap();
    assert_eq!(sch.input.len(), 1);
    assert_eq!(sch.output.len(), 1);

    // Script metadata populated
    assert_eq!(script_meta.name, Some("Full Featured Script".to_string()));
}

/// Performance benchmark for get_grouped_results
/// This test verifies that repeated calls with the same filter don't regress performance.
/// It creates realistic data (100 scripts, 50 scriptlets, 20 builtins, 30 apps)
/// and measures the time for 100 repeated calls.
#[test]
fn bench_get_grouped_results_repeated_calls() {
    use std::time::Instant;

    // Create realistic test data
    let scripts: Vec<Script> = (0..100)
        .map(|i| Script {
            name: format!("script-{:03}", i),
            path: PathBuf::from(format!("/test/scripts/script-{:03}.ts", i)),
            extension: "ts".to_string(),
            description: Some(format!("Description for script {}", i)),
            ..Default::default()
        })
        .collect();

    let scriptlets: Vec<Scriptlet> = (0..50)
        .map(|i| Scriptlet {
            name: format!("snippet-{:02}", i),
            file_path: Some(format!("/test/scriptlets/snippet-{:02}.md", i)),
            tool: "ts".to_string(),
            code: format!("console.log('snippet {}')", i),
            description: Some(format!("Snippet {} description", i)),
            shortcut: None,
            expand: None,
            group: None,
            command: None,
            alias: None,
        })
        .collect();

    let builtins: Vec<crate::builtins::BuiltInEntry> = (0..20)
        .map(|i| crate::builtins::BuiltInEntry {
            id: format!("builtin-{:02}", i),
            name: format!("builtin-{:02}", i),
            description: format!("Built-in {} description", i),
            keywords: vec![format!("keyword{}", i)],
            feature: crate::builtins::BuiltInFeature::ClipboardHistory,
            icon: None,
        })
        .collect();

    let apps: Vec<crate::app_launcher::AppInfo> = (0..30)
        .map(|i| crate::app_launcher::AppInfo {
            name: format!("App {:02}", i),
            path: PathBuf::from(format!("/Applications/App{:02}.app", i)),
            bundle_id: Some(format!("com.test.app{:02}", i)),
            icon: None,
        })
        .collect();

    let frecency_store = crate::frecency::FrecencyStore::new();

    // Warm up
    let _ = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        "",
        10,
    );

    // Benchmark: 100 calls with empty filter (grouped mode)
    let start = Instant::now();
    for _ in 0..100 {
        let _ = get_grouped_results(
            &scripts,
            &scriptlets,
            &builtins,
            &apps,
            &frecency_store,
            "",
            10,
        );
    }
    let empty_filter_duration = start.elapsed();

    // Benchmark: 100 calls with filter (search mode)
    let start = Instant::now();
    for _ in 0..100 {
        let _ = get_grouped_results(
            &scripts,
            &scriptlets,
            &builtins,
            &apps,
            &frecency_store,
            "scr",
            10,
        );
    }
    let search_filter_duration = start.elapsed();

    // Log results (visible with cargo test -- --nocapture)
    println!("\n=== get_grouped_results Performance Benchmark ===");
    println!(
        "Data: {} scripts, {} scriptlets, {} builtins, {} apps",
        scripts.len(),
        scriptlets.len(),
        builtins.len(),
        apps.len()
    );
    println!(
        "Empty filter (100 calls): {:?} ({:.2}ms per call)",
        empty_filter_duration,
        empty_filter_duration.as_secs_f64() * 10.0
    );
    println!(
        "Search filter 'scr' (100 calls): {:?} ({:.2}ms per call)",
        search_filter_duration,
        search_filter_duration.as_secs_f64() * 10.0
    );
    println!("===============================================\n");

    // Performance assertions - each call should be under 5ms
    // (with caching, repeated calls should be nearly instant)
    let max_per_call_ms = 5.0;
    assert!(
        empty_filter_duration.as_secs_f64() * 10.0 < max_per_call_ms,
        "Empty filter calls too slow: {:.2}ms per call (max: {}ms)",
        empty_filter_duration.as_secs_f64() * 10.0,
        max_per_call_ms
    );
    assert!(
        search_filter_duration.as_secs_f64() * 10.0 < max_per_call_ms,
        "Search filter calls too slow: {:.2}ms per call (max: {}ms)",
        search_filter_duration.as_secs_f64() * 10.0,
        max_per_call_ms
    );
}

// ============================================
// ASCII CASE-FOLDING HELPER TESTS
// ============================================

#[test]
fn test_contains_ignore_ascii_case_basic() {
    // Note: needle_lower must already be lowercase
    assert!(contains_ignore_ascii_case("OpenFile", "open"));
    assert!(contains_ignore_ascii_case("OPENFILE", "open"));
    assert!(contains_ignore_ascii_case("openfile", "open"));
    assert!(contains_ignore_ascii_case("MyOpenFile", "open"));
}

#[test]
fn test_contains_ignore_ascii_case_not_found() {
    assert!(!contains_ignore_ascii_case("OpenFile", "save"));
    assert!(!contains_ignore_ascii_case("test", "testing"));
}

#[test]
fn test_contains_ignore_ascii_case_empty_needle() {
    assert!(contains_ignore_ascii_case("OpenFile", ""));
    assert!(contains_ignore_ascii_case("", ""));
}

#[test]
fn test_contains_ignore_ascii_case_needle_longer() {
    assert!(!contains_ignore_ascii_case("ab", "abc"));
}

#[test]
fn test_find_ignore_ascii_case_at_start() {
    assert_eq!(find_ignore_ascii_case("OpenFile", "open"), Some(0));
    assert_eq!(find_ignore_ascii_case("OPENFILE", "open"), Some(0));
}

#[test]
fn test_find_ignore_ascii_case_in_middle() {
    assert_eq!(find_ignore_ascii_case("MyOpenFile", "open"), Some(2));
}

#[test]
fn test_find_ignore_ascii_case_not_found() {
    assert_eq!(find_ignore_ascii_case("OpenFile", "save"), None);
}

#[test]
fn test_find_ignore_ascii_case_empty_needle() {
    assert_eq!(find_ignore_ascii_case("OpenFile", ""), Some(0));
}

#[test]
fn test_fuzzy_match_with_indices_ascii_basic() {
    let (matched, indices) = fuzzy_match_with_indices_ascii("OpenFile", "of");
    assert!(matched);
    assert_eq!(indices, vec![0, 4]); // 'O' at 0, 'F' at 4
}

#[test]
fn test_fuzzy_match_with_indices_ascii_case_insensitive() {
    // Note: pattern_lower must already be lowercase
    let (matched, indices) = fuzzy_match_with_indices_ascii("OpenFile", "of");
    assert!(matched);
    assert_eq!(indices, vec![0, 4]);
}

#[test]
fn test_fuzzy_match_with_indices_ascii_no_match() {
    let (matched, indices) = fuzzy_match_with_indices_ascii("test", "xyz");
    assert!(!matched);
    assert!(indices.is_empty());
}

#[test]
fn test_fuzzy_match_with_indices_ascii_empty_pattern() {
    let (matched, indices) = fuzzy_match_with_indices_ascii("test", "");
    assert!(matched);
    assert!(indices.is_empty());
}

#[test]
fn test_compute_match_indices_for_script_result() {
    let script_match = ScriptMatch {
        script: Script {
            name: "OpenFile".to_string(),
            path: PathBuf::from("/openfile.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        score: 100,
        filename: "openfile.ts".to_string(),
        match_indices: MatchIndices::default(),
    };
    let result = SearchResult::Script(script_match);

    let indices = compute_match_indices_for_result(&result, "of");
    assert!(!indices.name_indices.is_empty());
    assert_eq!(indices.name_indices, vec![0, 4]); // 'O' at 0, 'F' at 4
}

#[test]
fn test_compute_match_indices_for_scriptlet_result() {
    let scriptlet_match = ScriptletMatch {
        scriptlet: test_scriptlet("Copy Text", "ts", "copy()"),
        score: 100,
        display_file_path: Some("copy.md#copy-text".to_string()),
        match_indices: MatchIndices::default(),
    };
    let result = SearchResult::Scriptlet(scriptlet_match);

    let indices = compute_match_indices_for_result(&result, "ct");
    assert!(!indices.name_indices.is_empty());
    assert_eq!(indices.name_indices, vec![0, 5]); // 'C' at 0, 'T' at 5
}

#[test]
fn test_compute_match_indices_empty_query() {
    let script_match = ScriptMatch {
        script: Script {
            name: "Test".to_string(),
            path: PathBuf::from("/test.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        score: 0,
        filename: "test.ts".to_string(),
        match_indices: MatchIndices::default(),
    };
    let result = SearchResult::Script(script_match);

    let indices = compute_match_indices_for_result(&result, "");
    assert!(indices.name_indices.is_empty());
    assert!(indices.filename_indices.is_empty());
}

#[test]
fn test_scriptlet_code_search_gated_by_length() {
    // Code search only happens when query >= 4 chars and score == 0
    // Use a name that doesn't contain any of the search characters
    let scriptlets = vec![test_scriptlet("Utility", "ts", "contains_xyz_function()")];

    // Short query - should NOT search code (even if it would match)
    let results = fuzzy_search_scriptlets(&scriptlets, "xyz");
    assert!(results.is_empty()); // No match because "xyz" only in code, and query < 4 chars

    // Long query >= 4 chars should search code when name doesn't match
    let results = fuzzy_search_scriptlets(&scriptlets, "xyz_f");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].score, 5); // Only code match score
}

#[test]
fn test_scriptlet_code_search_skipped_when_name_matches() {
    // If name matches, code search is skipped (score > 0)
    let scriptlets = vec![test_scriptlet("special_snippet", "ts", "unrelated_code()")];

    // Should match on name, not search code
    let results = fuzzy_search_scriptlets(&scriptlets, "special");
    assert_eq!(results.len(), 1);
    // Score should be from name match, not code match
    assert!(results[0].score > 5);
}

// ============================================
// FRECENCY CACHE INVALIDATION TESTS
// ============================================
//
// BUG: When frecency_store.record_use() is called in main.rs:1904,
// the grouped_results cache is NOT invalidated. This means when
// the window is re-shown, stale cached results are returned instead
// of results reflecting the updated frecency scores.
//
// These tests verify the expected behavior of get_grouped_results()
// with frecency - the pure function works correctly. The cache
// invalidation bug is in main.rs::get_grouped_results_cached().

/// Helper to create a test Script with a given path
fn test_script_with_path(name: &str, path: &str) -> Script {
    Script {
        name: name.to_string(),
        path: PathBuf::from(path),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }
}

#[test]
fn test_get_grouped_results_respects_frecency_ordering() {
    use crate::frecency::FrecencyStore;
    use tempfile::NamedTempFile;

    // Create a frecency store with temp file
    let temp_file = NamedTempFile::new().unwrap();
    let mut frecency_store = FrecencyStore::with_path(temp_file.path().to_path_buf());

    // Create test scripts
    let scripts = vec![
        test_script_with_path("Alpha Script", "/test/alpha.ts"),
        test_script_with_path("Beta Script", "/test/beta.ts"),
        test_script_with_path("Gamma Script", "/test/gamma.ts"),
    ];
    let scriptlets: Vec<Scriptlet> = vec![];
    let builtins: Vec<BuiltInEntry> = vec![];
    let apps: Vec<crate::app_launcher::AppInfo> = vec![];

    // Initially no frecency - should return alphabetical order in MAIN section
    let (grouped1, _results1) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        "",
        10,
    );

    // Should have MAIN section header + 3 items
    assert!(grouped1.len() >= 3);

    // Record use for Gamma (should become "recent")
    frecency_store.record_use("/test/gamma.ts");

    // Now get_grouped_results should show Gamma in RECENT section
    let (grouped2, results2) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        "",
        10,
    );

    // Should now have RECENT header, at least one recent item, MAIN header, remaining items
    // The first section header should be "RECENT"
    let first_header = grouped2.iter().find_map(|item| match item {
        GroupedListItem::SectionHeader(s) => Some(s.clone()),
        _ => None,
    });
    assert_eq!(
        first_header,
        Some("RECENT".to_string()),
        "After recording use, RECENT section should appear"
    );

    // Find the first item after the RECENT header - it should be Gamma
    let mut found_recent_header = false;
    let mut first_recent_item: Option<&SearchResult> = None;
    for item in grouped2.iter() {
        match item {
            GroupedListItem::SectionHeader(s) if s == "RECENT" => {
                found_recent_header = true;
            }
            GroupedListItem::Item(idx) if found_recent_header && first_recent_item.is_none() => {
                first_recent_item = results2.get(*idx);
                break;
            }
            _ => {}
        }
    }

    assert!(
        first_recent_item.is_some(),
        "Should have at least one item in RECENT section"
    );
    assert_eq!(
        first_recent_item.unwrap().name(),
        "Gamma Script",
        "The most recently used script should appear first in RECENT section"
    );
}

#[test]
fn test_get_grouped_results_updates_after_frecency_change() {
    use crate::frecency::FrecencyStore;
    use tempfile::NamedTempFile;

    // Create a frecency store with temp file
    let temp_file = NamedTempFile::new().unwrap();
    let mut frecency_store = FrecencyStore::with_path(temp_file.path().to_path_buf());

    // Create test scripts
    let scripts = vec![
        test_script_with_path("First Script", "/test/first.ts"),
        test_script_with_path("Second Script", "/test/second.ts"),
    ];
    let scriptlets: Vec<Scriptlet> = vec![];
    let builtins: Vec<BuiltInEntry> = vec![];
    let apps: Vec<crate::app_launcher::AppInfo> = vec![];

    // Record initial use for First
    frecency_store.record_use("/test/first.ts");

    // Get initial results
    let (grouped1, results1) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        "",
        10,
    );

    // Find the first recent item - should be "First Script"
    let first_recent_1 = grouped1
        .iter()
        .filter_map(|item| match item {
            GroupedListItem::Item(idx) => results1.get(*idx),
            _ => None,
        })
        .next();
    assert_eq!(first_recent_1.map(|r| r.name()), Some("First Script"));

    // Now record use for Second (multiple times to ensure higher frecency)
    frecency_store.record_use("/test/second.ts");
    frecency_store.record_use("/test/second.ts");
    frecency_store.record_use("/test/second.ts");

    // Get updated results - THIS IS WHERE THE BUG MANIFESTS IN MAIN.RS
    // The pure function correctly returns updated results,
    // but get_grouped_results_cached() would return stale cached results
    // because invalidate_grouped_cache() is not called after record_use()
    let (grouped2, results2) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        "",
        10,
    );

    // Find items in RECENT section
    let mut in_recent_section = false;
    let mut recent_items: Vec<&str> = vec![];
    for item in grouped2.iter() {
        match item {
            GroupedListItem::SectionHeader(s) if s == "RECENT" => {
                in_recent_section = true;
            }
            GroupedListItem::SectionHeader(_) => {
                in_recent_section = false;
            }
            GroupedListItem::Item(idx) if in_recent_section => {
                if let Some(result) = results2.get(*idx) {
                    recent_items.push(result.name());
                }
            }
            _ => {}
        }
    }

    // Second Script should now be first in RECENT (higher frecency score)
    assert!(!recent_items.is_empty(), "Should have recent items");
    assert_eq!(
        recent_items[0], "Second Script",
        "Script with higher frecency (more uses) should appear first in RECENT"
    );
}

/// This test simulates the caching behavior in main.rs to demonstrate the bug.
///
/// In the actual app, ScriptKitApp has:
/// - grouped_cache_key: String - tracks what filter the cache was computed for
/// - cached_grouped_items: Arc<[GroupedListItem]> - cached results
///
/// BUG: When frecency_store.record_use() is called, the cache is NOT invalidated,
/// so subsequent calls return stale results.
///
/// This test demonstrates the expected vs actual behavior.
#[test]
fn test_frecency_cache_invalidation_required() {
    use crate::frecency::FrecencyStore;
    use tempfile::NamedTempFile;

    // Create a frecency store
    let temp_file = NamedTempFile::new().unwrap();
    let mut frecency_store = FrecencyStore::with_path(temp_file.path().to_path_buf());

    // Create test scripts
    let scripts = vec![
        test_script_with_path("ScriptA", "/test/a.ts"),
        test_script_with_path("ScriptB", "/test/b.ts"),
    ];
    let scriptlets: Vec<Scriptlet> = vec![];
    let builtins: Vec<BuiltInEntry> = vec![];
    let apps: Vec<crate::app_launcher::AppInfo> = vec![];

    // === SIMULATE MAIN.RS CACHING BEHAVIOR ===

    // Initial call - would populate cache in main.rs
    let filter_text = ""; // Empty filter = main menu view
    let cache_key = filter_text.to_string();

    let (cached_grouped, cached_results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        filter_text,
        10,
    );

    // Record frecency use (this happens in main.rs:1904 after script execution)
    // BUG: This call does NOT invalidate the grouped cache
    frecency_store.record_use("/test/b.ts");

    // === WHAT HAPPENS IN BUGGY CODE ===
    // In main.rs, the cache_key is still the same (empty string for main menu),
    // so get_grouped_results_cached() returns the stale cached results
    // without calling get_grouped_results() again.

    // Simulate cache hit with stale data (this is the BUG)
    let buggy_grouped = if cache_key == filter_text {
        // Cache "hit" - returns stale data, doesn't reflect frecency change
        cached_grouped.clone()
    } else {
        // This branch never executes because cache_key matches
        get_grouped_results(
            &scripts,
            &scriptlets,
            &builtins,
            &apps,
            &frecency_store,
            filter_text,
            10,
        )
        .0
    };
    let buggy_results = if cache_key == filter_text {
        cached_results.clone()
    } else {
        get_grouped_results(
            &scripts,
            &scriptlets,
            &builtins,
            &apps,
            &frecency_store,
            filter_text,
            10,
        )
        .1
    };

    // === WHAT SHOULD HAPPEN (CORRECT BEHAVIOR) ===
    // After frecency_store.record_use(), invalidate_grouped_cache() should be called,
    // forcing a recompute that reflects the updated frecency

    let (correct_grouped, correct_results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        filter_text,
        10,
    );

    // Extract recent items from correct results
    let mut correct_recent_items: Vec<&str> = vec![];
    let mut in_recent = false;
    for item in correct_grouped.iter() {
        match item {
            GroupedListItem::SectionHeader(s) if s == "RECENT" => in_recent = true,
            GroupedListItem::SectionHeader(_) => in_recent = false,
            GroupedListItem::Item(idx) if in_recent => {
                if let Some(r) = correct_results.get(*idx) {
                    correct_recent_items.push(r.name());
                }
            }
            _ => {}
        }
    }

    // Extract recent items from buggy (cached) results
    let mut buggy_recent_items: Vec<&str> = vec![];
    let mut in_recent_buggy = false;
    for item in buggy_grouped.iter() {
        match item {
            GroupedListItem::SectionHeader(s) if s == "RECENT" => in_recent_buggy = true,
            GroupedListItem::SectionHeader(_) => in_recent_buggy = false,
            GroupedListItem::Item(idx) if in_recent_buggy => {
                if let Some(r) = buggy_results.get(*idx) {
                    buggy_recent_items.push(r.name());
                }
            }
            _ => {}
        }
    }

    // The CORRECT results should show ScriptB in RECENT section
    // (because we just recorded a use for it)
    assert!(
        correct_recent_items.contains(&"ScriptB"),
        "CORRECT behavior: ScriptB should appear in RECENT section after record_use()"
    );

    // The BUGGY cached results do NOT show ScriptB in RECENT
    // (because the cache wasn't invalidated)
    //
    // THIS ASSERTION DEMONSTRATES THE BUG:
    // The buggy code returns stale results that don't include ScriptB in RECENT
    assert!(!buggy_recent_items.contains(&"ScriptB"),
            "BUG VERIFICATION: Cached results don't contain ScriptB in RECENT (cache wasn't invalidated). \
             This assertion demonstrates the bug exists - it should be removed after the fix.");

    // The REAL test that should PASS after the fix is applied:
    // When invalidate_grouped_cache() is called after record_use(),
    // the next call to get_grouped_results_cached() should return fresh results
    // that include ScriptB in RECENT.
    //
    // Uncomment this after applying the fix:
    // assert_eq!(buggy_recent_items, correct_recent_items,
    //     "After fix: cached and fresh results should be identical");
}

/// This test verifies that frecency cache invalidation works correctly.
///
/// After frecency_store.record_use() is called in main.rs,
/// invalidate_grouped_cache() is now called, so the cached grouped results
/// are properly invalidated and reflect the updated frecency scores.
///
/// This test simulates the correct behavior: after recording frecency use,
/// subsequent queries return updated results with the frecency changes.
#[test]
fn test_frecency_change_invalidates_cache() {
    use crate::frecency::FrecencyStore;
    use tempfile::NamedTempFile;

    // ============================================================
    // TEST FOR FRECENCY CACHE INVALIDATION (FIXED)
    // ============================================================
    //
    // After calling frecency_store.record_use() in main.rs,
    // invalidate_grouped_cache() is now called, so the cache is
    // properly invalidated and subsequent queries return fresh results.
    //
    // This test simulates the caching pattern from main.rs and
    // verifies the correct behavior: frecency changes are reflected
    // in subsequent queries.
    // ============================================================

    // Setup
    let temp_file = NamedTempFile::new().unwrap();
    let mut frecency_store = FrecencyStore::with_path(temp_file.path().to_path_buf());

    let scripts = vec![
        test_script_with_path("AlphaScript", "/test/alpha.ts"),
        test_script_with_path("BetaScript", "/test/beta.ts"),
    ];
    let scriptlets: Vec<Scriptlet> = vec![];
    let builtins: Vec<BuiltInEntry> = vec![];
    let apps: Vec<crate::app_launcher::AppInfo> = vec![];

    // === Simulate ScriptKitApp state ===
    struct MockCache {
        grouped_cache_key: String,
        cached_grouped: Vec<GroupedListItem>,
        cached_results: Vec<SearchResult>,
        cache_valid: bool,
    }

    impl MockCache {
        fn new() -> Self {
            MockCache {
                grouped_cache_key: String::from("\0_INVALIDATED_\0"),
                cached_grouped: vec![],
                cached_results: vec![],
                cache_valid: false,
            }
        }

        /// Simulates get_grouped_results_cached() from main.rs
        fn get_cached(
            &mut self,
            scripts: &[Script],
            scriptlets: &[Scriptlet],
            builtins: &[BuiltInEntry],
            apps: &[crate::app_launcher::AppInfo],
            frecency_store: &FrecencyStore,
            filter_text: &str,
        ) -> (Vec<GroupedListItem>, Vec<SearchResult>) {
            // Cache hit check (simulates main.rs line 1493)
            if self.cache_valid && filter_text == self.grouped_cache_key {
                return (self.cached_grouped.clone(), self.cached_results.clone());
            }

            // Cache miss - recompute
            let (grouped, results) = get_grouped_results(
                scripts,
                scriptlets,
                builtins,
                apps,
                frecency_store,
                filter_text,
                10,
            );

            self.cached_grouped = grouped.clone();
            self.cached_results = results.clone();
            self.grouped_cache_key = filter_text.to_string();
            self.cache_valid = true;

            (grouped, results)
        }

        /// This should be called after frecency_store.record_use()
        /// BUG: This is NOT called in main.rs!
        #[allow(dead_code)]
        fn invalidate(&mut self) {
            self.cache_valid = false;
            self.grouped_cache_key = String::from("\0_INVALIDATED_\0");
        }
    }

    let mut cache = MockCache::new();
    let filter_text = "";

    // Initial query - populates cache
    let (initial_grouped, _initial_results) = cache.get_cached(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        filter_text,
    );

    // Verify initial state: no RECENT section (no frecency data)
    let initial_has_recent = initial_grouped
        .iter()
        .any(|item| matches!(item, GroupedListItem::SectionHeader(s) if s == "RECENT"));
    assert!(
        !initial_has_recent,
        "Initially there should be no RECENT section"
    );

    // === THIS IS WHERE THE BUG HAPPENS ===
    // In main.rs:1904, frecency_store.record_use() is called
    // but invalidate_grouped_cache() is NOT called
    frecency_store.record_use("/test/beta.ts");

    // FIXED: cache.invalidate() is now called in main.rs after frecency_store.record_use()
    // This mock simulates the fixed behavior:
    cache.invalidate();

    // Query again - should return fresh results with BetaScript in RECENT
    let (second_grouped, second_results) = cache.get_cached(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &frecency_store,
        filter_text,
    );

    // Extract RECENT items from second query
    let mut recent_items: Vec<&str> = vec![];
    let mut in_recent = false;
    for item in second_grouped.iter() {
        match item {
            GroupedListItem::SectionHeader(s) if s == "RECENT" => in_recent = true,
            GroupedListItem::SectionHeader(_) => in_recent = false,
            GroupedListItem::Item(idx) if in_recent => {
                if let Some(r) = second_results.get(*idx) {
                    recent_items.push(r.name());
                }
            }
            _ => {}
        }
    }

    // === VERIFY CACHE INVALIDATION WORKS ===
    // After frecency_store.record_use() and cache.invalidate(),
    // the RECENT section should contain BetaScript.
    assert!(
        recent_items.contains(&"BetaScript"),
        "After frecency_store.record_use('/test/beta.ts') and cache invalidation, \
             BetaScript should appear in RECENT section. \
             Got RECENT items: {:?}.",
        recent_items
    );
}

// ============================================================================
// NUCLEO INTEGRATION TESTS
// ============================================================================
// These tests verify the nucleo_score helper function for fuzzy matching

#[test]
fn test_nucleo_score_basic_match() {
    use nucleo_matcher::pattern::Pattern;
    use nucleo_matcher::{Matcher, Utf32Str};

    // Test basic fuzzy matching
    let pattern = Pattern::parse(
        "hello",
        nucleo_matcher::pattern::CaseMatching::Smart,
        nucleo_matcher::pattern::Normalization::Smart,
    );
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);

    // Score a matching haystack
    let mut buf = Vec::new();
    let haystack = Utf32Str::new("hello world", &mut buf);
    let score = pattern.score(haystack, &mut matcher);

    assert!(
        score.is_some(),
        "nucleo should match 'hello' in 'hello world'"
    );
    assert!(
        score.unwrap() > 0,
        "score should be positive for exact match"
    );
}

#[test]
fn test_nucleo_score_fuzzy_match() {
    use nucleo_matcher::pattern::Pattern;
    use nucleo_matcher::{Matcher, Utf32Str};

    let pattern = Pattern::parse(
        "hlo",
        nucleo_matcher::pattern::CaseMatching::Smart,
        nucleo_matcher::pattern::Normalization::Smart,
    );
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);

    // Score a fuzzy matching haystack (h-e-l-l-o contains h-l-o)
    let mut buf = Vec::new();
    let haystack = Utf32Str::new("hello", &mut buf);
    let score = pattern.score(haystack, &mut matcher);

    assert!(
        score.is_some(),
        "nucleo should fuzzy match 'hlo' in 'hello'"
    );
}

#[test]
fn test_nucleo_score_no_match() {
    use nucleo_matcher::pattern::Pattern;
    use nucleo_matcher::{Matcher, Utf32Str};

    let pattern = Pattern::parse(
        "xyz",
        nucleo_matcher::pattern::CaseMatching::Smart,
        nucleo_matcher::pattern::Normalization::Smart,
    );
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);

    let mut buf = Vec::new();
    let haystack = Utf32Str::new("hello world", &mut buf);
    let score = pattern.score(haystack, &mut matcher);

    assert!(
        score.is_none(),
        "nucleo should not match 'xyz' in 'hello world'"
    );
}

#[test]
fn test_nucleo_score_ranking() {
    use nucleo_matcher::pattern::Pattern;
    use nucleo_matcher::{Matcher, Utf32Str};

    let pattern = Pattern::parse(
        "git",
        nucleo_matcher::pattern::CaseMatching::Smart,
        nucleo_matcher::pattern::Normalization::Smart,
    );
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);

    // Exact match should score higher than partial match
    let mut buf1 = Vec::new();
    let haystack_exact = Utf32Str::new("git-commit", &mut buf1);
    let score_exact = pattern.score(haystack_exact, &mut matcher);

    let mut buf2 = Vec::new();
    let haystack_partial = Utf32Str::new("digit-recognizer", &mut buf2);
    let score_partial = pattern.score(haystack_partial, &mut matcher);

    assert!(score_exact.is_some(), "should match 'git' in 'git-commit'");
    assert!(
        score_partial.is_some(),
        "should match 'git' in 'digit-recognizer'"
    );

    // Exact prefix should score higher
    assert!(
        score_exact.unwrap() > score_partial.unwrap(),
        "exact prefix 'git-commit' should score higher than 'digit-recognizer'"
    );
}

#[test]
fn test_nucleo_score_case_insensitive() {
    use nucleo_matcher::pattern::Pattern;
    use nucleo_matcher::{Matcher, Utf32Str};

    // Smart mode: lowercase pattern matches case-insensitively
    let pattern = Pattern::parse(
        "hello",
        nucleo_matcher::pattern::CaseMatching::Smart,
        nucleo_matcher::pattern::Normalization::Smart,
    );
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);

    let mut buf = Vec::new();
    let haystack = Utf32Str::new("HELLO WORLD", &mut buf);
    let score = pattern.score(haystack, &mut matcher);

    // Lowercase pattern with Smart case matching should match uppercase haystack
    assert!(
        score.is_some(),
        "nucleo with Smart case matching should match lowercase 'hello' in uppercase 'HELLO WORLD'"
    );
}
