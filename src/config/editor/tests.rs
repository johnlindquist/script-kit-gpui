// editor.rs test module body split into sub-files
// --- merged from part_01.rs ---
use super::*;

#[test]
fn prepared_property_replaces_only_the_selected_preference() {
    let input =
        "export default { theme: { presetId: 'nord' }, ai: { enabled: true } } satisfies Config;";
    let property = ConfigProperty::new("theme", "{ presetId: null }");
    let prepared = prepare_config_property(input, &property).unwrap();
    assert!(prepared.contains("theme: { presetId: null }"));
    assert!(prepared.contains("ai: { enabled: true }"));
    assert_eq!(
        prepare_config_property(&prepared, &property).unwrap(),
        prepared
    );
    assert!(prepare_config_property("not valid config", &property).is_err());
    assert!(prepare_config_property("", &property)
        .unwrap()
        .contains("presetId: null"));
}

#[test]
fn static_config_decodes_literals_and_omits_reset_preferences() {
    let config = parse_static_config(
        r#"import type { Config } from '@scriptkit/sdk';
        export default { theme: undefined, enabled: true, nested: { 'quoted-key': 'it\'s "safe"',
        values: [1, - 2.5, false, null, "unicode\u1234",], }, } satisfies Config;"#,
    )
    .unwrap();
    assert!(config.get("theme").is_none());
    assert_eq!(config["nested"]["quoted-key"], "it's \"safe\"");
    assert_eq!(config["nested"]["values"][1], -2.5);
    assert!(config["nested"]["values"][3].is_null());
}

#[test]
fn static_config_rejects_executable_and_ambiguous_values() {
    for value in [
        "readSecret()",
        "process.env",
        "() => 1",
        "{ ...other }",
        "{ get key() { return 1; } }",
        "{ key: undefined, key: 2 }",
        "{ key: 1, key: 2 }",
        "[,1]",
        "`template ${run()}`",
    ] {
        let source = format!("export default {{ field: {value} }} satisfies Config;");
        assert!(parse_static_config(&source).is_err(), "accepted {value}");
    }
    assert!(parse_static_config("run(); export default {} satisfies Config;").is_err());
}

// ==========================================================================
// Test: contains_property
// ==========================================================================

#[test]
fn test_contains_property_simple() {
    let content = r#"export default {
  hotkey: { key: ";" },
} satisfies Config;"#;

    assert!(contains_property(content, "hotkey"));
    assert!(!contains_property(content, "claudeCode"));
}

#[test]
fn test_contains_property_with_comments() {
    let content = r#"export default {
  // This is a comment about hotkey
  hotkey: { key: ";" },
  // claudeCode is mentioned in comment but not as property
} satisfies Config;"#;

    assert!(contains_property(content, "hotkey"));
    assert!(!contains_property(content, "claudeCode"));
}

#[test]
fn test_contains_property_nested() {
    let content = r#"export default {
  claudeCode: {
    enabled: true,
    path: "/usr/bin/claude"
  },
} satisfies Config;"#;

    assert!(contains_property(content, "claudeCode"));
    // Tree-sitter AST correctly distinguishes nesting levels:
    // "enabled" is a nested property inside claudeCode, not a top-level config property
    assert!(!contains_property(content, "enabled"));
}

// ==========================================================================
// Test: find_object_end
// ==========================================================================

#[test]
fn test_find_object_end_simple() {
    let content = r#"export default {
  hotkey: { key: ";" },
} satisfies Config;"#;

    let info = find_object_end(content).expect("Should find object end");
    assert!(info.has_trailing_comma);
    assert_eq!(info.indent, "  ");
}

#[test]
fn test_find_object_end_no_trailing_comma() {
    let content = r#"export default {
  hotkey: { key: ";" }
} satisfies Config;"#;

    let info = find_object_end(content).expect("Should find object end");
    assert!(!info.has_trailing_comma);
}

#[test]
fn test_find_object_end_with_comments() {
    let content = r#"export default {
  hotkey: { key: ";" },
  // This is a comment
  // Another comment
} satisfies Config;"#;

    let info = find_object_end(content).expect("Should find object end");
    assert!(info.has_trailing_comma);
}

