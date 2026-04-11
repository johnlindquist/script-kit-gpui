// config_tests split into sub-files for maintainability
// --- merged from part_01.rs ---
use super::*;
use std::collections::HashMap;

#[test]
fn test_default_config() {
    let config = Config::default();
    assert_eq!(config.hotkey.modifiers, vec!["meta"]);
    assert_eq!(config.hotkey.key, "Semicolon");
    assert_eq!(config.bun_path, None);
    assert_eq!(config.editor, None);
}

#[test]
fn test_clipboard_history_max_text_length_default() {
    let config = Config::default();
    assert_eq!(
        config.get_clipboard_history_max_text_length(),
        DEFAULT_CLIPBOARD_HISTORY_MAX_TEXT_LENGTH
    );
}

#[test]
fn test_config_serialization() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["ctrl".to_string(), "alt".to_string()],
            key: "KeyA".to_string(),
        },
        bun_path: Some("/usr/local/bin/bun".to_string()),
        editor: Some("vim".to_string()),
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: Config = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.hotkey.modifiers, config.hotkey.modifiers);
    assert_eq!(deserialized.hotkey.key, config.hotkey.key);
    assert_eq!(deserialized.bun_path, config.bun_path);
    assert_eq!(deserialized.editor, config.editor);
}

#[test]
fn test_hotkey_config_default_values() {
    let hotkey = HotkeyConfig {
        modifiers: vec!["meta".to_string(), "shift".to_string()],
        key: "KeyK".to_string(),
    };
    assert_eq!(hotkey.modifiers.len(), 2);
    assert!(hotkey.modifiers.contains(&"meta".to_string()));
    assert!(hotkey.modifiers.contains(&"shift".to_string()));
}

#[test]
fn test_config_with_bun_path() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: Some("/custom/path/bun".to_string()),
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };
    assert_eq!(config.bun_path, Some("/custom/path/bun".to_string()));
}

#[test]
fn test_config_without_bun_path() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };
    assert_eq!(config.bun_path, None);
}

#[test]
fn test_config_serialization_skip_none_bun_path() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    let json = serde_json::to_string(&config).unwrap();
    // Verify that bun_path is not included when None
    assert!(!json.contains("null"));
    // Should contain hotkey config
    assert!(json.contains("meta"));
}

#[test]
fn test_config_serialization_preserves_multiple_modifiers() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["ctrl".to_string(), "shift".to_string(), "alt".to_string()],
            key: "KeyP".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: Config = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.hotkey.modifiers.len(), 3);
    assert_eq!(deserialized.hotkey.key, "KeyP");
}

#[test]
fn test_config_deserialization_with_custom_values() {
    let json = r#"{
        "hotkey": {
            "modifiers": ["shift", "alt"],
            "key": "KeyX"
        },
        "bun_path": "/usr/bin/bun"
    }"#;

    let config: Config = serde_json::from_str(json).unwrap();
    assert_eq!(config.hotkey.modifiers, vec!["shift", "alt"]);
    assert_eq!(config.hotkey.key, "KeyX");
    assert_eq!(config.bun_path, Some("/usr/bin/bun".to_string()));
}

#[test]
fn test_config_deserialization_minimal() {
    let json = r#"{
        "hotkey": {
            "modifiers": ["meta"],
            "key": "Semicolon"
        }
    }"#;

    let config: Config = serde_json::from_str(json).unwrap();
    assert_eq!(config.hotkey.modifiers, vec!["meta"]);
    assert_eq!(config.hotkey.key, "Semicolon");
    assert_eq!(config.bun_path, None);
}

#[test]
fn test_load_config_returns_config_struct() {
    // Test that load_config returns a valid Config struct
    // It may load from actual config or return defaults
    let config = load_config();
    // Verify it has required fields
    assert!(!config.hotkey.modifiers.is_empty());
    assert!(!config.hotkey.key.is_empty());
    // Either has bun_path set or it's None - both valid
    let _ = config.bun_path;
}

#[test]
fn test_config_clone_independence() {
    let config1 = Config::default();
    let config2 = config1.clone();

    // Verify they are equal but independent
    assert_eq!(config1.hotkey.modifiers, config2.hotkey.modifiers);
    assert_eq!(config1.hotkey.key, config2.hotkey.key);
    assert_eq!(config1.bun_path, config2.bun_path);
    assert_eq!(config1.editor, config2.editor);
}

#[test]
fn test_hotkey_config_clone() {
    let hotkey = HotkeyConfig {
        modifiers: vec!["meta".to_string(), "alt".to_string()],
        key: "KeyK".to_string(),
    };
    let cloned = hotkey.clone();

    assert_eq!(hotkey.modifiers, cloned.modifiers);
    assert_eq!(hotkey.key, cloned.key);
}

#[test]
fn test_config_with_empty_modifiers_list() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec![],
            key: "KeyA".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    assert_eq!(config.hotkey.modifiers.len(), 0);
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.hotkey.modifiers.len(), 0);
}

