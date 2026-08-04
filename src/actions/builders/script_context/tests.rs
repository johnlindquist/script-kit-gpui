use super::*;

fn find_action_title(actions: &[Action], id: &str) -> String {
    actions
        .iter()
        .find(|action| action.id == id)
        .map(|action| action.title.clone())
        .expect("action id should exist in script context actions")
}

fn find_action_description(actions: &[Action], id: &str) -> Option<String> {
    actions
        .iter()
        .find(|action| action.id == id)
        .and_then(|action| action.description.clone())
}

fn has_action(actions: &[Action], id: &str) -> bool {
    actions.iter().any(|action| action.id == id)
}

fn assert_all_actions_have_icons(context: &str, actions: &[Action]) {
    for action in actions {
        assert!(
            action.icon.is_some(),
            "context '{context}' action '{}' should include an icon",
            action.id
        );
    }
}

#[test]
fn test_get_script_context_actions_returns_empty_when_name_is_blank() {
    let mut script = ScriptInfo::new("Valid", "/tmp/valid.ts");
    script.name = "   ".to_string();

    let actions = get_script_context_actions(&script);
    assert!(actions.is_empty());
}

#[test]
fn test_get_script_context_actions_returns_empty_when_action_verb_is_blank() {
    let mut script = ScriptInfo::new("Valid", "/tmp/valid.ts");
    script.action_verb = "   ".to_string();

    let actions = get_script_context_actions(&script);
    assert!(actions.is_empty());
}

#[test]
fn test_get_script_context_actions_run_label_uses_title_case_verb() {
    let mut script = ScriptInfo::new("Valid", "/tmp/valid.ts");
    script.action_verb = "switch to".to_string();

    let actions = get_script_context_actions(&script);

    assert_eq!(find_action_title(&actions, "run_script"), "Switch To");
}

#[test]
fn test_get_script_context_actions_preserves_builtin_action_text() {
    let script = ScriptInfo::with_action_verb(
        "Agent Chat",
        "builtin:builtin/ai-chat",
        false,
        "Open Agent Chat",
    );

    let actions = get_script_context_actions(&script);

    assert_eq!(find_action_title(&actions, "run_script"), "Open Agent Chat");
    assert_eq!(
        find_action_description(&actions, "run_script").as_deref(),
        Some("Open Agent Chat")
    );
}

#[test]
fn test_primary_action_plan_classifies_context_kind_matrix() {
    let app = ScriptInfo::app("App", "/Applications/App.app", None, None, None);
    let agent = ScriptInfo::agent("Agent", "/tmp/agent.md", None, None);
    let scriptlet = ScriptInfo::scriptlet("Scriptlet", "/tmp/scriptlets.md", None, None);
    let script = ScriptInfo::new("Script", "/tmp/script.ts");
    let skill = ScriptInfo::with_action_verb("Skill", "skill:scriptkit:demo", false, "open");
    let builtin = ScriptInfo::with_action_verb(
        "Agent Chat",
        "builtin:builtin/ai-chat",
        false,
        "Open Agent Chat",
    );
    let generic = ScriptInfo::with_action_verb("Thing", "virtual:thing", false, "open");
    let typed_builtin_path =
        ScriptInfo::with_action_verb("Typed", "builtin:builtin/new-script", true, "run");

    assert_eq!(script_context_kind(&app), ScriptContextKind::App);
    assert_eq!(script_context_kind(&agent), ScriptContextKind::Agent);
    assert_eq!(
        script_context_kind(&scriptlet),
        ScriptContextKind::Scriptlet
    );
    assert_eq!(script_context_kind(&script), ScriptContextKind::Script);
    assert_eq!(script_context_kind(&skill), ScriptContextKind::Skill);
    assert_eq!(script_context_kind(&builtin), ScriptContextKind::BuiltIn);
    assert_eq!(script_context_kind(&generic), ScriptContextKind::Generic);
    assert_eq!(
        script_context_kind(&typed_builtin_path),
        ScriptContextKind::Script
    );
}

