// --- merged from part_000.rs ---
use super::*;
use crate::config::BuiltInConfig;
#[test]
fn test_builtin_config_default() {
    let config = BuiltInConfig::default();
    assert!(config.clipboard_history);
    assert!(config.app_launcher);
    assert!(config.window_switcher);
}
#[test]
fn test_builtin_config_custom() {
    let config = BuiltInConfig {
        clipboard_history: false,
        app_launcher: true,
        window_switcher: false,
    };
    assert!(!config.clipboard_history);
    assert!(config.app_launcher);
    assert!(!config.window_switcher);
}
#[test]
fn test_get_builtin_entries_all_enabled() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    // Core built-ins: Clipboard history, window switcher, AI chat, Notes, design gallery
    // Plus: system actions (28), window actions (6), notes commands (3), AI commands (1),
    // script commands (2), permission commands (3) = 43 new entries
    // Total: 5 + 43 = 48
    assert!(entries.len() >= 5); // At minimum the core built-ins should exist

    // Check clipboard history entry
    let clipboard = entries.iter().find(|e| e.id == "builtin/clipboard-history");
    assert!(clipboard.is_some());
    let clipboard = clipboard.unwrap();
    assert_eq!(clipboard.name, "Clipboard History");
    assert_eq!(clipboard.feature, BuiltInFeature::ClipboardHistory);
    assert!(clipboard.keywords.contains(&"clipboard".to_string()));
    assert!(clipboard.keywords.contains(&"history".to_string()));
    assert!(clipboard.keywords.contains(&"paste".to_string()));
    assert!(clipboard.keywords.contains(&"copy".to_string()));

    // Check paste sequentially entry
    let paste_sequentially = entries
        .iter()
        .find(|e| e.id == "builtin/paste-sequentially");
    assert!(paste_sequentially.is_some());
    let paste_sequentially = paste_sequentially.unwrap();
    assert_eq!(paste_sequentially.name, "Paste Next Clipboard Item");
    assert_eq!(
        paste_sequentially.feature,
        BuiltInFeature::PasteSequentially
    );
    assert!(paste_sequentially.keywords.contains(&"paste".to_string()));
    assert!(paste_sequentially
        .keywords
        .contains(&"sequential".to_string()));
    assert!(paste_sequentially
        .keywords
        .contains(&"clipboard".to_string()));
    assert!(paste_sequentially.keywords.contains(&"batch".to_string()));
    assert!(paste_sequentially.keywords.contains(&"paseq".to_string()));

    // Check window switcher entry
    let window_switcher = entries.iter().find(|e| e.id == "builtin/window-switcher");
    assert!(window_switcher.is_some());
    let window_switcher = window_switcher.unwrap();
    assert_eq!(window_switcher.name, "Window Switcher");
    assert_eq!(window_switcher.feature, BuiltInFeature::WindowSwitcher);
    assert!(window_switcher.keywords.contains(&"window".to_string()));
    assert!(window_switcher.keywords.contains(&"switch".to_string()));
    assert!(window_switcher.keywords.contains(&"tile".to_string()));
    assert!(window_switcher.keywords.contains(&"focus".to_string()));
    assert!(window_switcher.keywords.contains(&"manage".to_string()));
    assert!(window_switcher.keywords.contains(&"switcher".to_string()));

    // Check browser tabs entry
    let browser_tabs = entries.iter().find(|e| e.id == "builtin/browser-tabs");
    assert!(browser_tabs.is_some());
    let browser_tabs = browser_tabs.unwrap();
    assert_eq!(browser_tabs.name, "Search Browser Tabs");
    assert_eq!(browser_tabs.feature, BuiltInFeature::BrowserTabs);
    assert!(browser_tabs.keywords.contains(&"browser".to_string()));
    assert!(browser_tabs.keywords.contains(&"tabs".to_string()));
    assert!(browser_tabs.keywords.contains(&"raycast".to_string()));
    assert!(browser_tabs.keywords.contains(&"chrome".to_string()));
    assert!(browser_tabs.keywords.contains(&"safari".to_string()));

    // Check Agent Chat entry
    let ai_chat = entries.iter().find(|e| e.id == "builtin/ai-chat");
    assert!(ai_chat.is_some());
    let ai_chat = ai_chat.unwrap();
    assert_eq!(ai_chat.name, "Agent Chat");
    assert_eq!(ai_chat.feature, BuiltInFeature::AiChat);
    assert!(ai_chat.keywords.contains(&"ai".to_string()));
    assert!(ai_chat.keywords.contains(&"agent".to_string()));
    assert!(ai_chat.keywords.contains(&"harness".to_string()));
    assert!(ai_chat.keywords.contains(&"claude".to_string()));
    assert!(ai_chat.keywords.contains(&"gpt".to_string()));

    for variant in crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::EXPERIMENTS {
        let entry = entries.iter().find(|entry| entry.id == variant.menu_id());
        assert!(
            entry.is_some(),
            "missing Agent Chat UI variant entry {}",
            variant.menu_id()
        );
        let entry = entry.unwrap();
        assert_eq!(entry.name, variant.menu_name());
        assert_eq!(entry.feature, BuiltInFeature::AiChatVariant(variant));
    }

    // Check Emoji Picker entry
    let emoji_picker = entries.iter().find(|e| e.id == "builtin/emoji-picker");
    assert!(emoji_picker.is_some());
    let emoji_picker = emoji_picker.unwrap();
    assert_eq!(emoji_picker.name, "Emoji Picker");
    assert_eq!(emoji_picker.feature, BuiltInFeature::EmojiPicker);
    assert!(emoji_picker.keywords.contains(&"emoji".to_string()));
    assert!(emoji_picker.keywords.contains(&"picker".to_string()));

    // Note: App Launcher built-in removed - apps now appear directly in main search
}
#[test]
fn test_get_builtin_entries_clipboard_only() {
    let config = BuiltInConfig {
        clipboard_history: true,
        app_launcher: false,
        window_switcher: false,
    };
    let entries = get_builtin_entries(&config);

    // Check that core entries exist (plus all the new command entries)
    assert!(entries.iter().any(|e| e.id == "builtin/clipboard-history"));
    assert!(entries.iter().any(|e| e.id == "builtin/paste-sequentially"));
    assert!(entries.iter().any(|e| e.id == "builtin/ai-chat"));
    assert!(entries.iter().any(|e| e.id == "builtin/open-notes"));

    // Window switcher should NOT be present
    assert!(!entries.iter().any(|e| e.id == "builtin/window-switcher"));
}
#[test]
fn test_get_builtin_entries_app_launcher_only() {
    let config = BuiltInConfig {
        clipboard_history: false,
        app_launcher: true,
        window_switcher: false,
    };
    let entries = get_builtin_entries(&config);

    // App launcher no longer creates a built-in entry (apps appear in main search)
    // But Agent Chat and Notes are always enabled (plus new command entries)
    assert!(entries.iter().any(|e| e.id == "builtin/ai-chat"));
    assert!(entries.iter().any(|e| e.id == "builtin/open-notes"));

    // Clipboard history should NOT be present
    assert!(!entries.iter().any(|e| e.id == "builtin/clipboard-history"));
    assert!(!entries.iter().any(|e| e.id == "builtin/paste-sequentially"));
}
#[test]
fn test_get_builtin_entries_none_enabled() {
    let config = BuiltInConfig {
        clipboard_history: false,
        app_launcher: false,
        window_switcher: false,
    };
    let entries = get_builtin_entries(&config);

    // Agent Chat and Notes are always enabled (plus new command entries)
    assert!(entries.iter().any(|e| e.id == "builtin/ai-chat"));
    assert!(entries.iter().any(|e| e.id == "builtin/open-notes"));

    // Clipboard history and window switcher should NOT be present
    assert!(!entries.iter().any(|e| e.id == "builtin/clipboard-history"));
    assert!(!entries.iter().any(|e| e.id == "builtin/paste-sequentially"));
    assert!(!entries.iter().any(|e| e.id == "builtin/window-switcher"));
}
#[test]
fn test_get_builtin_entries_window_switcher_only() {
    let config = BuiltInConfig {
        clipboard_history: false,
        app_launcher: false,
        window_switcher: true,
    };
    let entries = get_builtin_entries(&config);

    // Window switcher + Agent Chat + Notes (always enabled, plus new command entries)
    assert!(entries.iter().any(|e| e.id == "builtin/window-switcher"));
    assert!(entries.iter().any(|e| e.id == "builtin/ai-chat"));
    assert!(entries.iter().any(|e| e.id == "builtin/open-notes"));

    // Verify window switcher has correct properties
    let window_switcher = entries
        .iter()
        .find(|e| e.id == "builtin/window-switcher")
        .unwrap();
    assert_eq!(window_switcher.feature, BuiltInFeature::WindowSwitcher);
    assert_eq!(window_switcher.icon, Some("app-window".to_string()));

    // Clipboard history should NOT be present
    assert!(!entries.iter().any(|e| e.id == "builtin/clipboard-history"));
    assert!(!entries.iter().any(|e| e.id == "builtin/paste-sequentially"));
}
#[test]
fn test_builtin_feature_equality() {
    assert_eq!(
        BuiltInFeature::ClipboardHistory,
        BuiltInFeature::ClipboardHistory
    );
    assert_eq!(BuiltInFeature::AppLauncher, BuiltInFeature::AppLauncher);
    assert_eq!(
        BuiltInFeature::WindowSwitcher,
        BuiltInFeature::WindowSwitcher
    );
    assert_eq!(BuiltInFeature::DesignGallery, BuiltInFeature::DesignGallery);
    assert_eq!(BuiltInFeature::AiChat, BuiltInFeature::AiChat);
    assert_eq!(BuiltInFeature::Favorites, BuiltInFeature::Favorites);
    assert_eq!(BuiltInFeature::EmojiPicker, BuiltInFeature::EmojiPicker);
    assert_eq!(
        BuiltInFeature::PasteSequentially,
        BuiltInFeature::PasteSequentially
    );
    assert_ne!(
        BuiltInFeature::ClipboardHistory,
        BuiltInFeature::AppLauncher
    );
    assert_ne!(
        BuiltInFeature::ClipboardHistory,
        BuiltInFeature::WindowSwitcher
    );
    assert_ne!(BuiltInFeature::AppLauncher, BuiltInFeature::WindowSwitcher);
    assert_ne!(
        BuiltInFeature::DesignGallery,
        BuiltInFeature::ClipboardHistory
    );
    assert_ne!(
        BuiltInFeature::PasteSequentially,
        BuiltInFeature::ClipboardHistory
    );
    assert_ne!(BuiltInFeature::AiChat, BuiltInFeature::ClipboardHistory);
    assert_ne!(BuiltInFeature::AiChat, BuiltInFeature::DesignGallery);
    assert_ne!(BuiltInFeature::Favorites, BuiltInFeature::ClipboardHistory);
    assert_ne!(
        BuiltInFeature::EmojiPicker,
        BuiltInFeature::ClipboardHistory
    );

    // Test App variant
    assert_eq!(
        BuiltInFeature::App("Safari".to_string()),
        BuiltInFeature::App("Safari".to_string())
    );
    assert_ne!(
        BuiltInFeature::App("Safari".to_string()),
        BuiltInFeature::App("Chrome".to_string())
    );
    assert_ne!(
        BuiltInFeature::App("Safari".to_string()),
        BuiltInFeature::AppLauncher
    );
}
#[test]
fn test_builtin_entry_new() {
    let entry = BuiltInEntry::new(
        "test-id",
        "Test Entry",
        "Test description",
        vec!["test", "keyword"],
        BuiltInFeature::ClipboardHistory,
    );

    assert_eq!(entry.id, "builtin/test-id");
    assert_eq!(entry.name, "Test Entry");
    assert_eq!(entry.description, "Test description");
    assert_eq!(
        entry.keywords,
        vec!["test".to_string(), "keyword".to_string()]
    );
    assert_eq!(entry.feature, BuiltInFeature::ClipboardHistory);
    assert_eq!(entry.icon, None);
}
#[test]
fn test_builtin_entry_new_with_icon() {
    let entry = BuiltInEntry::new_with_icon(
        "test-id",
        "Test Entry",
        "Test description",
        vec!["test"],
        BuiltInFeature::ClipboardHistory,
        "clipboard",
    );

    assert_eq!(entry.id, "builtin/test-id");
    assert_eq!(entry.name, "Test Entry");
    assert_eq!(entry.icon, Some("clipboard".to_string()));
}
#[test]
fn test_builtin_entry_clone() {
    let entry = BuiltInEntry::new_with_icon(
        "test-id",
        "Test Entry",
        "Test description",
        vec!["test"],
        BuiltInFeature::AppLauncher,
        "rocket",
    );

    let cloned = entry.clone();
    assert_eq!(entry.id, cloned.id);
    assert_eq!(entry.name, cloned.name);
    assert_eq!(entry.description, cloned.description);
    assert_eq!(entry.keywords, cloned.keywords);
    assert_eq!(entry.feature, cloned.feature);
    assert_eq!(entry.icon, cloned.icon);
}
#[test]
fn test_builtin_config_clone() {
    let config = BuiltInConfig {
        clipboard_history: true,
        app_launcher: false,
        window_switcher: true,
    };

    let cloned = config.clone();
    assert_eq!(config.clipboard_history, cloned.clipboard_history);
    assert_eq!(config.app_launcher, cloned.app_launcher);
    assert_eq!(config.window_switcher, cloned.window_switcher);
}
#[test]
fn test_system_action_entries_exist() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    // Check that system action entries exist
    assert!(entries.iter().any(|e| e.id == "builtin/empty-trash"));
    assert!(entries.iter().any(|e| e.id == "builtin/lock-screen"));
    assert!(entries.iter().any(|e| e.id == "builtin/toggle-dark-mode"));
    // Volume presets
    assert!(entries.iter().any(|e| e.id == "builtin/volume-0"));
    assert!(entries.iter().any(|e| e.id == "builtin/volume-50"));
    assert!(entries.iter().any(|e| e.id == "builtin/volume-100"));
    assert!(entries.iter().any(|e| e.id == "builtin/system-preferences"));
}
// NOTE: test_window_action_entries_exist removed - window actions now in extension