#[test]
fn test_config_key_preservation() {
    let keys = vec!["Semicolon", "KeyK", "KeyP", "Space", "Enter"];
    for key in keys {
        let config = Config {
            hotkey: HotkeyConfig {
                modifiers: vec!["meta".to_string()],
                key: key.to_string(),
            },
            bun_path: None,
            editor: None,
            padding: None,
            editor_font_size: None,
            terminal_font_size: None,
            ui_scale: None,
            built_ins: None,
            process_limits: None,
            clipboard_history_max_text_length: None,
            suggested: None,
            notes_hotkey: None,
            ai_hotkey: None,
            logs_hotkey: None,
            ai_hotkey_enabled: None,
            logs_hotkey_enabled: None,
            dictation_hotkey: None,
            dictation_hotkey_enabled: None,
            watcher: None,
            layout: None,
            theme: None,
            dictation: None,
            ai: None,
            window_management: None,
            commands: None,
            claude_code: None,
            mcp: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.hotkey.key, key);
    }
}

// Editor config tests
#[test]
fn test_config_with_editor() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: Some("vim".to_string()),
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("vim"));

    let deserialized: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.editor, Some("vim".to_string()));
}

#[test]
fn test_config_without_editor() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    let json = serde_json::to_string(&config).unwrap();
    // Editor should not appear in JSON when None (skip_serializing_if)
    assert!(!json.contains("editor"));

    let deserialized: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.editor, None);
}

// --- merged from part_02.rs ---
#[test]
fn test_get_editor_from_config() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: Some("nvim".to_string()),
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    // Config editor takes precedence
    assert_eq!(config.get_editor(), "nvim");
}

#[test]
fn test_get_editor_from_env() {
    // Save current EDITOR value
    let original_editor = std::env::var("EDITOR").ok();

    // Set EDITOR env var
    std::env::set_var("EDITOR", "emacs");

    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    // Should fall back to EDITOR env var
    assert_eq!(config.get_editor(), "emacs");

    // Restore original EDITOR value
    match original_editor {
        Some(val) => std::env::set_var("EDITOR", val),
        None => std::env::remove_var("EDITOR"),
    }
}

#[test]
fn test_get_editor_default() {
    // Save current EDITOR value
    let original_editor = std::env::var("EDITOR").ok();

    // Remove EDITOR env var
    std::env::remove_var("EDITOR");

    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    // Should fall back to "code" default
    assert_eq!(config.get_editor(), "code");

    // Restore original EDITOR value
    if let Some(val) = original_editor {
        std::env::set_var("EDITOR", val);
    }
}

#[test]
fn test_config_editor_priority() {
    // Save current EDITOR value
    let original_editor = std::env::var("EDITOR").ok();

    // Set EDITOR env var
    std::env::set_var("EDITOR", "emacs");

    // Config with editor set should take precedence over env var
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: Some("vim".to_string()),
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    // Config editor should win
    assert_eq!(config.get_editor(), "vim");

    // Restore original EDITOR value
    match original_editor {
        Some(val) => std::env::set_var("EDITOR", val),
        None => std::env::remove_var("EDITOR"),
    }
}

#[test]
fn test_config_deserialization_with_editor() {
    let json = r#"{
        "hotkey": {
            "modifiers": ["meta"],
            "key": "Semicolon"
        },
        "editor": "subl"
    }"#;

    let config: Config = serde_json::from_str(json).unwrap();
    assert_eq!(config.editor, Some("subl".to_string()));
    assert_eq!(config.get_editor(), "subl");
}

// ContentPadding tests
#[test]
fn test_content_padding_default() {
    let padding = ContentPadding::default();
    assert_eq!(padding.top, DEFAULT_PADDING_TOP);
    assert_eq!(padding.left, DEFAULT_PADDING_LEFT);
    assert_eq!(padding.right, DEFAULT_PADDING_RIGHT);
}

#[test]
fn test_content_padding_serialization() {
    let padding = ContentPadding {
        top: 10.0,
        left: 16.0,
        right: 16.0,
    };

    let json = serde_json::to_string(&padding).unwrap();
    let deserialized: ContentPadding = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.top, 10.0);
    assert_eq!(deserialized.left, 16.0);
    assert_eq!(deserialized.right, 16.0);
}

#[test]
fn test_content_padding_partial_deserialization() {
    // If only some fields are present, defaults should fill in
    let json = r#"{"top": 20.0}"#;
    let padding: ContentPadding = serde_json::from_str(json).unwrap();

    assert_eq!(padding.top, 20.0);
    assert_eq!(padding.left, DEFAULT_PADDING_LEFT);
    assert_eq!(padding.right, DEFAULT_PADDING_RIGHT);
}

// UI settings tests
#[test]
fn test_config_default_has_none_ui_settings() {
    let config = Config::default();
    assert!(config.padding.is_none());
    assert!(config.editor_font_size.is_none());
    assert!(config.terminal_font_size.is_none());
    assert!(config.ui_scale.is_none());
}

#[test]
fn test_config_get_padding_default() {
    let config = Config::default();
    let padding = config.get_padding();

    assert_eq!(padding.top, DEFAULT_PADDING_TOP);
    assert_eq!(padding.left, DEFAULT_PADDING_LEFT);
    assert_eq!(padding.right, DEFAULT_PADDING_RIGHT);
}

#[test]
fn test_config_get_padding_custom() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: Some(ContentPadding {
            top: 10.0,
            left: 20.0,
            right: 20.0,
        }),
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    let padding = config.get_padding();
    assert_eq!(padding.top, 10.0);
    assert_eq!(padding.left, 20.0);
    assert_eq!(padding.right, 20.0);
}

