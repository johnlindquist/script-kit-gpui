#![allow(unused_imports)]

use super::*;

mod builtin_dialog_schema_validation;
mod builtin_dialog_field_rules;
mod builtin_dialog_examples;
mod builtin_dialog_regressions;
mod table_driven_validation;

/// The compatibility builder represents an idle standalone chat with an empty
/// composer, installed Send/Close handlers, and models in a separate picker.
/// Compare semantic membership (including duplicates), not a historical row count.
pub(super) fn assert_chat_root_contract(info: &ChatPromptInfo, actions: &[Action]) {
    use crate::components::conversation_actions::{
        conversation_command_descriptor, ConversationCommandAvailability,
        ConversationCommandDisabledReason, ConversationCommandId,
    };

    let mut expected_ids = vec![
        "chat:change_model",
        "chat:continue_in_chat",
        "chat:capture_screen_area",
        "chat:send",
        "chat:close",
    ];
    if info.has_messages {
        expected_ids.push("chat:clear_conversation");
    }
    if info.has_response {
        expected_ids.push("chat:copy_response");
    }
    expected_ids.sort_unstable();
    let mut actual_ids: Vec<_> = actions.iter().map(|action| action.id.as_str()).collect();
    actual_ids.sort_unstable();
    assert_eq!(actual_ids, expected_ids, "chat root membership for {info:?}");

    for action in actions {
        let (command, availability) = match action.id.as_str() {
            "chat:send" => (
                ConversationCommandId::Send,
                ConversationCommandAvailability::disabled(
                    ConversationCommandDisabledReason::TypeMessageFirst,
                ),
            ),
            "chat:close" => (
                ConversationCommandId::Close,
                ConversationCommandAvailability::Enabled,
            ),
            "chat:copy_response" => (
                ConversationCommandId::CopyLastResponse,
                ConversationCommandAvailability::Enabled,
            ),
            _ => {
                assert_eq!(
                    action.disabled_reason(),
                    None,
                    "{} must be enabled",
                    action.id
                );
                continue;
            }
        };
        let descriptor = conversation_command_descriptor(command, availability);
        assert_eq!(action.title, descriptor.label, "{} label", action.id);
        assert_eq!(
            action.shortcut.as_deref(),
            descriptor.shortcut,
            "{} shortcut",
            action.id
        );
        assert_eq!(
            action.disabled_reason(),
            descriptor.availability.disabled_reason(),
            "{} availability",
            action.id
        );
    }
}
