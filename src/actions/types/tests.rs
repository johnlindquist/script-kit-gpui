#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_info_creation() {
        let script = ScriptInfo::new("test-script", "/path/to/test-script.ts");
        assert_eq!(script.name, "test-script");
        assert_eq!(script.path, "/path/to/test-script.ts");
        assert!(script.is_script);
        assert!(!script.is_scriptlet);
        assert!(script.shortcut.is_none());
        assert!(script.alias.is_none());
    }

    #[test]
    fn test_script_info_with_shortcut() {
        let script = ScriptInfo::with_shortcut(
            "test-script",
            "/path/to/test-script.ts",
            Some("cmd+shift+t".to_string()),
        );
        assert_eq!(script.name, "test-script");
        assert!(script.is_script);
        assert!(!script.is_scriptlet);
        assert_eq!(script.shortcut, Some("cmd+shift+t".to_string()));
    }

    #[test]
    fn test_script_info_scriptlet() {
        let scriptlet = ScriptInfo::scriptlet(
            "Open GitHub",
            "/path/to/url.md#open-github",
            Some("cmd+g".to_string()),
            Some("gh".to_string()),
        );
        assert_eq!(scriptlet.name, "Open GitHub");
        assert_eq!(scriptlet.path, "/path/to/url.md#open-github");
        assert!(!scriptlet.is_script);
        assert!(scriptlet.is_scriptlet);
        assert_eq!(scriptlet.shortcut, Some("cmd+g".to_string()));
        assert_eq!(scriptlet.alias, Some("gh".to_string()));
        assert_eq!(scriptlet.action_verb, "Run");
    }

    #[test]
    fn test_script_info_builtin() {
        let builtin = ScriptInfo::builtin("Clipboard History");
        assert_eq!(builtin.name, "Clipboard History");
        assert_eq!(builtin.path, "");
        assert!(!builtin.is_script);
        assert!(!builtin.is_scriptlet);
        assert!(builtin.shortcut.is_none());
        assert!(builtin.alias.is_none());
    }

    #[test]
    fn test_script_info_with_is_script() {
        let script = ScriptInfo::with_is_script("my-script", "/path/to/script.ts", true);
        assert!(script.is_script);
        assert!(!script.is_scriptlet);
        assert!(script.shortcut.is_none());

        let builtin = ScriptInfo::with_is_script("App Launcher", "", false);
        assert!(!builtin.is_script);
        assert!(!builtin.is_scriptlet);
    }

    #[test]
    fn test_script_info_with_action_verb_and_shortcut() {
        let script = ScriptInfo::with_action_verb_and_shortcut(
            "test",
            "/path",
            true,
            "Launch",
            Some("cmd+k".to_string()),
        );
        assert_eq!(script.action_verb, "Launch");
        assert!(!script.is_scriptlet);
        assert_eq!(script.shortcut, Some("cmd+k".to_string()));
    }

    #[test]
    fn test_action_with_shortcut() {
        let action =
            Action::new("test", "Test Action", None, ActionCategory::GlobalOps).with_shortcut("⌘T");
        assert_eq!(action.shortcut, Some("⌘T".to_string()));
    }

    #[test]
    fn test_action_new_defaults() {
        let action = Action::new(
            "id",
            "title",
            Some("desc".to_string()),
            ActionCategory::ScriptContext,
        );
        assert_eq!(action.id, "id");
        assert_eq!(action.title, "title");
        assert_eq!(action.description, Some("desc".to_string()));
        assert_eq!(action.category, ActionCategory::ScriptContext);
        assert!(action.is_enabled());
        assert_eq!(action.disabled_reason(), None);
        assert!(action.shortcut.is_none());
    }

    #[test]
    fn disabled_action_requires_and_exposes_a_reason() {
        let action = Action::new("id", "title", None, ActionCategory::ScriptContext)
            .disabled("Requires a selected file");
        assert!(!action.is_enabled());
        assert_eq!(action.disabled_reason(), Some("Requires a selected file"));
    }

    #[test]
    #[should_panic(expected = "disabled actions require a non-empty reason")]
    fn disabled_action_rejects_blank_reason() {
        let _ = Action::new("id", "title", None, ActionCategory::ScriptContext).disabled("  ");
    }

    #[test]
    fn test_script_info_with_shortcut_and_alias() {
        let script = ScriptInfo::with_shortcut_and_alias(
            "test-script",
            "/path/to/test-script.ts",
            Some("cmd+shift+t".to_string()),
            Some("ts".to_string()),
        );
        assert_eq!(script.name, "test-script");
        assert_eq!(script.shortcut, Some("cmd+shift+t".to_string()));
        assert_eq!(script.alias, Some("ts".to_string()));
    }

    #[test]
    fn test_script_info_with_all() {
        let script = ScriptInfo::with_all(
            "App Launcher",
            "builtin:app-launcher",
            false,
            "Open",
            Some("cmd+space".to_string()),
            Some("apps".to_string()),
        );
        assert_eq!(script.name, "App Launcher");
        assert_eq!(script.path, "builtin:app-launcher");
        assert!(!script.is_script);
        assert_eq!(script.action_verb, "Open");
        assert_eq!(script.shortcut, Some("cmd+space".to_string()));
        assert_eq!(script.alias, Some("apps".to_string()));
    }

    #[test]
    fn test_script_info_with_frecency() {
        // Test with_frecency builder method
        let script = ScriptInfo::new("test-script", "/path/to/script.ts")
            .with_frecency(true, Some("/path/to/script.ts".to_string()));

        assert!(script.is_suggested);
        assert_eq!(script.frecency_path, Some("/path/to/script.ts".to_string()));
    }

    #[test]
    fn test_script_info_default_frecency_values() {
        // Test that default values are correct (not suggested, no frecency path)
        let script = ScriptInfo::new("test-script", "/path/to/script.ts");
        assert!(!script.is_suggested);
        assert!(script.frecency_path.is_none());

        let scriptlet = ScriptInfo::scriptlet("Open GitHub", "/path/to/url.md", None, None);
        assert!(!scriptlet.is_suggested);
        assert!(scriptlet.frecency_path.is_none());

        let builtin = ScriptInfo::builtin("Clipboard History");
        assert!(!builtin.is_suggested);
        assert!(builtin.frecency_path.is_none());
    }

    #[test]
    fn test_script_info_frecency_chaining() {
        // Test that with_frecency can be chained with other constructors
        let script = ScriptInfo::with_shortcut_and_alias(
            "test-script",
            "/path/to/test.ts",
            Some("cmd+t".to_string()),
            Some("ts".to_string()),
        )
        .with_frecency(true, Some("frecency:path".to_string()));

        // Original fields preserved
        assert_eq!(script.shortcut, Some("cmd+t".to_string()));
        assert_eq!(script.alias, Some("ts".to_string()));

        // Frecency fields set
        assert!(script.is_suggested);
        assert_eq!(script.frecency_path, Some("frecency:path".to_string()));
    }

    #[test]
    fn test_script_info_default_sets_safe_empty_context() {
        let info = ScriptInfo::default();
        assert_eq!(info.name, "");
        assert_eq!(info.path, "");
        assert!(!info.is_script);
        assert!(!info.is_scriptlet);
        assert!(!info.is_agent);
        assert_eq!(info.action_verb, "Run");
        assert!(info.shortcut.is_none());
        assert!(info.alias.is_none());
        assert!(!info.is_suggested);
        assert!(info.frecency_path.is_none());
    }

    #[test]
    fn test_script_info_with_all_treats_whitespace_shortcut_and_alias_as_missing() {
        let info = ScriptInfo::with_all(
            "test",
            "/path/to/test.ts",
            true,
            "Run",
            Some("   ".to_string()),
            Some("\n\t".to_string()),
        );
        assert!(info.shortcut.is_none());
        assert!(info.alias.is_none());
    }

    #[test]
    fn test_script_info_with_action_verb_defaults_to_run_when_verb_is_blank() {
        let info = ScriptInfo::with_action_verb("test", "/path/to/test.ts", true, "   ");
        assert_eq!(info.action_verb, "Run");
    }

    #[test]
    fn test_script_info_with_frecency_disables_suggested_when_path_is_missing() {
        let info =
            ScriptInfo::new("test", "/path/to/test.ts").with_frecency(true, Some(" ".into()));
        assert!(!info.is_suggested);
        assert!(info.frecency_path.is_none());
    }

    #[test]
    fn test_script_info_from_str_tuple_creates_script() {
        let info = ScriptInfo::from(("test-script", "/path/to/test.ts"));
        assert_eq!(info.name, "test-script");
        assert_eq!(info.path, "/path/to/test.ts");
        assert!(info.is_script);
    }

    #[test]
    fn test_script_info_from_string_tuple_creates_script() {
        let info = ScriptInfo::from(("test-script".to_string(), "/path/to/test.ts".to_string()));
        assert_eq!(info.name, "test-script");
        assert_eq!(info.path, "/path/to/test.ts");
        assert!(info.is_script);
    }
}