#[test]
fn test_config_get_editor_font_size_default() {
    let config = Config::default();
    assert_eq!(config.get_editor_font_size(), DEFAULT_EDITOR_FONT_SIZE);
}

#[test]
fn test_config_get_editor_font_size_custom() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: Some(16.0),
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    assert_eq!(config.get_editor_font_size(), 16.0);
}

#[test]
fn test_config_get_editor_font_size_uses_default_for_invalid_values() {
    for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let config = Config {
            editor_font_size: Some(invalid),
            ..Config::default()
        };
        assert_eq!(config.get_editor_font_size(), DEFAULT_EDITOR_FONT_SIZE);
    }
}

#[test]
fn test_config_get_terminal_font_size_default() {
    let config = Config::default();
    assert_eq!(config.get_terminal_font_size(), DEFAULT_TERMINAL_FONT_SIZE);
}

#[test]
fn test_config_get_terminal_font_size_custom() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: Some(12.0),
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    assert_eq!(config.get_terminal_font_size(), 12.0);
}

#[test]
fn test_config_get_terminal_font_size_uses_default_for_invalid_values() {
    for invalid in [0.0, -4.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let config = Config {
            terminal_font_size: Some(invalid),
            ..Config::default()
        };
        assert_eq!(config.get_terminal_font_size(), DEFAULT_TERMINAL_FONT_SIZE);
    }
}

#[test]
fn test_config_deserialization_with_ui_settings() {
    let json = r#"{
        "hotkey": {
            "modifiers": ["meta"],
            "key": "Semicolon"
        },
        "padding": {
            "top": 10,
            "left": 16,
            "right": 16
        },
        "editorFontSize": 16,
        "terminalFontSize": 14,
        "uiScale": 1.2
    }"#;

    let config: Config = serde_json::from_str(json).unwrap();

    assert!(config.padding.is_some());
    let padding = config.get_padding();
    assert_eq!(padding.top, 10.0);
    assert_eq!(padding.left, 16.0);
    assert_eq!(padding.right, 16.0);

    assert_eq!(config.get_editor_font_size(), 16.0);
    assert_eq!(config.get_terminal_font_size(), 14.0);
    assert_eq!(config.ui_scale, Some(1.2));
}

// --- merged from part_03.rs ---
#[test]
fn test_config_deserialization_without_ui_settings() {
    // Existing configs without UI settings should still work
    let json = r#"{
        "hotkey": {
            "modifiers": ["meta"],
            "key": "Semicolon"
        }
    }"#;

    let config: Config = serde_json::from_str(json).unwrap();

    // All UI settings should be None
    assert!(config.padding.is_none());
    assert!(config.editor_font_size.is_none());
    assert!(config.terminal_font_size.is_none());
    assert!(config.ui_scale.is_none());

    // Getters should return defaults
    assert_eq!(config.get_padding().top, DEFAULT_PADDING_TOP);
    assert_eq!(config.get_editor_font_size(), DEFAULT_EDITOR_FONT_SIZE);
    assert_eq!(config.get_terminal_font_size(), DEFAULT_TERMINAL_FONT_SIZE);
}

#[test]
fn test_config_serialization_skips_none_ui_settings() {
    let config = Config::default();
    let json = serde_json::to_string(&config).unwrap();

    // None values should not appear in JSON
    assert!(!json.contains("padding"));
    assert!(!json.contains("editorFontSize"));
    assert!(!json.contains("terminalFontSize"));
    assert!(!json.contains("uiScale"));
}

#[test]
fn test_config_serialization_includes_set_ui_settings() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: Some(ContentPadding::default()),
        editor_font_size: Some(16.0),
        terminal_font_size: Some(12.0),
        ui_scale: Some(1.5),
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    let json = serde_json::to_string(&config).unwrap();

    assert!(json.contains("padding"));
    assert!(json.contains("editorFontSize"));
    assert!(json.contains("terminalFontSize"));
    assert!(json.contains("uiScale"));
}

#[test]
fn test_config_constants() {
    // Verify constants match expected defaults from task
    assert_eq!(DEFAULT_PADDING_TOP, 8.0);
    assert_eq!(DEFAULT_PADDING_LEFT, 12.0);
    assert_eq!(DEFAULT_PADDING_RIGHT, 12.0);
    assert_eq!(DEFAULT_EDITOR_FONT_SIZE, 16.0);
    assert_eq!(DEFAULT_TERMINAL_FONT_SIZE, 14.0);
    assert_eq!(DEFAULT_UI_SCALE, 1.0);
}

// BuiltInConfig tests
#[test]
fn test_builtin_config_default() {
    let config = BuiltInConfig::default();
    assert!(config.clipboard_history);
    assert!(config.app_launcher);
    assert!(config.window_switcher);
}

#[test]
fn test_builtin_config_serialization_camel_case() {
    let config = BuiltInConfig {
        clipboard_history: true,
        app_launcher: false,
        window_switcher: true,
    };

    let json = serde_json::to_string(&config).unwrap();

    // Should use camelCase in JSON
    assert!(json.contains("clipboardHistory"));
    assert!(json.contains("appLauncher"));
    assert!(json.contains("windowSwitcher"));
    // Should NOT use snake_case
    assert!(!json.contains("clipboard_history"));
    assert!(!json.contains("app_launcher"));
    assert!(!json.contains("window_switcher"));
}

