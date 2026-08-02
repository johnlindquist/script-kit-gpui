//! Curated actions API surface for consumers that prefer explicit imports.
//!
//! This module keeps action-related imports discoverable without pulling every
//! helper from `actions` root exports.
#![allow(unused_imports)]

pub use super::builders::{
    ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo, NewChatPresetInfo,
    NoteSwitcherNoteInfo, NotesInfo,
};
pub use super::command_bar::{
    CommandBar, CommandBarActionCallback, CommandBarConfig, CommandBarHost,
};
pub use super::dialog::ActionsDialog;
pub use super::types::{
    Action, ActionCallback, ActionCategory, ActionsDialogConfig, AnchorPosition, CloseCallback,
    ScriptInfo, SearchPosition, SectionStyle,
};
pub use super::window::{
    close_actions_window, is_actions_window, is_actions_window_open, notify_actions_window,
    open_actions_window, WindowPosition,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prelude_exports_include_public_actions_types_for_consumers() {
        fn assert_type_available<T: 'static>() {
            let _ = std::any::TypeId::of::<T>();
        }

        assert_type_available::<Action>();
        assert_type_available::<ActionCallback>();
        assert_type_available::<ActionCategory>();
        assert_type_available::<ActionsDialog>();
        assert_type_available::<ActionsDialogConfig>();
        assert_type_available::<AnchorPosition>();
        assert_type_available::<ChatModelInfo>();
        assert_type_available::<ChatPromptInfo>();
        assert_type_available::<ClipboardEntryInfo>();
        assert_type_available::<CloseCallback>();
        assert_type_available::<CommandBar>();
        assert_type_available::<CommandBarActionCallback>();
        assert_type_available::<CommandBarConfig>();
        assert_type_available::<NewChatModelInfo>();
        assert_type_available::<NewChatPresetInfo>();
        assert_type_available::<NoteSwitcherNoteInfo>();
        assert_type_available::<NotesInfo>();
        assert_type_available::<ScriptInfo>();
        assert_type_available::<SearchPosition>();
        assert_type_available::<SectionStyle>();
        assert_type_available::<WindowPosition>();
        let _ = Option::<&dyn CommandBarHost>::None;
    }
}
