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
    current_app_commands_launcher_label, current_app_commands_session_identity_changed,
    effective_do_in_current_app_query_for_submission, normalize_do_in_current_app_labeled_request,
    normalize_do_in_current_app_request, normalize_trace_current_app_intent_request,
    normalize_turn_this_into_a_command_request, parse_current_app_command_recipe_json,
    resolve_do_in_current_app_intent, suggest_current_app_command_name,
    verify_current_app_command_recipe, CurrentAppCommandsLiveIdentity, CurrentAppCommandsSession,
    DoInCurrentAppAction, FrontmostMenuSnapshot, CURRENT_APP_COMMAND_RECIPE_SCHEMA_VERSION,
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
fn current_app_commands_launcher_label_names_tracked_app_without_changing_stable_id() {
    assert_eq!(
        current_app_commands_launcher_label(Some("Safari")),
        "Safari Commands"
    );
    assert_eq!(
        current_app_commands_launcher_label(Some("  Finder  ")),
        "Finder Commands"
    );
    assert_eq!(
        current_app_commands_launcher_label(Some("")),
        "App Commands"
    );
    assert_eq!(current_app_commands_launcher_label(None), "App Commands");

    let entries = builtins::get_builtin_entries(&BuiltInConfig::default());
    let entry = entries
        .iter()
        .find(|e| e.id == "builtin/do-in-current-app")
        .expect("builtin/do-in-current-app must stay the stable launcher identity");
    assert_eq!(entry.name, "Do in Current App");
}

#[test]
fn root_launcher_renames_current_app_commands_row_from_frontmost_app_snapshot() {
    let source = std::fs::read_to_string("src/app_impl/filtering_cache.rs")
        .expect("must read filtering cache source");
    let compacted: String = source.chars().filter(|ch| !ch.is_whitespace()).collect();

    assert!(
        compacted.contains("current_app_commands_launcher_label(Some(app_name)"),
        "root launcher filtering should derive the visible current-app commands label from the tracked app"
    );
    assert!(
        compacted.contains("entry.id==\"builtin/do-in-current-app\""),
        "dynamic current-app relabeling must preserve the stable builtin/do-in-current-app id"
    );
    assert!(
        compacted.contains("entry.name=commands_label;"),
        "dynamic current-app relabeling should affect only the visible row name"
    );
    assert!(
        compacted.contains("entry.id==\"builtin/dictation\"")
            && compacted.contains("entry.name=format!(\"Dictateto{app_name}\");")
            && compacted.contains("entry.description=format!(\"Voicedictationfor{app_name}\");"),
        "root launcher filtering should derive the visible dictation row from the same tracked app"
    );
}

#[test]
fn do_in_current_app_current_command_alias_clears_palette_filter_without_clearing_plain_text() {
    assert_eq!(
        normalize_do_in_current_app_request(Some("Do in Current Command")),
        None
    );
    assert_eq!(normalize_do_in_current_app_request(Some("do")), Some("do"));
    assert_eq!(
        normalize_do_in_current_app_request(Some("do in")),
        Some("do in")
    );
    assert_eq!(
        normalize_do_in_current_app_request(Some("do in current")),
        Some("do in current")
    );
    assert_eq!(
        normalize_do_in_current_app_request(Some("Do in Current Commands")),
        None
    );
    assert_eq!(
        normalize_do_in_current_app_request(Some("Do in Current Command: close tab")),
        Some("close tab")
    );
}

