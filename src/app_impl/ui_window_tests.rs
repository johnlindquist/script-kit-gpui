use super::{
    about_footer_buttons, create_ai_preset_footer_buttons, flow_session_footer_buttons,
    main_list_loading_left_info, main_window_footer_chrome_should_render,
    main_window_result_action_label, micro_prompt_footer_buttons, notes_browse_footer_buttons,
    paste_into_frontmost_app_label, script_template_catalog_footer_buttons,
    sdk_reference_footer_buttons, term_prompt_footer_buttons,
};
use crate::footer_popup::FooterAction;
use crate::scripts::{MatchIndices, Scriptlet, ScriptletMatch};
use std::sync::Arc;

fn make_scriptlet_result(tool: &str) -> crate::scripts::SearchResult {
    crate::scripts::SearchResult::Scriptlet(ScriptletMatch {
        scriptlet: Arc::new(Scriptlet {
            name: "Test Scriptlet".to_string(),
            description: None,
            code: "echo test".to_string(),
            tool: tool.to_string(),
            shortcut: None,
            keyword: None,
            group: None,
            plugin_id: String::new(),
            plugin_title: None,
            file_path: None,
            command: None,
            alias: None,
            icon: None,
        }),
        score: 100,
        display_file_path: None,
        match_indices: MatchIndices::default(),
        match_evidence: None,
    })
}

#[test]
fn paste_into_frontmost_app_label_uses_app_name() {
    assert_eq!(
        paste_into_frontmost_app_label(Some("Safari")),
        "Paste into Safari"
    );
}

#[test]
fn paste_into_frontmost_app_label_falls_back_to_active_app() {
    assert_eq!(
        paste_into_frontmost_app_label(None),
        "Paste into Active App"
    );
}

#[test]
fn flow_session_footer_includes_left_pinned_terminate() {
    let buttons = flow_session_footer_buttons(true, true, false);
    let terminate = &buttons[0];
    assert_eq!(terminate.label.as_ref(), "Terminate Flow");
    assert_eq!(terminate.key.as_ref(), "⇧⌘⎋");
    assert!(terminate.left_pinned);
    assert!(terminate.enabled, "termination remains available mid-turn");
}

#[test]
fn main_window_result_action_label_uses_frontmost_app_for_paste_scriptlets() {
    let result = make_scriptlet_result("paste");
    assert_eq!(
        main_window_result_action_label(&result, Some("TextEdit")),
        "Paste into TextEdit"
    );
}

#[test]
fn main_window_result_action_label_keeps_default_for_non_paste_scriptlets() {
    let result = make_scriptlet_result("bash");
    assert_eq!(
        main_window_result_action_label(&result, Some("TextEdit")),
        "Run Command"
    );
}

/// The loading footer slot carries the kind's status label plus the braille
/// frame for the given elapsed time (0.9s cycle, 8 steps — 0.2s lands on
/// frame index 1).
#[test]
fn main_list_loading_left_info_uses_kind_label_and_current_braille_frame() {
    use crate::main_list_loading::MainListLoadingKind;

    let info = main_list_loading_left_info(MainListLoadingKind::BrowserHistory, 0.2);
    assert_eq!(info.model_name, "Fetching history");
    assert_eq!(
        info.spinner_glyph.as_deref(),
        Some(crate::components::braille_loading::BRAILLE_SPINNER_FRAMES[1])
    );
    assert!(info.action.is_none(), "loading status is not clickable");

    let tabs = main_list_loading_left_info(MainListLoadingKind::BrowserTabs, 0.0);
    assert_eq!(tabs.model_name, "Fetching tabs");
    let files = main_list_loading_left_info(MainListLoadingKind::RootFileSearch, 0.0);
    assert_eq!(files.model_name, "Searching files");
}

#[test]
fn main_window_footer_contract_term_prompt_buttons_use_native_keyboard_grammar() {
    use crate::footer_popup::FooterAction;

    let buttons = term_prompt_footer_buttons(true, false);
    let grammar: Vec<_> = buttons
        .iter()
        .map(|button| (button.action, button.key.as_ref(), button.label.as_ref()))
        .collect();
    assert_eq!(
        grammar,
        vec![
            (FooterAction::Run, "↵", "Continue"),
            (FooterAction::Actions, "⌘K", "Actions"),
            (FooterAction::Close, "Esc", "Cancel"),
        ]
    );
    assert!(buttons.iter().all(|button| button.enabled));
}

#[test]
fn main_window_footer_contract_legacy_view_buttons_match_real_keyboard_actions() {
    fn grammar(
        buttons: &[crate::footer_popup::FooterButtonConfig],
    ) -> Vec<(FooterAction, &str, &str)> {
        buttons
            .iter()
            .map(|button| (button.action, button.key.as_ref(), button.label.as_ref()))
            .collect()
    }

    assert_eq!(
        grammar(&about_footer_buttons(true)),
        vec![(FooterAction::Close, "Esc", "Back")]
    );
    assert_eq!(
        grammar(&micro_prompt_footer_buttons(true)),
        vec![
            (FooterAction::Run, "↵", "Submit"),
            (FooterAction::Close, "Esc", "Cancel"),
        ]
    );
    assert_eq!(
        grammar(&sdk_reference_footer_buttons(true, true)),
        vec![
            (FooterAction::Run, "↵", "Copy Markdown"),
            (FooterAction::Copy, "⌘C", "Copy"),
            (FooterAction::Close, "Esc", "Back"),
        ]
    );
    assert_eq!(
        grammar(&script_template_catalog_footer_buttons(true, true)),
        vec![
            (FooterAction::Run, "↵", "Create Local Script"),
            (FooterAction::Copy, "⌘C", "Copy"),
            (FooterAction::Close, "Esc", "Back"),
        ]
    );
    assert_eq!(
        grammar(&create_ai_preset_footer_buttons(true, true)),
        vec![
            (FooterAction::Run, "↵", "Save Preset"),
            (FooterAction::Ai, "⇥", "Next Field"),
            (FooterAction::Close, "Esc", "Cancel"),
        ]
    );
    assert_eq!(
        grammar(&notes_browse_footer_buttons(true, true, true)),
        vec![
            (FooterAction::Run, "↵", "Attach Note"),
            (FooterAction::Close, "Esc", "Cancel"),
        ]
    );
    assert_eq!(
        grammar(&notes_browse_footer_buttons(true, false, true)),
        vec![(FooterAction::Close, "Esc", "Back")],
        "standalone Notes Browse has no Enter behavior, so its footer must not advertise one"
    );

    assert!(!sdk_reference_footer_buttons(true, false)[0].enabled);
    assert!(!script_template_catalog_footer_buttons(true, false)[0].enabled);
    assert!(!create_ai_preset_footer_buttons(true, false)[0].enabled);
    assert!(!notes_browse_footer_buttons(true, true, false)[0].enabled);
}

#[test]
fn main_window_footer_contract_agent_chat_error_keeps_chrome() {
    assert!(main_window_footer_chrome_should_render(true, Some(true)));
    assert!(
        main_window_footer_chrome_should_render(true, Some(false)),
        "Agent Chat contextual controls may hide, but main-window footer chrome must remain"
    );
    assert!(main_window_footer_chrome_should_render(false, None));
}