#[test]
fn test_primary_action_copy_matrix() {
    let mut script = ScriptInfo::new("Script", "/tmp/script.ts");
    script.action_verb = "run".to_string();
    let mut app = ScriptInfo::app("App", "/Applications/App.app", None, None, None);
    app.action_verb = "launch".to_string();
    let mut scriptlet = ScriptInfo::scriptlet("Scriptlet", "/tmp/scriptlets.md", None, None);
    scriptlet.action_verb = "run".to_string();
    let mut agent = ScriptInfo::agent("Agent", "/tmp/agent.md", None, None);
    agent.action_verb = "open".to_string();
    let skill = ScriptInfo::with_action_verb("Skill", "skill:scriptkit:demo", false, "open");
    let generic = ScriptInfo::with_action_verb("Thing", "virtual:thing", false, "open");
    let builtin = ScriptInfo::with_action_verb(
        "Agent Chat",
        "builtin:builtin/ai-chat",
        false,
        "Open Agent Chat",
    );

    for (context, script, expected) in [
        (
            "script",
            script,
            PrimaryActionCopy {
                title: "Run".to_string(),
                description: "run this script".to_string(),
            },
        ),
        (
            "app",
            app,
            PrimaryActionCopy {
                title: "Launch".to_string(),
                description: "launch this application".to_string(),
            },
        ),
        (
            "scriptlet",
            scriptlet,
            PrimaryActionCopy {
                title: "Run".to_string(),
                description: "run this scriptlet".to_string(),
            },
        ),
        (
            "agent",
            agent,
            PrimaryActionCopy {
                title: "Open".to_string(),
                description: "open this agent".to_string(),
            },
        ),
        (
            "skill",
            skill,
            PrimaryActionCopy {
                title: "Open".to_string(),
                description: "open this skill".to_string(),
            },
        ),
        (
            "generic",
            generic,
            PrimaryActionCopy {
                title: "Open".to_string(),
                description: "open this item".to_string(),
            },
        ),
        (
            "builtin",
            builtin,
            PrimaryActionCopy {
                title: "Open Agent Chat".to_string(),
                description: "Open Agent Chat".to_string(),
            },
        ),
    ] {
        assert_eq!(primary_action_copy(&script), expected, "{context}");
    }
}

#[test]
fn test_favorite_action_copy_returns_add_copy_when_not_favorite() {
    let (title, description) = favorite_action_copy(false);

    assert_eq!(title, "Add to Favorites");
    assert_eq!(description, "Save this item to your favorites list");
}

#[test]
fn test_favorite_action_copy_returns_remove_copy_when_favorite() {
    let (title, description) = favorite_action_copy(true);

    assert_eq!(title, "Remove from Favorites");
    assert_eq!(description, "Remove this item from your favorites list");
}

#[test]
fn test_get_script_context_actions_includes_toggle_favorite_for_script_items() {
    let script = ScriptInfo::new("Valid", "/tmp/script-context-favorites-test.ts");

    let actions = get_script_context_actions(&script);

    assert!(has_action(&actions, "toggle_favorite"));
}

#[test]
fn test_get_script_context_actions_skips_toggle_favorite_for_builtin_items() {
    let script = ScriptInfo::builtin("Clipboard History");

    let actions = get_script_context_actions(&script);

    assert!(!has_action(&actions, "toggle_favorite"));
}

#[test]
fn test_get_script_context_actions_labels_use_consistent_verb_style() {
    let mut script = ScriptInfo::new("Valid", "/tmp/valid.ts");
    script.shortcut = Some("cmd-shift-k".to_string());
    script.alias = Some("v".to_string());
    script.is_suggested = true;

    let actions = get_script_context_actions(&script);

    assert_eq!(find_action_title(&actions, "run_script"), "Run");
    assert_eq!(
        find_action_title(&actions, "update_shortcut"),
        "Edit Keyboard Shortcut"
    );
    assert_eq!(
        find_action_title(&actions, "remove_shortcut"),
        "Delete Keyboard Shortcut"
    );
    assert_eq!(find_action_title(&actions, "update_alias"), "Edit Alias");
    assert_eq!(find_action_title(&actions, "remove_alias"), "Delete Alias");
    assert_eq!(find_action_title(&actions, "view_logs"), "Show Logs");
    assert_eq!(
        find_action_title(&actions, "reveal_in_finder"),
        "Open in Finder"
    );
    assert_eq!(find_action_title(&actions, "copy_deeplink"), "Share");
    assert_eq!(
        find_action_title(&actions, "reset_ranking"),
        "Delete Ranking Entry"
    );

    for action in &actions {
        assert!(
            !action.title.ends_with("..."),
            "label should not end with ellipsis: {}",
            action.title
        );
        assert!(
            action.title.chars().count() < 30,
            "label should stay concise: {}",
            action.title
        );
    }
}