#[test]
fn test_notes_command_entries_exist() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    assert!(entries.iter().any(|e| e.id == "builtin/open-notes"));
    assert!(
        entries.iter().any(|e| e.id == "builtin/new-note"),
        "builtin/new-note should be a distinct root command"
    );
    assert!(
        entries.iter().any(|e| e.id == "builtin/search-notes"),
        "builtin/search-notes should be a distinct root command"
    );
    assert!(entries.iter().any(|e| e.id == "builtin/quick-capture"));

    // Verify Open Notes absorbed the keywords from the collapsed entries
    let open_notes = entries
        .iter()
        .find(|e| e.id == "builtin/open-notes")
        .unwrap();
    assert!(open_notes.keywords.contains(&"new".to_string()));
    assert!(open_notes.keywords.contains(&"create".to_string()));
    assert!(open_notes.keywords.contains(&"search".to_string()));
    assert!(open_notes.keywords.contains(&"find".to_string()));
}
#[test]
fn test_get_builtin_entries_includes_open_notes_and_generate_script() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    let open_notes = entries.iter().find(|e| e.id == "builtin/open-notes");
    assert!(open_notes.is_some(), "builtin/open-notes should exist");
    assert_eq!(
        open_notes.unwrap().feature,
        BuiltInFeature::NotesCommand(NotesCommandType::OpenNotes)
    );

    // Legacy AI window commands (OpenAi, MiniAi, NewConversation) are no longer registered.
    assert!(
        entries.iter().find(|e| e.id == "builtin/open-ai").is_none(),
        "builtin/open-ai should be removed (routes to harness now)"
    );
    assert!(
        entries.iter().find(|e| e.id == "builtin/mini-ai").is_none(),
        "builtin/mini-ai should be removed (routes to harness now)"
    );
    assert!(
        entries
            .iter()
            .find(|e| e.id == "builtin/new-conversation")
            .is_none(),
        "builtin/new-conversation should be removed (routes to harness now)"
    );

    let generate_script = entries
        .iter()
        .find(|e| e.id == "builtin/generate-script-with-ai");
    assert!(
        generate_script.is_some(),
        "builtin/generate-script-with-ai should still exist (routes to harness)"
    );
    let generate_script = generate_script.unwrap();
    assert_eq!(
        generate_script.feature,
        BuiltInFeature::AiCommand(AiCommandType::GenerateScript)
    );
}
#[test]
fn test_get_builtin_entries_prunes_debug_stub_commands() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    let stub_ids = [
        "builtin/browse-kit-store",
        "builtin/manage-installed-kits",
        "builtin/update-all-kits",
        "builtin/create-ai-preset",
        "builtin/import-ai-presets",
        "builtin/export-ai-presets",
        "builtin/search-ai-presets",
    ];

    for id in stub_ids {
        assert!(
            !entries.iter().any(|e| e.id == id),
            "stub command {id} should not be registered in the launcher"
        );
    }
}