#[test]
fn do_in_current_app_submission_clears_any_plain_launcher_filter_when_switching_lists() {
    for launcher_filter in ["do", "do in", "do in current", "automation", "close tab"] {
        assert_eq!(
            effective_do_in_current_app_query_for_submission(launcher_filter, None),
            "",
            "plain launcher filter '{launcher_filter}' should not prefill CurrentAppCommandsView"
        );
    }

    assert_eq!(
        normalize_do_in_current_app_labeled_request(Some("Do in Current Command: close tab")),
        Some("close tab")
    );
    for malformed_label_prefix in [
        "Do in Current Appclose tab",
        "Do in Current Application close",
        "Current App CommandsX close tab",
        "Do in Current Commandments: close",
    ] {
        assert_eq!(
            normalize_do_in_current_app_labeled_request(Some(malformed_label_prefix)),
            None,
            "malformed label prefix '{malformed_label_prefix}' must not prefill CurrentAppCommandsView"
        );
        assert_eq!(
            effective_do_in_current_app_query_for_submission(malformed_label_prefix, None),
            "",
            "malformed label prefix '{malformed_label_prefix}' should behave like ordinary launcher text"
        );
    }
    assert_eq!(
        effective_do_in_current_app_query_for_submission("Do in Current Command: close tab", None,),
        "close tab"
    );
    assert_eq!(
        effective_do_in_current_app_query_for_submission("close tab", Some("close tab")),
        "close tab",
        "explicit query overrides from programmatic paths should still prefill/route"
    );
}

#[test]
fn do_in_current_app_execution_uses_effective_query_for_list_switching() {
    let source = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("must read builtin execution source");
    let helper_start = source
        .find("fn execute_utility_do_in_current_app_builtin(")
        .expect("DoInCurrentApp execution helper must exist");
    let helper_body = &source[helper_start..];
    let helper_end = helper_body
        .find("fn execute_permission_command_builtin(")
        .expect("next builtin helper must exist");
    let arm_body = &helper_body[..helper_end];
    let compacted_arm: String = arm_body.chars().filter(|ch| !ch.is_whitespace()).collect();

    assert!(
        compacted_arm.contains("effective_do_in_current_app_query_for_submission("),
        "DoInCurrentApp must derive a boundary-safe effective query before routing"
    );
    assert!(
        compacted_arm
            .contains("resolve_do_in_current_app_intent(&entries,effective_query_for_router,"),
        "DoInCurrentApp must route using the effective query, not the ScriptList filter"
    );
    assert!(
        compacted_arm
            .contains("self.present_current_app_commands_entries(entries,&snapshot_receipt,snapshot_pid,&effective_query,"),
        "CurrentAppCommandsView must open with the effective query"
    );
    assert!(
        !compacted_arm.contains("do_in_current_app_empty_snapshot"),
        "DoInCurrentApp should present the empty CurrentAppCommandsView rather than erroring before the list switch"
    );
    assert!(
        !compacted_arm
            .contains("resolve_do_in_current_app_intent(&entries,Some(&raw_query_owned),"),
        "raw ScriptList filter text must not feed DoIn routing"
    );
}

#[test]
fn current_app_commands_presentation_resets_scroll_to_top() {
    let source = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("must read builtin execution source");
    let fn_start = source
        .find("pub(crate) fn present_current_app_commands_session(")
        .expect("present_current_app_commands_session must exist");
    let fn_body = &source[fn_start..];
    let fn_end = fn_body
        .find("pub(crate) fn open_current_app_commands_from_tray(")
        .expect("next function must exist");
    let fn_body = &fn_body[..fn_end];
    let compacted: String = fn_body.chars().filter(|ch| !ch.is_whitespace()).collect();

    assert!(
        compacted.contains(
            "current_app_commands_scroll_handle.scroll_to_item(0,gpui::ScrollStrategy::Top);"
        ),
        "opening CurrentAppCommandsView should reset the shared scroll handle to row 0"
    );
}

