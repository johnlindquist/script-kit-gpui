use super::types::{Action, ActionCategory};
use crate::ai::context_contract::{
    context_attachment_specs, ContextAttachmentKind, CLEAR_CONTEXT_ACTION_ID,
    CLEAR_CONTEXT_ACTION_TITLE, CONTEXT_SECTION,
};
use crate::designs::icon_variations::IconName;

/// Information about a chat prompt for action building
#[derive(Debug, Clone)]
pub struct ChatPromptInfo {
    pub current_model: Option<String>,
    pub available_models: Vec<ChatModelInfo>,
    pub has_messages: bool,
    pub has_response: bool,
}

/// Information about an available chat model
#[derive(Debug, Clone)]
pub struct ChatModelInfo {
    pub id: String,
    pub display_name: String,
    pub provider: String,
}

fn has_missing_chat_model_fields(model: &ChatModelInfo) -> bool {
    model.id.trim().is_empty()
        || model.display_name.trim().is_empty()
        || model.provider.trim().is_empty()
}

/// Validate chat prompt info. Returns true if valid.
fn is_chat_prompt_info_valid(info: &ChatPromptInfo) -> bool {
    let has_blank_current_model = info
        .current_model
        .as_ref()
        .map(|model| model.trim().is_empty())
        .unwrap_or(false);
    if has_blank_current_model {
        tracing::warn!(
            target: "script_kit::actions",
            model_count = info.available_models.len(),
            has_messages = info.has_messages,
            has_response = info.has_response,
            "Invalid chat prompt info: current model name is blank; returning no actions"
        );
        return false;
    }
    let invalid_model_count = info
        .available_models
        .iter()
        .filter(|model| has_missing_chat_model_fields(model))
        .count();
    if invalid_model_count > 0 {
        tracing::warn!(
            target: "script_kit::actions",
            invalid_model_count,
            model_count = info.available_models.len(),
            has_messages = info.has_messages,
            has_response = info.has_response,
            "Invalid chat prompt info: model metadata missing required fields; returning no actions"
        );
        return false;
    }
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChatTranscriptActionPlan {
    NoTranscriptActions,
    CopyLastResponse,
    ClearConversation,
    CopyLastResponseAndClearConversation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChatModelPickerRowPlan {
    CurrentModel,
    AvailableModel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChatChangeModelActionPlan {
    CurrentModelSelected,
    NoCurrentModelSelected,
}

impl ChatTranscriptActionPlan {
    fn from_info(info: &ChatPromptInfo) -> Self {
        match (info.has_response, info.has_messages) {
            (false, false) => Self::NoTranscriptActions,
            (true, false) => Self::CopyLastResponse,
            (false, true) => Self::ClearConversation,
            (true, true) => Self::CopyLastResponseAndClearConversation,
        }
    }
}

impl ChatModelPickerRowPlan {
    fn from_model(info: &ChatPromptInfo, model: &ChatModelInfo) -> Self {
        if info
            .current_model
            .as_ref()
            .map(|current| current == &model.display_name)
            .unwrap_or(false)
        {
            Self::CurrentModel
        } else {
            Self::AvailableModel
        }
    }

    fn title(self, model: &ChatModelInfo, favorite: bool) -> String {
        let base = match self {
            Self::CurrentModel => format!("{} ✓", model.display_name),
            Self::AvailableModel => model.display_name.clone(),
        };
        if favorite {
            format!("★ {base}")
        } else {
            base
        }
    }

    fn description(self, model: &ChatModelInfo) -> String {
        match self {
            Self::CurrentModel | Self::AvailableModel => format!("Uses {}", model.provider),
        }
    }
}

impl ChatChangeModelActionPlan {
    fn from_info(info: &ChatPromptInfo) -> Self {
        if info.current_model.is_some() {
            Self::CurrentModelSelected
        } else {
            Self::NoCurrentModelSelected
        }
    }

    fn description(self, info: &ChatPromptInfo) -> String {
        match self {
            Self::CurrentModelSelected => format!(
                "Current: {}",
                info.current_model.as_deref().unwrap_or_default()
            ),
            Self::NoCurrentModelSelected => "Select a model".to_string(),
        }
    }
}

fn copy_last_response_action() -> Action {
    Action::new(
        "chat:copy_response",
        "Copy Last Response",
        Some("Copies the last assistant response".to_string()),
        ActionCategory::ScriptContext,
    )
    .with_shortcut("⇧⌘C")
    .with_icon(IconName::Copy)
}

fn clear_conversation_action() -> Action {
    Action::new(
        "chat:clear_conversation",
        "Clear Conversation",
        Some("Clears all messages".to_string()),
        ActionCategory::ScriptContext,
    )
    .with_shortcut("⌘⌫")
    .with_icon(IconName::Trash)
}

fn append_transcript_actions(actions: &mut Vec<Action>, plan: ChatTranscriptActionPlan) {
    match plan {
        ChatTranscriptActionPlan::NoTranscriptActions => {}
        ChatTranscriptActionPlan::CopyLastResponse => actions.push(copy_last_response_action()),
        ChatTranscriptActionPlan::ClearConversation => actions.push(clear_conversation_action()),
        ChatTranscriptActionPlan::CopyLastResponseAndClearConversation => {
            actions.push(copy_last_response_action());
            actions.push(clear_conversation_action());
        }
    }
}

// ── Chat route constants ─────────────────────────────────────────────

pub const CHAT_CHANGE_MODEL_ACTION_ID: &str = "chat:change_model";
pub const CHAT_ROOT_ROUTE_ID: &str = "chat:root";
pub const CHAT_MODEL_PICKER_ROUTE_ID: &str = "chat:model_picker";

/// Build the root route for a chat prompt's actions dialog.
#[allow(
    dead_code,
    reason = "the exported compatibility builder remains available to downstream callers and behavior tests"
)]
pub fn get_chat_root_route(info: &ChatPromptInfo) -> crate::actions::ActionsDialogRoute {
    get_chat_root_route_with_facts(
        info,
        crate::components::conversation_actions::chat_prompt_conversation_commands(
            false,
            false,
            info.has_response,
        ),
    )
}

pub(crate) fn get_chat_root_route_with_facts(
    info: &ChatPromptInfo,
    command_bindings: Vec<
        crate::components::conversation_actions::BoundConversationCommand<
            crate::components::conversation_actions::ChatPromptConversationCommand,
        >,
    >,
) -> crate::actions::ActionsDialogRoute {
    let context_title = info
        .current_model
        .clone()
        .unwrap_or_else(|| "Chat".to_string());

    crate::actions::ActionsDialogRoute {
        id: CHAT_ROOT_ROUTE_ID.to_string(),
        actions: get_chat_context_actions_with_facts(info, command_bindings),
        context_title: Some(context_title),
        search_placeholder: Some("Search chat actions...".to_string()),
        initial_selected_action_id: Some(CHAT_CHANGE_MODEL_ACTION_ID.to_string()),
    }
}

/// Build the model-picker route (second-level drill-down) for a chat prompt.
pub fn get_chat_model_picker_route(info: &ChatPromptInfo) -> crate::actions::ActionsDialogRoute {
    let initial_selected = info.current_model.as_ref().and_then(|current| {
        info.available_models
            .iter()
            .find(|m| &m.display_name == current)
            .map(|m| format!("chat:select_model_{}", m.id))
    });

    crate::actions::ActionsDialogRoute {
        id: CHAT_MODEL_PICKER_ROUTE_ID.to_string(),
        actions: get_chat_model_picker_actions(info),
        context_title: Some("Change Model".to_string()),
        search_placeholder: Some("Search models...".to_string()),
        initial_selected_action_id: initial_selected,
    }
}

/// Build the model-picker actions (second-level drill-down).
///
/// Returns flat `chat:select_model_*` rows with checkmark on the current model.
pub fn get_chat_model_picker_actions(info: &ChatPromptInfo) -> Vec<Action> {
    if !is_chat_prompt_info_valid(info) {
        return Vec::new();
    }

    let mut actions = Vec::new();
    for model in &info.available_models {
        let row_plan = ChatModelPickerRowPlan::from_model(info, model);

        let action = Action::new(
            format!("chat:select_model_{}", model.id),
            row_plan.title(
                model,
                crate::ai::agent_chat::ui::favorite_models::is_favorite_model_id(&model.id),
            ),
            Some(row_plan.description(model)),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::Settings);
        actions.push(action);
    }
    actions
}

/// Get actions specific to a chat prompt.
///
/// Returns root-level actions with a `chat:change_model` drill-down instead
/// of flat model rows. Use [`get_chat_model_picker_actions`] for model-level
/// actions.
#[allow(
    dead_code,
    reason = "the exported compatibility builder remains available to downstream callers and behavior tests"
)]
pub fn get_chat_context_actions(info: &ChatPromptInfo) -> Vec<Action> {
    get_chat_context_actions_with_facts(
        info,
        crate::components::conversation_actions::chat_prompt_conversation_commands(
            false,
            false,
            info.has_response,
        ),
    )
}