#[test]
fn test_get_builtin_entries_includes_favorites_command() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    let favorites = entries.iter().find(|e| e.id == "builtin/favorites");
    assert!(favorites.is_some(), "builtin/favorites should exist");

    let favorites = favorites.unwrap();
    assert_eq!(favorites.name, "Favorites");
    assert_eq!(favorites.feature, BuiltInFeature::Favorites);
    assert!(
        favorites
            .keywords
            .iter()
            .any(|keyword| keyword.eq_ignore_ascii_case("star")),
        "Favorites command should be discoverable with 'star'"
    );
}
#[test]
fn test_script_command_entries_exist() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    // Check that script command entries exist
    assert!(entries.iter().any(|e| e.id == "builtin/new-script"));
    assert!(entries.iter().any(|e| e.id == "builtin/new-extension"));
}

#[test]
fn builtins_include_new_script_fast_path_and_template_path() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    let fast_path = entries
        .iter()
        .find(|e| e.id == "builtin/new-script")
        .expect("builtin/new-script fast path should exist");
    assert_eq!(
        fast_path.feature,
        BuiltInFeature::ScriptCommand(ScriptCommandType::NewScript),
        "builtin/new-script must keep routing to ScriptCommandType::NewScript"
    );

    let template_path = entries
        .iter()
        .find(|e| e.id == "builtin/new-script-from-template")
        .expect("builtin/new-script-from-template catalog path should exist");
    assert_eq!(
        template_path.feature,
        BuiltInFeature::NewScriptFromTemplate,
        "builtin/new-script-from-template must route to NewScriptFromTemplate",
    );
}
#[test]
fn test_new_creation_commands_are_discoverable() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    let new_script = entries
        .iter()
        .find(|e| e.id == "builtin/new-script")
        .expect("builtin/new-script should exist");
    assert!(
        new_script.name.to_lowercase().contains("new"),
        "New Script entry name should prominently include 'new'"
    );

    let new_extension = entries
        .iter()
        .find(|e| e.id == "builtin/new-extension")
        .expect("builtin/new-extension should exist");
    assert!(
        new_extension
            .keywords
            .iter()
            .any(|k| k.eq_ignore_ascii_case("scriptlet")),
        "New Scriptlet entry should be discoverable via 'scriptlet'"
    );
    assert!(
        new_extension
            .keywords
            .iter()
            .any(|k| k.eq_ignore_ascii_case("frontmatter")),
        "New Scriptlet entry should be discoverable via 'frontmatter'"
    );
}