#[test]
fn test_builtin_config_deserialization_camel_case() {
    let json = r#"{
        "clipboardHistory": false,
        "appLauncher": true,
        "windowSwitcher": false
    }"#;

    let config: BuiltInConfig = serde_json::from_str(json).unwrap();

    assert!(!config.clipboard_history);
    assert!(config.app_launcher);
    assert!(!config.window_switcher);
}

#[test]
fn test_builtin_config_deserialization_with_defaults() {
    // Partial config - missing fields should use defaults
    let json = r#"{"clipboardHistory": false}"#;
    let config: BuiltInConfig = serde_json::from_str(json).unwrap();

    assert!(!config.clipboard_history);
    assert!(config.app_launcher); // Default true
    assert!(config.window_switcher); // Default true
}

#[test]
fn test_config_with_builtins() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: Some(BuiltInConfig {
            clipboard_history: true,
            app_launcher: false,
            window_switcher: true,
        }),
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    let builtins = config.get_builtins();
    assert!(builtins.clipboard_history);
    assert!(!builtins.app_launcher);
    assert!(builtins.window_switcher);
}

#[test]
fn test_config_get_builtins_default() {
    let config = Config::default();
    let builtins = config.get_builtins();

    // Should return defaults when built_ins is None
    assert!(builtins.clipboard_history);
    assert!(builtins.app_launcher);
    assert!(builtins.window_switcher);
}

#[test]
fn test_config_deserialization_with_builtins() {
    let json = r#"{
        "hotkey": {
            "modifiers": ["meta"],
            "key": "Semicolon"
        },
        "builtIns": {
            "clipboardHistory": true,
            "appLauncher": false,
            "windowSwitcher": true
        }
    }"#;

    let config: Config = serde_json::from_str(json).unwrap();

    assert!(config.built_ins.is_some());
    let builtins = config.get_builtins();
    assert!(builtins.clipboard_history);
    assert!(!builtins.app_launcher);
    assert!(builtins.window_switcher);
}

#[test]
fn test_config_deserialization_without_builtins() {
    // Existing configs without builtIns should still work
    let json = r#"{
        "hotkey": {
            "modifiers": ["meta"],
            "key": "Semicolon"
        }
    }"#;

    let config: Config = serde_json::from_str(json).unwrap();

    assert!(config.built_ins.is_none());

    // Getter should return defaults
    let builtins = config.get_builtins();
    assert!(builtins.clipboard_history);
    assert!(builtins.app_launcher);
    assert!(builtins.window_switcher);
}

#[test]
fn test_config_serialization_skips_none_builtins() {
    let config = Config::default();
    let json = serde_json::to_string(&config).unwrap();

    // None values should not appear in JSON
    assert!(!json.contains("builtIns"));
}

#[test]
fn test_config_serialization_includes_set_builtins() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: Some(BuiltInConfig::default()),
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    let json = serde_json::to_string(&config).unwrap();

    assert!(json.contains("builtIns"));
    assert!(json.contains("clipboardHistory"));
    assert!(json.contains("appLauncher"));
    assert!(json.contains("windowSwitcher"));
}

#[test]
fn test_builtin_config_roundtrip() {
    // Test full roundtrip serialization/deserialization
    let original = BuiltInConfig {
        clipboard_history: false,
        app_launcher: true,
        window_switcher: true,
    };

    let json = serde_json::to_string(&original).unwrap();
    let restored: BuiltInConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(original.clipboard_history, restored.clipboard_history);
    assert_eq!(original.app_launcher, restored.app_launcher);
    assert_eq!(original.window_switcher, restored.window_switcher);
}

// ProcessLimits tests
#[test]
fn test_process_limits_default() {
    let limits = ProcessLimits::default();
    assert_eq!(limits.max_memory_mb, None);
    assert_eq!(limits.max_runtime_seconds, None);
    assert_eq!(
        limits.health_check_interval_ms,
        DEFAULT_HEALTH_CHECK_INTERVAL_MS
    );
}

#[test]
fn test_process_limits_default_constant() {
    assert_eq!(DEFAULT_HEALTH_CHECK_INTERVAL_MS, 5000);
}

#[test]
fn test_process_limits_serialization_camel_case() {
    let limits = ProcessLimits {
        max_memory_mb: Some(512),
        max_runtime_seconds: Some(300),
        health_check_interval_ms: 3000,
    };

    let json = serde_json::to_string(&limits).unwrap();

    // Should use camelCase in JSON
    assert!(json.contains("maxMemoryMb"));
    assert!(json.contains("maxRuntimeSeconds"));
    assert!(json.contains("healthCheckIntervalMs"));
    // Should NOT use snake_case
    assert!(!json.contains("max_memory_mb"));
    assert!(!json.contains("max_runtime_seconds"));
    assert!(!json.contains("health_check_interval_ms"));
}

#[test]
fn test_process_limits_deserialization_camel_case() {
    let json = r#"{
        "maxMemoryMb": 1024,
        "maxRuntimeSeconds": 600,
        "healthCheckIntervalMs": 2000
    }"#;

    let limits: ProcessLimits = serde_json::from_str(json).unwrap();

    assert_eq!(limits.max_memory_mb, Some(1024));
    assert_eq!(limits.max_runtime_seconds, Some(600));
    assert_eq!(limits.health_check_interval_ms, 2000);
}