#[test]
fn test_get_script_context_actions_assigns_icons_for_all_contexts() {
    let script = ScriptInfo::new("Script", "/tmp/script-context-icon-test.ts");
    let builtin = ScriptInfo::builtin("Clipboard History");
    let scriptlet =
        ScriptInfo::scriptlet("Scriptlet", "/tmp/script-context-icon-test.md", None, None);
    let agent = ScriptInfo::agent(
        "Agent",
        "/tmp/script-context-icon-test.agent.md",
        None,
        None,
    );

    let script_actions = get_script_context_actions(&script);
    assert!(
        !script_actions.is_empty(),
        "script actions should not be empty"
    );
    assert_all_actions_have_icons("script", &script_actions);

    let builtin_actions = get_script_context_actions(&builtin);
    assert!(
        !builtin_actions.is_empty(),
        "builtin actions should not be empty"
    );
    assert_all_actions_have_icons("builtin", &builtin_actions);

    let scriptlet_actions = get_script_context_actions(&scriptlet);
    assert!(
        !scriptlet_actions.is_empty(),
        "scriptlet actions should not be empty"
    );
    assert_all_actions_have_icons("scriptlet", &scriptlet_actions);

    let agent_actions = get_script_context_actions(&agent);
    assert!(
        !agent_actions.is_empty(),
        "agent actions should not be empty"
    );
    assert_all_actions_have_icons("agent", &agent_actions);
}

#[test]
fn test_script_context_actions_include_toggle_info_with_cmd_i() {
    let script = ScriptInfo::new("TestScript", "/tmp/info-test.ts");
    let actions = get_script_context_actions(&script);

    let info_action = actions
        .iter()
        .find(|a| a.id == "toggle_info")
        .expect("script context actions must include toggle_info");

    assert_eq!(info_action.title, "Show Info");
    assert_eq!(
        info_action.shortcut.as_deref(),
        Some("⌘I"),
        "toggle_info action must have ⌘I shortcut for discoverability"
    );
    assert_eq!(
        info_action.section.as_deref(),
        Some("Actions"),
        "toggle_info must appear in the Actions section"
    );
    assert!(
        info_action.icon.is_some(),
        "toggle_info must have an icon for visual consistency"
    );
}

#[test]
fn test_get_script_context_actions_includes_app_actions_when_is_app() {
    let script = ScriptInfo::app(
        "Google Chrome",
        "/Applications/Google Chrome.app",
        Some("com.google.Chrome".to_string()),
        None,
        None,
    );
    let actions = get_script_context_actions(&script);

    // App-specific actions
    assert!(has_action(&actions, "reveal_in_finder"));
    assert!(has_action(&actions, "show_info_in_finder"));
    assert!(has_action(&actions, "show_package_contents"));
    assert!(has_action(&actions, "copy_name"));
    assert!(has_action(&actions, "copy_path"));
    assert!(has_action(&actions, "copy_bundle_id"));
    assert!(has_action(&actions, "quit_app"));
    assert!(has_action(&actions, "force_quit_app"));
    assert!(has_action(&actions, "restart_app"));

    // Common actions still present
    assert!(has_action(&actions, "run_script"));
    assert!(has_action(&actions, "toggle_info"));
    assert!(has_action(&actions, "copy_deeplink"));
}