#[test]
fn new_script_fast_path_label_does_not_claim_template() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    let fast_path = entries
        .iter()
        .find(|e| e.id == "builtin/new-script")
        .expect("builtin/new-script should exist");

    assert_eq!(
        fast_path.name, "New Script",
        "fast path name must be plain 'New Script' — the template catalog owns the 'Template' label"
    );
    let description_lower = fast_path.description.to_lowercase();
    assert!(
        !description_lower.contains("template"),
        "fast-path description must not claim template behavior: {description_lower:?}"
    );
    assert!(
        !description_lower.contains("starter"),
        "fast-path description must not claim starter behavior: {description_lower:?}"
    );
    assert_eq!(
        fast_path.feature,
        BuiltInFeature::ScriptCommand(ScriptCommandType::NewScript),
        "fast-path must keep routing to the direct naming-prompt flow"
    );
}

#[test]
fn new_script_fast_path_does_not_own_template_keywords() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    let fast_path = entries
        .iter()
        .find(|e| e.id == "builtin/new-script")
        .expect("builtin/new-script should exist");

    for forbidden in ["template", "starter", "boilerplate", "scaffold"] {
        assert!(
            !fast_path
                .keywords
                .iter()
                .any(|k| k.eq_ignore_ascii_case(forbidden)),
            "fast path must not advertise '{forbidden}' — that keyword belongs to builtin/new-script-from-template"
        );
    }
}

#[test]
fn new_script_from_template_owns_template_discovery_terms() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    let template_path = entries
        .iter()
        .find(|e| e.id == "builtin/new-script-from-template")
        .expect("builtin/new-script-from-template should exist");

    assert_eq!(
        template_path.feature,
        BuiltInFeature::NewScriptFromTemplate,
        "template path must route to BuiltInFeature::NewScriptFromTemplate"
    );

    for required in [
        "template",
        "starter",
        "boilerplate",
        "scaffold",
        "choice",
        "arg",
    ] {
        assert!(
            template_path
                .keywords
                .iter()
                .any(|k| k.eq_ignore_ascii_case(required)),
            "template path must own the '{required}' discovery keyword"
        );
    }
}
#[test]
fn test_permission_command_entries_exist() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    assert!(entries.iter().any(|e| e.id == "builtin/check-permissions"));
    assert!(entries
        .iter()
        .any(|e| e.id == "builtin/request-accessibility"));
    assert!(entries
        .iter()
        .any(|e| e.id == "builtin/accessibility-settings"));
}
#[test]
fn test_system_action_type_equality() {
    assert_eq!(SystemActionType::EmptyTrash, SystemActionType::EmptyTrash);
    assert_ne!(SystemActionType::EmptyTrash, SystemActionType::LockScreen);
}
// NOTE: test_window_action_type_equality removed - WindowActionType no longer in builtins

#[test]
fn test_builtin_feature_system_action() {
    let feature = BuiltInFeature::SystemAction(SystemActionType::ToggleDarkMode);
    assert_eq!(
        feature,
        BuiltInFeature::SystemAction(SystemActionType::ToggleDarkMode)
    );
    assert_ne!(
        feature,
        BuiltInFeature::SystemAction(SystemActionType::Sleep)
    );
}
// --- merged from part_001.rs ---
// NOTE: test_builtin_feature_window_action removed - WindowAction no longer in BuiltInFeature

#[test]
fn test_file_search_builtin_exists() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    // Check that FileSearch entry exists
    let file_search = entries.iter().find(|e| e.id == "builtin/file-search");
    assert!(
        file_search.is_some(),
        "FileSearch builtin should exist in the main menu"
    );

    let file_search = file_search.unwrap();
    assert_eq!(file_search.name, "Search Files");
    assert_eq!(file_search.feature, BuiltInFeature::FileSearch);
    assert!(file_search.keywords.contains(&"file".to_string()));
    assert!(file_search.keywords.contains(&"search".to_string()));
    assert!(file_search.keywords.contains(&"find".to_string()));
    assert!(file_search.keywords.contains(&"directory".to_string()));
    assert!(file_search.icon.is_some());
}
#[test]
fn test_get_builtin_entries_includes_process_manager_command() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    let process_manager = entries.iter().find(|e| e.id == "builtin/process-manager");
    assert!(
        process_manager.is_some(),
        "Process Manager builtin should exist in the main menu"
    );

    let process_manager = process_manager.unwrap();
    assert_eq!(
        process_manager.feature,
        BuiltInFeature::UtilityCommand(UtilityCommandType::ProcessManager)
    );
    assert!(process_manager.keywords.iter().any(|k| k == "process"));
    assert!(process_manager.keywords.iter().any(|k| k == "running"));
    assert!(process_manager.keywords.iter().any(|k| k == "kill"));
}

#[test]
fn test_get_builtin_entries_includes_main_window_command() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    let main_window = entries.iter().find(|e| e.id == "builtin/main-window");
    assert!(
        main_window.is_some(),
        "Main Window builtin should exist in the main menu"
    );

    let main_window = main_window.unwrap();
    assert_eq!(
        main_window.feature,
        BuiltInFeature::UtilityCommand(UtilityCommandType::MainWindow)
    );
    assert_eq!(main_window.icon.as_deref(), Some("search"));
    assert!(main_window.keywords.iter().any(|k| k == "launcher"));
}

#[test]
fn test_process_manager_absorbs_stop_all_processes_command() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    assert!(
        entries
            .iter()
            .all(|entry| entry.id != "builtin/stop-all-processes"),
        "Stop all running scripts should be exposed through Process Manager instead of the top-level registry"
    );
}