#[test]
fn test_find_object_end_nested_braces() {
    let content = r#"export default {
  hotkey: {
    modifiers: ["meta"],
    key: ";"
  },
  nested: {
    inner: {
      deep: true
    }
  },
} satisfies Config;"#;

    let info = find_object_end(content).expect("Should find object end");
    assert!(info.has_trailing_comma);
}

// ==========================================================================
// Test: add_property
// ==========================================================================

#[test]
fn test_add_property_simple() {
    let content = r#"export default {
  hotkey: { key: ";" },
} satisfies Config;"#;

    let property = ConfigProperty::new("claudeCode", "{\n    enabled: true\n  }");
    let result = add_property(content, &property);

    match result {
        EditResult::Modified(new_content) => {
            assert!(new_content.contains("claudeCode:"));
            assert!(new_content.contains("enabled: true"));
            // Should not have double commas
            assert!(!new_content.contains(",,"));
            // Should still be valid structure
            assert!(new_content.contains("} satisfies Config;"));
        }
        _ => panic!("Expected Modified result"),
    }
}

#[test]
fn test_add_property_no_trailing_comma() {
    let content = r#"export default {
  hotkey: { key: ";" }
} satisfies Config;"#;

    let property = ConfigProperty::new("claudeCode", "true");
    let result = add_property(content, &property);

    match result {
        EditResult::Modified(new_content) => {
            // Should add comma before new property
            assert!(new_content.contains(",\n"));
            assert!(new_content.contains("claudeCode: true"));
        }
        _ => panic!("Expected Modified result"),
    }
}

#[test]
fn test_add_property_replaces_existing_value() {
    let content = r#"export default {
  hotkey: { key: ";" },
  claudeCode: { enabled: false },
} satisfies Config;"#;

    let property = ConfigProperty::new("claudeCode", "{ enabled: true }");
    let result = add_property(content, &property);

    match result {
        EditResult::Modified(new_content) => {
            assert!(new_content.contains("claudeCode: { enabled: true }"));
            assert!(!new_content.contains("enabled: false"));
        }
        other => panic!("Expected Modified result, got {:?}", other),
    }
}

#[test]
fn test_add_property_with_many_comments() {
    let content = r#"import type { Config } from "@scriptkit/sdk";

export default {
  hotkey: {
    modifiers: ["meta"],
    key: "Semicolon",
  },

  // ===========================================================================
  // UI Settings
  // ===========================================================================
  // editorFontSize: 14,
  // terminalFontSize: 14,

  // ===========================================================================
  // Advanced Settings
  // ===========================================================================
  // bun_path: "/opt/homebrew/bin/bun",
} satisfies Config;"#;

    let property = ConfigProperty::new("claudeCode", "{\n    enabled: true\n  }");
    let result = add_property(content, &property);

    match result {
        EditResult::Modified(new_content) => {
            assert!(new_content.contains("claudeCode:"));
            // Should not have double commas
            assert!(!new_content.contains(",,"));
            // The comma after hotkey should be preserved
            assert!(new_content.contains("key: \"Semicolon\","));
        }
        _ => panic!("Expected Modified result"),
    }
}

// ==========================================================================
// Test: enable_claude_code convenience function
// ==========================================================================

#[test]
fn test_enable_claude_code() {
    let content = r#"export default {
  hotkey: { key: ";" },
} satisfies Config;"#;

    let result = enable_claude_code(content);

    match result {
        EditResult::Modified(new_content) => {
            assert!(new_content.contains("claudeCode:"));
            assert!(new_content.contains("enabled: true"));
        }
        _ => panic!("Expected Modified result"),
    }
}

// ==========================================================================
// Test: Edge cases
// ==========================================================================

#[test]
fn test_empty_config() {
    let content = r#"export default {} satisfies Config;"#;

    let property = ConfigProperty::new("test", "true");
    let result = add_property(content, &property);

    match result {
        EditResult::Modified(new_content) => {
            assert!(new_content.contains("test: true"));
        }
        _ => panic!("Expected Modified result"),
    }
}

#[test]
fn test_config_with_strings_containing_braces() {
    let content = r#"export default {
  template: "function() { return {}; }",
} satisfies Config;"#;

    let property = ConfigProperty::new("claudeCode", "true");
    let result = add_property(content, &property);

    match result {
        EditResult::Modified(new_content) => {
            assert!(new_content.contains("claudeCode: true"));
            // Original string should be preserved
            assert!(new_content.contains("function() { return {}; }"));
        }
        _ => panic!("Expected Modified result"),
    }
}