#[test]
fn builtin_rows_include_copy_command_id_but_not_script_or_app_actions() {
    let script = ScriptInfo::with_all(
        "Clipboard History",
        "builtin:clipboard-history",
        false,
        "Open Clipboard History",
        None,
        None,
    );
    let actions = get_script_context_actions(&script);

    assert!(has_action(&actions, "copy_command_id"));
    assert!(has_action(&actions, "run_script"));
    assert!(has_action(&actions, "toggle_info"));
    assert!(!has_action(&actions, "edit_script"));
    assert!(!has_action(&actions, "reveal_in_finder"));
    assert!(!has_action(&actions, "quit_app"));
}

#[test]
fn test_get_script_context_actions_omits_copy_bundle_id_when_none() {
    let script = ScriptInfo::app("MyApp", "/Applications/MyApp.app", None, None, None);
    let actions = get_script_context_actions(&script);

    assert!(!has_action(&actions, "copy_bundle_id"));
    // Other app actions still present
    assert!(has_action(&actions, "reveal_in_finder"));
    assert!(has_action(&actions, "quit_app"));
}

#[test]
fn test_get_script_context_actions_app_does_not_include_script_only_actions() {
    let script = ScriptInfo::app(
        "Safari",
        "/Applications/Safari.app",
        Some("com.apple.Safari".to_string()),
        None,
        None,
    );
    let actions = get_script_context_actions(&script);

    assert!(!has_action(&actions, "edit_script"));
    assert!(!has_action(&actions, "view_logs"));
    assert!(!has_action(&actions, "copy_content"));
    assert!(!has_action(&actions, "delete_script"));
    assert!(!has_action(&actions, "edit_scriptlet"));
}

#[test]
fn test_get_script_context_actions_includes_favorites_for_apps() {
    let script = ScriptInfo::app(
        "Safari",
        "/Applications/Safari.app",
        Some("com.apple.Safari".to_string()),
        None,
        None,
    );
    let actions = get_script_context_actions(&script);

    assert!(has_action(&actions, "toggle_favorite"));
}

#[test]
fn test_get_script_context_actions_app_actions_all_have_icons() {
    let script = ScriptInfo::app(
        "Chrome",
        "/Applications/Google Chrome.app",
        Some("com.google.Chrome".to_string()),
        None,
        None,
    );
    let actions = get_script_context_actions(&script);
    assert_all_actions_have_icons("app", &actions);
}

#[test]
fn test_toggle_info_appears_for_all_script_types() {
    let script = ScriptInfo::new("Script", "/tmp/all-types-info.ts");
    let builtin = ScriptInfo::builtin("Clipboard History");
    let scriptlet = ScriptInfo::scriptlet("Scriptlet", "/tmp/all-types-info.md", None, None);
    let agent = ScriptInfo::agent("Agent", "/tmp/all-types-info.agent.md", None, None);

    for (label, actions) in [
        ("script", get_script_context_actions(&script)),
        ("builtin", get_script_context_actions(&builtin)),
        ("scriptlet", get_script_context_actions(&scriptlet)),
        ("agent", get_script_context_actions(&agent)),
    ] {
        assert!(
            actions.iter().any(|a| a.id == "toggle_info"),
            "toggle_info must be present in {label} context actions"
        );
    }
}

fn default_agent_chat_command_facts(
) -> crate::components::conversation_actions::AgentChatConversationCommandFacts {
    crate::components::conversation_actions::AgentChatConversationCommandFacts {
        has_response: true,
        ..Default::default()
    }
}

#[test]
fn agent_chat_actions_project_the_live_primary_command_binding() {
    let idle = get_agent_chat_actions_with_facts(default_agent_chat_command_facts());
    let send = idle
        .iter()
        .find(|action| action.id == "agent_chat_send")
        .expect("idle Agent Chat must project its Send descriptor into Actions");
    assert_eq!(send.shortcut.as_deref(), Some("↵"));
    assert_eq!(send.disabled_reason(), Some("Type a message first."));
    assert!(!idle.iter().any(|action| action.id == "agent_chat_stop"));

    let streaming = get_agent_chat_actions_with_facts(
        crate::components::conversation_actions::AgentChatConversationCommandFacts {
            response_in_progress: true,
            ..default_agent_chat_command_facts()
        },
    );
    let stop = streaming
        .iter()
        .find(|action| action.id == "agent_chat_stop")
        .expect("streaming Agent Chat must project its Stop descriptor into Actions");
    assert_eq!(stop.shortcut.as_deref(), Some("⌘."));
    assert_eq!(stop.disabled_reason(), None);
    assert!(!streaming
        .iter()
        .any(|action| action.id == "agent_chat_send"));
}