#[test]
fn test_settings_command_entries_exist() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    assert!(entries
        .iter()
        .all(|entry| entry.id != "builtin/configure-vercel-api"));
    assert!(entries
        .iter()
        .all(|entry| entry.id != "builtin/configure-openai-api"));
    assert!(entries
        .iter()
        .all(|entry| entry.id != "builtin/configure-anthropic-api"));
    assert!(entries
        .iter()
        .any(|entry| entry.id == "builtin/choose-theme"));
    assert!(entries
        .iter()
        .any(|entry| entry.id == "builtin/select-microphone"));
}

#[test]
fn test_clear_suggested_command_exists() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    assert!(entries
        .iter()
        .any(|entry| entry.id == "builtin/clear-suggested"));
}

#[test]
fn test_dictation_hub_absorbs_forced_app_and_notes_entries() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    for id in ["builtin/dictation-to-app", "builtin/dictation-to-notes"] {
        assert!(
            entries.iter().all(|entry| entry.id != id),
            "{id} should remain an internal dictation route instead of a top-level launcher entry"
        );
    }

    assert!(
        entries
            .iter()
            .any(|entry| entry.id == "builtin/dictation-to-ai"),
        "builtin/dictation-to-ai should remain available as an explicit Agent Chat route"
    );
}

#[test]
fn test_resolve_builtin_entry_supports_hidden_dictation_routes() {
    let config = BuiltInConfig::default();

    let app_route = resolve_builtin_entry("builtin/dictation-to-app", &config)
        .expect("dictation-to-app route should resolve");
    assert_eq!(app_route.feature, BuiltInFeature::DictationToFrontmostApp);

    let notes_route = resolve_builtin_entry("builtin/dictation-to-notes", &config)
        .expect("dictation-to-notes route should resolve");
    assert_eq!(notes_route.feature, BuiltInFeature::DictationToNotes);
}

#[test]
fn test_builtin_descriptions_use_clear_action_phrasing() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    let notes = entries
        .iter()
        .find(|e| e.id == "builtin/open-notes")
        .unwrap();
    assert_eq!(notes.description, "Open the Notes window");

    let quick_capture = entries
        .iter()
        .find(|e| e.id == "builtin/quick-capture")
        .unwrap();
    assert_eq!(
        quick_capture.description,
        "Capture a new note without opening the full Notes window"
    );

    let file_search = entries
        .iter()
        .find(|e| e.id == "builtin/file-search")
        .unwrap();
    assert_eq!(
        file_search.description,
        "Browse directories, search files, and open results"
    );

    let webcam = entries.iter().find(|e| e.id == "builtin/webcam").unwrap();
    assert_eq!(
        webcam.description,
        "Open the webcam prompt and capture a photo"
    );
}

#[test]
fn test_builtin_enter_labels_use_action_specific_phrasing() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    let open_notes = entries
        .iter()
        .find(|e| e.id == "builtin/open-notes")
        .unwrap();
    assert_eq!(open_notes.default_action_text(), "Open Notes");

    let quick_capture = entries
        .iter()
        .find(|e| e.id == "builtin/quick-capture")
        .unwrap();
    assert_eq!(quick_capture.default_action_text(), "Start Quick Capture");

    let file_search = entries
        .iter()
        .find(|e| e.id == "builtin/file-search")
        .unwrap();
    assert_eq!(file_search.default_action_text(), "Search Files");

    let settings = entries.iter().find(|e| e.id == "builtin/settings").unwrap();
    assert_eq!(settings.default_action_text(), "Open Script Kit Settings");

    let process_manager = entries
        .iter()
        .find(|e| e.id == "builtin/process-manager")
        .unwrap();
    assert_eq!(
        process_manager.default_action_text(),
        "Open Process Manager"
    );

    let clipboard = entries
        .iter()
        .find(|e| e.id == "builtin/clipboard-history")
        .unwrap();
    assert_eq!(clipboard.default_action_text(), "Open Clipboard History");

    let volume = entries
        .iter()
        .find(|e| e.id == "builtin/volume-25")
        .unwrap();
    assert_eq!(volume.default_action_text(), "Set Volume to 25%");

    let send_browser_tab = entries
        .iter()
        .find(|e| e.id == "builtin/send-browser-tab-to-ai")
        .unwrap();
    assert_eq!(
        send_browser_tab.default_action_text(),
        "Send Tab to Agent Chat"
    );

    let webcam = entries.iter().find(|e| e.id == "builtin/webcam").unwrap();
    assert_eq!(webcam.default_action_text(), "Open Webcam");
}

#[test]
fn agent_chat_route_labels_name_agent_chat_not_generic_ai() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    for (id, expected_name, expected_action) in [
        (
            "builtin/send-screen-to-ai",
            "Send Screen to Agent Chat",
            "Send Screen to Agent Chat",
        ),
        (
            "builtin/send-focused-window-to-ai",
            "Send Focused Window to Agent Chat",
            "Send Window to Agent Chat",
        ),
        (
            "builtin/send-selected-text-to-ai",
            "Send Selected Text to Agent Chat",
            "Send Selection to Agent Chat",
        ),
        (
            "builtin/send-browser-tab-to-ai",
            "Send Focused Browser Tab to Agent Chat",
            "Send Tab to Agent Chat",
        ),
        (
            "builtin/dictation-to-ai",
            "Dictate to Agent Chat",
            "Start Dictation to Agent Chat",
        ),
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry.id == id)
            .unwrap_or_else(|| panic!("missing builtin entry {id}"));
        assert_eq!(entry.name, expected_name, "{id} name");
        assert_eq!(entry.default_action_text(), expected_action, "{id} action");
        assert!(
            !entry.name.contains(" to AI") && !entry.default_action_text().contains(" to AI"),
            "{id} should name the concrete Agent Chat destination"
        );
    }

    let dictation = entries
        .iter()
        .find(|entry| entry.id == "builtin/dictation-to-ai")
        .expect("dictation-to-ai entry should exist");
    assert_eq!(dictation.footer_action_text(), "Dictate Chat");
}

#[test]
fn test_builtin_footer_labels_stay_compact() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    for entry in entries {
        let label = entry.footer_action_text();
        let word_count = label.split_whitespace().count();
        assert!(
            word_count <= 2 || label == "Privacy & Security",
            "footer label '{}' for '{}' should stay compact",
            label,
            entry.name
        );
    }

    let settings = get_builtin_entries(&config)
        .into_iter()
        .find(|e| e.id == "builtin/settings")
        .unwrap();
    assert_eq!(settings.footer_action_text(), "Kit Settings");
}