#[test]
fn test_process_limits_deserialization_with_defaults() {
    // Partial config - missing fields should use defaults
    let json = r#"{"maxMemoryMb": 256}"#;
    let limits: ProcessLimits = serde_json::from_str(json).unwrap();

    assert_eq!(limits.max_memory_mb, Some(256));
    assert_eq!(limits.max_runtime_seconds, None); // Default
    assert_eq!(
        limits.health_check_interval_ms,
        DEFAULT_HEALTH_CHECK_INTERVAL_MS
    ); // Default
}

#[test]
fn test_process_limits_deserialization_empty() {
    // Empty object should use all defaults
    let json = r#"{}"#;
    let limits: ProcessLimits = serde_json::from_str(json).unwrap();

    assert_eq!(limits.max_memory_mb, None);
    assert_eq!(limits.max_runtime_seconds, None);
    assert_eq!(
        limits.health_check_interval_ms,
        DEFAULT_HEALTH_CHECK_INTERVAL_MS
    );
}

#[test]
fn test_process_limits_serialization_skips_none() {
    let limits = ProcessLimits::default();
    let json = serde_json::to_string(&limits).unwrap();

    // None values should not appear in JSON
    assert!(!json.contains("maxMemoryMb"));
    assert!(!json.contains("maxRuntimeSeconds"));
    // But healthCheckIntervalMs always appears (has value)
    assert!(json.contains("healthCheckIntervalMs"));
}

// --- merged from part_04.rs ---
#[test]
fn test_config_with_process_limits() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: Some(ProcessLimits {
            max_memory_mb: Some(512),
            max_runtime_seconds: Some(300),
            health_check_interval_ms: 3000,
        }),
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    let limits = config.get_process_limits();
    assert_eq!(limits.max_memory_mb, Some(512));
    assert_eq!(limits.max_runtime_seconds, Some(300));
    assert_eq!(limits.health_check_interval_ms, 3000);
}

#[test]
fn test_config_get_process_limits_default() {
    let config = Config::default();
    let limits = config.get_process_limits();

    // Should return defaults when process_limits is None
    assert_eq!(limits.max_memory_mb, None);
    assert_eq!(limits.max_runtime_seconds, None);
    assert_eq!(
        limits.health_check_interval_ms,
        DEFAULT_HEALTH_CHECK_INTERVAL_MS
    );
}

#[test]
fn test_config_get_process_limits_uses_default_interval_when_zero() {
    let config = Config {
        process_limits: Some(ProcessLimits {
            max_memory_mb: Some(1024),
            max_runtime_seconds: Some(60),
            health_check_interval_ms: 0,
        }),
        ..Config::default()
    };

    let limits = config.get_process_limits();

    assert_eq!(limits.max_memory_mb, Some(1024));
    assert_eq!(limits.max_runtime_seconds, Some(60));
    assert_eq!(
        limits.health_check_interval_ms,
        DEFAULT_HEALTH_CHECK_INTERVAL_MS
    );
}

#[test]
fn test_config_deserialization_with_process_limits() {
    let json = r#"{
        "hotkey": {
            "modifiers": ["meta"],
            "key": "Semicolon"
        },
        "processLimits": {
            "maxMemoryMb": 1024,
            "maxRuntimeSeconds": 600,
            "healthCheckIntervalMs": 2000
        }
    }"#;

    let config: Config = serde_json::from_str(json).unwrap();

    assert!(config.process_limits.is_some());
    let limits = config.get_process_limits();
    assert_eq!(limits.max_memory_mb, Some(1024));
    assert_eq!(limits.max_runtime_seconds, Some(600));
    assert_eq!(limits.health_check_interval_ms, 2000);
}

#[test]
fn test_config_deserialization_without_process_limits() {
    // Existing configs without processLimits should still work
    let json = r#"{
        "hotkey": {
            "modifiers": ["meta"],
            "key": "Semicolon"
        }
    }"#;

    let config: Config = serde_json::from_str(json).unwrap();

    assert!(config.process_limits.is_none());

    // Getter should return defaults
    let limits = config.get_process_limits();
    assert_eq!(limits.max_memory_mb, None);
    assert_eq!(limits.max_runtime_seconds, None);
    assert_eq!(
        limits.health_check_interval_ms,
        DEFAULT_HEALTH_CHECK_INTERVAL_MS
    );
}

#[test]
fn test_config_serialization_skips_none_process_limits() {
    let config = Config::default();
    let json = serde_json::to_string(&config).unwrap();

    // None values should not appear in JSON
    assert!(!json.contains("processLimits"));
}

#[test]
fn test_config_serialization_includes_set_process_limits() {
    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: Some(ProcessLimits::default()),
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: None,
        claude_code: None,
        mcp: None,
    };

    let json = serde_json::to_string(&config).unwrap();

    assert!(json.contains("processLimits"));
    assert!(json.contains("healthCheckIntervalMs"));
}

#[test]
fn test_process_limits_roundtrip() {
    // Test full roundtrip serialization/deserialization
    let original = ProcessLimits {
        max_memory_mb: Some(256),
        max_runtime_seconds: Some(120),
        health_check_interval_ms: 10000,
    };

    let json = serde_json::to_string(&original).unwrap();
    let restored: ProcessLimits = serde_json::from_str(&json).unwrap();

    assert_eq!(original.max_memory_mb, restored.max_memory_mb);
    assert_eq!(original.max_runtime_seconds, restored.max_runtime_seconds);
    assert_eq!(
        original.health_check_interval_ms,
        restored.health_check_interval_ms
    );
}

