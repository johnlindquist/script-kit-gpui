//! Integration tests for current-app command routing and built-ins.
//!
//! These tests verify registration, snapshot contract, and entry shaping
//! without requiring a live frontmost app or Accessibility permission.

use script_kit_gpui::builtins::{
    self, AiCommandType, BuiltInEntry, BuiltInFeature, BuiltInGroup, MenuBarActionInfo,
    UtilityCommandType,
};
use script_kit_gpui::config::BuiltInConfig;
use script_kit_gpui::menu_bar::current_app_commands::{
    build_current_app_command_recipe, build_current_app_intent_trace_receipt,
    build_generate_script_prompt_from_snapshot, build_generated_script_prompt_from_recipe,
    normalize_trace_current_app_intent_request, normalize_turn_this_into_a_command_request,
    parse_current_app_command_recipe_json, resolve_do_in_current_app_intent,
    suggest_current_app_command_name, verify_current_app_command_recipe, DoInCurrentAppAction,
    FrontmostMenuSnapshot, CURRENT_APP_COMMAND_RECIPE_SCHEMA_VERSION,
};
use script_kit_gpui::menu_bar::{KeyboardShortcut, MenuBarItem, ModifierFlags};

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

#[test]
fn do_in_current_app_builtin_is_registered() {
    let entries = builtins::get_builtin_entries(&BuiltInConfig::default());
    let entry = entries
        .iter()
        .find(|e| e.id == "builtin/do-in-current-app")
        .expect("builtin/do-in-current-app must be in the registry");

    assert_eq!(
        entry.feature,
        BuiltInFeature::UtilityCommand(UtilityCommandType::DoInCurrentApp)
    );
    assert!(
        entry
            .keywords
            .contains(&"turn this into a command".to_string()),
        "builtin/do-in-current-app should absorb the collapsed turn-this alias phrase"
    );
}

#[test]
fn current_app_commands_builtin_is_no_longer_registered() {
    let entries = builtins::get_builtin_entries(&BuiltInConfig::default());
    assert!(
        entries
            .iter()
            .all(|e| e.id != "builtin/current-app-commands"),
        "builtin/current-app-commands should no longer be in the registry"
    );
}

#[test]
fn trace_current_app_intent_builtin_is_registered() {
    let entries = builtins::get_builtin_entries(&BuiltInConfig::default());
    let entry = entries
        .iter()
        .find(|e| e.id == "builtin/trace-current-app-intent")
        .expect("builtin/trace-current-app-intent must be in the registry");

    assert_eq!(
        entry.feature,
        BuiltInFeature::UtilityCommand(UtilityCommandType::TraceCurrentAppIntent)
    );
}

// ---------------------------------------------------------------------------
// leaf_name
// ---------------------------------------------------------------------------

#[test]
fn menu_bar_leaf_name_returns_last_path_segment() {
    let entry = BuiltInEntry::new_with_group(
        "menubar-com.apple.Safari-file-new-tab",
        "File → New Tab",
        "Safari  ⌘T",
        vec!["file".into(), "new".into(), "tab".into()],
        BuiltInFeature::MenuBarAction(MenuBarActionInfo {
            bundle_id: "com.apple.Safari".into(),
            menu_path: vec!["File".into(), "New Tab".into()],
            enabled: true,
            shortcut: Some("⌘T".into()),
        }),
        Some("📁".into()),
        BuiltInGroup::MenuBar,
    );

    assert_eq!(entry.leaf_name(), "New Tab");
}

// ---------------------------------------------------------------------------
// Frecency key derivation
// ---------------------------------------------------------------------------

#[test]
fn frecency_keys_for_builtins_are_derived_from_entry_id() {
    let entry = BuiltInEntry::new_with_group(
        "menubar-com.apple.Safari-file-new-tab",
        "File → New Tab",
        "Safari  ⌘T",
        vec!["file".into(), "new".into(), "tab".into()],
        BuiltInFeature::MenuBarAction(MenuBarActionInfo {
            bundle_id: "com.apple.Safari".into(),
            menu_path: vec!["File".into(), "New Tab".into()],
            enabled: true,
            shortcut: Some("⌘T".into()),
        }),
        Some("📁".into()),
        BuiltInGroup::MenuBar,
    );

    let key = format!("builtin:{}", entry.id);
    assert_eq!(key, "builtin:menubar-com.apple.Safari-file-new-tab");
    // Must differ from name-based key
    assert_ne!(key, format!("builtin:{}", entry.name));
}

// ---------------------------------------------------------------------------
// menu_bar_items_to_entries: representative menu path & shortcut shape
// ---------------------------------------------------------------------------

fn sample_safari_items() -> Vec<MenuBarItem> {
    vec![
        // Apple menu (skipped by convention)
        MenuBarItem {
            title: "Apple".into(),
            enabled: true,
            shortcut: None,
            children: vec![],
            ax_element_path: vec![0],
        },
        // File menu with mixed children
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
                MenuBarItem::separator(vec![1, 1]),
                MenuBarItem {
                    title: "Close All".into(),
                    enabled: false,
                    shortcut: None,
                    children: vec![],
                    ax_element_path: vec![1, 2],
                },
            ],
            ax_element_path: vec![1],
        },
    ]
}

