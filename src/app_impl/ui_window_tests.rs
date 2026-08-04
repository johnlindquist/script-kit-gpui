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

/// Flow session native footer grammar (Oracle 2026-07-21 adjudication):
/// idle = `↵ Send · ⌘K Actions · Esc Background`; working =
/// `⌘. Stop · ⌘K Actions · Esc Background`.
/// No permanent Terminate (destructive expert command → ⌘K Actions, shortcut
/// ⇧⌘⎋ still handled) and no disabled "Working…" pseudo-button (the leading
/// status text carries Working/Connecting).
///
/// Stop replaces Send while working. The working row used to omit it entirely,
/// so `⌘.` — the one key that cancels a runaway turn — was the only live flow
/// session binding the footer never named.
#[test]
fn flow_session_native_footer_matches_idle_and_working_contract() {
    use crate::footer_popup::FooterAction;

    let idle = flow_session_footer_buttons(false, true, false);
    let idle_shape: Vec<(FooterAction, &str, &str)> = idle
        .iter()
        .map(|b| (b.action, b.key.as_ref(), b.label.as_ref()))
        .collect();
    assert_eq!(
        idle_shape,
        vec![
            (FooterAction::Run, "↵", "Send"),
            (FooterAction::Actions, "⌘K", "Actions"),
            (FooterAction::Close, "Esc", "Background"),
        ],
        "idle flow footer must be exactly Send · Actions · Desk"
    );
    assert!(idle.iter().all(|b| b.enabled));

    let working = flow_session_footer_buttons(true, true, false);
    let working_shape: Vec<(FooterAction, &str, &str)> = working
        .iter()
        .map(|b| (b.action, b.key.as_ref(), b.label.as_ref()))
        .collect();
    assert_eq!(
        working_shape,
        vec![
            (FooterAction::Stop, "⌘.", "Stop"),
            (FooterAction::Actions, "⌘K", "Actions"),
            (FooterAction::Close, "Esc", "Background"),
        ],
        "working flow footer must be exactly Stop · Actions · Desk — no Send, no Terminate"
    );
    assert!(
        working.iter().all(|b| b.enabled),
        "a Stop button the user cannot press is worse than no Stop button"
    );
}

/// The hint-strip footer and the native footer are two renderings of ONE
/// grammar. They are built by different functions in different modules, so
/// nothing but this test stops them from drifting apart — and a user who sees
/// `⌘. Stop` in one rendering and not the other cannot tell which is true.
#[test]
fn flow_session_native_footer_and_hint_strip_agree_on_the_same_grammar() {
    let native_labels = |working: bool| -> Vec<String> {
        flow_session_footer_buttons(working, true, false)
            .iter()
            .map(|b| format!("{} {}", b.key, b.label))
            .collect()
    };

    assert_eq!(
        native_labels(false),
        crate::flow_session_footer_hints_for_tests(false)
            .iter()
            .map(|hint| hint.to_string())
            .collect::<Vec<_>>(),
        "idle grammar must match between the native footer and the hint strip"
    );
    assert_eq!(
        native_labels(true),
        crate::flow_session_footer_hints_for_tests(true)
            .iter()
            .map(|hint| hint.to_string())
            .collect::<Vec<_>>(),
        "working grammar must match between the native footer and the hint strip"
    );
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

/// Agent Chat and Flow must name the SAME chord for Stop.
///
/// They did not. Agent Chat's footer said `Esc Stop`; Flow's said `⌘. Stop`.
/// Each was honest about its own surface — but Escape *backgrounds* a Flow
/// session rather than stopping it, so a user who learned "Esc stops the
/// model" in Agent Chat and applied it in Flow left the turn running. `⌘.`
/// is the one chord that already stops a turn on both, so both footers now
/// advertise it.
///
/// This test crosses the surface boundary deliberately: it renders Agent
/// Chat's Stop hint through Agent Chat's own label mapper and compares it to
/// the string Flow actually shows. Re-inlining a literal on either side fails
/// here rather than shipping as a quiet split.
#[test]
fn agent_chat_and_flow_advertise_the_same_stop_chord() {
    use crate::ai::agent_chat::ui::view::{AgentChatFooterButtonSpec, AgentChatView};
    use crate::components::footer_chrome::{FOOTER_AI_STOP_KEY, FOOTER_AI_STOP_LABEL};

    let agent_chat_stop = AgentChatFooterButtonSpec {
        action: crate::footer_popup::FooterAction::Stop,
        key: FOOTER_AI_STOP_KEY,
        label: FOOTER_AI_STOP_LABEL,
        selected: false,
        enabled: true,
        disabled_reason: None,
    };

    let flow_stop_hint = crate::flow_session_footer_hints_for_tests(true)
        .first()
        .expect("the working flow footer leads with Stop")
        .to_string();

    assert_eq!(
        AgentChatView::footer_hint_label(&agent_chat_stop),
        flow_stop_hint,
        "both AI surfaces must advertise one Stop chord"
    );
    assert_eq!(
        flow_stop_hint, "⌘. Stop",
        "the shared Stop chord is ⌘.; changing it must be a deliberate edit \
         that fails here first"
    );
}
