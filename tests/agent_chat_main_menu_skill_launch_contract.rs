//! Source-level contract for main-menu skill launch into Agent Chat.
//!
//! Selecting a skill from the main menu must behave like accepting that skill
//! from Agent Chat slash mode: visible `/skill ` composer text, attached skill context,
//! and no automatic submit.

const TAB_AI_MODE: &str = include_str!("../src/app_impl/agent_handoff/mod.rs");
const AGENT_CHAT_VIEW: &str = include_str!("../src/ai/agent_chat/ui/view.rs");

fn source_between<'a>(source: &'a str, start_pat: &str, end_pat: &str) -> &'a str {
    let start = source
        .find(start_pat)
        .unwrap_or_else(|| panic!("missing source start: {start_pat}"));
    let end_rel = source[start..]
        .find(end_pat)
        .unwrap_or_else(|| panic!("missing source end: {end_pat}"));
    &source[start..start + end_rel]
}

fn compact(source: &str) -> String {
    source.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn main_menu_skill_launch_opens_agent_chat_without_entry_intent_submit() {
    let body = source_between(
        TAB_AI_MODE,
        "pub(crate) fn open_agent_chat_with_selected_skill(",
        "    pub(crate) fn open_tab_ai_agent_chat_with_slash_picker(",
    );

    assert!(
        body.contains("build_skill_slash_command_text"),
        "main-menu skill launch must build slash command text"
    );
    assert!(
        body.contains("AgentChatContextPolicy::PluginSkill")
            && body.contains("AgentChatEntryIntent::add(Some(command_text))"),
        "main-menu skill launch must carry the skill through typed Add staging"
    );
    assert!(
        body.contains("self.open_and_observe_agent_chat_from_entry_request(")
            && body.contains("AgentChatEntryOrigin::PluginSkill"),
        "main-menu skill launch must dispatch and observe the PluginSkill request"
    );
    assert!(
        body.contains("stage_selected_plugin_skill_from_main_menu"),
        "main-menu skill launch must stage selected plugin skill on the view"
    );

    assert!(
        !body.contains("build_staged_skill_prompt"),
        "main-menu skill launch must not build an auto-submit entry prompt"
    );
    assert!(
        !body.contains("open_tab_ai_agent_chat_with_entry_intent(Some"),
        "main-menu skill launch must not pass selected skills as entry intents"
    );
}

#[test]
fn agent_chat_stages_main_menu_skill_like_slash_selection_without_submit() {
    let body = source_between(
        AGENT_CHAT_VIEW,
        "pub(crate) fn stage_selected_plugin_skill_from_main_menu(",
        "    /// Reuse the current live thread for a fresh external entry intent.",
    );

    for required in [
        "build_skill_slash_command_text(&skill.skill_id)",
        "build_skill_context_part(&skill.title, owner, &skill.skill_id, &skill.path)",
        "thread.add_or_replace_skill_context(identity, part, cx);",
        "thread.input.set_text(command_text.clone());",
        "thread.input.set_cursor(cursor_after);",
        "thread.mark_context_bootstrap_ready(cx);",
    ] {
        assert!(
            body.contains(required),
            "Agent Chat skill staging must preserve slash-selection behavior: {required}"
        );
    }

    assert!(
        !body.contains("submit_input("),
        "Agent Chat skill staging must not auto-submit"
    );
}