fn sample_agent_chat_model(
    id: &str,
    display_name: &str,
) -> crate::ai::agent_chat::ui::config::AgentChatModelEntry {
    crate::ai::agent_chat::ui::config::AgentChatModelEntry {
        id: id.to_string(),
        display_name: Some(display_name.to_string()),
        context_window: None,
    }
}

#[test]
fn test_agent_chat_close_shortcut_is_only_advertised_for_detached_host() {
    let shared = get_agent_chat_root_route_for_host(
        &[],
        None,
        0,
        &[],
        &[],
        default_agent_chat_command_facts(),
        AgentChatActionsDialogHost::Shared,
    );
    let notes = get_agent_chat_root_route_for_host(
        &[],
        None,
        0,
        &[],
        &[],
        default_agent_chat_command_facts(),
        AgentChatActionsDialogHost::Notes,
    );
    let detached = get_agent_chat_root_route_for_host(
        &[],
        None,
        0,
        &[],
        &[],
        default_agent_chat_command_facts(),
        AgentChatActionsDialogHost::Detached,
    );

    let shared_close = shared
        .actions
        .iter()
        .find(|action| action.id == "agent_chat_close")
        .expect("shared agent_chat_close action should exist");
    let notes_close = notes
        .actions
        .iter()
        .find(|action| action.id == "agent_chat_close")
        .expect("notes agent_chat_close action should exist");
    let detached_close = detached
        .actions
        .iter()
        .find(|action| action.id == "agent_chat_close")
        .expect("detached agent_chat_close action should exist");

    assert!(shared_close.shortcut.is_none());
    assert!(notes_close.shortcut.is_none());
    assert_eq!(detached_close.shortcut.as_deref(), Some("⌘W"));
}

#[test]
fn test_agent_chat_host_action_plan_matrix() {
    for host in [
        AgentChatActionsDialogHost::Shared,
        AgentChatActionsDialogHost::Notes,
    ] {
        assert_eq!(
            agent_chat_host_action_plan(host, "agent_chat_close"),
            AgentChatHostActionPlan::IncludeWithoutShortcut,
            "{host:?} should keep close available without advertising Cmd-W"
        );
    }

    assert_eq!(
        agent_chat_host_action_plan(AgentChatActionsDialogHost::Detached, "agent_chat_close"),
        AgentChatHostActionPlan::IncludeWithShortcut
    );
    assert_eq!(
        agent_chat_host_action_plan(AgentChatActionsDialogHost::Notes, "agent_chat_save_as_note"),
        AgentChatHostActionPlan::IncludeWithShortcut
    );
    assert_eq!(
        agent_chat_host_action_plan(
            AgentChatActionsDialogHost::Shared,
            "agent_chat_append_last_response_to_today"
        ),
        AgentChatHostActionPlan::Exclude,
        "normal shared Agent Chat should not show the Day Page return action"
    );
    assert_eq!(
        agent_chat_host_action_plan(
            AgentChatActionsDialogHost::DayPage,
            "agent_chat_append_last_response_to_today"
        ),
        AgentChatHostActionPlan::IncludeWithShortcut,
        "Day-launched Agent Chat should expose the return-to-Today action"
    );
    assert_eq!(
        agent_chat_host_action_plan(
            AgentChatActionsDialogHost::Detached,
            "agent_chat_show_history"
        ),
        AgentChatHostActionPlan::IncludeWithShortcut
    );

    for host in [
        AgentChatActionsDialogHost::Shared,
        AgentChatActionsDialogHost::DayPage,
        AgentChatActionsDialogHost::Notes,
        AgentChatActionsDialogHost::Detached,
    ] {
        assert_eq!(
            agent_chat_host_action_plan(host, "agent_chat_switch_model:gpt"),
            AgentChatHostActionPlan::IncludeWithShortcut
        );
    }
}