#[test]
fn test_process_limits_clone() {
    let original = ProcessLimits {
        max_memory_mb: Some(512),
        max_runtime_seconds: Some(300),
        health_check_interval_ms: 5000,
    };
    let cloned = original.clone();

    assert_eq!(original.max_memory_mb, cloned.max_memory_mb);
    assert_eq!(original.max_runtime_seconds, cloned.max_runtime_seconds);
    assert_eq!(
        original.health_check_interval_ms,
        cloned.health_check_interval_ms
    );
}

// Confirmation required tests
#[test]
fn test_default_confirmation_commands_constant() {
    // Verify the constant contains expected dangerous commands
    assert!(DEFAULT_CONFIRMATION_COMMANDS.contains(&"builtin/shut-down"));
    assert!(DEFAULT_CONFIRMATION_COMMANDS.contains(&"builtin/restart"));
    assert!(DEFAULT_CONFIRMATION_COMMANDS.contains(&"builtin/log-out"));
    assert!(DEFAULT_CONFIRMATION_COMMANDS.contains(&"builtin/empty-trash"));
    assert!(DEFAULT_CONFIRMATION_COMMANDS.contains(&"builtin/sleep"));
    assert!(DEFAULT_CONFIRMATION_COMMANDS.contains(&"builtin/force-quit"));
    assert!(DEFAULT_CONFIRMATION_COMMANDS.contains(&"builtin/stop-all-processes"));
    assert!(DEFAULT_CONFIRMATION_COMMANDS.contains(&"builtin/clear-suggested"));
    assert!(DEFAULT_CONFIRMATION_COMMANDS.contains(&"builtin/test-confirmation"));
}

#[test]
fn test_requires_confirmation_default_commands() {
    // Default commands should require confirmation
    let config = Config::default();

    assert!(config.requires_confirmation("builtin/shut-down"));
    assert!(config.requires_confirmation("builtin/restart"));
    assert!(config.requires_confirmation("builtin/log-out"));
    assert!(config.requires_confirmation("builtin/empty-trash"));
    assert!(config.requires_confirmation("builtin/sleep"));
    assert!(config.requires_confirmation("builtin/force-quit"));
    assert!(config.requires_confirmation("builtin/stop-all-processes"));
    assert!(config.requires_confirmation("builtin/clear-suggested"));
    assert!(config.requires_confirmation("builtin/test-confirmation"));
}

#[test]
fn test_requires_confirmation_non_dangerous_commands() {
    // Non-dangerous commands should NOT require confirmation
    let config = Config::default();

    assert!(!config.requires_confirmation("builtin/clipboard-history"));
    assert!(!config.requires_confirmation("builtin/app-launcher"));
    assert!(!config.requires_confirmation("script/hello-world"));
    assert!(!config.requires_confirmation("app/com.apple.Safari"));
}

#[test]
fn test_requires_confirmation_user_override_disable() {
    // User can disable confirmation for a default dangerous command
    let mut commands = HashMap::new();
    commands.insert(
        "builtin/shut-down".to_string(),
        CommandConfig {
            shortcut: None,
            hidden: None,
            confirmation_required: Some(false), // User explicitly disables
        },
    );

    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: Some(commands),
        claude_code: None,
        mcp: None,
    };

    // Should NOT require confirmation because user disabled it
    assert!(!config.requires_confirmation("builtin/shut-down"));
    // Other default commands still require it
    assert!(config.requires_confirmation("builtin/restart"));
}

#[test]
fn test_requires_confirmation_user_override_enable() {
    // User can enable confirmation for a non-default command
    let mut commands = HashMap::new();
    commands.insert(
        "script/dangerous-script".to_string(),
        CommandConfig {
            shortcut: None,
            hidden: None,
            confirmation_required: Some(true), // User explicitly enables
        },
    );

    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: Some(commands),
        claude_code: None,
        mcp: None,
    };

    // Should require confirmation because user enabled it
    assert!(config.requires_confirmation("script/dangerous-script"));
    // Non-configured commands still use defaults
    assert!(!config.requires_confirmation("script/safe-script"));
}

#[test]
fn test_command_config_confirmation_required_serialization() {
    let cmd_config = CommandConfig {
        shortcut: None,
        hidden: None,
        confirmation_required: Some(true),
    };

    let json = serde_json::to_string(&cmd_config).unwrap();

    // Should use camelCase in JSON
    assert!(json.contains("confirmationRequired"));
    assert!(!json.contains("confirmation_required"));

    let deserialized: CommandConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.confirmation_required, Some(true));
}

#[test]
fn test_command_config_confirmation_required_deserialization() {
    let json = r#"{"confirmationRequired": false}"#;
    let cmd_config: CommandConfig = serde_json::from_str(json).unwrap();

    assert_eq!(cmd_config.confirmation_required, Some(false));
    assert!(cmd_config.shortcut.is_none());
    assert!(cmd_config.hidden.is_none());
}

