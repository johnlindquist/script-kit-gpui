// ============================================
// FILENAME SEARCH TESTS
// ============================================

#[test]
fn test_fuzzy_search_scripts_by_file_extension() {
    // Users should be able to search by typing ".ts" to find TypeScript scripts
    let scripts = wrap_scripts(vec![
        Script {
            name: "My Script".to_string(),
            path: PathBuf::from("/home/user/.scriptkit/scripts/my-script.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "Other Script".to_string(),
            path: PathBuf::from("/home/user/.scriptkit/scripts/other.js"),
            extension: "js".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ]);

    let results = fuzzy_search_scripts(&scripts, ".ts");
    assert_eq!(results.len(), 1, "Should find scripts by file extension");
    assert_eq!(results[0].script.name, "My Script");
    assert_eq!(results[0].filename, "my-script.ts");
    assert!(results[0].score > 0);
}

#[test]
fn test_fuzzy_search_scripts_by_filename() {
    // Users should be able to search by filename
    let scripts = wrap_scripts(vec![
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
    ]);

    // Search by filename (not matching the name "Open File")
    let results = fuzzy_search_scripts(&scripts, "dialog");
    assert_eq!(results.len(), 1, "Should find scripts by filename content");
    assert_eq!(results[0].script.name, "Open File");
    assert_eq!(results[0].filename, "open-file-dialog.ts");
}

#[test]
fn test_fuzzy_search_scripts_filename_returns_correct_filename() {
    let scripts = wrap_scripts(vec![Script {
        name: "Test".to_string(),
        path: PathBuf::from("/path/to/my-test-script.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }]);

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
    let scripts = wrap_scripts(vec![
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
    ]);

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
    let scripts = wrap_scripts(vec![Script {
        name: "openfile".to_string(),
        path: PathBuf::from("/scripts/test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }]);

    // The tiered match contract no longer admits sparse fuzzy matches like
    // "opf" (fuzzy requires >= 4 chars and a compact/abbreviation shape);
    // contiguous substrings are the stable name tier, so use one to exercise
    // the lazy index computation.
    let results = fuzzy_search_scripts(&scripts, "enfi");
    assert_eq!(results.len(), 1);
    // Match indices are now computed lazily - verify using compute_match_indices_for_result
    let indices =
        compute_match_indices_for_result(&SearchResult::Script(results[0].clone()), "enfi");
    // "enfi" matches indices 2..=5 in "openfile"
    assert_eq!(
        indices.name_indices,
        vec![2, 3, 4, 5],
        "Should return correct match indices for name"
    );
}

#[test]
fn test_fuzzy_search_scripts_match_indices_for_filename() {
    let scripts = wrap_scripts(vec![Script {
        name: "Other Name".to_string(), // Name doesn't match
        path: PathBuf::from("/scripts/my-test.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: None,
        alias: None,
        shortcut: None,
        ..Default::default()
    }]);

    // Filename matching is restricted to exact contiguous substrings by the
    // tiered match contract (sparse fuzzy like "mts" no longer matches), so
    // use a substring of the filename that does not occur in the name.
    let results = fuzzy_search_scripts(&scripts, "test");
    assert_eq!(results.len(), 1);
    // Match indices are now computed lazily - verify using compute_match_indices_for_result
    let indices =
        compute_match_indices_for_result(&SearchResult::Script(results[0].clone()), "test");
    // "test" matches indices 3..=6 in "my-test.ts"
    assert_eq!(
        indices.filename_indices,
        vec![3, 4, 5, 6],
        "Should return correct match indices for filename when name doesn't match"
    );
}

#[test]
fn test_fuzzy_search_scriptlets_by_file_path() {
    // Users should be able to search by ".md" to find scriptlets
    let scriptlets = wrap_scriptlets(vec![
        Scriptlet {
            icon: None,
            name: "Open GitHub".to_string(),
            description: Some("Opens GitHub in browser".to_string()),
            code: "open('https://github.com')".to_string(),
            tool: "ts".to_string(),
            shortcut: None,
            keyword: None,
            group: Some("URLs".to_string()),
            file_path: Some("/path/to/urls.md#open-github".to_string()),
            command: Some("open-github".to_string()),
            alias: None,
            plugin_id: String::new(),
            plugin_title: None,
        },
        Scriptlet {
            icon: None,
            name: "Copy Text".to_string(),
            description: Some("Copies text".to_string()),
            code: "copy()".to_string(),
            tool: "ts".to_string(),
            shortcut: None,
            keyword: None,
            group: None,
            file_path: Some("/path/to/clipboard.md#copy-text".to_string()),
            command: Some("copy-text".to_string()),
            alias: None,
            plugin_id: String::new(),
            plugin_title: None,
        },
    ]);

    let results = fuzzy_search_scriptlets(&scriptlets, ".md");
    assert_eq!(results.len(), 2, "Should find scriptlets by .md extension");
}

#[test]
fn test_fuzzy_search_scriptlets_by_anchor() {
    // Users should be able to search by anchor slug
    let scriptlets = wrap_scriptlets(vec![
        Scriptlet {
            icon: None,
            name: "Open GitHub".to_string(),
            description: None,
            code: "code".to_string(),
            tool: "ts".to_string(),
            shortcut: None,
            keyword: None,
            group: None,
            file_path: Some("/path/to/file.md#open-github".to_string()),
            command: Some("open-github".to_string()),
            alias: None,
            plugin_id: String::new(),
            plugin_title: None,
        },
        Scriptlet {
            icon: None,
            name: "Close Tab".to_string(),
            description: None,
            code: "code".to_string(),
            tool: "ts".to_string(),
            shortcut: None,
            keyword: None,
            group: None,
            file_path: Some("/path/to/file.md#close-tab".to_string()),
            command: Some("close-tab".to_string()),
            alias: None,
            plugin_id: String::new(),
            plugin_title: None,
        },
    ]);

    let results = fuzzy_search_scriptlets(&scriptlets, "github");
    assert_eq!(results.len(), 1, "Should find scriptlet by anchor slug");
    assert_eq!(results[0].scriptlet.name, "Open GitHub");
}

#[test]
fn test_fuzzy_search_scriptlets_display_file_path() {
    // display_file_path should be the filename#anchor format
    let scriptlets = wrap_scriptlets(vec![Scriptlet {
        icon: None,
        name: "Test".to_string(),
        description: None,
        code: "code".to_string(),
        tool: "ts".to_string(),
        shortcut: None,
        keyword: None,
        group: None,
        file_path: Some("/home/user/.scriptkit/scriptlets/urls.md#test-slug".to_string()),
        command: Some("test-slug".to_string()),
        alias: None,
        plugin_id: String::new(),
        plugin_title: None,
    }]);

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
    let scriptlets = wrap_scriptlets(vec![Scriptlet {
        icon: None,
        name: "Other".to_string(), // Name doesn't match
        description: None,
        code: "code".to_string(),
        tool: "ts".to_string(),
        shortcut: None,
        keyword: None,
        group: None,
        file_path: Some("/path/urls.md#test".to_string()),
        command: None,
        alias: None,
        plugin_id: String::new(),
        plugin_title: None,
    }]);

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
    let scripts = wrap_scripts(vec![Script {
        name: "Test".to_string(),
        path: PathBuf::from("/path/my-script.ts"),
        extension: "ts".to_string(),
        ..Default::default()
    }]);

    let results = fuzzy_search_scripts(&scripts, "");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].filename, "my-script.ts");
}