#[test]
fn menu_bar_items_to_entries_shape() {
    let entries =
        builtins::menu_bar_items_to_entries(&sample_safari_items(), "com.apple.Safari", "Safari");

    // Separator and disabled item are excluded
    assert_eq!(entries.len(), 1);

    let e = &entries[0];
    assert_eq!(e.name, "File → New Tab");
    assert_eq!(e.id, "menubar-com.apple.Safari-file-new-tab");
    assert!(e.description.contains("⌘T"));
    assert!(e.description.contains("Safari"));
    assert_eq!(e.group, BuiltInGroup::MenuBar);

    if let BuiltInFeature::MenuBarAction(ref info) = e.feature {
        assert_eq!(info.bundle_id, "com.apple.Safari");
        assert_eq!(info.menu_path, vec!["File", "New Tab"]);
        assert!(info.enabled);
    } else {
        panic!("expected MenuBarAction");
    }
}

// ---------------------------------------------------------------------------
// FrontmostMenuSnapshot contract
// ---------------------------------------------------------------------------

#[test]
fn snapshot_into_entries_delegates_correctly() {
    let snapshot = FrontmostMenuSnapshot {
        app_name: "Safari".into(),
        bundle_id: "com.apple.Safari".into(),
        items: sample_safari_items(),
    };

    let placeholder = snapshot.placeholder();
    assert!(
        placeholder.contains("Safari"),
        "placeholder should contain app name"
    );

    let entries = snapshot.into_entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "File → New Tab");
}

#[test]
fn snapshot_empty_items_yields_empty_entries() {
    let snapshot = FrontmostMenuSnapshot {
        app_name: "TestApp".into(),
        bundle_id: "com.example.TestApp".into(),
        items: vec![],
    };

    assert!(snapshot.into_entries().is_empty());
}

// ---------------------------------------------------------------------------
// Keyword enrichment: app name + shortcut aliases
// ---------------------------------------------------------------------------

#[test]
fn menu_bar_items_to_entries_skips_disabled_items_and_separators() {
    let entries = script_kit_gpui::builtins::menu_bar_items_to_entries(
        &sample_safari_items(),
        "com.apple.Safari",
        "Safari",
    );

    let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, vec!["File → New Tab"]);
}

#[test]
fn menu_bar_entry_keywords_include_app_name_and_shortcut_aliases() {
    let entries = script_kit_gpui::builtins::menu_bar_items_to_entries(
        &sample_safari_items(),
        "com.apple.Safari",
        "Safari",
    );

    let entry = entries
        .iter()
        .find(|entry| entry.name == "File → New Tab")
        .expect("New Tab entry should exist");

    assert!(
        entry.keywords.contains(&"safari".to_string()),
        "keywords should contain app name; got: {:?}",
        entry.keywords
    );
    assert!(
        entry.keywords.contains(&"⌘t".to_string()),
        "keywords should contain ⌘t; got: {:?}",
        entry.keywords
    );
    assert!(
        entry.keywords.contains(&"cmd+t".to_string()),
        "keywords should contain cmd+t; got: {:?}",
        entry.keywords
    );
    assert!(
        entry.keywords.contains(&"cmd t".to_string()),
        "keywords should contain 'cmd t'; got: {:?}",
        entry.keywords
    );
    assert!(
        entry.keywords.contains(&"cmdt".to_string()),
        "keywords should contain cmdt; got: {:?}",
        entry.keywords
    );
}

#[test]
fn menu_bar_entry_query_matching_supports_multi_term_queries() {
    let entries = script_kit_gpui::builtins::menu_bar_items_to_entries(
        &sample_safari_items(),
        "com.apple.Safari",
        "Safari",
    );

    let entry = entries
        .iter()
        .find(|entry| entry.name == "File → New Tab")
        .expect("New Tab entry should exist");

    assert!(script_kit_gpui::builtins::menu_bar_entry_matches_query(
        entry, "new tab"
    ));
    assert!(script_kit_gpui::builtins::menu_bar_entry_matches_query(
        entry, "safari"
    ));
    assert!(script_kit_gpui::builtins::menu_bar_entry_matches_query(
        entry, "⌘t"
    ));
    assert!(script_kit_gpui::builtins::menu_bar_entry_matches_query(
        entry, "cmd+t"
    ));
    assert!(script_kit_gpui::builtins::menu_bar_entry_matches_query(
        entry,
        "safari cmd+t"
    ));
    assert!(!script_kit_gpui::builtins::menu_bar_entry_matches_query(
        entry,
        "close all"
    ));
}

// ---------------------------------------------------------------------------
// Generate Script from Current App: registration
// ---------------------------------------------------------------------------