#[test]
fn test_menu_bar_entries_use_menu_specific_enter_label() {
    let entry = BuiltInEntry::new_with_group(
        "menubar-com.apple.Safari-file-new-tab",
        "File → New Tab",
        "Safari  ⌘T",
        vec!["file".into(), "new tab".into(), "safari".into()],
        BuiltInFeature::MenuBarAction(MenuBarActionInfo {
            bundle_id: "com.apple.Safari".into(),
            menu_path: vec!["File".into(), "New Tab".into()],
            enabled: true,
            shortcut: Some("⌘T".into()),
        }),
        Some("file".into()),
        BuiltInGroup::MenuBar,
    );

    assert_eq!(entry.default_action_text(), "Execute Menu Item");
    assert_eq!(entry.footer_action_text(), "Menu Item");
}
#[test]
fn test_file_search_feature_equality() {
    assert_eq!(BuiltInFeature::FileSearch, BuiltInFeature::FileSearch);
    assert_ne!(BuiltInFeature::FileSearch, BuiltInFeature::ClipboardHistory);
    assert_ne!(BuiltInFeature::FileSearch, BuiltInFeature::Notes);
}
#[test]
fn test_system_action_hud_message_volume_presets() {
    assert_eq!(
        system_action_hud_message(SystemActionType::Volume0, None),
        Some("Volume 0%".to_string())
    );
    assert_eq!(
        system_action_hud_message(SystemActionType::Volume50, None),
        Some("Volume 50%".to_string())
    );
    assert_eq!(
        system_action_hud_message(SystemActionType::Volume100, None),
        Some("Volume 100%".to_string())
    );
    assert_eq!(
        system_action_hud_message(SystemActionType::VolumeMute, None),
        Some("Volume Muted".to_string())
    );
}
#[test]
fn test_system_action_hud_message_dark_mode() {
    assert_eq!(
        system_action_hud_message(SystemActionType::ToggleDarkMode, Some(true)),
        Some("Dark Mode On".to_string())
    );
    assert_eq!(
        system_action_hud_message(SystemActionType::ToggleDarkMode, Some(false)),
        Some("Dark Mode Off".to_string())
    );
    assert_eq!(
        system_action_hud_message(SystemActionType::ToggleDarkMode, None),
        Some("Dark Mode Toggled".to_string())
    );
}

// -----------------------------------------------------------------------
// Builtin metadata audit — unique IDs/names and minimum metadata
// -----------------------------------------------------------------------

#[test]
fn test_builtin_entries_have_unique_ids_and_names() {
    use std::collections::BTreeMap;

    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    assert!(!entries.is_empty(), "Expected at least one builtin entry");

    let mut id_counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut name_counts: BTreeMap<&str, usize> = BTreeMap::new();

    for entry in &entries {
        *id_counts.entry(&entry.id).or_default() += 1;
        *name_counts.entry(&entry.name).or_default() += 1;
    }

    let duplicate_ids: Vec<&str> = id_counts
        .iter()
        .filter(|(_, &count)| count > 1)
        .map(|(&id, _)| id)
        .collect();
    let duplicate_names: Vec<&str> = name_counts
        .iter()
        .filter(|(_, &count)| count > 1)
        .map(|(&name, _)| name)
        .collect();

    assert!(
        duplicate_ids.is_empty(),
        "Duplicate builtin ids: {duplicate_ids:?}"
    );
    assert!(
        duplicate_names.is_empty(),
        "Duplicate builtin names: {duplicate_names:?}"
    );
}

#[test]
fn test_builtin_entries_use_stable_prefix_and_minimum_metadata() {
    let config = BuiltInConfig::default();
    let entries = get_builtin_entries(&config);

    let invalid_ids: Vec<&str> = entries
        .iter()
        .filter(|entry| !entry.id.starts_with("builtin/"))
        .map(|entry| entry.id.as_str())
        .collect();
    let missing_descriptions: Vec<&str> = entries
        .iter()
        .filter(|entry| entry.description.trim().is_empty())
        .map(|entry| entry.id.as_str())
        .collect();
    let sparse_keywords: Vec<String> = entries
        .iter()
        .filter(|entry| entry.keywords.len() < 3)
        .map(|entry| format!("{} ({})", entry.id, entry.keywords.len()))
        .collect();

    assert!(
        invalid_ids.is_empty(),
        "Builtin ids must start with 'builtin-': {invalid_ids:?}"
    );
    assert!(
        missing_descriptions.is_empty(),
        "Builtin entries missing descriptions: {missing_descriptions:?}"
    );
    assert!(
        sparse_keywords.is_empty(),
        "Builtin entries need at least 3 keywords: {sparse_keywords:?}"
    );
}

// =====================================================================
// Current App Commands collapsed into Do in Current App
// =====================================================================

#[test]
fn current_app_commands_builtin_is_no_longer_registered() {
    let entries = get_builtin_entries(&BuiltInConfig::default());
    let found = entries
        .iter()
        .find(|e| e.id == "builtin/current-app-commands");
    assert!(
        found.is_none(),
        "builtin/current-app-commands should no longer be registered (collapsed into Do in Current App)"
    );
}

#[test]
fn do_in_current_app_has_commands_keywords() {
    let entries = get_builtin_entries(&BuiltInConfig::default());
    let entry = entries
        .iter()
        .find(|e| e.id == "builtin/do-in-current-app")
        .expect("builtin/do-in-current-app must be registered");
    assert!(
        entry.keywords.contains(&"commands".to_string()),
        "Do in Current App should include 'commands' keyword from collapsed entry"
    );
    assert!(
        entry.keywords.contains(&"menubar".to_string()),
        "Do in Current App should include 'menubar' keyword from collapsed entry"
    );
}

#[test]
fn turn_this_into_command_builtin_is_no_longer_registered() {
    let entries = get_builtin_entries(&BuiltInConfig::default());
    let found = entries
        .iter()
        .find(|e| e.id == "builtin/turn-this-into-a-command");
    assert!(
        found.is_none(),
        "builtin/turn-this-into-a-command should no longer be registered (collapsed into Do in Current App)"
    );
}