#[test]
fn test_command_config_confirmation_required_skips_none() {
    let cmd_config = CommandConfig {
        shortcut: None,
        hidden: None,
        confirmation_required: None,
    };

    let json = serde_json::to_string(&cmd_config).unwrap();

    // None values should not appear in JSON
    assert!(!json.contains("confirmationRequired"));
}

#[test]
fn test_config_deserialization_with_confirmation_required() {
    let json = r#"{
        "hotkey": {
            "modifiers": ["meta"],
            "key": "Semicolon"
        },
        "commands": {
            "builtin/shut-down": {
                "confirmationRequired": false
            },
            "script/my-script": {
                "confirmationRequired": true
            }
        }
    }"#;

    let config: Config = serde_json::from_str(json).unwrap();

    // User disabled confirmation for shut-down
    assert!(!config.requires_confirmation("builtin/shut-down"));
    // User enabled confirmation for custom script
    assert!(config.requires_confirmation("script/my-script"));
    // Other default commands still require it
    assert!(config.requires_confirmation("builtin/restart"));
}

#[test]
fn test_requires_confirmation_with_partial_command_config() {
    // Command config exists but doesn't specify confirmation_required
    // Should fall back to defaults
    let mut commands = HashMap::new();
    commands.insert(
        "builtin/shut-down".to_string(),
        CommandConfig {
            shortcut: Some(HotkeyConfig {
                modifiers: vec!["meta".to_string()],
                key: "KeyX".to_string(),
            }),
            hidden: None,
            confirmation_required: None, // Not specified
        },
    );

    let config = Config {
        hotkey: HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        },
        bun_path: None,
        editor: None,
        padding: None,
        editor_font_size: None,
        terminal_font_size: None,
        ui_scale: None,
        built_ins: None,
        process_limits: None,
        clipboard_history_max_text_length: None,
        suggested: None,
        notes_hotkey: None,
        ai_hotkey: None,
        logs_hotkey: None,
        ai_hotkey_enabled: None,
        logs_hotkey_enabled: None,
        dictation_hotkey: None,
        dictation_hotkey_enabled: None,
        watcher: None,
        layout: None,
        theme: None,
        dictation: None,
        ai: None,
        window_management: None,
        commands: Some(commands),
        claude_code: None,
        mcp: None,
    };

    // Should still require confirmation (falls back to default)
    assert!(config.requires_confirmation("builtin/shut-down"));
}

// ============================================
// Command ID module tests
// ============================================

#[test]
fn canonical_builtin_command_id_normalizes_legacy_and_canonical_inputs() {
    use crate::config::canonical_builtin_command_id;

    // Legacy dash-style → canonical slash-style
    assert_eq!(
        canonical_builtin_command_id("builtin-clipboard-history"),
        "builtin/clipboard-history"
    );
    // Bare identifier → canonical
    assert_eq!(
        canonical_builtin_command_id("clipboard-history"),
        "builtin/clipboard-history"
    );
    // Already canonical → unchanged
    assert_eq!(
        canonical_builtin_command_id("builtin/clipboard-history"),
        "builtin/clipboard-history"
    );
}

#[test]
fn parse_command_id_accepts_all_categories() {
    use crate::config::{parse_command_id, CommandCategory};

    let (cat, id) = parse_command_id("builtin/clipboard-history").unwrap();
    assert_eq!(cat, CommandCategory::Builtin);
    assert_eq!(id, "clipboard-history");

    let (cat, id) = parse_command_id("app/com.apple.Safari").unwrap();
    assert_eq!(cat, CommandCategory::App);
    assert_eq!(id, "com.apple.Safari");

    let (cat, id) = parse_command_id("script/hello-world").unwrap();
    assert_eq!(cat, CommandCategory::Script);
    assert_eq!(id, "hello-world");

    let (cat, id) = parse_command_id("scriptlet/my-snippet").unwrap();
    assert_eq!(cat, CommandCategory::Scriptlet);
    assert_eq!(id, "my-snippet");
}

#[test]
fn parse_command_id_rejects_invalid_formats() {
    use crate::config::parse_command_id;

    // No slash
    assert!(parse_command_id("builtin-clipboard-history").is_err());
    // Unknown category
    assert!(parse_command_id("foo/bar").is_err());
    // Empty identifier
    assert!(parse_command_id("builtin/").is_err());
    // No category
    assert!(parse_command_id("/clipboard-history").is_err());
}

#[test]
fn is_valid_command_id_accepts_supported_prefixes() {
    use crate::config::is_valid_command_id;

    assert!(is_valid_command_id("builtin/clipboard-history"));
    assert!(is_valid_command_id("app/com.apple.Safari"));
    assert!(is_valid_command_id("script/hello-world"));
    assert!(is_valid_command_id("scriptlet/my-snippet"));
}

#[test]
fn is_valid_command_id_rejects_unsupported_prefixes() {
    use crate::config::is_valid_command_id;

    assert!(!is_valid_command_id("builtin-clipboard-history"));
    assert!(!is_valid_command_id("foo/bar"));
}

#[test]
fn build_command_id_constructs_canonical_ids() {
    use crate::config::{build_command_id, CommandCategory};

    assert_eq!(
        build_command_id(CommandCategory::Builtin, "clipboard-history").unwrap(),
        "builtin/clipboard-history"
    );
    assert_eq!(
        build_command_id(CommandCategory::App, "com.apple.Safari").unwrap(),
        "app/com.apple.Safari"
    );
    assert!(build_command_id(CommandCategory::Script, "").is_err());
}

