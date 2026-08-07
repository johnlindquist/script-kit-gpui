use std::ops::Range;

use crate::ai::agent_chat::ui::portal_contract::{
    apply_portal_replacement, clear_terminal_portal_state, decide_portal_open,
    exact_replacement_target_for_range, next_portal_state, picker_portal_query,
    portal_target_from_inline_token, portal_target_from_part, AgentChatPortalOpenRefusal,
    AgentChatPortalSessionEvent, AgentChatPortalSessionState,
};
use crate::ai::context_selector::types::ContextPortalKind;
use crate::ai::message_parts::AiContextPart;

fn parse_kind(kind: &str) -> Option<ContextPortalKind> {
    match kind {
        "file_search" => Some(ContextPortalKind::FileSearch),
        "browser_history" => Some(ContextPortalKind::BrowserHistory),
        "browser_tabs" => Some(ContextPortalKind::BrowserTabs),
        "clipboard_history" => Some(ContextPortalKind::ClipboardHistory),
        "dictation_history" => Some(ContextPortalKind::DictationHistory),
        "script_search" => Some(ContextPortalKind::ScriptSearch),
        "scriptlet_search" => Some(ContextPortalKind::ScriptletSearch),
        "skill_search" => Some(ContextPortalKind::SkillSearch),
        "notes_browse" => Some(ContextPortalKind::NotesBrowse),
        "agent_chat_history" => Some(ContextPortalKind::AgentChatHistory),
        "terminal" => Some(ContextPortalKind::Terminal),
        _ => None,
    }
}

fn format_kind(kind: ContextPortalKind) -> String {
    match kind {
        ContextPortalKind::FileSearch => "file_search",
        ContextPortalKind::BrowserHistory => "browser_history",
        ContextPortalKind::BrowserTabs => "browser_tabs",
        ContextPortalKind::ClipboardHistory => "clipboard_history",
        ContextPortalKind::DictationHistory => "dictation_history",
        ContextPortalKind::ScriptSearch => "script_search",
        ContextPortalKind::ScriptletSearch => "scriptlet_search",
        ContextPortalKind::SkillSearch => "skill_search",
        ContextPortalKind::NotesBrowse => "notes_browse",
        ContextPortalKind::AgentChatHistory => "agent_chat_history",
        ContextPortalKind::Terminal => "terminal",
    }
    .to_string()
}

#[doc(hidden)]
pub fn picker_query(kind: &str, session_query: &str) -> Option<String> {
    Some(picker_portal_query(parse_kind(kind)?, session_query))
}

#[doc(hidden)]
pub fn inline_target(token: &str) -> Option<(String, String)> {
    portal_target_from_inline_token(token).map(|(kind, query)| (format_kind(kind), query))
}

#[doc(hidden)]
pub fn part_target(part: &AiContextPart) -> Option<(String, String)> {
    portal_target_from_part(part).map(|(kind, query)| (format_kind(kind), query))
}

#[doc(hidden)]
pub fn dictation_part_token(id: &str) -> Option<String> {
    let part = AiContextPart::ResourceUri {
        uri: format!("kit://dictation-history?id={id}"),
        label: format!("Dictation: {id}"),
    };
    crate::ai::context_mentions::part_to_inline_token(&part)
}

#[doc(hidden)]
pub fn production_dictation_part(id: &str, preview: &str) -> AiContextPart {
    let entry = crate::dictation::DictationHistoryEntry {
        version: crate::dictation::DICTATION_HISTORY_ENTRY_VERSION,
        id: id.to_string(),
        timestamp: "2026-04-16T00:00:00Z".to_string(),
        transcript: preview.to_string(),
        preview: preview.to_string(),
        target_id: crate::dictation::DictationTarget::TabAiHarness
            .sticky_label()
            .to_string(),
        target_label_snapshot: "Agent Chat".to_string(),
        audio_duration_ms: 0,
    };
    crate::ai::context_mentions::dictation_history_part_for_entry(&entry)
}

#[doc(hidden)]
pub fn part_inline_token(part: &AiContextPart) -> Option<String> {
    crate::ai::context_mentions::part_to_inline_token(part)
}

#[doc(hidden)]
pub fn clipboard_part_token(id: &str) -> Option<String> {
    let _ = id;
    Some("@clipboard".to_string())
}

#[doc(hidden)]
pub fn replacement(
    current_text: &str,
    char_range: Range<usize>,
    fallback_cursor: usize,
    replacement_text: &str,
) -> (String, usize, bool) {
    let target = exact_replacement_target_for_range(current_text, char_range, fallback_cursor);
    apply_portal_replacement(current_text, &target, replacement_text)
}

#[doc(hidden)]
pub fn open_refusal(allowed: bool, has_host_callback: bool) -> Option<&'static str> {
    match decide_portal_open(allowed, has_host_callback) {
        Ok(()) => None,
        Err(AgentChatPortalOpenRefusal::UnsupportedByHost) => Some("unsupported_by_host"),
        Err(AgentChatPortalOpenRefusal::MissingHostCallback) => Some("missing_host_callback"),
    }
}

fn parse_state(value: &str) -> Option<AgentChatPortalSessionState> {
    match value {
        "idle" => Some(AgentChatPortalSessionState::Idle),
        "staged" => Some(AgentChatPortalSessionState::Staged),
        "active" => Some(AgentChatPortalSessionState::Active),
        "accepted" => Some(AgentChatPortalSessionState::Accepted),
        "cancelled" => Some(AgentChatPortalSessionState::Cancelled),
        "orphaned" => Some(AgentChatPortalSessionState::Orphaned),
        _ => None,
    }
}

fn format_state(value: AgentChatPortalSessionState) -> String {
    match value {
        AgentChatPortalSessionState::Idle => "idle",
        AgentChatPortalSessionState::Staged => "staged",
        AgentChatPortalSessionState::Active => "active",
        AgentChatPortalSessionState::Accepted => "accepted",
        AgentChatPortalSessionState::Cancelled => "cancelled",
        AgentChatPortalSessionState::Orphaned => "orphaned",
    }
    .to_string()
}

#[doc(hidden)]
pub fn state_transition(state: &str, event: &str) -> Option<String> {
    let state = parse_state(state)?;
    let event = match event {
        "stage" => AgentChatPortalSessionEvent::Stage,
        "activate" => AgentChatPortalSessionEvent::Activate,
        "accept" => AgentChatPortalSessionEvent::Accept,
        "cancel" => AgentChatPortalSessionEvent::Cancel,
        "orphan" => AgentChatPortalSessionEvent::Orphan,
        _ => return None,
    };
    next_portal_state(state, event).map(format_state)
}

#[doc(hidden)]
pub fn clear_terminal_state(state: &str) -> Option<String> {
    parse_state(state)
        .map(clear_terminal_portal_state)
        .map(format_state)
}