#[test]
fn do_in_current_app_absorbs_turn_this_into_command_keywords() {
    let entries = get_builtin_entries(&BuiltInConfig::default());
    let entry = entries
        .iter()
        .find(|e| e.id == "builtin/do-in-current-app")
        .expect("builtin/do-in-current-app must be registered");
    assert!(
        entry
            .keywords
            .contains(&"turn this into a command".to_string()),
        "Do in Current App should include the collapsed turn-this alias phrase"
    );
    assert!(
        entry.keywords.contains(&"teach".to_string()),
        "Do in Current App should include the collapsed 'teach' keyword"
    );
    assert!(
        entry.keywords.contains(&"recipe".to_string()),
        "Do in Current App should include the collapsed 'recipe' keyword"
    );
}

#[test]
fn menu_bar_leaf_name_returns_last_segment() {
    let entry = BuiltInEntry::new_with_group(
        "menubar-com.apple.Safari-file-new-tab",
        "File → New Tab",
        "Safari  ⌘T",
        vec!["file".to_string(), "new".to_string(), "tab".to_string()],
        BuiltInFeature::MenuBarAction(MenuBarActionInfo {
            bundle_id: "com.apple.Safari".into(),
            menu_path: vec!["File".into(), "New Tab".into()],
            enabled: true,
            shortcut: Some("⌘T".into()),
        }),
        Some("folder".into()),
        BuiltInGroup::MenuBar,
    );

    assert_eq!(entry.leaf_name(), "New Tab");
}

#[test]
fn leaf_name_single_segment() {
    let entry = BuiltInEntry::new_with_group(
        "menubar-com.apple.Safari-quit",
        "Quit",
        "Safari",
        vec!["quit".to_string()],
        BuiltInFeature::MenuBarAction(MenuBarActionInfo {
            bundle_id: "com.apple.Safari".into(),
            menu_path: vec!["Quit".into()],
            enabled: true,
            shortcut: None,
        }),
        Some("pin".into()),
        BuiltInGroup::MenuBar,
    );

    assert_eq!(entry.leaf_name(), "Quit");
}

#[test]
fn leaf_name_core_builtin_returns_full_name() {
    let entry = BuiltInEntry::new_with_icon(
        "builtin/clipboard-history",
        "Clipboard History",
        "View and paste from clipboard",
        vec!["clipboard"],
        BuiltInFeature::ClipboardHistory,
        "clipboard",
    );
    // Core group → leaf_name returns full name unchanged
    assert_eq!(entry.leaf_name(), "Clipboard History");
}

#[test]
fn builtin_frecency_key_uses_id() {
    let entry = BuiltInEntry::new_with_group(
        "menubar-com.apple.Safari-file-new-tab",
        "File → New Tab",
        "Safari  ⌘T",
        vec!["file".to_string(), "new".to_string(), "tab".to_string()],
        BuiltInFeature::MenuBarAction(MenuBarActionInfo {
            bundle_id: "com.apple.Safari".into(),
            menu_path: vec!["File".into(), "New Tab".into()],
            enabled: true,
            shortcut: Some("⌘T".into()),
        }),
        Some("folder".into()),
        BuiltInGroup::MenuBar,
    );

    let frecency_key = format!("builtin:{}", entry.id);
    assert_eq!(
        frecency_key,
        "builtin:menubar-com.apple.Safari-file-new-tab"
    );
    // Key is derived from id, not from name
    assert_ne!(frecency_key, format!("builtin:{}", entry.name));
}

#[test]
fn menu_bar_items_to_entries_representative_path() {
    use crate::menu_bar::{KeyboardShortcut, MenuBarItem, ModifierFlags};

    let items = vec![
        // Apple menu (index 0) — should be skipped
        MenuBarItem {
            title: "Apple".into(),
            enabled: true,
            shortcut: None,
            children: vec![],
            ax_element_path: vec![0],
        },
        // File menu with children
        MenuBarItem {
            title: "File".into(),
            enabled: true,
            shortcut: None,
            children: vec![
                MenuBarItem {
                    title: "New Tab".into(),
                    enabled: true,
                    shortcut: Some(KeyboardShortcut::new("T".into(), ModifierFlags::COMMAND)),
                    children: vec![],
                    ax_element_path: vec![1, 0],
                },
                // Separator — should be skipped
                MenuBarItem::separator(vec![1, 1]),
                // Disabled item — should be skipped
                MenuBarItem {
                    title: "Close All".into(),
                    enabled: false,
                    shortcut: None,
                    children: vec![],
                    ax_element_path: vec![1, 2],
                },
                MenuBarItem {
                    title: "New Window".into(),
                    enabled: true,
                    shortcut: Some(KeyboardShortcut::new("N".into(), ModifierFlags::COMMAND)),
                    children: vec![],
                    ax_element_path: vec![1, 3],
                },
            ],
            ax_element_path: vec![1],
        },
    ];

    let entries = menu_bar_items_to_entries(&items, "com.apple.Safari", "Safari");

    // Should have exactly 2 entries (New Tab, New Window) — separator and disabled skipped
    assert_eq!(entries.len(), 2, "expected 2 entries, got: {entries:?}");

    // First entry: File → New Tab
    let new_tab = &entries[0];
    assert_eq!(new_tab.name, "File → New Tab");
    assert_eq!(new_tab.id, "menubar-com.apple.Safari-file-new-tab");
    assert!(
        new_tab.description.contains("Safari"),
        "description should contain app name"
    );
    assert!(
        new_tab.description.contains("⌘T"),
        "description should contain shortcut"
    );
    assert_eq!(new_tab.group, BuiltInGroup::MenuBar);

    // Verify MenuBarAction info
    if let BuiltInFeature::MenuBarAction(ref info) = new_tab.feature {
        assert_eq!(info.bundle_id, "com.apple.Safari");
        assert_eq!(info.menu_path, vec!["File", "New Tab"]);
        assert!(info.enabled);
        assert_eq!(info.shortcut, Some("⌘T".into()));
    } else {
        panic!("expected MenuBarAction feature");
    }

    // Second entry: File → New Window
    let new_window = &entries[1];
    assert_eq!(new_window.name, "File → New Window");
    assert_eq!(new_window.id, "menubar-com.apple.Safari-file-new-window");
}