#[test]
fn test_agent_chat_root_actions_add_change_model_when_models_exist() {
    let actions = get_agent_chat_root_actions(
        &[
            sample_agent_chat_model("claude-sonnet-4-6", "Sonnet 4.6"),
            sample_agent_chat_model("claude-opus-4-6", "Opus 4.6"),
        ],
        Some("claude-sonnet-4-6"),
        0,
        &[],
        &[],
        default_agent_chat_command_facts(),
    );

    let change_model = actions
        .iter()
        .find(|action| action.id == AGENT_CHAT_CHANGE_MODEL_ACTION_ID)
        .expect("change model action should exist");
    assert_eq!(change_model.section.as_deref(), Some("Agent"));
    assert_eq!(
        change_model.description.as_deref(),
        Some("Current: Sonnet 4.6")
    );
}

#[test]
fn test_agent_chat_root_actions_surface_review_approvals_only_when_grants_exist() {
    let without_grants =
        get_agent_chat_root_actions(&[], None, 0, &[], &[], default_agent_chat_command_facts());
    assert!(
        !without_grants
            .iter()
            .any(|action| action.id == AGENT_CHAT_REVIEW_APPROVALS_ACTION_ID),
        "no review action when the session has no standing grants"
    );

    let with_grants =
        get_agent_chat_root_actions(&[], None, 2, &[], &[], default_agent_chat_command_facts());
    let review = with_grants
        .iter()
        .find(|action| action.id == AGENT_CHAT_REVIEW_APPROVALS_ACTION_ID)
        .expect("review action should exist when standing grants exist");
    assert_eq!(review.title, "Review Auto-Approvals (2)");
    assert_eq!(review.section.as_deref(), Some("Agent"));
}

#[test]
fn test_agent_chat_root_actions_surface_thread_switcher() {
    let summaries = vec![crate::ai::agent_chat::ui::AgentChatThreadSummary {
        ui_thread_id: "thread-abc".to_string(),
        title: "Refactor the parser".to_string(),
        unread: 3,
        is_streaming: true,
    }];
    let actions = get_agent_chat_root_actions(
        &[],
        None,
        0,
        &summaries,
        &[],
        default_agent_chat_command_facts(),
    );

    let new_thread = actions
        .iter()
        .find(|action| action.id == AGENT_CHAT_NEW_THREAD_ACTION_ID)
        .expect("New Thread action should always exist");
    assert_eq!(new_thread.section.as_deref(), Some("Threads"));
    assert_eq!(new_thread.shortcut.as_deref(), Some("⌘N"));

    let switch = actions
        .iter()
        .find(|action| action.id == "agent_chat_switch_thread:thread-abc")
        .expect("switch action should exist per retained thread");
    assert_eq!(switch.title, "Switch to: Refactor the parser (3 new)");
    assert_eq!(switch.section.as_deref(), Some("Threads"));

    assert_eq!(
        agent_chat_switch_thread_id_from_action("agent_chat_switch_thread:thread-abc"),
        Some("thread-abc")
    );
    assert_eq!(
        agent_chat_switch_thread_id_from_action("agent_chat_new_thread"),
        None
    );
}

#[test]
fn test_agent_chat_root_actions_surface_rewind_only_with_fork_points() {
    let points = vec![crate::ai::agent_chat::ui::AgentChatForkPoint {
        entry_id: "entry-7".to_string(),
        text: "fix the parser bug".to_string(),
    }];

    let without =
        get_agent_chat_root_actions(&[], None, 0, &[], &[], default_agent_chat_command_facts());
    assert!(
        !without
            .iter()
            .any(|action| action.id == AGENT_CHAT_REWIND_ACTION_ID),
        "no rewind action without checkpoints"
    );

    let with = get_agent_chat_root_actions(
        &[],
        None,
        0,
        &[],
        &points,
        default_agent_chat_command_facts(),
    );
    let rewind = with
        .iter()
        .find(|action| action.id == AGENT_CHAT_REWIND_ACTION_ID)
        .expect("rewind action should exist when fork points exist");
    assert_eq!(rewind.section.as_deref(), Some("Session"));

    let picker = get_agent_chat_fork_picker_actions(&points);
    assert_eq!(picker.len(), 1);
    assert_eq!(picker[0].id, "agent_chat_fork_edit:entry-7");
    assert_eq!(picker[0].title, "fix the parser bug");

    assert_eq!(
        agent_chat_fork_edit_entry_from_action("agent_chat_fork_edit:entry-7"),
        Some("entry-7")
    );
    assert_eq!(
        agent_chat_fork_edit_entry_from_action("agent_chat_rewind_edit"),
        None
    );
}

