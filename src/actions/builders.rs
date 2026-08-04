//! Action builders
//!
//! Factory functions for creating context-specific action lists.

// Re-export action types into the builders module namespace so
// submodules can import them as `super::types::*`.
pub(super) mod types {
    pub use crate::actions::types::{Action, ActionCategory, ScriptInfo};
}

// Flat re-exports for test submodules that do `use super::*`.
#[cfg(test)]
pub(super) use crate::actions::types::{Action, ActionCategory, ScriptInfo};

mod chat;
mod clipboard;
mod emoji;
mod file_path;
mod notes;
mod script_context;
mod scriptlet;
mod shared;

pub use chat::{ChatModelInfo, ChatPromptInfo};
pub use clipboard::ClipboardEntryInfo;
pub use emoji::{get_emoji_context_actions, EmojiActionInfo};
pub use notes::{NewChatModelInfo, NewChatPresetInfo, NoteSwitcherNoteInfo, NotesInfo};
pub use shared::to_deeplink_name;

#[allow(unused_imports)]
pub(crate) use scriptlet::get_scriptlet_defined_actions;
#[allow(unused_imports)]
pub(crate) use shared::format_shortcut_hint;

#[allow(unused_imports)]
pub use chat::get_chat_model_picker_actions;
pub(crate) use chat::get_chat_root_route_with_facts;
#[allow(unused_imports)]
pub use chat::{get_ai_command_bar_actions, get_chat_context_actions};
#[allow(unused_imports)]
pub use chat::{
    get_chat_model_picker_route, get_chat_root_route, CHAT_CHANGE_MODEL_ACTION_ID,
    CHAT_MODEL_PICKER_ROUTE_ID, CHAT_ROOT_ROUTE_ID,
};
pub use clipboard::get_clipboard_history_context_actions;
pub(crate) use file_path::resolve_file_search_secondary_action_id;
pub use file_path::{
    get_file_context_actions, get_file_search_directory_actions, get_path_context_actions,
    FileSearchDirectoryInfo, FileSearchSortMode,
};
pub use notes::{
    get_day_note_switcher_actions, get_new_chat_actions, get_note_switcher_actions,
    get_notes_command_bar_actions,
};
pub(crate) use script_context::{
    agent_chat_fork_edit_entry_from_action, agent_chat_receipt_history_request_id_from_action,
    agent_chat_switch_model_id_from_action, agent_chat_switch_profile_id_from_action,
    agent_chat_switch_thread_id_from_action, get_agent_chat_actions, get_agent_chat_history_route,
    get_agent_chat_receipt_history_route, get_global_actions, get_script_context_actions,
    AGENT_CHAT_HISTORY_SELECT_ACTION_PREFIX, AGENT_CHAT_RECEIPT_HISTORY_COPY_ACTION_PREFIX,
    AGENT_CHAT_RECEIPT_HISTORY_ROUTE_ID, AGENT_CHAT_SHOW_RECEIPT_HISTORY_ACTION_ID,
};
#[allow(unused_imports)]
pub(crate) use script_context::{
    get_agent_chat_fork_picker_route_for_host, get_agent_chat_model_picker_route,
    get_agent_chat_model_picker_route_for_host, get_agent_chat_profile_picker_route,
    get_agent_chat_profile_picker_route_for_host, get_agent_chat_root_route,
    get_agent_chat_root_route_for_host, get_focused_text_agent_chat_root_route,
    AgentChatActionsDialogHost, AGENT_CHAT_CHANGE_MODEL_ACTION_ID,
    AGENT_CHAT_CHANGE_PROFILE_ACTION_ID, AGENT_CHAT_REWIND_ACTION_ID,
};
pub use scriptlet::get_scriptlet_context_actions_with_custom;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "builders_tests.rs"]
mod builders_extended_tests;