#[test]
fn menu_bar_items_to_entries_no_shortcut() {
    use crate::menu_bar::MenuBarItem;

    let items = vec![
        // Apple menu — skipped
        MenuBarItem {
            title: "Apple".into(),
            enabled: true,
            shortcut: None,
            children: vec![],
            ax_element_path: vec![0],
        },
        // Edit menu with one child, no shortcut
        MenuBarItem {
            title: "Edit".into(),
            enabled: true,
            shortcut: None,
            children: vec![MenuBarItem {
                title: "Paste".into(),
                enabled: true,
                shortcut: None,
                children: vec![],
                ax_element_path: vec![1, 0],
            }],
            ax_element_path: vec![1],
        },
    ];

    let entries = menu_bar_items_to_entries(&items, "com.apple.Finder", "Finder");

    assert_eq!(entries.len(), 1);
    // No shortcut → description is just the app name
    assert_eq!(entries[0].description, "Finder");
}

#[test]
fn menu_bar_items_to_entries_empty_items() {
    let entries: Vec<BuiltInEntry> = menu_bar_items_to_entries(&[], "com.example.App", "Example");
    assert!(entries.is_empty());
}

#[test]
fn shortcut_search_tokens_expands_symbolic_shortcuts() {
    let tokens = shortcut_search_tokens("⌘⇧T");

    assert!(tokens.contains(&"⌘⇧t".to_string()));
    assert!(tokens.contains(&"cmdshiftt".to_string()));
    assert!(tokens.contains(&"cmd shift t".to_string()));
    assert!(tokens.contains(&"cmd+shift+t".to_string()));
}

#[test]
fn menu_bar_entry_matches_shortcut_aliases() {
    let entry = BuiltInEntry::new_with_group(
        "menubar-com.apple.Safari-file-new-tab",
        "File → New Tab",
        "Safari  ⌘T",
        vec![
            "file".into(),
            "new".into(),
            "tab".into(),
            "safari".into(),
            "⌘t".into(),
            "cmdt".into(),
            "cmd t".into(),
            "cmd+t".into(),
        ],
        BuiltInFeature::MenuBarAction(MenuBarActionInfo {
            bundle_id: "com.apple.Safari".into(),
            menu_path: vec!["File".into(), "New Tab".into()],
            enabled: true,
            shortcut: Some("⌘T".into()),
        }),
        Some("folder".into()),
        BuiltInGroup::MenuBar,
    );

    assert!(menu_bar_entry_matches_query(&entry, "cmd+t"));
    assert!(menu_bar_entry_matches_query(&entry, "cmd t"));
    assert!(menu_bar_entry_matches_query(&entry, "safari new"));
    assert!(!menu_bar_entry_matches_query(&entry, "cmd+p"));
}

#[test]
fn filter_menu_bar_entries_reports_counts() {
    let entries = vec![
        BuiltInEntry::new_with_group(
            "menubar-com.apple.Safari-file-new-tab",
            "File → New Tab",
            "Safari  ⌘T",
            vec!["file".into(), "new".into(), "tab".into(), "cmd+t".into()],
            BuiltInFeature::MenuBarAction(MenuBarActionInfo {
                bundle_id: "com.apple.Safari".into(),
                menu_path: vec!["File".into(), "New Tab".into()],
                enabled: true,
                shortcut: Some("⌘T".into()),
            }),
            Some("folder".into()),
            BuiltInGroup::MenuBar,
        ),
        BuiltInEntry::new_with_group(
            "menubar-com.apple.Safari-file-new-window",
            "File → New Window",
            "Safari  ⌘N",
            vec!["file".into(), "new".into(), "window".into(), "cmd+n".into()],
            BuiltInFeature::MenuBarAction(MenuBarActionInfo {
                bundle_id: "com.apple.Safari".into(),
                menu_path: vec!["File".into(), "New Window".into()],
                enabled: true,
                shortcut: Some("⌘N".into()),
            }),
            Some("folder".into()),
            BuiltInGroup::MenuBar,
        ),
    ];

    let (filtered, receipt) = filter_menu_bar_entries(&entries, "cmd+t");

    assert_eq!(receipt.query, "cmd+t");
    assert_eq!(receipt.normalized_query, "cmd+t");
    assert_eq!(receipt.total_entries, 2);
    assert_eq!(receipt.matched_entries, 1);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].1.name, "File → New Tab");
}
#[test]
fn test_generate_script_from_current_app_builtin_is_registered() {
    let entries = get_builtin_entries(&BuiltInConfig::default());

    let entry = entries
        .iter()
        .find(|e| e.id == "builtin/generate-script-from-current-app")
        .expect("builtin/generate-script-from-current-app must exist");

    assert_eq!(entry.name, "Generate Script from Current App");
    assert_eq!(
        entry.feature,
        BuiltInFeature::AiCommand(AiCommandType::GenerateScriptFromCurrentApp)
    );
    assert!(entry.keywords.contains(&"current".to_string()));
    assert!(entry.keywords.contains(&"app".to_string()));
    assert!(entry.keywords.contains(&"menu".to_string()));
    assert!(entry.keywords.contains(&"browser".to_string()));
}

#[test]
fn harness_first_ai_entries_are_registered_and_legacy_window_entries_are_not() {
    let entries = get_builtin_entries(&BuiltInConfig::default());
    let ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();

    // Legacy AI window commands must not be registered
    for legacy_id in [
        "builtin/open-ai-chat",
        "builtin/mini-ai-chat",
        "builtin/new-conversation",
        "builtin/clear-conversation",
    ] {
        assert!(
            !ids.contains(&legacy_id),
            "{legacy_id} should not be registered once AI is harness-first"
        );
    }

    // Harness-first AI entries must remain registered
    for expected_id in [
        "builtin/ai-chat",
        "builtin/generate-script-with-ai",
        "builtin/generate-script-from-current-app",
        "builtin/send-screen-to-ai",
        "builtin/send-focused-window-to-ai",
        "builtin/send-selected-text-to-ai",
        "builtin/send-browser-tab-to-ai",
        "builtin/new-script",
        "builtin/new-extension",
    ] {
        assert!(
            ids.contains(&expected_id),
            "{expected_id} should remain registered"
        );
    }

    // Preview-only screen-area AI entry should be hidden until real region context works
    assert!(
        !ids.contains(&"builtin/send-screen-area-to-ai"),
        "hide the preview-only screen-area AI entry until it can attach real region context"
    );
}