pub(crate) fn get_chat_context_actions_with_facts(
    info: &ChatPromptInfo,
    command_bindings: Vec<
        crate::components::conversation_actions::BoundConversationCommand<
            crate::components::conversation_actions::ChatPromptConversationCommand,
        >,
    >,
) -> Vec<Action> {
    if !is_chat_prompt_info_valid(info) {
        return Vec::new();
    }

    let mut actions = vec![Action::new(
        "chat:change_model",
        "Change Model",
        Some(ChatChangeModelActionPlan::from_info(info).description(info)),
        ActionCategory::ScriptContext,
    )
    .with_icon(IconName::Settings)];

    // Compatibility-only: these actions are retained for non-primary legacy
    // chat flows. The default AI surface is the harness terminal (Tab).
    actions.push(
        Action::new(
            "chat:continue_in_chat",
            "Continue in Harness Terminal",
            Some("Opens the PTY-backed harness terminal (compatibility)".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘↵")
        .with_icon(IconName::MessageCircle),
    );

    append_transcript_actions(&mut actions, ChatTranscriptActionPlan::from_info(info));

    actions.push(
        Action::new(
            "chat:capture_screen_area",
            "Capture Screen Area",
            Some("Select a screen region to attach as image".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::MagnifyingGlass),
    );

    actions.retain(|action| action.id != "chat:copy_response");
    for binding in command_bindings {
        use crate::components::conversation_actions::{
            ChatPromptConversationCommand, ConversationCommandAvailability, ConversationCommandId,
        };
        let (action_id, description, section, icon) = match binding.handler {
            ChatPromptConversationCommand::Send => (
                "chat:send",
                "Send the current message",
                "Conversation",
                IconName::ArrowRight,
            ),
            ChatPromptConversationCommand::Stop => (
                "chat:stop",
                "Stop the current response",
                "Conversation",
                IconName::Close,
            ),
            ChatPromptConversationCommand::Retry => (
                "chat:retry",
                "Retry the same prepared request",
                "Conversation",
                IconName::Refresh,
            ),
            ChatPromptConversationCommand::Dismiss => match binding.descriptor.id {
                ConversationCommandId::Back => (
                    "chat:back",
                    "Return to the conversation origin",
                    "Conversation",
                    IconName::ArrowRight,
                ),
                ConversationCommandId::Close => (
                    "chat:close",
                    "Close this conversation",
                    "Conversation",
                    IconName::Close,
                ),
                _ => unreachable!("ChatPrompt dismiss bindings validate Back or Close"),
            },
            ChatPromptConversationCommand::CopyLastResponse => (
                "chat:copy_response",
                "Copy the most recent assistant response",
                "Response",
                IconName::Copy,
            ),
        };
        let mut action = Action::new(
            action_id,
            binding.descriptor.label,
            Some(description.to_string()),
            ActionCategory::ScriptContext,
        )
        .with_section(section)
        .with_icon(icon);
        if let Some(shortcut) = binding.descriptor.shortcut {
            action = action.with_shortcut(shortcut);
        }
        if let ConversationCommandAvailability::Disabled { reason } =
            binding.descriptor.availability
        {
            action = action.disabled(reason.as_str());
        }
        actions.push(action);
    }

    actions
}

fn context_description(kind: ContextAttachmentKind) -> &'static str {
    match kind {
        ContextAttachmentKind::Current => "Adds minimal desktop context",
        ContextAttachmentKind::Full => "Adds full desktop context snapshot",
        ContextAttachmentKind::Selection => "Adds only the current selection",
        ContextAttachmentKind::Browser => "Adds the focused browser tab URL",
        ContextAttachmentKind::Window => "Adds focused window metadata",
        ContextAttachmentKind::Diagnostics => "Adds capture diagnostics for debugging",
        ContextAttachmentKind::Screenshot => "Adds a screenshot of the focused window",
        ContextAttachmentKind::Clipboard => "Adds recent clipboard history entries",
        ContextAttachmentKind::FrontmostApp => "Adds frontmost application info",
        ContextAttachmentKind::MenuBar => "Adds menu bar items from the frontmost app",
        ContextAttachmentKind::RecentScripts => "Adds the list of available scripts",
        ContextAttachmentKind::GitStatus => "Adds git status of the working directory",
        ContextAttachmentKind::GitDiff => "Adds git diff of staged and unstaged changes",
        ContextAttachmentKind::Processes => "Adds top running processes",
        ContextAttachmentKind::System => "Adds basic system information",
        ContextAttachmentKind::Dictation => "Adds voice dictation input",
        ContextAttachmentKind::Calendar => "Adds upcoming calendar events",
        ContextAttachmentKind::Notifications => "Adds recent notifications",
    }
}

fn context_icon(kind: ContextAttachmentKind) -> IconName {
    match kind {
        ContextAttachmentKind::Current => IconName::Code,
        ContextAttachmentKind::Full => IconName::FileCode,
        ContextAttachmentKind::Selection => IconName::File,
        ContextAttachmentKind::Browser => IconName::MagnifyingGlass,
        ContextAttachmentKind::Window => IconName::File,
        ContextAttachmentKind::Diagnostics => IconName::Settings,
        ContextAttachmentKind::Screenshot => IconName::File,
        ContextAttachmentKind::Clipboard => IconName::File,
        ContextAttachmentKind::FrontmostApp => IconName::File,
        ContextAttachmentKind::MenuBar => IconName::File,
        ContextAttachmentKind::RecentScripts => IconName::Code,
        ContextAttachmentKind::GitStatus => IconName::Code,
        ContextAttachmentKind::GitDiff => IconName::Code,
        ContextAttachmentKind::Processes => IconName::Settings,
        ContextAttachmentKind::System => IconName::Settings,
        ContextAttachmentKind::Dictation => IconName::File,
        ContextAttachmentKind::Calendar => IconName::File,
        ContextAttachmentKind::Notifications => IconName::File,
    }
}

/// Build the Context section actions from the canonical contract.
///
/// Returns exactly 7 actions: one per `ContextAttachmentSpec` (6) plus
/// Clear Context. The inspect action is deliberately excluded from the
/// command-bar surface — it remains reachable via its keyboard shortcut.
fn context_actions() -> Vec<Action> {
    let mut actions = Vec::with_capacity(7);

    for spec in context_attachment_specs() {
        actions.push(
            Action::new(
                spec.action_id,
                spec.action_title,
                Some(context_description(spec.kind).to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(context_icon(spec.kind))
            .with_section(CONTEXT_SECTION),
        );
    }

    actions.push(
        Action::new(
            CLEAR_CONTEXT_ACTION_ID,
            CLEAR_CONTEXT_ACTION_TITLE,
            Some("Removes all pending context chips".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::Trash)
        .with_section(CONTEXT_SECTION),
    );

    actions
}

/// Get actions for the AI chat command bar (Cmd+K menu).
#[allow(dead_code)]
pub fn get_ai_command_bar_actions() -> Vec<Action> {
    let mut actions = vec![
        Action::new(
            "chat:copy_response",
            "Copy Response",
            Some("Copies the latest AI response".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⇧⌘C")
        .with_icon(IconName::Copy)
        .with_section("Response"),
        Action::new(
            "chat:copy_chat",
            "Copy Chat",
            Some("Copies the full conversation".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌥⇧⌘C")
        .with_icon(IconName::Copy)
        .with_section("Response"),
        Action::new(
            "chat:copy_last_code",
            "Copy Last Code Block",
            Some("Copies the latest code block".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌥⌘C")
        .with_icon(IconName::Code)
        .with_section("Response"),
        Action::new(
            "chat:submit",
            "Submit",
            Some("Sends your message".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("↵")
        .with_icon(IconName::ArrowUp)
        .with_section("Actions"),
        Action::new(
            "chat:new_chat",
            "New Chat",
            Some("Starts a new conversation".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘N")
        .with_icon(IconName::Plus)
        .with_section("Actions"),
        Action::new(
            "chat:delete_chat",
            "Delete Chat",
            Some("Deletes the current conversation".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘⌫")
        .with_icon(IconName::Trash)
        .with_section("Actions"),
        Action::new(
            "chat:add_file",
            "Add File",
            Some("Browse for a file to attach".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⇧⌘A")
        .with_icon(IconName::File)
        .with_section("Attachments"),
        Action::new(
            "chat:add_image",
            "Add Image",
            Some("Browse for an image to attach".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::File)
        .with_section("Attachments"),
        Action::new(
            "chat:paste_image",
            "Paste Image from Clipboard",
            Some("Pastes an image from the clipboard".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘V")
        .with_icon(IconName::File)
        .with_section("Attachments"),
        Action::new(
            "chat:capture_screen_area",
            "Capture Screen Area",
            Some("Select a screen region to attach as image".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::MagnifyingGlass)
        .with_section("Attachments"),
        Action::new(
            "chat:export_markdown",
            "Copy Chat as Markdown",
            Some("Copy chat as Markdown to the clipboard".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⇧⌘E")
        .with_icon(IconName::FileCode)
        .with_section("Export"),
    ];

    // === Context Section — derived from the canonical contract ===
    actions.extend(context_actions());

    // Inspect action: toggle for the context preflight inspector panel.
    actions.push(
        Action::new(
            "chat:inspect_context",
            "Inspect Context Receipt",
            Some("Shows the last context preflight audit as machine-readable JSON".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("\u{2325}\u{2318}I")
        .with_icon(IconName::FileCode)
        .with_section(CONTEXT_SECTION),
    );

    actions.extend([
        Action::new(
            "chat:branch_from_last",
            "Branch from Last Message",
            Some("Creates a new chat from the last message".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::ArrowRight)
        .with_section("Actions"),
        Action::new(
            "chat:toggle_favorite_model",
            "Favorite Current Model",
            Some("Stars or unstars the current AI model".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("\u{2325}\u{2318}F")
        .with_icon(IconName::Star)
        .with_section("Actions"),
        Action::new(
            "chat:cycle_favorite_model",
            "Cycle Favorite Model",
            Some("Switches to the next favorited model".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("\u{2325}\u{2318}M")
        .with_icon(IconName::Settings)
        .with_section("Actions"),
        Action::new(
            "chat:expand_composer",
            "Expand Composer",
            Some("Toggles a taller multi-line composer".to_string()),
            ActionCategory::ScriptContext,
        )
        // ⌥⌘E: ⇧⌘E is already taken by chat:export_markdown; keep the ⌥⌘
        // pattern of the sibling new actions (⌥⌘F favorite, ⌥⌘M cycle model).
        .with_shortcut("\u{2325}\u{2318}E")
        .with_icon(IconName::Settings)
        .with_section("Actions"),
        Action::new(
            "chat:toggle_shortcuts_help",
            "Keyboard Shortcuts",
            Some("Shows keyboard shortcuts".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘/")
        .with_icon(IconName::Star)
        .with_section("Help"),
        Action::new(
            "chat:change_model",
            "Change Model",
            Some("Selects a different AI model".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::Settings)
        .with_section("Settings"),
        Action::new(
            "chat:toggle_window_mode",
            "Toggle Mini / Full Mode",
            Some("Switches between compact and full AI window".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("\u{21e7}\u{2318}M")
        .with_icon(IconName::Settings)
        .with_section("Settings"),
    ]);

    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_prompt_info() -> ChatPromptInfo {
        ChatPromptInfo {
            current_model: Some("GPT-5".to_string()),
            available_models: vec![
                ChatModelInfo {
                    id: "gpt-5".to_string(),
                    display_name: "GPT-5".to_string(),
                    provider: "OpenAI".to_string(),
                },
                ChatModelInfo {
                    id: "claude-4".to_string(),
                    display_name: "Claude 4".to_string(),
                    provider: "Anthropic".to_string(),
                },
            ],
            has_messages: true,
            has_response: true,
        }
    }

    #[test]
    fn test_get_chat_context_actions_prefixes_ids_with_chat_namespace() {
        let actions = get_chat_context_actions(&sample_prompt_info());
        assert!(actions.iter().all(|action| action.id.starts_with("chat:")));
    }

    #[test]
    fn test_get_ai_command_bar_actions_prefixes_ids_with_chat_namespace() {
        let actions = get_ai_command_bar_actions();
        assert!(actions.iter().all(|action| action.id.starts_with("chat:")));
    }

    #[test]
    fn test_get_ai_command_bar_actions_exposes_direct_attachment_actions() {
        let actions = get_ai_command_bar_actions();

        let add_file = actions
            .iter()
            .find(|action| action.id == "chat:add_file")
            .expect("missing add_file action");
        let add_image = actions
            .iter()
            .find(|action| action.id == "chat:add_image")
            .expect("missing add_image action");

        assert_eq!(add_file.title, "Add File");
        assert_eq!(
            add_file.description.as_deref(),
            Some("Browse for a file to attach")
        );
        assert_eq!(add_file.icon, Some(IconName::File));
        assert_eq!(add_file.section.as_deref(), Some("Attachments"));

        assert_eq!(add_image.title, "Add Image");
        assert_eq!(
            add_image.description.as_deref(),
            Some("Browse for an image to attach")
        );
        assert_eq!(add_image.icon, Some(IconName::File));
        assert_eq!(add_image.section.as_deref(), Some("Attachments"));

        assert!(actions
            .iter()
            .all(|action| action.id != "chat:add_attachment"));
    }

    #[test]
    fn test_get_chat_context_actions_returns_empty_when_model_metadata_missing() {
        let mut info = sample_prompt_info();
        info.available_models[0].id = "   ".to_string();

        let actions = get_chat_context_actions(&info);

        assert!(actions.is_empty());
    }

    #[test]
    fn test_get_chat_context_actions_returns_empty_when_current_model_is_blank() {
        let mut info = sample_prompt_info();
        info.current_model = Some("   ".to_string());

        let actions = get_chat_context_actions(&info);

        assert!(actions.is_empty());
    }

    #[test]
    fn test_get_chat_context_actions_assigns_consistent_primary_icons() {
        let actions = get_chat_context_actions(&sample_prompt_info());

        let change_model = actions
            .iter()
            .find(|action| action.id == "chat:change_model")
            .expect("missing change_model action");
        let continue_in_chat = actions
            .iter()
            .find(|action| action.id == "chat:continue_in_chat")
            .expect("missing continue_in_chat action");
        let copy_response = actions
            .iter()
            .find(|action| action.id == "chat:copy_response")
            .expect("missing copy_response action");
        let clear_conversation = actions
            .iter()
            .find(|action| action.id == "chat:clear_conversation")
            .expect("missing clear_conversation action");

        assert_eq!(change_model.icon, Some(IconName::Settings));
        assert_eq!(continue_in_chat.icon, Some(IconName::MessageCircle));
        assert_eq!(copy_response.icon, Some(IconName::Copy));
        assert_eq!(clear_conversation.icon, Some(IconName::Trash));
    }
}
