use script_kit_gpui::{builtins::get_builtin_entries, config::BuiltInConfig};

#[test]
fn agent_chat_destination_builtins_name_agent_chat_not_generic_ai() {
    let entries = get_builtin_entries(&BuiltInConfig::default());
    let builtins_source = super::read_source("src/builtins/mod.rs");
    let execution_source = super::read_source("src/app_execute/builtin_execution.rs");
    let agent_chat = entries
        .iter()
        .find(|entry| entry.id == "builtin/ai-chat")
        .expect("agent chat entry should exist");
    assert_eq!(agent_chat.name, "Agent Chat");
    assert_eq!(agent_chat.default_action_text(), "Open Agent Chat");
    assert_eq!(agent_chat.footer_action_text(), "Agent");
    assert!(
        builtins_source
            .contains("AiCommandType::SendScreenAreaToAi => \"Select Area for Agent Chat\"")
            && !builtins_source.contains("Select Area for AI"),
        "screen-area Agent Chat label should not regress to generic AI wording"
    );
    assert!(
        execution_source.contains("Send Screen Area to Agent Chat is unavailable")
            && execution_source.contains("Opening Dictation to Agent Chat")
            && !execution_source.contains("Send Screen Area to AI is unavailable")
            && !execution_source.contains("Dictation-to-AI"),
        "Agent Chat execution messages should not regress to generic AI wording"
    );

    for (id, expected_name, expected_action) in [
        (
            "builtin/send-screen-to-ai",
            "Send Full Screen to Agent Chat",
            "Send Screen to Agent Chat",
        ),
        (
            "builtin/send-focused-window-to-ai",
            "Send Focused Window to Agent Chat",
            "Send Window to Agent Chat",
        ),
        (
            "builtin/send-selected-text-to-ai",
            "Send Selection to Agent Chat",
            "Send Selection to Agent Chat",
        ),
        (
            "builtin/send-browser-tab-to-ai",
            "Send Current Browser Tab to Agent Chat",
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
fn contextual_dictation_builtin_names_the_frontmost_app_destination() {
    let entries = get_builtin_entries(&BuiltInConfig::default());
    let builtins_source = super::read_source("src/builtins/mod.rs");
    let dictation = entries
        .iter()
        .find(|entry| entry.id == "builtin/dictation")
        .expect("dictation entry should exist");

    assert_eq!(dictation.name, "Dictate to Current App");
    assert_eq!(
        dictation.default_action_text(),
        "Start Dictation to Current App"
    );
    assert!(
        builtins_source.contains("fn current_dictation_entry_name()")
            && builtins_source.contains("crate::frontmost_app_tracker::get_last_real_app()")
            && builtins_source.contains("format!(\"Dictate to {name}\")"),
        "contextual dictation entry must use the tracked frontmost app name when available"
    );
    let old_name = ["Dictate", "Here"].join(" ");
    let old_action = ["Start Dictation", "Here"].join(" ");
    assert!(
        !builtins_source.contains(&old_name) && !builtins_source.contains(&old_action),
        "contextual dictation should not use the old 'Here' wording"
    );
}

#[test]
fn generate_script_actions_name_agent_chat_handoff() {
    let entries = get_builtin_entries(&BuiltInConfig::default());

    for (id, expected_name, expected_action, expected_description) in [
        (
            "builtin/generate-script-with-ai",
            "Generate Script with Agent Chat",
            "Open Agent Chat to Generate Script",
            "Open Agent Chat to generate a Script Kit script from your prompt text",
        ),
        (
            "builtin/generate-script-from-current-app",
            "Generate Script from Current App",
            "Open Agent Chat to Generate App Script",
            "Generate a Script Kit script using the frontmost app's menu, selection, and browser context",
        ),
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry.id == id)
            .unwrap_or_else(|| panic!("missing generate script builtin {id}"));

        assert_eq!(entry.name, expected_name, "{id} name");
        assert_eq!(entry.default_action_text(), expected_action, "{id} action");
        assert_eq!(entry.description, expected_description, "{id} description");
        assert!(
            entry.default_action_text().contains("Agent Chat"),
            "{id} action should name the Agent Chat handoff"
        );
    }
}

#[test]
fn builtin_actions_dialog_uses_builtin_default_action_text() {
    let focused_info = super::read_source("src/app_render/focused_info.rs");
    let script_context_actions = super::read_source("src/actions/builders/script_context.rs");

    assert!(
        focused_info.contains("m.entry.default_action_text()"),
        "built-in rows should pass their concrete default action text into the actions dialog"
    );
    assert!(
        script_context_actions.contains("enum ScriptContextKind")
            && script_context_actions.contains("enum PrimaryActionPlan")
            && script_context_actions.contains("PrimaryActionPlan::PreserveCatalogActionText")
            && script_context_actions.contains("script.action_verb.clone()")
            && script_context_actions.contains("ScriptContextKind::BuiltIn"),
        "built-in action rows should use an explicit text plan instead of hiding preservation/normalization in ad hoc conditionals"
    );
}

#[test]
fn agent_chat_history_text_names_agent_chat_conversations() {
    let entries = get_builtin_entries(&BuiltInConfig::default());
    let agent_chat_history = entries
        .iter()
        .find(|entry| entry.id == "builtin/agent_chat-history")
        .expect("agent_chat-history entry should exist");
    let root_actions = super::read_source("src/app_impl/root_unified_result_actions.rs");
    let group_header = super::read_source("src/app_render/group_header_item.rs");
    let script_context_actions = super::read_source("src/actions/builders/script_context.rs");
    let payload = super::read_source("src/menu_syntax/payload.rs");
    let action_helpers = super::read_source("src/action_helpers.rs");
    let focused_info = super::read_source("src/app_render/focused_info.rs");

    assert_eq!(agent_chat_history.name, "Agent Chat History");
    assert_eq!(
        agent_chat_history.default_action_text(),
        "Open Agent Chat History"
    );
    assert_eq!(
        agent_chat_history.description,
        "Browse and manage past Agent Chat conversations"
    );
    assert!(
        root_actions.contains("Self::AgentChatHistory(_) => \"Agent Chat Conversations\""),
        "root-unified Agent Chat history action context should name Agent Chat conversations"
    );
    assert!(
        group_header.contains("BuiltInFeature::AgentChatHistory => \"Agent Chat History\""),
        "Agent Chat history group header should match the visible command name"
    );
    assert!(
        script_context_actions.contains("\"agent_chat_show_history\"")
            && script_context_actions.contains("\"Agent Chat History\"")
            && script_context_actions.contains("Browse and manage past Agent Chat conversations"),
        "Agent Chat actions should not expose generic conversation-history text"
    );
    assert!(
        payload.contains("label: \"Agent Chat Conversations\"")
            && payload.contains("Self::Conversations => \"Agent Chat Conversations\"")
            && action_helpers.contains("Cannot edit Agent Chat conversations"),
        "source filters and disabled action text should name Agent Chat conversations"
    );
    // Formatting-insensitive: rustfmt may wrap the call, so assert the label
    // and the indicator helper separately instead of pinning one line shape.
    assert!(
        focused_info.contains("\"Agent Chat Conversation\"")
            && focused_info.contains("focused_info_type_indicator("),
        "Agent Chat history info panel type indicator should name Agent Chat"
    );
    assert!(
        !agent_chat_history.description.contains("AI conversations")
            && !root_actions.contains("Self::AgentChatHistory(_) => \"AI Conversations\""),
        "Agent Chat history text should not use generic AI conversation wording"
    );
}

#[test]
fn permission_assistant_commands_do_not_claim_to_grant_permissions() {
    let entries = get_builtin_entries(&BuiltInConfig::default());
    let settings = super::read_source("src/render_builtins/settings.rs");

    for (id, expected_name, expected_action, expected_description) in [
        (
            "builtin/allow-accessibility",
            "Open Accessibility Assistant",
            "Open Accessibility Assistant",
            "Open the Permission Assistant for Accessibility",
        ),
        (
            "builtin/allow-screen-recording",
            "Open Screen Recording Assistant",
            "Open Screen Recording Assistant",
            "Open the Permission Assistant for Screen Recording",
        ),
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry.id == id)
            .unwrap_or_else(|| panic!("missing permission assistant builtin {id}"));

        assert_eq!(entry.name, expected_name, "{id} name");
        assert_eq!(entry.default_action_text(), expected_action, "{id} action");
        assert_eq!(entry.description, expected_description, "{id} description");
        assert!(
            !entry.name.starts_with("Allow ") && !entry.default_action_text().starts_with("Allow "),
            "{id} should not imply Script Kit can directly grant macOS permission"
        );
        assert!(
            settings.contains(expected_name) && settings.contains(expected_description),
            "settings hub should use the same assistant wording for {id}"
        );
    }

    let accessibility_settings = entries
        .iter()
        .find(|entry| entry.id == "builtin/accessibility-settings")
        .expect("accessibility settings builtin should exist");
    assert_eq!(
        accessibility_settings.description,
        "Open Accessibility settings in macOS System Settings"
    );
    assert!(
        !accessibility_settings
            .description
            .contains("System Preferences")
            && settings.contains("Open Accessibility settings in macOS System Settings"),
        "Accessibility Settings text should use modern macOS System Settings wording"
    );
}

#[test]
fn system_settings_builtins_name_the_settings_pane_they_open() {
    let entries = get_builtin_entries(&BuiltInConfig::default());

    for (id, expected_name, expected_action, expected_footer, expected_description) in [
        (
            "builtin/system-preferences",
            "Open System Settings",
            "Open macOS System Settings",
            "macOS Settings",
            "Open macOS System Settings",
        ),
        (
            "builtin/privacy-settings",
            "Open Privacy & Security Settings",
            "Open Privacy & Security Settings",
            "Privacy & Security",
            "Open Privacy & Security settings",
        ),
        (
            "builtin/display-settings",
            "Open Displays Settings",
            "Open Displays Settings",
            "Displays",
            "Open Displays settings",
        ),
        (
            "builtin/sound-settings",
            "Open Sound Settings",
            "Open Sound Settings",
            "Sound",
            "Open Sound settings",
        ),
        (
            "builtin/network-settings",
            "Open Network Settings",
            "Open Network Settings",
            "Network",
            "Open Network settings",
        ),
        (
            "builtin/keyboard-settings",
            "Open Keyboard Settings",
            "Open Keyboard Settings",
            "Keyboard",
            "Open Keyboard settings",
        ),
        (
            "builtin/bluetooth-settings",
            "Open Bluetooth Settings",
            "Open Bluetooth Settings",
            "Bluetooth",
            "Open Bluetooth settings",
        ),
        (
            "builtin/notifications-settings",
            "Open Notifications Settings",
            "Open Notifications Settings",
            "Notifications",
            "Open Notifications settings",
        ),
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry.id == id)
            .unwrap_or_else(|| panic!("missing settings builtin {id}"));

        assert_eq!(entry.name, expected_name, "{id} name");
        assert_eq!(entry.default_action_text(), expected_action, "{id} action");
        assert_eq!(entry.footer_action_text(), expected_footer, "{id} footer");
        assert_eq!(entry.description, expected_description, "{id} description");
        assert!(
            !entry.name.contains("System Preferences")
                && !entry.description.contains("System Preferences")
                && !entry.default_action_text().contains("System Preferences"),
            "{id} should use modern macOS System Settings wording"
        );
    }
}

#[test]
fn force_quit_command_text_names_the_dialog_it_opens() {
    let entries = get_builtin_entries(&BuiltInConfig::default());
    let system_actions = super::read_source("src/system_actions/mod.rs");
    let execution = super::read_source("src/app_execute/builtin_execution.rs");

    let force_quit = entries
        .iter()
        .find(|entry| entry.id == "builtin/force-quit")
        .expect("force quit builtin should exist");

    assert_eq!(force_quit.name, "Force Quit Applications");
    assert_eq!(force_quit.default_action_text(), "Open Force Quit");
    assert_eq!(
        force_quit.description,
        "Open the macOS Force Quit Applications dialog"
    );
    assert!(
        system_actions.contains("Opening Force Quit Applications dialog")
            && system_actions.contains("command down, option down"),
        "Force Quit Apps builtin should open the macOS dialog instead of directly terminating apps"
    );
    assert!(
        execution.contains("\"builtin/force-quit\"")
            && execution.contains("\"Open Force Quit Apps\"")
            && execution.contains("\"Open Force Quit Apps?\""),
        "Force Quit confirmation should name the dialog-opening action"
    );
}