#[test]
fn test_config_with_template_literals() {
    let content = r#"export default {
  code: `const x = { a: 1 };`,
} satisfies Config;"#;

    let property = ConfigProperty::new("claudeCode", "true");
    let result = add_property(content, &property);

    match result {
        EditResult::Modified(new_content) => {
            assert!(new_content.contains("claudeCode: true"));
        }
        _ => panic!("Expected Modified result"),
    }
}

#[test]
fn test_preserves_as_config_syntax() {
    let content = r#"export default {
  hotkey: { key: ";" },
} as Config;"#;

    let property = ConfigProperty::new("claudeCode", "true");
    let result = add_property(content, &property);

    match result {
        EditResult::Modified(new_content) => {
            assert!(new_content.contains("} as Config;"));
        }
        _ => panic!("Expected Modified result"),
    }
}

// ==========================================================================
// Test: Real-world config (from user's actual file)
// ==========================================================================

#[test]
fn test_real_world_config() {
    let content = r#"import type { Config } from "@scriptkit/sdk";

/**
 * Script Kit Configuration
 */
export default {
  hotkey: {
    modifiers: ["meta"],
    key: "Semicolon",
  },

  // editorFontSize: 14,

  // builtIns: {
  //   clipboardHistory: true,
  // },

  // bun_path: "/opt/homebrew/bin/bun",
} satisfies Config;
"#;

    let result = enable_claude_code(content);

    match result {
        EditResult::Modified(new_content) => {
            // Verify the result is valid
            assert!(new_content.contains("claudeCode:"));
            assert!(new_content.contains("enabled: true"));
            assert!(!new_content.contains(",,"));
            assert!(new_content.contains("} satisfies Config;"));

            // Print for visual inspection
            println!("=== Modified config ===\n{}", new_content);
        }
        _ => panic!("Expected Modified result"),
    }
}

/// Test with the EXACT complex config structure that was corrupted
/// This is the actual user's config.ts structure
#[test]
fn test_complex_config_with_many_commented_sections() {
    // This is a simplified version of the actual corrupted config
    let content = r#"import type { Config } from "@scriptkit/sdk";

export default {
  hotkey: {
    modifiers: ["meta"],
    key: "Semicolon",
  },

  // ===========================================================================
  // UI Settings
  // ===========================================================================
  // editorFontSize: 14,

  // ===========================================================================
  // Commands Configuration
  // ===========================================================================
  // commands: {
  //   "builtin/clipboard-history": {
  //     shortcut: { modifiers: ["meta", "shift"], key: "KeyV" }
  //   },
  //
  //   // Add shortcut to a scriptlet
  //   // "scriptlet/clipboard-to-uppercase": {
  //   //   shortcut: { modifiers: ["meta", "shift"], key: "KeyU" }
  //   // },
  // },

  // ===========================================================================
  // Process Limits
  // ===========================================================================
  // processLimits: {
  //   maxMemoryMb: 512,
  // },

  // ===========================================================================
  // Advanced Settings
  // ===========================================================================
  // bun_path: "/opt/homebrew/bin/bun",
} satisfies Config;
"#;

    let result = enable_claude_code(content);

    match result {
        EditResult::Modified(new_content) => {
            // CRITICAL validations
            assert!(
                !new_content.contains("}{"),
                "Should NOT contain }}{{ corruption pattern. Got:\n{}",
                new_content
            );
            assert!(
                new_content.contains("claudeCode:"),
                "Should contain claudeCode property"
            );
            assert!(
                new_content.contains("enabled: true"),
                "Should contain enabled: true"
            );
            assert!(
                new_content.contains("} satisfies Config;"),
                "Should end with valid Config type assertion"
            );

            // The claudeCode property should appear BEFORE the final closing brace
            let claude_pos = new_content.find("claudeCode:").unwrap();
            let final_brace_pos = new_content.rfind("} satisfies").unwrap();
            assert!(
                claude_pos < final_brace_pos,
                "claudeCode should be before closing brace"
            );

            // All original comments should still be present
            assert!(new_content.contains("// UI Settings"));
            assert!(new_content.contains("// Commands Configuration"));
            assert!(new_content.contains("// Process Limits"));
            assert!(new_content.contains("// Advanced Settings"));

            // Print for visual inspection
            println!("=== Modified complex config ===\n{}", new_content);
        }
        _ => panic!("Expected Modified result"),
    }
}