#[test]
fn current_app_commands_presentation_opens_mini_filterable_view() {
    let source = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("must read builtin execution source");
    let fn_start = source
        .find("pub(crate) fn present_current_app_commands_session(")
        .expect("present_current_app_commands_session must exist");
    let fn_body = &source[fn_start..];
    let fn_end = fn_body
        .find("pub(crate) fn open_current_app_commands_from_tray(")
        .expect("next function must exist");
    let fn_body = &fn_body[..fn_end];
    let compacted: String = fn_body.chars().filter(|ch| !ch.is_whitespace()).collect();

    assert!(
        compacted.contains(
            "self.open_builtin_filterable_view_with_filter(AppView::CurrentAppCommandsView{filter:filter.to_string(),selected_index:0,},filter,&session.placeholder,false,cx,);"
        ),
        "present_current_app_commands_session must open CurrentAppCommandsView through the shared Mini helper path"
    );
    assert!(
        !compacted.contains("resize_to_view_sync(ViewType::ScriptList"),
        "present_current_app_commands_session must not directly request the wide ScriptList mode"
    );
}

#[test]
fn current_app_commands_view_executes_via_guarded_helper() {
    let source = std::fs::read_to_string("src/render_builtins/current_app_commands.rs")
        .expect("must read current_app_commands renderer");
    assert!(
        source.contains("execute_selected_current_app_command("),
        "current app commands view must execute through the guarded helper"
    );
    assert!(
        source.contains("let original_entry_index = *orig_idx;")
            && source.contains("execute_selected_current_app_command(")
            && source.contains("original_entry_index,"),
        "current app commands clicks must execute using the original cached entry index"
    );
}

#[test]
fn builtin_execution_tracks_current_app_commands_session_switches() {
    let source = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("must read builtin execution source");
    assert!(
        source.contains("\"current_app_commands.session_switched\""),
        "builtin execution must log current-app session switches"
    );
    assert!(
        source.contains("load_live_current_app_commands_identity()"),
        "builtin execution must consult the live current-app identity before refreshing"
    );
    assert!(
        source.contains("present_current_app_commands_session("),
        "builtin execution must present a current-app session"
    );
    assert!(
        source.contains("invalidate_current_app_commands_session("),
        "builtin execution must invalidate stale current-app sessions when refresh fails"
    );
}

#[test]
fn app_state_persists_current_app_commands_session_metadata() {
    let source = std::fs::read_to_string("src/main_sections/app_state.rs")
        .expect("must read app state source");
    assert!(
        source.contains("current_app_commands_session"),
        "app state must retain current-app session metadata"
    );
}