#[test]
fn generate_script_from_current_app_builtin_is_registered() {
    let entries = builtins::get_builtin_entries(&BuiltInConfig::default());
    let entry = entries
        .iter()
        .find(|e| e.id == "builtin/generate-script-from-current-app")
        .expect("builtin/generate-script-from-current-app must be in the registry");

    assert_eq!(
        entry.feature,
        BuiltInFeature::AiCommand(AiCommandType::GenerateScriptFromCurrentApp)
    );
}

// ---------------------------------------------------------------------------
// Generate Script from Current App: prompt shaping (integration)
// ---------------------------------------------------------------------------

fn safari_snapshot_with_menus() -> FrontmostMenuSnapshot {
    FrontmostMenuSnapshot {
        app_name: "Safari".into(),
        bundle_id: "com.apple.Safari".into(),
        items: vec![
            // Apple menu (skipped)
            MenuBarItem {
                title: "Apple".into(),
                enabled: true,
                shortcut: None,
                children: vec![],
                ax_element_path: vec![0],
            },
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
                    MenuBarItem {
                        title: "Close Window".into(),
                        enabled: true,
                        shortcut: Some(KeyboardShortcut::new("W".into(), ModifierFlags::COMMAND)),
                        children: vec![],
                        ax_element_path: vec![1, 1],
                    },
                ],
                ax_element_path: vec![1],
            },
        ],
    }
}

#[test]
fn prompt_shaping_includes_user_request_selected_text_and_browser_url() {
    let (prompt, receipt) = build_generate_script_prompt_from_snapshot(
        safari_snapshot_with_menus(),
        Some("close duplicate tabs"),
        Some("some selected text"),
        Some("https://example.com/page"),
    );

    assert_eq!(receipt.app_name, "Safari");
    assert_eq!(receipt.bundle_id, "com.apple.Safari");
    assert!(receipt.included_user_request);
    assert!(receipt.included_selected_text);
    assert!(receipt.included_browser_url);

    assert!(prompt.contains("User Request:\nclose duplicate tabs"));
    assert!(prompt.contains("Frontmost App: Safari"));
    assert!(prompt.contains("Bundle ID: com.apple.Safari"));
    assert!(prompt.contains("Selected Text:\n```text\nsome selected text\n```"));
    assert!(prompt.contains("Focused Browser URL:\nhttps://example.com/page"));
}

#[test]
fn prompt_shaping_includes_menu_shortcut_formatting() {
    let (prompt, receipt) =
        build_generate_script_prompt_from_snapshot(safari_snapshot_with_menus(), None, None, None);

    // Receipt tracks correct menu item count (Apple menu items are skipped)
    assert_eq!(receipt.included_menu_items, 2);
    assert!(!receipt.included_user_request);
    assert!(!receipt.included_selected_text);
    assert!(!receipt.included_browser_url);

    // Shortcuts should be formatted in parentheses
    assert!(
        prompt.contains("(⌘T)"),
        "Prompt should contain ⌘T shortcut, got:\n{}",
        prompt
    );
    assert!(
        prompt.contains("(⌘W)"),
        "Prompt should contain ⌘W shortcut, got:\n{}",
        prompt
    );
}

#[test]
fn prompt_shaping_truncates_to_20_menu_items() {
    let children: Vec<MenuBarItem> = (0..30)
        .map(|idx| MenuBarItem {
            title: format!("Action {}", idx),
            enabled: true,
            shortcut: None,
            children: vec![],
            ax_element_path: vec![1, idx],
        })
        .collect();

    let snapshot = FrontmostMenuSnapshot {
        app_name: "BigApp".into(),
        bundle_id: "com.example.BigApp".into(),
        items: vec![
            MenuBarItem {
                title: "Apple".into(),
                enabled: true,
                shortcut: None,
                children: vec![],
                ax_element_path: vec![0],
            },
            MenuBarItem {
                title: "Edit".into(),
                enabled: true,
                shortcut: None,
                children,
                ax_element_path: vec![1],
            },
        ],
    };

    let (prompt, receipt) = build_generate_script_prompt_from_snapshot(snapshot, None, None, None);

    assert_eq!(receipt.total_menu_items, 30);
    assert_eq!(receipt.included_menu_items, 20);
    assert!(prompt.contains("showing 20 of 30"));
}

// ---------------------------------------------------------------------------
// No-match intent → GenerateScript bridge
// ---------------------------------------------------------------------------

#[test]
fn do_in_current_app_no_match_bridges_cleanly_into_current_app_script_prompt() {
    let snap = safari_snapshot_with_menus();
    let entries = snap.clone().into_entries();

    // Step 1: Router returns GenerateScript for a query that matches no menu entry
    let (action, receipt) =
        resolve_do_in_current_app_intent(&entries, Some("close duplicate tabs"));

    assert_eq!(action, DoInCurrentAppAction::GenerateScript);
    assert_eq!(receipt.filtered_entries, 0);
    assert_eq!(receipt.exact_matches, 0);
    assert_eq!(receipt.action, "generate_script");

    // Step 2: The same snapshot feeds the prompt builder with full context
    let (prompt, prompt_receipt) = build_generate_script_prompt_from_snapshot(
        safari_snapshot_with_menus(),
        Some("close duplicate tabs"),
        Some("pricing"),
        Some("https://example.com/pricing"),
    );

    // Prompt includes user request, app metadata, and optional context
    assert!(
        prompt.contains("User Request:\nclose duplicate tabs"),
        "Prompt must include user request"
    );
    assert!(
        prompt.contains("Frontmost App: Safari"),
        "Prompt must include frontmost app name"
    );
    assert!(
        prompt.contains("Bundle ID: com.apple.Safari"),
        "Prompt must include bundle ID"
    );
    assert!(
        prompt.contains("Focused Browser URL:\nhttps://example.com/pricing"),
        "Prompt must include browser URL when provided"
    );

    // Receipt flags confirm context inclusion
    assert!(prompt_receipt.included_user_request);
    assert!(prompt_receipt.included_selected_text);
    assert!(prompt_receipt.included_browser_url);
}

