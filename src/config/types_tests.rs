#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkey_config_to_display_string_maps_punctuation_key_codes() {
        let semicolon = HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        };
        assert_eq!(semicolon.to_display_string(), "⌘;");
        let letter = HotkeyConfig {
            modifiers: vec!["meta".to_string(), "shift".to_string()],
            key: "KeyK".to_string(),
        };
        assert_eq!(letter.to_display_string(), "⇧⌘K");
        let word = HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Space".to_string(),
        };
        assert_eq!(word.to_display_string(), "⌘Space");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_basic() {
        let config = HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "KeyK".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "cmd+k");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_multiple_modifiers() {
        let config = HotkeyConfig {
            modifiers: vec!["meta".to_string(), "shift".to_string()],
            key: "KeyV".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "cmd+shift+v");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_all_modifiers() {
        let config = HotkeyConfig {
            modifiers: vec![
                "alt".to_string(),
                "meta".to_string(),
                "ctrl".to_string(),
                "shift".to_string(),
            ],
            key: "KeyA".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "alt+cmd+ctrl+shift+a");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_digit_key() {
        let config = HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Digit0".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "cmd+0");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_special_key() {
        let config = HotkeyConfig {
            modifiers: vec!["meta".to_string(), "shift".to_string()],
            key: "Space".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "cmd+shift+space");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_semicolon() {
        let config = HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "cmd+semicolon");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_ctrl_modifier() {
        let config = HotkeyConfig {
            modifiers: vec!["meta".to_string(), "ctrl".to_string()],
            key: "KeyI".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "cmd+ctrl+i");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_option_alias() {
        // "option" should be treated as "alt"
        let config = HotkeyConfig {
            modifiers: vec!["option".to_string()],
            key: "KeyN".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "alt+n");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_cmd_alias() {
        // "cmd" should work as well as "meta"
        let config = HotkeyConfig {
            modifiers: vec!["cmd".to_string()],
            key: "KeyJ".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "cmd+j");
    }

    // Command ID validation and deeplink tests have moved to config_tests/mod.rs
    // and now use the public crate::config::command_ids module.

    #[test]
    fn test_get_ui_scale_returns_default_when_unset() {
        let config = Config::default();
        assert_eq!(config.get_ui_scale(), DEFAULT_UI_SCALE);
    }

    #[test]
    fn test_get_ui_scale_returns_configured_value_when_positive() {
        let config = Config {
            ui_scale: Some(1.5),
            ..Config::default()
        };
        assert_eq!(config.get_ui_scale(), 1.5);
    }

    #[test]
    fn test_get_ui_scale_returns_default_for_invalid_values() {
        for invalid in [0.0, -0.5, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let config = Config {
                ui_scale: Some(invalid),
                ..Config::default()
            };
            assert_eq!(config.get_ui_scale(), DEFAULT_UI_SCALE);
        }
    }

    #[test]
    fn test_get_layout_returns_default_when_unset() {
        let config = Config::default();
        assert_eq!(
            config.get_layout().standard_height,
            DEFAULT_LAYOUT_STANDARD_HEIGHT
        );
        assert_eq!(config.get_layout().max_height, DEFAULT_LAYOUT_MAX_HEIGHT);
    }

    #[test]
    fn test_get_layout_returns_configured_layout() {
        let config = Config {
            layout: Some(LayoutConfig {
                standard_height: 420.0,
                max_height: 840.0,
            }),
            ..Config::default()
        };

        let layout = config.get_layout();
        assert_eq!(layout.standard_height, 420.0);
        assert_eq!(layout.max_height, 840.0);
    }

    #[test]
    fn test_is_command_hidden_returns_false_when_missing() {
        let config = Config::default();
        assert!(!config.is_command_hidden("script/missing"));
    }

    #[test]
    fn test_is_command_hidden_returns_configured_hidden_value() {
        let mut commands = HashMap::new();
        commands.insert(
            "script/hidden".to_string(),
            CommandConfig {
                shortcut: None,
                hidden: Some(true),
                confirmation_required: None,
            },
        );
        commands.insert(
            "script/visible".to_string(),
            CommandConfig {
                shortcut: None,
                hidden: Some(false),
                confirmation_required: None,
            },
        );

        let config = Config {
            commands: Some(commands),
            ..Config::default()
        };
        assert!(config.is_command_hidden("script/hidden"));
        assert!(!config.is_command_hidden("script/visible"));
    }

    #[test]
    fn test_get_notes_hotkey_returns_default_when_unset() {
        let config = Config::default();
        assert_eq!(
            config.get_notes_hotkey(),
            Some(HotkeyConfig::default_notes_hotkey())
        );
    }

    #[test]
    fn test_get_notes_hotkey_returns_configured_value_when_enabled() {
        let hotkey = HotkeyConfig {
            modifiers: vec!["meta".to_string(), "shift".to_string()],
            key: "KeyN".to_string(),
        };
        let config = Config {
            notes_hotkey: Some(hotkey.clone()),
            notes_hotkey_enabled: Some(true),
            ..Config::default()
        };
        assert_eq!(config.get_notes_hotkey(), Some(hotkey));
    }

    #[test]
    fn test_get_notes_hotkey_returns_none_when_disabled() {
        let config = Config {
            notes_hotkey: Some(HotkeyConfig {
                modifiers: vec!["meta".to_string()],
                key: "KeyN".to_string(),
            }),
            notes_hotkey_enabled: Some(false),
            ..Config::default()
        };
        assert_eq!(config.get_notes_hotkey(), None);
    }

    #[test]
    fn test_get_dictation_hotkey_returns_default_when_unset() {
        let config = Config::default();
        assert_eq!(
            config.get_dictation_hotkey(),
            Some(HotkeyConfig::default_dictation_hotkey())
        );
    }

    #[test]
    fn test_get_dictation_hotkey_returns_configured_value_when_enabled() {
        let hotkey = HotkeyConfig {
            modifiers: vec!["meta".to_string(), "shift".to_string()],
            key: "KeyD".to_string(),
        };
        let config = Config {
            dictation_hotkey: Some(hotkey.clone()),
            dictation_hotkey_enabled: Some(true),
            ..Config::default()
        };
        assert_eq!(config.get_dictation_hotkey(), Some(hotkey));
    }

    #[test]
    fn test_get_dictation_hotkey_returns_none_when_disabled() {
        let config = Config {
            dictation_hotkey: Some(HotkeyConfig {
                modifiers: vec!["meta".to_string()],
                key: "KeyD".to_string(),
            }),
            dictation_hotkey_enabled: Some(false),
            ..Config::default()
        };
        assert_eq!(config.get_dictation_hotkey(), None);
    }

    #[test]
    fn test_get_inline_ai_hotkey_returns_default_when_unset() {
        let config = Config::default();
        assert_eq!(
            config.get_inline_ai_hotkey(),
            Some(HotkeyConfig::default_inline_ai_hotkey())
        );
    }

    #[test]
    fn test_get_inline_ai_hotkey_returns_configured_value_when_enabled() {
        let hotkey = HotkeyConfig {
            modifiers: vec!["meta".to_string(), "alt".to_string()],
            key: "KeyI".to_string(),
        };
        let config = Config {
            inline_ai_hotkey: Some(hotkey.clone()),
            inline_ai_hotkey_enabled: Some(true),
            ..Config::default()
        };
        assert_eq!(config.get_inline_ai_hotkey(), Some(hotkey));
    }

    #[test]
    fn test_get_inline_ai_hotkey_returns_none_when_disabled() {
        let config = Config {
            inline_ai_hotkey: Some(HotkeyConfig {
                modifiers: vec!["meta".to_string(), "alt".to_string()],
                key: "KeyI".to_string(),
            }),
            inline_ai_hotkey_enabled: Some(false),
            ..Config::default()
        };
        assert_eq!(config.get_inline_ai_hotkey(), None);
    }

    #[test]
    fn test_get_rewrite_hotkey_returns_default_when_unset() {
        let config = Config::default();
        assert_eq!(
            config.get_rewrite_hotkey(),
            Some(HotkeyConfig::default_rewrite_hotkey())
        );
    }

    #[test]
    fn test_get_rewrite_hotkey_returns_configured_value_when_enabled() {
        let hotkey = HotkeyConfig {
            modifiers: vec!["meta".to_string(), "alt".to_string()],
            key: "KeyR".to_string(),
        };
        let config = Config {
            rewrite_hotkey: Some(hotkey.clone()),
            rewrite_hotkey_enabled: Some(true),
            ..Config::default()
        };
        assert_eq!(config.get_rewrite_hotkey(), Some(hotkey));
    }

    #[test]
    fn test_get_rewrite_hotkey_returns_none_when_disabled() {
        let config = Config {
            rewrite_hotkey: Some(HotkeyConfig {
                modifiers: vec!["meta".to_string(), "alt".to_string()],
                key: "KeyR".to_string(),
            }),
            rewrite_hotkey_enabled: Some(false),
            ..Config::default()
        };
        assert_eq!(config.get_rewrite_hotkey(), None);
    }

    #[test]
    fn mcp_config_defaults_to_enabled_with_no_servers() {
        let config = McpConfig::default();
        assert!(config.enabled);
        assert!(config.servers.is_empty());
    }

    #[test]
    fn mcp_server_config_round_trips_stdio_variant() {
        let json = r#"{
            "transport": "stdio",
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-memory"]
        }"#;

        let config: McpServerConfig = serde_json::from_str(json).expect("stdio MCP config");
        match config {
            McpServerConfig::Stdio(config) => {
                assert_eq!(config.command, "npx");
                assert_eq!(
                    config.args,
                    vec!["-y", "@modelcontextprotocol/server-memory"]
                );
                assert!(config.enabled);
            }
            McpServerConfig::Http(_) => panic!("expected stdio config"),
        }
    }

    #[test]
    fn config_get_mcp_returns_default_when_missing() {
        let config = Config::default();
        let mcp = config.get_mcp();
        assert!(mcp.enabled);
        assert!(mcp.servers.is_empty());
    }
}