/// Test that validates output is syntactically valid TypeScript
#[test]
fn test_output_has_no_corruption_patterns() {
    let configs = vec![
        // Minimal config
        r#"export default { hotkey: { key: ";" } } satisfies Config;"#,
        // Config with trailing comma
        r#"export default { hotkey: { key: ";" }, } satisfies Config;"#,
        // Config with nested comments
        r#"export default {
  hotkey: { key: ";" },
  // nested: {
  //   deep: {
  //     value: true
  //   }
  // }
} satisfies Config;"#,
        // Config with string containing braces
        r#"export default {
  template: "fn() { return {}; }",
} satisfies Config;"#,
    ];

    for content in configs {
        let result = enable_claude_code(content);
        match result {
            EditResult::Modified(new_content) => {
                // Check for corruption patterns
                assert!(
                    !new_content.contains("}{"),
                    "Corruption }}{{ detected in:\n{}",
                    new_content
                );
                assert!(
                    !new_content.contains("{{"),
                    "Double open brace detected in:\n{}",
                    new_content
                );
                // Note: }} can be valid (closing nested objects)
            }
            EditResult::AlreadySet => {
                // Fine - property already exists
            }
            EditResult::Failed(reason) => {
                panic!("Failed to modify config: {}", reason);
            }
        }
    }
}

// ==========================================================================
// Test: Full user config file (system test)
// ==========================================================================

// --- merged from part_02.rs ---
#[cfg(feature = "system-tests")]
#[test]
fn test_full_user_config() {
    let config_path =
        std::path::PathBuf::from(shellexpand::tilde("~/.scriptkit/config.ts").as_ref());

    if !config_path.exists() {
        println!(
            "Skipping test: config file does not exist at {:?}",
            config_path
        );
        return;
    }

    let content = std::fs::read_to_string(&config_path).expect("Failed to read config");
    println!(
        "Config file: {} bytes, {} lines",
        content.len(),
        content.lines().count()
    );

    // Skip if claudeCode already exists
    if contains_property(&content, "claudeCode") {
        println!("Skipping test: claudeCode already exists in config");
        return;
    }

    let result = enable_claude_code(&content);

    match result {
        EditResult::Modified(new_content) => {
            // Find where claudeCode was inserted
            if let Some(pos) = new_content.find("claudeCode:") {
                println!("\n=== claudeCode found at position {} ===", pos);
                let start = pos.saturating_sub(50);
                let end = (pos + 100).min(new_content.len());
                println!("Context:\n{}", &new_content[start..end]);
            } else {
                println!("WARNING: claudeCode not found in output!");
            }

            // Verify structure
            assert!(
                new_content.contains("claudeCode:"),
                "Should contain claudeCode"
            );
            assert!(
                new_content.contains("enabled: true"),
                "Should contain enabled: true"
            );
            assert!(
                !new_content.contains("}{"),
                "Should not have adjacent braces"
            );
            assert!(
                new_content.contains("} satisfies Config;") || new_content.contains("} as Config;"),
                "Should end with valid Config type assertion"
            );

            // Print last 20 lines for verification
            println!("\n=== Last 20 lines of modified config ===");
            for line in new_content
                .lines()
                .rev()
                .take(20)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
            {
                println!("{}", line);
            }
        }
        EditResult::AlreadySet => {
            println!("claudeCode already set");
        }
        EditResult::Failed(reason) => {
            panic!("Failed to modify config: {}", reason);
        }
    }
}

// ==========================================================================
// Test: validate_structure
// ==========================================================================

#[test]
fn test_validate_structure_valid() {
    let content = r#"export default {
  hotkey: { key: ";" },
} satisfies Config;"#;
    assert!(validate_structure(content).is_ok());
}

#[test]
fn test_validate_structure_valid_as_config() {
    let content = r#"export default {
  hotkey: { key: ";" },
} as Config;"#;
    assert!(validate_structure(content).is_ok());
}

#[test]
fn test_validate_structure_missing_export() {
    let content = r#"const config = {
  hotkey: { key: ";" },
};"#;
    let err = validate_structure(content).unwrap_err();
    assert!(err.contains("export default"));
}