// ---------------------------------------------------------------------------
// Trace Current App Intent: label normalization
// ---------------------------------------------------------------------------

#[test]
fn normalize_trace_current_app_intent_request_strips_builtin_label_prefix() {
    assert_eq!(
        normalize_trace_current_app_intent_request(Some("Trace Current App Intent")),
        None
    );
    assert_eq!(
        normalize_trace_current_app_intent_request(Some(
            "Trace Current App Intent close duplicate tabs"
        )),
        Some("close duplicate tabs".to_string())
    );
    assert_eq!(
        normalize_trace_current_app_intent_request(Some("trace current app intent: cmd+t")),
        Some("cmd+t".to_string())
    );
    assert_eq!(
        normalize_trace_current_app_intent_request(Some("   ")),
        None
    );
    assert_eq!(normalize_trace_current_app_intent_request(None), None);
    // Unrelated input is returned as-is
    assert_eq!(
        normalize_trace_current_app_intent_request(Some("close duplicate tabs")),
        Some("close duplicate tabs".to_string())
    );
}

// ---------------------------------------------------------------------------
// Trace Current App Intent: trace receipt builder
// ---------------------------------------------------------------------------

#[test]
fn trace_receipt_reports_execute_entry_for_shortcut_keyword() {
    let receipt = build_current_app_intent_trace_receipt(
        safari_snapshot_with_menus(),
        Some("Trace Current App Intent cmd+t"),
    );

    assert_eq!(receipt.action, "execute_entry");
    assert_eq!(receipt.filtered_entries, 1);
    assert_eq!(receipt.exact_matches, 1);
    assert_eq!(
        receipt
            .selected_entry
            .as_ref()
            .map(|item| item.leaf_name.as_str()),
        Some("New Tab")
    );
    assert!(receipt.prompt_preview.is_none());
    assert!(receipt.prompt_receipt.is_none());
    assert_eq!(receipt.schema_version, 1);
    assert_eq!(receipt.app_name, "Safari");
    assert_eq!(receipt.bundle_id, "com.apple.Safari");
}

#[test]
fn trace_receipt_reports_generate_script_and_includes_prompt_preview() {
    let receipt = build_current_app_intent_trace_receipt(
        safari_snapshot_with_menus(),
        Some("Trace Current App Intent close duplicate tabs"),
    );

    assert_eq!(receipt.action, "generate_script");
    assert_eq!(receipt.filtered_entries, 0);
    assert_eq!(receipt.exact_matches, 0);
    assert!(receipt.selected_entry.is_none());
    assert!(receipt.candidates.is_empty());
    assert!(receipt
        .prompt_preview
        .as_ref()
        .expect("prompt_preview should be present")
        .contains("User Request:\nclose duplicate tabs"));
    assert!(
        receipt
            .prompt_receipt
            .as_ref()
            .expect("prompt_receipt should be present")
            .included_user_request
    );
}