#[test]
fn trace_current_app_intent_is_collapsed_into_do_in_current_app() {
    let entries = builtins::get_builtin_entries(&BuiltInConfig::default());
    assert!(
        entries
            .iter()
            .all(|e| e.id != "builtin/trace-current-app-intent"),
        "trace-current-app-intent is no longer a standalone launcher builtin"
    );

    let do_entry = entries
        .iter()
        .find(|e| e.id == "builtin/do-in-current-app")
        .expect("builtin/do-in-current-app must stay registered");
    assert_eq!(
        do_entry.feature,
        BuiltInFeature::UtilityCommand(UtilityCommandType::DoInCurrentApp)
    );
    assert!(
        do_entry.keywords.iter().any(|keyword| keyword == "intent")
            && do_entry
                .keywords
                .iter()
                .any(|keyword| keyword == "automation"),
        "Do in Current App owns the current-app intent/automation launcher vocabulary"
    );

    let menu_bar_source = std::fs::read_to_string("src/menu_bar/current_app_commands.rs")
        .expect("must read current-app commands source");
    assert!(
        menu_bar_source.contains("build_current_app_intent_trace_receipt")
            && menu_bar_source.contains("normalize_trace_current_app_intent_request"),
        "trace receipt helpers remain in the current-app command domain even without a standalone builtin"
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
        pid: 42,
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
        pid: 42,
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
fn keyboard_shortcut_decodes_ax_modifiers_with_implicit_command() {
    let command_only = KeyboardShortcut::from_ax_values("T", 0);
    assert_eq!(command_only.to_display_string(), "⌘T");

    let command_shift_option_control = KeyboardShortcut::from_ax_values("T", 1 | 2 | 4);
    assert_eq!(command_shift_option_control.to_display_string(), "⌃⌥⇧⌘T");

    let no_command_option = KeyboardShortcut::from_ax_values("T", 2 | 8);
    assert_eq!(no_command_option.to_display_string(), "⌥T");

    let legacy_carbon_command_shift = KeyboardShortcut::from_ax_values("T", 256 | 512);
    assert_eq!(legacy_carbon_command_shift.to_display_string(), "⇧⌘T");
}

#[test]
fn keyboard_shortcut_falls_back_to_virtual_key_and_searchable_special_key_tokens() {
    let shortcut = KeyboardShortcut::from_ax_components(None, Some(123), Some(0))
        .expect("left-arrow virtual key should decode");
    assert_eq!(shortcut.to_display_string(), "⌘←");

    let shortcut = KeyboardShortcut::from_ax_values(" ", 0);
    assert_eq!(shortcut.to_display_string(), "⌘Space");

    let tokens = builtins::shortcut_search_tokens(&shortcut.to_display_string());
    assert!(tokens.contains(&"cmdspace".to_string()));
    assert!(tokens.contains(&"cmd space".to_string()));
    assert!(tokens.contains(&"cmd+space".to_string()));
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
        pid: 42,
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
        pid: 42,
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
        pid: 84,
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
fn current_app_session_keeps_same_bundle_same_pid() {
    let session = CurrentAppCommandsSession {
        pid: 100,
        app_name: "Safari".into(),
        bundle_id: "com.apple.Safari".into(),
        placeholder: "Search Safari commands…".into(),
        top_level_menu_count: 4,
        leaf_entry_count: 12,
        source: "frontmost_app_tracker",
        entries: vec![],
    };
    let live = CurrentAppCommandsLiveIdentity {
        pid: 100,
        bundle_id: "com.apple.Safari".into(),
    };

    assert!(
        !current_app_commands_session_identity_changed(&session, Some(&live)),
        "same bundle + same pid should keep the existing session"
    );
}

#[test]
fn current_app_session_detects_same_bundle_different_pid() {
    let session = CurrentAppCommandsSession {
        pid: 100,
        app_name: "Safari".into(),
        bundle_id: "com.apple.Safari".into(),
        placeholder: "Search Safari commands…".into(),
        top_level_menu_count: 4,
        leaf_entry_count: 12,
        source: "frontmost_app_tracker",
        entries: vec![],
    };
    let live = CurrentAppCommandsLiveIdentity {
        pid: 200,
        bundle_id: "com.apple.Safari".into(),
    };

    assert!(
        current_app_commands_session_identity_changed(&session, Some(&live)),
        "same bundle + different pid must invalidate or refresh the session"
    );
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
fn generated_script_prompt_from_recipe_embeds_contract_without_recipe_headers() {
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
        prompt.contains("Write the captured app names, menu labels, URLs, and other values directly in the code where they are used."),
        "prompt must require inline captured values"
    );
    assert!(
        prompt.contains(
            "Do not add machine-readable recipe headers or encoded metadata blocks to the script."
        ),
        "prompt must forbid machine-readable recipe headers"
    );
    assert!(
        prompt.contains("Bias toward direct menu-command automation"),
        "prompt must bias toward menu automation"
    );
}

#[test]
fn generated_script_prompt_keeps_recipe_context_in_plain_text() {
    let recipe = build_current_app_command_recipe(
        safari_snapshot_with_menus(),
        Some("close duplicate tabs"),
        Some("selected text here"),
        Some("https://example.com"),
    );

    let prompt = build_generated_script_prompt_from_recipe(&recipe);

    assert_eq!(recipe.suggested_script_name, "Safari Close Duplicate Tabs");
    assert!(prompt.contains("close duplicate tabs"));
    assert!(prompt.contains("selected text here"));
    assert!(prompt.contains("https://example.com"));
    assert!(!prompt.contains("Current-App-Recipe-Base64:"));
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