#[test]
fn command_id_deeplink_roundtrip() {
    use crate::config::{command_id_from_deeplink, command_id_to_deeplink};

    let deeplink = command_id_to_deeplink("builtin/clipboard-history").unwrap();
    assert_eq!(deeplink, "scriptkit://commands/builtin/clipboard-history");

    let parsed = command_id_from_deeplink(&deeplink).unwrap();
    assert_eq!(parsed, "builtin/clipboard-history");
}

#[test]
fn command_id_to_deeplink_uses_scriptkit_scheme() {
    use crate::config::command_id_to_deeplink;

    let deeplink = command_id_to_deeplink("builtin/test").unwrap();
    assert!(
        deeplink.starts_with("scriptkit://"),
        "Deeplink should use scriptkit:// scheme, got: {}",
        deeplink
    );
    assert!(
        !deeplink.starts_with("kit://"),
        "Deeplink should NOT use kit:// scheme, got: {}",
        deeplink
    );
}

#[test]
fn command_id_from_deeplink_rejects_invalid() {
    use crate::config::command_id_from_deeplink;

    assert!(command_id_from_deeplink("kit://commands/builtin/test").is_err());
    assert!(command_id_from_deeplink("https://example.com").is_err());
}

#[test]
fn normalize_builtin_identifier_strips_prefixes() {
    use crate::config::normalize_builtin_identifier;

    assert_eq!(
        normalize_builtin_identifier("builtin-clipboard-history"),
        "clipboard-history"
    );
    assert_eq!(
        normalize_builtin_identifier("builtin/clipboard-history"),
        "clipboard-history"
    );
    assert_eq!(
        normalize_builtin_identifier("clipboard-history"),
        "clipboard-history"
    );
}

// ---------------------------------------------------------------------------
// Real config.ts evaluation tests (require bun on PATH)
// ---------------------------------------------------------------------------

/// Writes a real config.ts with an `import` and runtime evaluation,
/// sets SK_PATH to the temp dir, calls `load_config()`, and asserts
/// the expected hotkey and `bun_path` values come back.
#[test]
fn test_load_config_direct_import_reads_real_config_ts() {
    if which::which("bun").is_err() {
        eprintln!("skipping config direct-import test because bun is not installed");
        return;
    }

    let _lock = crate::test_utils::SK_PATH_TEST_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let tmp = tempfile::tempdir().expect("create temp dir");
    let kit_dir = tmp.path().join("kit");
    std::fs::create_dir_all(&kit_dir).expect("create kit dir");

    let config_ts = kit_dir.join("config.ts");
    std::fs::write(
        &config_ts,
        r#"import { join } from "node:path";
export default {
  hotkey: { modifiers: ["cmd"], key: ";" },
  bun_path: join("/opt", "homebrew", "bin", "bun"),
};
"#,
    )
    .expect("write config.ts");

    // Point load_config() at the temp directory.
    let prev = std::env::var("SK_PATH").ok();
    std::env::set_var("SK_PATH", tmp.path());

    let config = load_config();

    // Restore env immediately.
    match prev {
        Some(val) => std::env::set_var("SK_PATH", val),
        None => std::env::remove_var("SK_PATH"),
    }

    assert_eq!(config.hotkey.modifiers, vec!["cmd"]);
    assert_eq!(config.hotkey.key, ";");
    assert_eq!(
        config.bun_path.as_deref(),
        Some("/opt/homebrew/bin/bun"),
        "bun_path should reflect the runtime join() result"
    );
}

/// Rewrites config.ts between two `load_config()` calls and asserts
/// the second call sees the new value, proving the fast path does not
/// serve stale config between launches.
#[test]
fn test_load_config_reloads_updated_config_ts() {
    if which::which("bun").is_err() {
        eprintln!("skipping config reload test because bun is not installed");
        return;
    }

    let _lock = crate::test_utils::SK_PATH_TEST_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let tmp = tempfile::tempdir().expect("create temp dir");
    let kit_dir = tmp.path().join("kit");
    std::fs::create_dir_all(&kit_dir).expect("create kit dir");

    let config_ts = kit_dir.join("config.ts");
    std::fs::write(
        &config_ts,
        r#"export default {
  hotkey: { modifiers: ["ctrl"], key: "a" },
};
"#,
    )
    .expect("write initial config.ts");

    let prev = std::env::var("SK_PATH").ok();
    std::env::set_var("SK_PATH", tmp.path());

    let first = load_config();
    assert_eq!(first.hotkey.key, "a");

    // Overwrite with a new value.
    std::fs::write(
        &config_ts,
        r#"export default {
  hotkey: { modifiers: ["ctrl"], key: "z" },
  bun_path: "/new/bun",
};
"#,
    )
    .expect("rewrite config.ts");

    let second = load_config();

    // Restore env immediately.
    match prev {
        Some(val) => std::env::set_var("SK_PATH", val),
        None => std::env::remove_var("SK_PATH"),
    }

    assert_eq!(
        second.hotkey.key, "z",
        "second load must see the updated key"
    );
    assert_eq!(
        second.bun_path.as_deref(),
        Some("/new/bun"),
        "second load must see the newly added bun_path"
    );
}