#[test]
fn test_agent_chat_fork_picker_lists_latest_first() {
    let points = vec![
        crate::ai::agent_chat::ui::AgentChatForkPoint {
            entry_id: "e0".to_string(),
            text: "older".to_string(),
        },
        crate::ai::agent_chat::ui::AgentChatForkPoint {
            entry_id: "e1".to_string(),
            text: "newest".to_string(),
        },
    ];
    let route =
        get_agent_chat_fork_picker_route_for_host(&points, AgentChatActionsDialogHost::Shared);
    assert_eq!(route.id, AGENT_CHAT_FORK_PICKER_ROUTE_ID);
    assert_eq!(route.actions[0].title, "newest");
    assert_eq!(
        route.initial_selected_action_id.as_deref(),
        Some("agent_chat_fork_edit:e1"),
        "latest message preselected"
    );
}

#[test]
fn detached_agent_chat_history_routes_through_actions_dialog() {
    let detached = get_agent_chat_root_route_for_host(
        &[],
        None,
        0,
        &[],
        &[],
        default_agent_chat_command_facts(),
        AgentChatActionsDialogHost::Detached,
    );
    assert!(
        detached
            .actions
            .iter()
            .any(|action| action.id == "agent_chat_show_history"),
        "detached Agent Chat must expose history in Cmd+K instead of relying on a PromptPopup-only shortcut"
    );

    assert!(
        agent_chat_history_select_action_id("session-123")
            .starts_with(AGENT_CHAT_HISTORY_SELECT_ACTION_PREFIX),
        "history rows must dispatch through stable session-id action ids"
    );
}

#[test]
fn test_agent_chat_model_picker_actions_mark_selected_model() {
    let actions = get_agent_chat_model_picker_actions(
        &[
            sample_agent_chat_model("claude-sonnet-4-6", "Sonnet 4.6"),
            sample_agent_chat_model("claude-opus-4-6", "Opus 4.6"),
        ],
        Some("claude-opus-4-6"),
    );

    let current = actions
        .iter()
        .find(|action| action.id == "agent_chat_switch_model:claude-opus-4-6")
        .expect("current model action should exist");
    assert_eq!(current.title, "Opus 4.6 ✓");

    let alternate = actions
        .iter()
        .find(|action| action.id == "agent_chat_switch_model:claude-sonnet-4-6")
        .expect("alternate model action should exist");
    assert_eq!(alternate.title, "Sonnet 4.6");
}

#[test]
fn agent_chat_actions_include_cmux_codex_prompt_handoff() {
    let actions = get_agent_chat_actions();
    let action = actions
        .iter()
        .find(|action| action.id == crate::ai::agent_prompt_handoff::CMUX_CODEX_ACTION_ID)
        .expect("Agent Chat actions should expose cmux Codex prompt handoff");

    assert_eq!(action.title, "Send Prompt to cmux Codex");
    assert_eq!(action.section.as_deref(), Some("Handoff"));
    assert!(action.shortcut.is_none());
}

#[test]
fn test_agent_chat_switch_model_action_parser_returns_model_id() {
    assert_eq!(
        agent_chat_switch_model_id_from_action("agent_chat_switch_model:claude-sonnet-4-6"),
        Some("claude-sonnet-4-6")
    );
    assert_eq!(
        agent_chat_switch_model_id_from_action("agent_chat_retry_last"),
        None
    );
}