#[test]
fn trace_receipt_serializes_to_valid_json() {
    let receipt = build_current_app_intent_trace_receipt(
        safari_snapshot_with_menus(),
        Some("Trace Current App Intent cmd+t"),
    );

    let json = serde_json::to_string_pretty(&receipt).expect("receipt must serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("JSON must be valid");

    assert_eq!(parsed["schema_version"], 1);
    assert_eq!(parsed["action"], "execute_entry");
    assert_eq!(parsed["app_name"], "Safari");
}

// ---------------------------------------------------------------------------
// Turn This Into a Command — registration & contract
// ---------------------------------------------------------------------------

#[test]
fn turn_this_into_a_command_builtin_is_no_longer_registered() {
    let entries = builtins::get_builtin_entries(&BuiltInConfig::default());
    assert!(
        entries
            .iter()
            .all(|e| e.id != "builtin/turn-this-into-a-command"),
        "builtin/turn-this-into-a-command should no longer be in the registry"
    );
}

#[test]
fn normalize_turn_this_into_a_command_request_strips_builtin_label_prefix() {
    assert_eq!(
        normalize_turn_this_into_a_command_request(Some("Turn This Into a Command")),
        None
    );
    assert_eq!(
        normalize_turn_this_into_a_command_request(Some(
            "Turn This Into a Command close duplicate tabs"
        )),
        Some("close duplicate tabs".to_string())
    );
    assert_eq!(
        normalize_turn_this_into_a_command_request(Some("turn this into a command: cmd+t")),
        Some("cmd+t".to_string())
    );
    assert_eq!(
        normalize_turn_this_into_a_command_request(Some(
            "Turn This Into a Command \u{2014} archive inbox zero"
        )),
        Some("archive inbox zero".to_string())
    );
    assert_eq!(
        normalize_turn_this_into_a_command_request(Some("   ")),
        None
    );
    assert_eq!(normalize_turn_this_into_a_command_request(None), None);
}

#[test]
fn suggest_current_app_command_name_is_stable_and_human_readable() {
    assert_eq!(
        suggest_current_app_command_name("Safari", "close duplicate tabs"),
        "Safari Close Duplicate Tabs"
    );
    assert_eq!(
        suggest_current_app_command_name("Safari", "cmd+t"),
        "Safari Cmd T"
    );
    assert_eq!(
        suggest_current_app_command_name("Safari", ""),
        "Safari Command"
    );
}

#[test]
fn turn_this_into_a_command_recipe_keeps_trace_prompt_in_sync_for_generate_script() {
    let recipe = build_current_app_command_recipe(
        safari_snapshot_with_menus(),
        Some("Turn This Into a Command close duplicate tabs"),
        Some("pricing"),
        Some("browser-tab-url"),
    );

    assert_eq!(
        recipe.schema_version,
        CURRENT_APP_COMMAND_RECIPE_SCHEMA_VERSION
    );
    assert_eq!(recipe.recipe_type, "currentAppCommand");
    assert_eq!(
        recipe.raw_query,
        "Turn This Into a Command close duplicate tabs"
    );
    assert_eq!(recipe.effective_query, "close duplicate tabs");
    assert_eq!(recipe.suggested_script_name, "Safari Close Duplicate Tabs");

    assert_eq!(recipe.trace.action, "generate_script");
    assert_eq!(
        recipe.trace.raw_query,
        "Turn This Into a Command close duplicate tabs"
    );
    assert_eq!(recipe.trace.effective_query, "close duplicate tabs");
    assert_eq!(
        recipe.trace.prompt_preview.as_deref(),
        Some(recipe.prompt.as_str())
    );
    assert_eq!(
        recipe.trace.prompt_receipt,
        Some(recipe.prompt_receipt.clone())
    );

    assert!(recipe
        .prompt
        .contains("User Request:\nclose duplicate tabs"));
    assert!(recipe
        .prompt
        .contains("Selected Text:\n```text\npricing\n```"));
    assert!(recipe
        .prompt
        .contains("Focused Browser URL:\nbrowser-tab-url"));
    assert!(recipe.prompt_receipt.included_user_request);
    assert!(recipe.prompt_receipt.included_selected_text);
    assert!(recipe.prompt_receipt.included_browser_url);
}

#[test]
fn turn_this_into_a_command_recipe_preserves_exact_match_trace_without_overwriting_it() {
    let recipe = build_current_app_command_recipe(
        safari_snapshot_with_menus(),
        Some("Turn This Into a Command new tab"),
        Some("selection"),
        Some("browser-tab-url"),
    );

    assert_eq!(recipe.effective_query, "new tab");
    assert_eq!(recipe.trace.action, "execute_entry");
    assert_eq!(
        recipe
            .trace
            .selected_entry
            .as_ref()
            .map(|item| item.leaf_name.as_str()),
        Some("New Tab")
    );
    assert!(recipe.trace.prompt_preview.is_none());
    assert!(recipe.trace.prompt_receipt.is_none());

    assert!(recipe.prompt.contains("User Request:\nnew tab"));
    assert!(recipe
        .prompt
        .contains("Selected Text:\n```text\nselection\n```"));
    assert!(recipe
        .prompt
        .contains("Focused Browser URL:\nbrowser-tab-url"));
}

#[test]
fn turn_this_into_a_command_recipe_serializes_to_valid_json() {
    let recipe = build_current_app_command_recipe(
        safari_snapshot_with_menus(),
        Some("Turn This Into a Command close duplicate tabs"),
        Some("pricing"),
        Some("browser-tab-url"),
    );

    let json = serde_json::to_string_pretty(&recipe).expect("recipe must serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("JSON must be valid");

    assert_eq!(
        parsed["schemaVersion"],
        CURRENT_APP_COMMAND_RECIPE_SCHEMA_VERSION
    );
    assert_eq!(parsed["recipeType"], "currentAppCommand");
    assert_eq!(parsed["effectiveQuery"], "close duplicate tabs");
    assert_eq!(parsed["suggestedScriptName"], "Safari Close Duplicate Tabs");
    assert_eq!(parsed["trace"]["action"], "generate_script");
}

// ---------------------------------------------------------------------------
// Verify Current App Recipe — parser
// ---------------------------------------------------------------------------

#[test]
fn parse_current_app_command_recipe_json_rejects_empty_input() {
    let error = parse_current_app_command_recipe_json("").unwrap_err();
    assert!(
        error.contains("empty"),
        "Expected empty error, got: {error}"
    );
}

#[test]
fn parse_current_app_command_recipe_json_rejects_invalid_json() {
    let error = parse_current_app_command_recipe_json("not json at all").unwrap_err();
    assert!(
        error.contains("valid JSON"),
        "Expected JSON parse error, got: {error}"
    );
}

#[test]
fn parse_current_app_command_recipe_json_rejects_wrong_recipe_type() {
    let input = r#"{"schemaVersion":1,"recipeType":"contextSnapshot"}"#;
    let error = parse_current_app_command_recipe_json(input).unwrap_err();
    assert!(
        error.contains("currentAppCommand"),
        "Expected recipeType error, got: {error}"
    );
}

#[test]
fn parse_current_app_command_recipe_json_rejects_unsupported_schema_version() {
    let input = r#"{
        "schemaVersion":99,
        "recipeType":"currentAppCommand",
        "rawQuery":"close duplicate tabs",
        "effectiveQuery":"close duplicate tabs",
        "suggestedScriptName":"Safari Close Duplicate Tabs",
        "trace":{
            "schema_version":1,
            "source":"frontmost_menu_bar",
            "app_name":"Safari",
            "bundle_id":"com.apple.Safari",
            "raw_query":"close duplicate tabs",
            "effective_query":"close duplicate tabs",
            "normalized_query":"close duplicate tabs",
            "top_level_menu_count":1,
            "leaf_entry_count":1,
            "filtered_entries":0,
            "exact_matches":0,
            "action":"generate_script",
            "selected_entry":null,
            "candidates":[],
            "prompt_receipt":null,
            "prompt_preview":null
        },
        "promptReceipt":{
            "app_name":"Safari",
            "bundle_id":"com.apple.Safari",
            "total_menu_items":1,
            "included_menu_items":1,
            "included_user_request":true,
            "included_selected_text":false,
            "included_browser_url":false
        },
        "prompt":"Generate a Script Kit script..."
    }"#;

    let error = parse_current_app_command_recipe_json(input).unwrap_err();
    assert!(
        error.contains("Unsupported recipe schema_version"),
        "Expected schema version error, got: {error}"
    );
}

#[test]
fn parse_current_app_command_recipe_json_round_trips_valid_recipe() {
    let recipe = build_current_app_command_recipe(
        safari_snapshot_with_menus(),
        Some("Turn This Into a Command close duplicate tabs"),
        Some("pricing"),
        Some("https://example.com"),
    );

    let json = serde_json::to_string_pretty(&recipe).expect("recipe must serialize");
    let parsed = parse_current_app_command_recipe_json(&json).expect("should parse valid recipe");

    assert_eq!(parsed.schema_version, recipe.schema_version);
    assert_eq!(parsed.recipe_type, recipe.recipe_type);
    assert_eq!(parsed.effective_query, recipe.effective_query);
    assert_eq!(parsed.suggested_script_name, recipe.suggested_script_name);
    assert_eq!(parsed.prompt, recipe.prompt);
}

// ---------------------------------------------------------------------------
// Verify Current App Recipe — verifier
// ---------------------------------------------------------------------------

#[test]
fn verify_current_app_command_recipe_reports_match_when_context_identical() {
    let snapshot = safari_snapshot_with_menus();
    let recipe = build_current_app_command_recipe(
        snapshot.clone(),
        Some("Turn This Into a Command close duplicate tabs"),
        None,
        None,
    );

    let verification = verify_current_app_command_recipe(&recipe, snapshot, None, None);

    assert_eq!(verification.status, "match");
    assert!(verification.app_name_matches);
    assert!(verification.bundle_id_matches);
    assert!(verification.effective_query_matches);
    assert!(verification.route_matches);
    assert!(verification.prompt_matches);
    assert_eq!(verification.warning_count, 0);
    assert!(verification.warnings.is_empty());
}

#[test]
fn verify_current_app_command_recipe_reports_browser_url_drift() {
    let snapshot = safari_snapshot_with_menus();
    let recipe = build_current_app_command_recipe(
        snapshot.clone(),
        Some("Turn This Into a Command close duplicate tabs"),
        None,
        Some("https://example.com"),
    );

    // Verify with browser URL missing
    let verification = verify_current_app_command_recipe(&recipe, snapshot, None, None);

    assert_eq!(verification.status, "drift");
    assert!(verification.browser_url_expected);
    assert!(!verification.browser_url_present);
    assert!(!verification.prompt_matches);
    assert!(verification.warning_count >= 1);

    let warning_text = verification.warnings.join(" ");
    assert!(
        warning_text.contains("browser URL"),
        "Expected browser URL drift warning, got: {:?}",
        verification.warnings
    );
}

#[test]
fn verify_current_app_command_recipe_reports_app_name_drift() {
    let safari_snapshot = safari_snapshot_with_menus();
    let recipe = build_current_app_command_recipe(
        safari_snapshot,
        Some("Turn This Into a Command close duplicate tabs"),
        None,
        None,
    );

    // Verify against a different app
    let different_app = FrontmostMenuSnapshot {
        app_name: "Finder".into(),
        bundle_id: "com.apple.finder".into(),
        items: vec![],
    };

    let verification = verify_current_app_command_recipe(&recipe, different_app, None, None);

    assert_eq!(verification.status, "drift");
    assert!(!verification.app_name_matches);
    assert!(!verification.bundle_id_matches);
    assert_eq!(verification.expected_app_name, "Safari");
    assert_eq!(verification.actual_app_name, "Finder");
    assert!(verification.warning_count >= 2);
}

#[test]
fn build_current_app_command_verification_hud_message_format() {
    use script_kit_gpui::menu_bar::current_app_commands::build_current_app_command_verification_hud_message;

    let snapshot = safari_snapshot_with_menus();
    let recipe = build_current_app_command_recipe(
        snapshot.clone(),
        Some("Turn This Into a Command close duplicate tabs"),
        None,
        None,
    );

    // Match case
    let verification = verify_current_app_command_recipe(&recipe, snapshot.clone(), None, None);
    let msg = build_current_app_command_verification_hud_message(&verification);
    assert!(
        msg.starts_with("Recipe verified:"),
        "Expected 'Recipe verified:' prefix, got: {msg}"
    );

    // Drift case: use a recipe WITH browser URL and verify without it
    let recipe_with_url = build_current_app_command_recipe(
        safari_snapshot_with_menus(),
        Some("Turn This Into a Command close duplicate tabs"),
        None,
        Some("https://example.com"),
    );
    let verification_drift = verify_current_app_command_recipe(
        &recipe_with_url,
        safari_snapshot_with_menus(),
        None,
        None,
    );
    let msg_drift = build_current_app_command_verification_hud_message(&verification_drift);
    assert!(
        msg_drift.starts_with("Recipe drift detected:"),
        "Expected 'Recipe drift detected:' prefix, got: {msg_drift}"
    );
}

#[test]
fn verify_current_app_command_recipe_serializes_to_valid_json() {
    let snapshot = safari_snapshot_with_menus();
    let recipe = build_current_app_command_recipe(
        snapshot.clone(),
        Some("Turn This Into a Command close duplicate tabs"),
        None,
        Some("https://example.com"),
    );

    let verification = verify_current_app_command_recipe(&recipe, snapshot, None, None);

    let json = serde_json::to_string_pretty(&verification).expect("must serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("JSON must be valid");

    assert_eq!(parsed["schemaVersion"], 1);
    assert_eq!(parsed["verificationType"], "currentAppCommandVerification");
    assert_eq!(parsed["status"], "drift");
    assert!(parsed["browserUrlExpected"].as_bool().unwrap_or(false));
    assert!(!parsed["browserUrlPresent"].as_bool().unwrap_or(true));
    assert!(parsed["warningCount"].as_u64().unwrap_or(0) >= 1);
}

// ---------------------------------------------------------------------------
// Recipe → generated script prompt
// ---------------------------------------------------------------------------

#[test]
fn build_current_app_command_recipe_marks_context_flags() {
    let recipe = build_current_app_command_recipe(
        safari_snapshot_with_menus(),
        Some("close duplicate tabs"),
        Some("tab 1\ntab 2"),
        Some("https://example.com"),
    );

    assert_eq!(recipe.recipe_type, "currentAppCommand");
    assert_eq!(recipe.effective_query, "close duplicate tabs");
    assert_eq!(recipe.trace.action, "generate_script");
    assert!(recipe.prompt_receipt.included_user_request);
    assert!(recipe.prompt_receipt.included_selected_text);
    assert!(recipe.prompt_receipt.included_browser_url);
    assert_eq!(recipe.suggested_script_name, "Safari Close Duplicate Tabs");
}

#[test]
fn generated_script_prompt_from_recipe_embeds_contract_and_recipe_header() {
    let recipe = build_current_app_command_recipe(
        safari_snapshot_with_menus(),
        Some("close duplicate tabs"),
        None,
        Some("https://example.com"),
    );

    let prompt = build_generated_script_prompt_from_recipe(&recipe);

    assert!(
        prompt.contains("OUTPUT CONTRACT:"),
        "prompt must include OUTPUT CONTRACT section"
    );
    assert!(
        prompt.contains("Return only runnable Script Kit TypeScript."),
        "prompt must request Script Kit TypeScript"
    );
    assert!(
        prompt.contains("Current-App-Recipe-Base64:"),
        "prompt must embed base64 recipe header"
    );
    assert!(
        prompt.contains("Current-App-Recipe-Name:"),
        "prompt must embed recipe name header"
    );
    assert!(
        prompt.contains("Safari Close Duplicate Tabs"),
        "prompt must include suggested script name"
    );
    assert!(
        prompt.contains("Bias toward direct menu-command automation"),
        "prompt must bias toward menu automation"
    );
}

#[test]
fn generated_script_prompt_base64_roundtrips_recipe() {
    use base64::Engine as _;

    let recipe = build_current_app_command_recipe(
        safari_snapshot_with_menus(),
        Some("close duplicate tabs"),
        Some("selected text here"),
        Some("https://example.com"),
    );

    let prompt = build_generated_script_prompt_from_recipe(&recipe);

    // Extract base64 from the prompt
    let base64_prefix = "Current-App-Recipe-Base64: ";
    let base64_start = prompt
        .find(base64_prefix)
        .expect("prompt must contain base64 prefix")
        + base64_prefix.len();
    let base64_end = prompt[base64_start..]
        .find('\n')
        .expect("base64 line must end with newline")
        + base64_start;
    let base64_str = &prompt[base64_start..base64_end];

    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_str)
        .expect("base64 must decode");
    let decoded_json: serde_json::Value =
        serde_json::from_slice(&decoded_bytes).expect("decoded bytes must be valid JSON");

    assert_eq!(decoded_json["recipeType"], "currentAppCommand");
    assert_eq!(decoded_json["effectiveQuery"], "close duplicate tabs");
    assert_eq!(
        decoded_json["suggestedScriptName"],
        "Safari Close Duplicate Tabs"
    );
}

// ---------------------------------------------------------------------------
// Ranking: DoInCurrentApp outranks weaker generation paths
// ---------------------------------------------------------------------------

/// Helper: run fuzzy search and return the position of a builtin by id.
fn rank_of(entries: &[builtins::BuiltInEntry], query: &str, target_id: &str) -> Option<usize> {
    let matches = script_kit_gpui::scripts::fuzzy_search_builtins(entries, query);
    matches.iter().position(|m| m.entry.id == target_id)
}

#[test]
fn do_in_current_app_outranks_generate_script_from_current_app_for_intent_queries() {
    let entries = builtins::get_builtin_entries(&BuiltInConfig::default());

    let do_in_id = "builtin/do-in-current-app";
    let gen_app_id = "builtin/generate-script-from-current-app";

    // For intent-oriented queries, DoInCurrentApp (recipe-backed) must
    // outrank GenerateScriptFromCurrentApp (weaker raw-prompt path).
    // These overlap on keywords; DoInCurrentApp wins because it has
    // more keyword hits and its name/description match "automation" etc.
    let intent_queries = vec![
        "automation",
        "automate",
        "current",
        "intent",
        "execute",
        "do",
        "shortcut",
    ];

    for query in &intent_queries {
        let do_in_rank = rank_of(&entries, query, do_in_id);
        let gen_app_rank = rank_of(&entries, query, gen_app_id);

        assert!(
            do_in_rank.is_some(),
            "DoInCurrentApp must match query '{query}'"
        );

        if let (Some(do_pos), Some(gen_app_pos)) = (do_in_rank, gen_app_rank) {
            assert!(
                do_pos <= gen_app_pos,
                "DoInCurrentApp (rank {do_pos}) must rank <= GenerateScriptFromCurrentApp (rank {gen_app_pos}) for query '{query}'"
            );
        }
    }
}

#[test]
fn do_in_current_app_matches_all_generation_keywords() {
    let entries = builtins::get_builtin_entries(&BuiltInConfig::default());

    // After keyword promotion, DoInCurrentApp must match every keyword
    // that GenerateScript or GenerateScriptFromCurrentApp would match.
    let generation_keywords = vec![
        "generate",
        "ai",
        "create",
        "code",
        "script",
        "context",
        "browser",
        "selection",
        "automation",
        "menu",
        "frontmost",
    ];

    for keyword in &generation_keywords {
        let pos = rank_of(&entries, keyword, "builtin/do-in-current-app");
        assert!(
            pos.is_some(),
            "DoInCurrentApp must match keyword '{keyword}'"
        );
    }
}

#[test]
fn do_in_current_app_matches_context_and_selection_keywords() {
    let entries = builtins::get_builtin_entries(&BuiltInConfig::default());

    // These keywords previously only matched GenerateScriptFromCurrentApp.
    // After promotion, DoInCurrentApp must also match them.
    for query in &["context", "browser", "selection"] {
        let pos = rank_of(&entries, query, "builtin/do-in-current-app");
        assert!(pos.is_some(), "DoInCurrentApp must match keyword '{query}'");
    }
}

#[test]
fn unrelated_builtins_ranking_stable_after_promotion() {
    let entries = builtins::get_builtin_entries(&BuiltInConfig::default());

    // Clipboard History should still rank first for "clipboard"
    let matches = script_kit_gpui::scripts::fuzzy_search_builtins(&entries, "clipboard");
    assert!(!matches.is_empty(), "clipboard query must return results");
    assert_eq!(
        matches[0].entry.id, "builtin/clipboard-history",
        "Clipboard History must still be the top result for 'clipboard'"
    );

    // Scratch Pad should still rank first for "scratch"
    let matches = script_kit_gpui::scripts::fuzzy_search_builtins(&entries, "scratch");
    assert!(!matches.is_empty(), "scratch query must return results");
    assert_eq!(
        matches[0].entry.id, "builtin/scratch-pad",
        "Scratch Pad must still be the top result for 'scratch'"
    );
}