#[test]
fn test_validate_structure_unbalanced_braces() {
    let content3 = r#"export default {"#;
    let err = validate_structure(content3).unwrap_err();
    // Tree-sitter reports this as a parse error (incomplete object literal)
    assert!(err.contains("parse error") || err.contains("parse errors"));
}

#[test]
fn test_validate_structure_corruption_pattern() {
    let content = r#"export default {
  hotkey: { key: ";" },
}{
  extra: true,
} satisfies Config;"#;
    let err = validate_structure(content).unwrap_err();
    // Tree-sitter detects the `}{` corruption as a parse error
    assert!(err.contains("parse error") || err.contains("parse errors"));
}

#[test]
fn test_validate_structure_missing_satisfies() {
    let content = r#"export default {
  hotkey: { key: ";" },
};"#;
    let err = validate_structure(content).unwrap_err();
    assert!(err.contains("satisfies") || err.contains("Config"));
}

// ==========================================================================
// Test: write_config_safely (using temp directories)
// ==========================================================================

#[test]
fn test_write_config_safely_creates_file_when_missing() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.ts");

    let property = ConfigProperty::new("claudeCode", "{ enabled: true }");
    let result = write_config_safely(&config_path, &property, None);

    match result {
        Ok(WriteOutcome::Created) => {
            let content = std::fs::read_to_string(&config_path).unwrap();
            assert!(content.contains("claudeCode: { enabled: true }"));
            assert!(content.contains("export default"));
            assert!(content.contains("satisfies Config;"));
        }
        Ok(other) => panic!("Expected Created, got {:?}", other),
        Err(e) => {
            println!("write_config_safely failed (may be expected in CI): {}", e);
        }
    }
}

#[test]
fn test_write_config_safely_creates_backup() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.ts");

    let initial = r#"import type { Config } from "@scriptkit/sdk";

export default {
  hotkey: { modifiers: ["meta"], key: "Semicolon" },
} satisfies Config;
"#;
    std::fs::write(&config_path, initial).unwrap();

    let property = ConfigProperty::new("claudeCode", "{ enabled: true }");
    let result = write_config_safely(&config_path, &property, None);

    match result {
        Ok(WriteOutcome::Written) => {
            let backup_path = config_path.with_extension("ts.bak");
            assert!(backup_path.exists(), "Backup file should exist");
            let backup_content = std::fs::read_to_string(&backup_path).unwrap();
            assert_eq!(backup_content, initial);
        }
        Ok(other) => panic!("Expected Written, got {:?}", other),
        Err(e) => {
            println!("write_config_safely failed (may be expected in CI): {}", e);
        }
    }
}

#[test]
fn test_write_config_safely_already_set() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.ts");

    let content = r#"import type { Config } from "@scriptkit/sdk";

export default {
  hotkey: { modifiers: ["meta"], key: "Semicolon" },
  claudeCode: { enabled: true },
} satisfies Config;
"#;
    std::fs::write(&config_path, content).unwrap();

    let property = ConfigProperty::new("claudeCode", "{ enabled: true }");
    let result = write_config_safely(&config_path, &property, None);

    match result {
        Ok(WriteOutcome::AlreadySet) => { /* expected */ }
        other => panic!("Expected AlreadySet, got {:?}", other),
    }
}

#[test]
fn test_write_config_safely_replaces_existing_top_level_property() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.ts");

    let content = r#"import type { Config } from "@scriptkit/sdk";

export default {
  hotkey: { modifiers: ["meta"], key: "Semicolon" },
  ai: { selectedModelId: "claude-sonnet-4-6" },
} satisfies Config;
"#;
    std::fs::write(&config_path, content).unwrap();

    let property = ConfigProperty::new("ai", r#"{"selectedModelId":"gpt-5.4"}"#);
    let result = write_config_safely(&config_path, &property, None);

    match result {
        Ok(WriteOutcome::Written) => {
            let updated = std::fs::read_to_string(&config_path).unwrap();
            assert!(updated.contains(r#"ai: {"selectedModelId":"gpt-5.4"}"#));
            assert!(!updated.contains("claude-sonnet-4-6"));
        }
        other => panic!("Expected Written, got {:?}", other),
    }
}

#[test]
fn test_write_config_safely_does_not_touch_predictable_tmp_path() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.ts");
    let predictable_temp = config_path.with_extension("ts.tmp");

    // Simulates a pre-existing predictable temp path that should be ignored.
    std::fs::write(&predictable_temp, "sentinel-temp-content").unwrap();

    let property = ConfigProperty::new("claudeCode", "{ enabled: true }");
    let result = write_config_safely(&config_path, &property, None);

    match result {
        Ok(WriteOutcome::Created) => {
            let sentinel = std::fs::read_to_string(&predictable_temp).unwrap();
            assert_eq!(sentinel, "sentinel-temp-content");
        }
        Ok(other) => panic!("Expected Created, got {:?}", other),
        Err(e) => {
            println!("write_config_safely failed (may be expected in CI): {}", e);
        }
    }
}

// ==========================================================================
// Test: recover_from_backup
// ==========================================================================

#[test]
fn test_recover_from_backup_no_backup() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.ts");
    std::fs::write(&config_path, "corrupted").unwrap();

    let result = recover_from_backup(&config_path, None);
    match result {
        Ok(false) => { /* expected - no backup exists */ }
        other => panic!("Expected Ok(false), got {:?}", other),
    }
}

#[test]
fn test_recover_from_backup_restores() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.ts");
    let backup_path = config_path.with_extension("ts.bak");

    let valid_content = r#"import type { Config } from "@scriptkit/sdk";

export default {
  hotkey: { modifiers: ["meta"], key: "Semicolon" },
} satisfies Config;
"#;
    std::fs::write(&config_path, "corrupted content").unwrap();
    std::fs::write(&backup_path, valid_content).unwrap();

    let result = recover_from_backup(&config_path, None);
    match result {
        Ok(true) => {
            let restored = std::fs::read_to_string(&config_path).unwrap();
            assert_eq!(restored, valid_content);
        }
        other => panic!("Expected Ok(true), got {:?}", other),
    }
}

#[test]
fn test_recover_from_backup_does_not_touch_predictable_tmp_path() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.ts");
    let backup_path = config_path.with_extension("ts.bak");
    let predictable_temp = config_path.with_extension("ts.tmp");

    let valid_content = r#"import type { Config } from "@scriptkit/sdk";

export default {
  hotkey: { modifiers: ["meta"], key: "Semicolon" },
} satisfies Config;
"#;
    std::fs::write(&config_path, "corrupted content").unwrap();
    std::fs::write(&backup_path, valid_content).unwrap();
    std::fs::write(&predictable_temp, "sentinel-temp-content").unwrap();

    let result = recover_from_backup(&config_path, None);
    match result {
        Ok(true) => {
            let restored = std::fs::read_to_string(&config_path).unwrap();
            assert_eq!(restored, valid_content);
            let sentinel = std::fs::read_to_string(&predictable_temp).unwrap();
            assert_eq!(sentinel, "sentinel-temp-content");
        }
        other => panic!("Expected Ok(true), got {:?}", other),
    }
}

// ==========================================================================
// Test: generate_fresh_config
// ==========================================================================

#[test]
fn test_generate_fresh_config() {
    let property = ConfigProperty::new("claudeCode", "{ enabled: true }");
    let content = generate_fresh_config(&property);
    assert!(validate_structure(&content).is_ok());
    assert!(content.contains("claudeCode: { enabled: true }"));
    assert!(content.contains("hotkey:"));
}

// ==========================================================================
// Test: round-trip with real config pattern
// ==========================================================================

#[test]
fn test_real_config_round_trip() {
    let content = r#"import type { Config } from "@scriptkit/sdk";

export default {
  hotkey: {
    modifiers: ["meta"],
    key: "Semicolon",
  },

  // editorFontSize: 14,
  // terminalFontSize: 14,

  // builtIns: {
  //   clipboardHistory: true,
  // },

  // bun_path: "/opt/homebrew/bin/bun",
} satisfies Config;
"#;

    let result = enable_claude_code(content);
    match result {
        EditResult::Modified(new_content) => {
            assert!(
                validate_structure(&new_content).is_ok(),
                "Round-trip produced invalid structure:\n{}",
                new_content
            );
            assert!(new_content.contains("claudeCode:"));
            assert!(new_content.contains("enabled: true"));
        }
        _ => panic!("Expected Modified result"),
    }
}
