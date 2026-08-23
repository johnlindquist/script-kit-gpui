/// Dispatch and execution of user-triggered actions (copy, paste, reveal, terminal, AI, etc.).
use crate::action_helpers::{ActionOutcomeStatus, DispatchContext, DispatchOutcome};
use crate::ai::agent_chat::ui::export::build_agent_chat_conversation_markdown_from_thread;

/// A code block extracted from markdown with optional language hint.
struct CodeBlock {
    code: String,
    language: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentChatLastResponseHandlerAction {
    CopyToClipboard,
    PasteToFrontmost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentChatConversationSessionHandlerAction {
    NewConversation,
    ClearConversation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentChatRetryLastHandlerAction {
    RetryLastMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentChatCodeCopyHandlerAction {
    CopyAllCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentChatConversationMarkdownHandlerAction {
    CopyToClipboard,
    SaveAsNote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentChatConversationMarkdownBlockedReason {
    NoMessages,
    EmptyRenderableMessages,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentChatLastCodeBlockHandlerAction {
    SaveAsScript,
    RunLastCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentChatPanelWindowHandlerAction {
    ShowHistory,
    DetachWindow,
    ReattachPanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentChatHistoryMutationHandlerAction {
    ClearHistory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AsyncExternalToolFeedbackAction {
    RevealInFileManager,
    LaunchEditor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentChatModelSwitchHandlerAction {
    SwitchModel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentChatProfileSwitchHandlerAction {
    SwitchProfile,
}

impl AgentChatLastResponseHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "agent_chat_copy_last_response" => Some(Self::CopyToClipboard),
            "agent_chat_paste_to_frontmost" => Some(Self::PasteToFrontmost),
            _ => None,
        }
    }

    fn success_message(self) -> &'static str {
        match self {
            Self::CopyToClipboard => "Copied last response to clipboard",
            Self::PasteToFrontmost => "Pasting to frontmost app\u{2026}",
        }
    }
}

impl AsyncExternalToolFeedbackAction {
    fn failure_message(self, tool_name: &str, error: impl std::fmt::Display) -> String {
        match self {
            Self::RevealInFileManager => format!("Failed to reveal in {tool_name}: {error}"),
            Self::LaunchEditor => format!("Failed to open in {tool_name}: {error}"),
        }
    }
}

impl AgentChatConversationSessionHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "agent_chat_new_conversation" => Some(Self::NewConversation),
            "agent_chat_clear_conversation" => Some(Self::ClearConversation),
            _ => None,
        }
    }

    fn preserves_session(self) -> bool {
        match self {
            Self::NewConversation => true,
            Self::ClearConversation => false,
        }
    }
}

impl AgentChatRetryLastHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "agent_chat_retry_last" => Some(Self::RetryLastMessage),
            _ => None,
        }
    }

    fn missing_user_message(self) -> &'static str {
        match self {
            Self::RetryLastMessage => "No previous message to retry",
        }
    }
}

impl AgentChatCodeCopyHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "agent_chat_copy_all_code" => Some(Self::CopyAllCode),
            _ => None,
        }
    }

    fn result_message(self, copied_any_code: bool) -> &'static str {
        match (self, copied_any_code) {
            (Self::CopyAllCode, true) => "All code blocks copied",
            (Self::CopyAllCode, false) => "No code blocks found",
        }
    }
}

impl AgentChatConversationMarkdownHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "agent_chat_export_markdown" => Some(Self::CopyToClipboard),
            "agent_chat_save_as_note" => Some(Self::SaveAsNote),
            _ => None,
        }
    }

    fn empty_message(self) -> &'static str {
        match self {
            Self::CopyToClipboard => "No Agent Chat messages to copy",
            Self::SaveAsNote => "No Agent Chat messages to save",
        }
    }

    fn success_message(self) -> &'static str {
        match self {
            Self::CopyToClipboard => "Copied Agent Chat conversation as markdown",
            Self::SaveAsNote => "Saved Agent Chat conversation to Notes",
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> Option<String> {
        match self {
            Self::CopyToClipboard => None,
            Self::SaveAsNote => Some(format!("Failed to save note: {error}")),
        }
    }

    fn blocked_reason(self, message_count: usize) -> AgentChatConversationMarkdownBlockedReason {
        let _ = self;
        AgentChatConversationMarkdownBlockedReason::from_message_count(message_count)
    }
}

impl AgentChatConversationMarkdownBlockedReason {
    fn from_message_count(message_count: usize) -> Self {
        match message_count {
            0 => Self::NoMessages,
            _ => Self::EmptyRenderableMessages,
        }
    }

    fn trace_value(self) -> &'static str {
        match self {
            Self::NoMessages => "no_messages",
            Self::EmptyRenderableMessages => "empty_renderable_messages",
        }
    }
}

impl AgentChatLastCodeBlockHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "agent_chat_save_as_script" => Some(Self::SaveAsScript),
            "agent_chat_run_last_code" => Some(Self::RunLastCode),
            _ => None,
        }
    }

    fn missing_code_message(self) -> &'static str {
        match self {
            Self::SaveAsScript => "No code block found in last response",
            Self::RunLastCode => "No code block found",
        }
    }

    fn saved_script_message(self, name: &str, ext: &str) -> Option<String> {
        match self {
            Self::SaveAsScript => Some(format!("Saved as {name}.{ext}")),
            Self::RunLastCode => None,
        }
    }

    fn temp_write_failure_message(self, error: impl std::fmt::Display) -> Option<String> {
        match self {
            Self::RunLastCode => Some(format!("Failed to write temp file: {error}")),
            Self::SaveAsScript => None,
        }
    }

    fn running_message(self, name: &str) -> Option<String> {
        match self {
            Self::RunLastCode => Some(format!("Running `{name}`...")),
            Self::SaveAsScript => None,
        }
    }

    fn run_success_message(self, stdout: &str) -> Option<String> {
        match self {
            Self::RunLastCode => {
                if stdout.is_empty() {
                    Some("Finished (no output)".to_string())
                } else {
                    Some(format!("```\n{stdout}\n```"))
                }
            }
            Self::SaveAsScript => None,
        }
    }

    fn run_failure_message(self, status: std::process::ExitStatus, output: &str) -> Option<String> {
        match self {
            Self::RunLastCode => Some(format!("Error (exit {status}):\n```\n{output}\n```")),
            Self::SaveAsScript => None,
        }
    }

    fn run_spawn_failure_message(self, error: impl std::fmt::Display) -> Option<String> {
        match self {
            Self::RunLastCode => Some(format!("Failed to run: {error}")),
            Self::SaveAsScript => None,
        }
    }
}

impl AgentChatPanelWindowHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "agent_chat_show_history" => Some(Self::ShowHistory),
            "agent_chat_detach_window" => Some(Self::DetachWindow),
            "agent_chat_reattach_panel" => Some(Self::ReattachPanel),
            _ => None,
        }
    }

    fn success_message(self) -> Option<&'static str> {
        match self {
            Self::ShowHistory => Some("Opened conversation history"),
            Self::DetachWindow => Some("Chat kept open in window"),
            Self::ReattachPanel => Some("Chat returned to panel"),
        }
    }

    fn history_search_placeholder(self) -> Option<&'static str> {
        match self {
            Self::ShowHistory => Some("Search conversation history..."),
            Self::DetachWindow | Self::ReattachPanel => None,
        }
    }
}

impl AgentChatHistoryMutationHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "agent_chat_clear_history" => Some(Self::ClearHistory),
            _ => None,
        }
    }

    fn history_index_path(self, kit: &std::path::Path) -> std::path::PathBuf {
        match self {
            Self::ClearHistory => kit.join("agent_chat-history.jsonl"),
        }
    }

    fn conversations_dir(self, kit: &std::path::Path) -> std::path::PathBuf {
        match self {
            Self::ClearHistory => kit.join("agent_chat-conversations"),
        }
    }

    fn success_message(self) -> &'static str {
        match self {
            Self::ClearHistory => "Conversation history cleared",
        }
    }
}

impl AgentChatModelSwitchHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        crate::actions::agent_chat_switch_model_id_from_action(action_id).map(|_| Self::SwitchModel)
    }

    fn unavailable_message(self, model_id: &str) -> String {
        match self {
            Self::SwitchModel => format!("Model '{model_id}' is no longer available"),
        }
    }

    fn already_selected_message(self, display_name: &str) -> String {
        match self {
            Self::SwitchModel => format!("Already using {display_name}"),
        }
    }

    fn hud_message(self, display_name: &str) -> String {
        match self {
            Self::SwitchModel => format!("Model: {display_name}"),
        }
    }

    fn switched_message(self, display_name: &str) -> String {
        match self {
            Self::SwitchModel => format!("Switched model to {display_name}"),
        }
    }
}

impl AgentChatProfileSwitchHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        crate::actions::agent_chat_switch_profile_id_from_action(action_id)
            .map(|_| Self::SwitchProfile)
    }

    fn unavailable_message(self, profile_id: &str) -> String {
        match self {
            Self::SwitchProfile => format!("Profile '{profile_id}' is no longer available"),
        }
    }

    fn persist_failure_message(self, profile_name: &str, error: impl std::fmt::Display) -> String {
        match self {
            Self::SwitchProfile => format!("Failed to persist profile '{profile_name}': {error}"),
        }
    }

    fn relaunch_message(self, profile_name: &str) -> String {
        match self {
            Self::SwitchProfile => format!("Switching profile to {profile_name}\u{2026}"),
        }
    }

    fn selected_message(self, profile_name: &str) -> String {
        match self {
            Self::SwitchProfile => format!("Profile: {profile_name}"),
        }
    }
}

/// Extract the last fenced code block (```lang\n...\n```) from markdown text.
fn extract_last_code_block(text: &str) -> Option<String> {
    extract_last_code_block_with_lang(text).map(|b| b.code)
}

/// Extract the last fenced code block with language hint.
fn extract_last_code_block_with_lang(text: &str) -> Option<CodeBlock> {
    let mut last_block: Option<CodeBlock> = None;
    let mut in_block = false;
    let mut current_code = String::new();
    let mut current_lang: Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some(fence_suffix) = trimmed.strip_prefix("```") {
            if in_block {
                last_block = Some(CodeBlock {
                    code: current_code.clone(),
                    language: current_lang.clone(),
                });
                current_code.clear();
                current_lang = None;
                in_block = false;
            } else {
                in_block = true;
                current_code.clear();
                // Parse language from ```typescript or ```ts etc.
                let lang = fence_suffix.trim();
                current_lang = if lang.is_empty() {
                    None
                } else {
                    Some(lang.to_string())
                };
            }
        } else if in_block {
            if !current_code.is_empty() {
                current_code.push('\n');
            }
            current_code.push_str(line);
        }
    }
    last_block
}

// Action dispatch facade.
//
// This module splits the monolithic action handler into semantic submodules:
//   - clipboard.rs:  all clipboard_* actions
//   - scripts.rs:    script management (create, edit, remove, settings, quit)
//   - shortcuts.rs:  shortcut and alias configuration
//   - files.rs:      file search, reveal, copy path/deeplink
//   - scriptlets.rs: scriptlet editing, reveal, and dynamic actions

/// Maximum number of clipboard entries to cache for the clipboard history view.
const CLIPBOARD_CACHE_SIZE: usize = 100;

enum DeferredAgentChatAction {
    OpenOnly,
    SetInput {
        text: String,
        submit: bool,
    },
    SetInputWithImage {
        text: String,
        image_base64: String,
        submit: bool,
    },
    AddAttachment {
        path: String,
    },
    ApplyPreset {
        preset_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeferredAgentChatActionKind {
    OpenOnly,
    SetInput,
    SetInputSubmit,
    SetInputWithImage,
    SetInputWithImageSubmit,
    AddAttachment,
    ApplyPreset,
}

impl DeferredAgentChatActionKind {
    fn name(self) -> &'static str {
        match self {
            Self::OpenOnly => "open_only",
            Self::SetInputSubmit => "set_input_submit",
            Self::SetInput => "set_input",
            Self::SetInputWithImageSubmit => "set_input_with_image_submit",
            Self::SetInputWithImage => "set_input_with_image",
            Self::AddAttachment => "add_attachment",
            Self::ApplyPreset => "apply_preset",
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::OpenOnly => format!("Failed to open Agent Chat: {error}"),
            Self::AddAttachment => format!("Failed to attach file to Agent Chat: {error}"),
            Self::ApplyPreset => format!("Failed to apply AI preset: {error}"),
            Self::SetInput
            | Self::SetInputSubmit
            | Self::SetInputWithImage
            | Self::SetInputWithImageSubmit => format!("Failed to send to Agent Chat: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeferredAiImageAttachmentStage {
    DecodeClipboardImage,
    WriteClipboardImage,
}

impl DeferredAiImageAttachmentStage {
    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::DecodeClipboardImage => format!("Failed to decode image attachment: {error}"),
            Self::WriteClipboardImage => format!("Failed to write image attachment: {error}"),
        }
    }
}

impl DeferredAgentChatAction {
    fn kind(&self) -> DeferredAgentChatActionKind {
        match self {
            Self::OpenOnly => DeferredAgentChatActionKind::OpenOnly,
            Self::SetInput { submit: true, .. } => DeferredAgentChatActionKind::SetInputSubmit,
            Self::SetInput { submit: false, .. } => DeferredAgentChatActionKind::SetInput,
            Self::SetInputWithImage { submit: true, .. } => {
                DeferredAgentChatActionKind::SetInputWithImageSubmit
            }
            Self::SetInputWithImage { submit: false, .. } => {
                DeferredAgentChatActionKind::SetInputWithImage
            }
            Self::AddAttachment { .. } => DeferredAgentChatActionKind::AddAttachment,
            Self::ApplyPreset { .. } => DeferredAgentChatActionKind::ApplyPreset,
        }
    }

    fn apply_to_agent_chat(
        self,
        entity: Entity<crate::ai::agent_chat::ui::AgentChatView>,
        cx: &mut App,
    ) -> Result<&'static str, String> {
        entity.update(cx, move |chat, cx| match self {
            Self::OpenOnly => Ok("open_only"),
            Self::SetInput { text, submit } => {
                if chat.is_setup_mode() {
                    return Err("Agent Chat is in setup mode".to_string());
                }
                chat.set_input(text, cx);
                if submit {
                    let Some(thread) = chat.thread() else {
                        return Err("Agent Chat thread unavailable".to_string());
                    };
                    thread
                        .update(cx, |thread, cx| thread.submit_input(cx))
                        .map_err(|error| error.to_string())?;
                }
                Ok("set_input")
            }
            Self::SetInputWithImage {
                text,
                image_base64,
                submit,
            } => {
                if chat.is_setup_mode() {
                    return Err("Agent Chat is in setup mode".to_string());
                }

                use base64::Engine as _;

                let png_bytes = base64::engine::general_purpose::STANDARD
                    .decode(image_base64)
                    .map_err(|error| {
                        DeferredAiImageAttachmentStage::DecodeClipboardImage.failure_message(error)
                    })?;
                let temp_path = std::env::temp_dir().join(format!(
                    "script-kit-agent_chat-clipboard-{}.png",
                    uuid::Uuid::new_v4()
                ));
                std::fs::write(&temp_path, png_bytes).map_err(|error| {
                    DeferredAiImageAttachmentStage::WriteClipboardImage.failure_message(error)
                })?;
                let path = temp_path.to_string_lossy().into_owned();

                chat.live_thread()
                    .update(cx, |thread, cx| {
                        thread.add_context_part(
                            crate::ai::AiContextPart::FilePath {
                                path,
                                label: "Clipboard Image".to_string(),
                            },
                            cx,
                        );
                        thread.set_input(text, cx);
                        if submit {
                            thread.submit_input(cx)?;
                        }
                        Ok::<(), String>(())
                    })
                    .map_err(|error| error.to_string())?;

                Ok("set_input_with_image")
            }
            Self::AddAttachment { path } => {
                if chat.is_setup_mode() {
                    return Err("Agent Chat is in setup mode".to_string());
                }

                let label = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.to_string())
                    .unwrap_or_else(|| path.clone());

                chat.live_thread().update(cx, |thread, cx| {
                    thread.add_context_part(crate::ai::AiContextPart::FilePath { path, label }, cx);
                });

                Ok("add_attachment")
            }
            Self::ApplyPreset { preset_id } => {
                chat.apply_preset_by_id(&preset_id, cx)?;
                Ok("apply_preset")
            }
        })
    }
}

impl ScriptListApp {
    /// Show an error toast and call cx.notify() to ensure the UI updates.
    ///
    /// Consolidates the repeated pattern of pushing an error toast, setting the
    /// duration to TOAST_ERROR_MS, and calling cx.notify().
    ///
    /// The optional `error_code` is logged for machine-readable diagnostics but
    /// never shown to the user.  Use the stable constants from
    /// `crate::action_helpers` (e.g. `ERROR_LAUNCH_FAILED`).
    fn show_error_toast(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.show_error_toast_with_code(message, None, cx);
    }

    /// Like `show_error_toast` but also logs a stable error code.
    fn show_error_toast_with_code(
        &mut self,
        message: impl Into<String>,
        error_code: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let msg: String = message.into();
        if let Some(code) = error_code {
            tracing::warn!(
                error_code = code,
                message = %msg,
                "Action error"
            );
        }
        self.toast_manager.push(
            components::toast::Toast::error(msg, &self.theme).duration_ms(Some(TOAST_ERROR_MS)),
        );
        cx.notify();
    }

    /// Copy text to the system clipboard with consistent success/error feedback.
    ///
    /// On success, shows a HUD with the given message and optionally hides the
    /// main window. On failure, shows an error toast.
    fn copy_to_clipboard_with_feedback(
        &mut self,
        text: &str,
        success_message: String,
        close_after: bool,
        cx: &mut Context<Self>,
    ) {
        let copy_result = {
            #[cfg(target_os = "macos")]
            {
                self.pbcopy(text)
                    .map_err(|e| format!("Clipboard write failed: {}", e))
            }

            #[cfg(not(target_os = "macos"))]
            {
                use arboard::Clipboard;
                Clipboard::new()
                    .and_then(|mut c| c.set_text(text))
                    .map_err(|e| format!("Clipboard write failed: {}", e))
            }
        };

        match copy_result {
            Ok(()) => {
                self.show_hud(success_message, Some(HUD_MEDIUM_MS), cx);
                if close_after {
                    self.hide_main_and_reset(cx);
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Clipboard write failed");
                self.show_error_toast("Failed to copy to clipboard", cx);
            }
        }
    }

    /// Show a consistent "not supported on this platform" warning toast.
    ///
    /// Uses Toast::warning (not error) per the feedback matrix — unsupported
    /// platform is a warning, not an error.  Internally logs with the
    /// `unsupported_platform` error code.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    fn show_unsupported_platform_toast(&mut self, feature: &str, cx: &mut Context<Self>) {
        tracing::warn!(
            error_code = crate::action_helpers::ERROR_UNSUPPORTED_PLATFORM,
            feature = feature,
            "Unsupported platform"
        );
        self.toast_manager.push(
            components::toast::Toast::warning(unsupported_platform_message(feature), &self.theme)
                .duration_ms(Some(TOAST_WARNING_MS)),
        );
        cx.notify();
    }

    pub(crate) fn hide_main_and_reset(&self, cx: &mut Context<Self>) {
        if let Some((x, y, w, h)) = platform::get_main_window_bounds() {
            let bounds = crate::window_state::PersistedWindowBounds::new(x, y, w, h);
            let displays = platform::get_macos_displays();
            let _ =
                crate::window_state::save_main_position_with_display_detection(bounds, &displays);
        }
        set_main_window_visible(false);
        NEEDS_RESET.store(true, Ordering::SeqCst);
        // Use deferred platform-specific hide that only hides the main window,
        // not the entire app (cx.hide() would hide HUD too).
        // Must be deferred to avoid RefCell reentrancy from macOS callbacks.
        platform::defer_hide_main_window(cx);
    }

    fn open_agent_chat_after_main_hide(
        &mut self,
        source_action: &str,
        trace_id: &str,
        deferred_action: DeferredAgentChatAction,
        cx: &mut Context<Self>,
    ) {
        self.hide_main_and_reset(cx);
        self.open_agent_chat_after_already_hidden(source_action, trace_id, deferred_action, cx);
    }

    fn open_agent_chat_after_already_hidden(
        &mut self,
        source_action: &str,
        trace_id: &str,
        deferred_action: DeferredAgentChatAction,
        cx: &mut Context<Self>,
    ) {
        let source_action = source_action.to_string();
        let trace_id = trace_id.to_string();
        let deferred_action_kind = deferred_action.kind();
        let deferred_action_name = deferred_action_kind.name();

        tracing::info!(
            category = "AI",
            event = "ai_handoff_defer_open_start",
            source_action = %source_action,
            trace_id = %trace_id,
            deferred_action = deferred_action_name,
            "Opening deferred chat handoff after main window already hidden"
        );

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1))
                .await;

            let started_at = std::time::Instant::now();

            let open_result = match this.update(cx, |this, cx| {
                this.open_tab_ai_agent_chat_with_entry_intent(None, cx);
                Ok::<(), String>(())
            }) {
                Ok(result) => result,
                Err(error) => Err(error.to_string()),
            };

            if open_result.is_ok() {
                let ready_now = this
                    .update(cx, |this, _cx| this.active_agent_chat_entity().is_some())
                    .unwrap_or(false);
                if !ready_now {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(16))
                        .await;
                }
            }

            let handoff_result = open_result.and_then(|()| {
                this.update(cx, |this, cx| {
                    let Some(entity) = this.active_agent_chat_entity() else {
                        return Err("Agent Chat not ready after open".to_string());
                    };
                    deferred_action.apply_to_agent_chat(entity, cx)
                })
                .map_err(|error| error.to_string())?
            });

            match handoff_result {
                Ok(apply_stage) => {
                    let _ = this.update(cx, |_this, cx| {
                        tracing::info!(
                            category = "AI",
                            event = "ai_handoff_defer_open_success",
                            source_action = %source_action,
                            trace_id = %trace_id,
                            deferred_action = deferred_action_name,
                            apply_stage,
                            duration_ms = started_at.elapsed().as_millis() as u64,
                            "AI handoff completed"
                        );
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        tracing::error!(
                            category = "AI",
                            event = "ai_handoff_defer_open_failed",
                            source_action = %source_action,
                            trace_id = %trace_id,
                            deferred_action = deferred_action_name,
                            error = %error,
                            duration_ms = started_at.elapsed().as_millis() as u64,
                            "Failed to complete deferred chat handoff after hiding main window"
                        );
                        this.show_error_toast(deferred_action_kind.failure_message(&error), cx);
                    });
                }
            }
        })
        .detach();
    }

    fn active_agent_chat_entity(&self) -> Option<Entity<crate::ai::agent_chat::ui::AgentChatView>> {
        crate::ai::agent_chat::ui::chat_window::get_detached_agent_chat_view_entity().or_else(
            || {
                let AppView::AgentChatView { entity } = &self.current_view else {
                    return None;
                };
                Some(entity.clone())
            },
        )
    }

    /// Reveal a path and return completion back to the UI thread for HUD feedback.
    fn reveal_in_finder_with_feedback_async(
        &self,
        path: &std::path::Path,
        trace_id: &str,
    ) -> async_channel::Receiver<Result<(), String>> {
        let path_str = path.to_string_lossy().to_string();
        let trace_id = trace_id.to_string();
        let (result_tx, result_rx) = async_channel::bounded::<Result<(), String>>(1);

        std::thread::spawn(move || {
            let file_manager = if cfg!(target_os = "macos") {
                "Finder"
            } else if cfg!(target_os = "windows") {
                "Explorer"
            } else {
                "File Manager"
            };
            let feedback_action = AsyncExternalToolFeedbackAction::RevealInFileManager;

            tracing::info!(
                category = "UI",
                event = "action_reveal_in_finder_start",
                trace_id = %trace_id,
                file_manager,
                path = %path_str,
                "Reveal in file manager started"
            );

            let reveal_result = match crate::file_search::reveal_in_finder(&path_str) {
                Ok(()) => {
                    tracing::info!(
                        category = "UI",
                        event = "action_reveal_in_finder_success",
                        trace_id = %trace_id,
                        file_manager,
                        path = %path_str,
                        "Reveal in file manager succeeded"
                    );
                    Ok(())
                }
                Err(error) => {
                    tracing::error!(
                        event = "action_reveal_in_finder_failed",
                        attempted = "reveal_in_finder",
                        trace_id = %trace_id,
                        file_manager,
                        path = %path_str,
                        error = %error,
                        "Reveal in file manager failed"
                    );
                    Err(feedback_action.failure_message(file_manager, error))
                }
            };

            let _ = result_tx.send_blocking(reveal_result);
        });

        result_rx
    }

    /// Launch the configured editor and return completion back to the UI thread for HUD feedback.
    fn launch_editor_with_feedback_async(
        &self,
        path: &std::path::Path,
        trace_id: &str,
    ) -> async_channel::Receiver<Result<(), String>> {
        let editor = self.config.get_editor();
        let path_str = path.to_string_lossy().to_string();
        let trace_id = trace_id.to_string();
        let feedback_action = AsyncExternalToolFeedbackAction::LaunchEditor;
        let (result_tx, result_rx) = async_channel::bounded::<Result<(), String>>(1);

        std::thread::spawn(move || {
            use std::process::Command;

            tracing::info!(
                category = "UI",
                event = "action_editor_launch_start",
                trace_id = %trace_id,
                editor = %editor,
                path = %path_str,
                "Editor launch started"
            );

            let launch_result = match Command::new(&editor).arg(&path_str).spawn() {
                Ok(_) => {
                    tracing::info!(
                        category = "UI",
                        event = "action_editor_launch_success",
                        trace_id = %trace_id,
                        editor = %editor,
                        path = %path_str,
                        "Editor launch succeeded"
                    );
                    Ok(())
                }
                Err(error) => {
                    tracing::error!(
                        event = "action_editor_launch_failed",
                        attempted = "launch_editor",
                        trace_id = %trace_id,
                        editor = %editor,
                        path = %path_str,
                        error = %error,
                        "Editor launch failed"
                    );
                    Err(feedback_action.failure_message(&editor, error))
                }
            };

            let _ = result_tx.send_blocking(launch_result);
        });

        result_rx
    }

    /// Copy text to clipboard using pbcopy on macOS.
    /// Critical: This properly closes stdin before waiting to prevent hangs.
    #[cfg(target_os = "macos")]
    fn pbcopy(&self, text: &str) -> Result<(), std::io::Error> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn()?;

        // Take ownership of stdin, write, then drop to signal EOF
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
            // stdin is dropped here => EOF delivered to pbcopy
        }

        // Now it's safe to wait - pbcopy has received EOF
        let status = child.wait()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "pbcopy exited with status: {}",
                status
            )));
        }
        Ok(())
    }

    /// Return the currently selected clipboard entry metadata when in ClipboardHistoryView.
    fn selected_clipboard_entry(&self) -> Option<clipboard_history::ClipboardEntryMeta> {
        if let Some(ref entry_id) = self.focused_clipboard_entry_id {
            if let Some(entry) = self
                .cached_clipboard_entries
                .iter()
                .find(|entry| &entry.id == entry_id)
            {
                return Some(entry.clone());
            }
        }

        let AppView::ClipboardHistoryView {
            filter,
            selected_index,
        } = &self.current_view
        else {
            return None;
        };

        select_clipboard_entry_meta(&self.cached_clipboard_entries, filter, *selected_index)
            .cloned()
    }

    fn selected_dictation_history_entry(&self) -> Option<crate::dictation::DictationHistoryEntry> {
        let AppView::DictationHistoryView {
            filter,
            selected_index,
            visible_limit,
        } = &self.current_view
        else {
            return None;
        };

        self.dictation_history_current_or_previous_page(filter, *visible_limit)
            .and_then(|page| page.rows.into_iter().nth(*selected_index))
    }

    /// Return true when the current view has any available actions.
    fn has_actions(&mut self) -> bool {
        match &self.current_view {
            AppView::AgentChatView { .. } => true,
            AppView::ClipboardHistoryView { .. } => {
                let has = self.selected_clipboard_entry().is_some();
                tracing::debug!(
                    event = "has_actions.clipboard",
                    has_selected_entry = has,
                    "has_actions (clipboard)",
                );
                has
            }
            AppView::DictationHistoryView { .. } => {
                self.selected_dictation_history_entry().is_some()
            }
            _ => {
                let script_info = self.get_focused_script_info();
                let has_script_info = script_info.is_some();
                let script_name = script_info
                    .as_ref()
                    .map(|s| s.name.clone())
                    .unwrap_or_default();
                let mut actions = Vec::new();

                if let Some(ref script) = script_info {
                    if script.is_scriptlet {
                        actions.extend(crate::actions::get_scriptlet_context_actions_with_custom(
                            script, None,
                        ));
                    } else {
                        actions.extend(crate::actions::get_script_context_actions(script));
                    }
                }

                let global_count_before = actions.len();
                actions.extend(crate::actions::get_global_actions());
                // Run 12 Pass 7 — Power Syntax composer states ALWAYS have
                // Cmd+K actions available (cancel, copy filter, default time,
                // edit argv, …). The legacy `has_actions` only counted script-
                // and global-rows, so composing `+cal …` with no script match
                // would silently swallow Cmd+K. Treat the composer states as
                // self-sufficient for the gate.
                let composing_power_syntax = {
                    let raw = self.filter_text();
                    self.menu_syntax_mode.capture_for(raw).is_some()
                        || self.menu_syntax_mode.command_for(raw).is_some()
                        || self.menu_syntax_mode.advanced_query_for(raw).is_some()
                };
                // Run 13 Pass 1 (user bug report) — Cmd+K on the main script
                // list MUST always open the actions dialog, even when the
                // selected entry has no script-context actions and
                // get_global_actions() is currently empty. The legacy gate
                // here returned false in that case, silently swallowing the
                // keystroke and matching the user's "Cmd+K doesn't work at
                // all" report. Always-true on ScriptList lets the dialog
                // surface its built-in/global rows (or an empty placeholder)
                // so the keystroke is visible and discoverable.
                let on_script_list = matches!(self.current_view, AppView::ScriptList);
                let result = !actions.is_empty() || composing_power_syntax || on_script_list;
                tracing::debug!(
                    event = "has_actions.check",
                    has_script_info = has_script_info,
                    script_name = %script_name,
                    script_actions = global_count_before,
                    total_actions = actions.len(),
                    result = result,
                    selected_index = self.selected_index,
                    "has_actions: script_info={}", has_script_info,
                );
                result
            }
        }
    }

    /// Return to script list after non-inline action handling.
    ///
    /// Centralizes state transition so actions don't directly mutate legacy
    /// focus fields (`pending_focus`) in multiple places.
    fn transition_to_script_list_after_action(&mut self, cx: &mut Context<Self>) {
        self.current_view = AppView::ScriptList;
        self.request_focus(FocusTarget::MainFilter, cx);
    }

    /// Simple percent-encoding for URL query strings.
    fn percent_encode_for_url(&self, input: &str) -> String {
        let mut encoded = String::with_capacity(input.len() * 3);
        for byte in input.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    encoded.push(byte as char);
                }
                b' ' => encoded.push_str("%20"),
                _ => {
                    encoded.push('%');
                    encoded.push_str(&format!("{:02X}", byte));
                }
            }
        }
        encoded
    }

    /// Derive user-facing toast feedback from a `DispatchOutcome` at the
    /// dispatch boundary.
    ///
    /// Shows an error toast when the outcome carries an error with a
    /// user-facing message.  Success, NoEffect, and Cancelled outcomes
    /// produce no feedback here — success HUDs are the handler's
    /// responsibility since only the handler knows the right message.
    fn show_outcome_feedback(&mut self, outcome: &DispatchOutcome, cx: &mut Context<Self>) {
        if outcome.status == ActionOutcomeStatus::Error {
            if let Some(ref msg) = outcome.user_message {
                self.show_error_toast_with_code(msg.clone(), outcome.error_code, cx);
            }
        }
    }

    fn current_main_prompt_handoff_payload(
        &mut self,
        adapter_id: crate::ai::agent_prompt_handoff::AgentPromptHandoffAdapterId,
    ) -> Result<
        crate::ai::agent_prompt_handoff::AgentPromptHandoffPayload,
        crate::ai::agent_prompt_handoff::AgentPromptHandoffError,
    > {
        let raw = self.filter_text().to_string();
        if raw.trim().is_empty() {
            return Err(crate::ai::agent_prompt_handoff::AgentPromptHandoffError::EmptyPrompt);
        }

        self.set_spine_parse_from_filter_and_cursor(&raw, raw.len());
        let plan = crate::spine::prompt_plan::build_spine_prompt_plan_with_aliases(
            &self.spine_parse,
            &self.spine_mention_aliases,
        );
        let cwd = self
            .spine_cwd_for_agent_chat_launch()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| std::path::PathBuf::from("/"));
        crate::ai::agent_prompt_handoff::compile_handoff_payload_from_spine_plan(
            adapter_id,
            raw,
            cwd,
            None,
            Vec::new(),
            plan,
        )
    }

    pub(crate) fn launch_prompt_target_from_main_prompt(
        &mut self,
        adapter_id: crate::ai::agent_prompt_handoff::AgentPromptHandoffAdapterId,
        cx: &mut Context<Self>,
    ) -> Result<
        crate::ai::agent_prompt_handoff::AgentPromptHandoffReceipt,
        crate::ai::agent_prompt_handoff::AgentPromptHandoffError,
    > {
        let payload = self.current_main_prompt_handoff_payload(adapter_id)?;
        let receipt = crate::ai::agent_prompt_handoff::launch_prompt_handoff(&payload)?;
        tracing::info!(
            target: "script_kit::agent_handoff",
            event = "main_prompt_handoff_succeeded",
            adapter_id = %receipt.adapter_id,
            action_id = %receipt.action_id,
            dry_run = receipt.dry_run,
            prompt_chars = receipt.prompt_chars,
            prompt_sha256 = %receipt.diagnostic_prompt_fingerprint(),
            spawned = receipt.spawned,
            pid = ?receipt.pid,
        );
        cx.notify();
        Ok(receipt)
    }

    pub(crate) fn export_prompt_from_main_prompt(
        &mut self,
        action: crate::ai::agent_prompt_handoff::AgentPromptActionId,
        cx: &mut Context<Self>,
    ) -> Result<
        crate::ai::agent_prompt_handoff::AgentPromptExportReceipt,
        crate::ai::agent_prompt_handoff::AgentPromptHandoffError,
    > {
        let payload = self.current_main_prompt_handoff_payload(
            crate::ai::agent_prompt_handoff::AgentPromptHandoffAdapterId::CmuxCodex,
        )?;
        let receipt = crate::ai::agent_prompt_handoff::export_prompt(&payload, action)?;
        tracing::info!(
            target: "script_kit::agent_handoff",
            event = "main_prompt_export_succeeded",
            action_id = %receipt.action_id,
            dry_run = receipt.dry_run,
            export_kind = %receipt.export_kind,
            context_part_count = receipt.context_part_count,
            prompt_builder_segment_count = receipt.prompt_builder_segment_count,
            clipboard_written = receipt.clipboard_written,
            prompt_chars = receipt.prompt_chars,
            prompt_sha256 = %receipt.diagnostic_prompt_fingerprint(),
            path_sha256 = ?receipt.diagnostic_path_fingerprint(),
            url_sha256 = ?receipt.diagnostic_url_fingerprint(),
        );
        cx.notify();
        Ok(receipt)
    }

    fn handle_main_prompt_handoff_action(
        &mut self,
        action_id: &str,
        cx: &mut Context<Self>,
    ) -> DispatchOutcome {
        let Some(adapter_id) = crate::ai::agent_prompt_handoff::adapter_from_action_id(action_id)
        else {
            return DispatchOutcome::not_handled();
        };

        if !matches!(self.current_view, AppView::ScriptList) {
            return DispatchOutcome::not_handled();
        }

        match self.launch_prompt_target_from_main_prompt(adapter_id, cx) {
            Ok(receipt) => {
                let mut outcome = DispatchOutcome::success();
                outcome.user_message = Some(format!("Sent prompt to {}", receipt.adapter_id));
                outcome
            }
            Err(error) => DispatchOutcome::error(
                crate::action_helpers::ERROR_ACTION_FAILED,
                error.user_message(),
            ),
        }
    }

    fn handle_main_prompt_export_action(
        &mut self,
        action_id: &str,
        cx: &mut Context<Self>,
    ) -> DispatchOutcome {
        let Some(prompt_action) =
            crate::ai::agent_prompt_handoff::prompt_action_from_action_id(action_id)
        else {
            return DispatchOutcome::not_handled();
        };

        if !matches!(self.current_view, AppView::ScriptList) {
            return DispatchOutcome::not_handled();
        }

        match self.export_prompt_from_main_prompt(prompt_action, cx) {
            Ok(receipt) => {
                let mut outcome = DispatchOutcome::success();
                outcome.user_message = Some(match receipt.export_kind.as_str() {
                    "file" => receipt
                        .path
                        .as_deref()
                        .map(|path| format!("Exported prompt to {path}"))
                        .unwrap_or_else(|| "Exported prompt to file".to_string()),
                    "gist" => receipt
                        .url
                        .as_deref()
                        .map(|url| format!("Exported prompt to {url}"))
                        .unwrap_or_else(|| "Exported prompt to gist".to_string()),
                    "clipboard" => "Copied prompt to clipboard".to_string(),
                    _ => "Exported prompt".to_string(),
                });
                outcome
            }
            Err(error) => DispatchOutcome::error(
                crate::action_helpers::ERROR_ACTION_FAILED,
                error.user_message(),
            ),
        }
    }

    /// Handle action selection from the actions dialog
    fn handle_agent_chat_action(
        &mut self,
        action_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DispatchOutcome {
        tracing::info!(
            event = "agent_chat_actions_menu_selected",
            host = "shared",
            action_id,
            "Selected Agent Chat Actions Menu item"
        );

        let AppView::AgentChatView { ref entity } = self.current_view else {
            return DispatchOutcome::not_handled();
        };
        let entity = entity.clone();

        if entity.update(cx, |view, cx| {
            view.dispatch_ai_recovery_action(action_id, cx)
        }) {
            tracing::info!(
                target: "script_kit::agent_chat",
                event = "agent_chat_recovery_action_dispatched",
                action_id,
            );
            return DispatchOutcome::success();
        }

        if let Some(adapter_id) = crate::ai::agent_prompt_handoff::adapter_from_action_id(action_id)
        {
            let payload = entity.update(cx, |view, cx| {
                view.current_prompt_handoff_payload(adapter_id, cx)
            });
            return match payload.and_then(|payload| {
                crate::ai::agent_prompt_handoff::launch_prompt_handoff(&payload)
            }) {
                Ok(receipt) => {
                    tracing::info!(
                        target: "script_kit::agent_handoff",
                        event = "agent_prompt_handoff_succeeded",
                        adapter_id = %receipt.adapter_id,
                        action_id = %receipt.action_id,
                        dry_run = receipt.dry_run,
                        prompt_chars = receipt.prompt_chars,
                        prompt_sha256 = %receipt.diagnostic_prompt_fingerprint(),
                        spawned = receipt.spawned,
                        pid = ?receipt.pid,
                    );
                    let mut outcome = DispatchOutcome::success();
                    outcome.user_message = Some("Sent prompt to cmux Codex".to_string());
                    outcome
                }
                Err(error) => DispatchOutcome::error(
                    crate::action_helpers::ERROR_ACTION_FAILED,
                    error.user_message(),
                ),
            };
        }

        if let Some(prompt_action) =
            crate::ai::agent_prompt_handoff::prompt_action_from_action_id(action_id)
        {
            let payload = entity.update(cx, |view, cx| {
                view.current_prompt_handoff_payload(
                    crate::ai::agent_prompt_handoff::AgentPromptHandoffAdapterId::CmuxCodex,
                    cx,
                )
            });
            return match payload.and_then(|payload| {
                crate::ai::agent_prompt_handoff::export_prompt(&payload, prompt_action)
            }) {
                Ok(receipt) => {
                    tracing::info!(
                        target: "script_kit::agent_handoff",
                        event = "agent_prompt_export_succeeded",
                        action_id = %receipt.action_id,
                        dry_run = receipt.dry_run,
                        export_kind = %receipt.export_kind,
                        context_part_count = receipt.context_part_count,
                        prompt_builder_segment_count = receipt.prompt_builder_segment_count,
                        clipboard_written = receipt.clipboard_written,
                        prompt_chars = receipt.prompt_chars,
                        prompt_sha256 = %receipt.diagnostic_prompt_fingerprint(),
                        path_sha256 = ?receipt.diagnostic_path_fingerprint(),
                        url_sha256 = ?receipt.diagnostic_url_fingerprint(),
                    );
                    let mut outcome = DispatchOutcome::success();
                    outcome.user_message = Some(match receipt.export_kind.as_str() {
                        "file" => "Exported prompt to file".to_string(),
                        "gist" => "Exported prompt to gist".to_string(),
                        "clipboard" => "Copied prompt to clipboard".to_string(),
                        _ => "Exported prompt".to_string(),
                    });
                    outcome
                }
                Err(error) => DispatchOutcome::error(
                    crate::action_helpers::ERROR_ACTION_FAILED,
                    error.user_message(),
                ),
            };
        }

        if let Some(action) =
            crate::ai::agent_chat::ui::view::FocusedTextMiniAction::from_action_id(action_id)
        {
            let receipt = entity.update(cx, |view, cx| {
                view.perform_focused_text_mini_action(action, cx)
            });
            tracing::info!(
                target: "script_kit::focused_text",
                event = "focused_text_mini_action_dispatched",
                action_id,
                success = receipt.success,
                changed_text = receipt.changed_text,
                copied_to_clipboard = receipt.copied_to_clipboard,
                before_ui_variant = %receipt.before_ui_variant,
                after_ui_variant = %receipt.after_ui_variant,
                output_length = receipt.output_length,
                error_code = ?receipt.error_code,
            );
            return DispatchOutcome::success();
        }

        if matches!(
            action_id,
            crate::ai::agent_chat::ui::view::AGENT_CHAT_RECOVERY_SIGN_IN_ACTION_ID
                | crate::ai::agent_chat::ui::view::AGENT_CHAT_RECOVERY_SWITCH_ACCOUNT_ACTION_ID
                | crate::ai::agent_chat::ui::view::AGENT_CHAT_RECOVERY_COPY_DETAILS_ACTION_ID
        ) {
            let handled = entity.update(cx, |view, cx| {
                view.dispatch_ai_recovery_action(action_id, cx)
            });
            return if handled {
                DispatchOutcome::success()
            } else {
                DispatchOutcome::not_handled()
            };
        }

        if let Some(model_id) = crate::actions::agent_chat_switch_model_id_from_action(action_id) {
            let Some(model_action) = AgentChatModelSwitchHandlerAction::from_action_id(action_id)
            else {
                return DispatchOutcome::not_handled();
            };
            let Some((current_selected_model_id, model_display_name)) = ({
                let view = entity.read(cx);
                view.thread().and_then(|thread| {
                    let thread = thread.read(cx);
                    let current_selected_model_id = thread.selected_model_id().map(str::to_string);
                    let model_display_name = thread
                        .available_models()
                        .iter()
                        .find(|entry| entry.id == model_id)
                        .map(|entry| {
                            entry
                                .display_name
                                .clone()
                                .unwrap_or_else(|| entry.id.clone())
                        })?;
                    Some((current_selected_model_id, model_display_name))
                })
            }) else {
                return DispatchOutcome::error(
                    crate::action_helpers::ERROR_ACTION_FAILED,
                    model_action.unavailable_message(model_id),
                );
            };

            if current_selected_model_id.as_deref() == Some(model_id) {
                let mut outcome = DispatchOutcome::success();
                outcome.user_message =
                    Some(model_action.already_selected_message(&model_display_name));
                return outcome;
            }

            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_switch_model_requested",
                model_id,
                model_display_name = %model_display_name,
            );

            entity.update(cx, |view, cx| {
                if let Some(thread) = view.thread() {
                    thread.update(cx, |thread, cx| {
                        thread.select_model(model_id, cx);
                    });
                }
            });
            self.show_hud(
                model_action.hud_message(&model_display_name),
                Some(HUD_SHORT_MS),
                cx,
            );

            let mut outcome = DispatchOutcome::success();
            outcome.user_message = Some(model_action.switched_message(&model_display_name));
            return outcome;
        }

        if let Some(profile_id) =
            crate::actions::agent_chat_switch_profile_id_from_action(action_id)
        {
            let Some(profile_action) =
                AgentChatProfileSwitchHandlerAction::from_action_id(action_id)
            else {
                return DispatchOutcome::not_handled();
            };

            let mut prefs = crate::config::load_user_preferences();
            let ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
            let Some(profile) =
                crate::ai::agent_chat::profiles::persist_agent_chat_profile_selection(
                    &mut prefs.ai,
                    profile_id,
                    &ctx,
                )
            else {
                return DispatchOutcome::error(
                    crate::action_helpers::ERROR_ACTION_FAILED,
                    profile_action.unavailable_message(profile_id),
                );
            };

            let persist_result = crate::config::save_user_preferences(&prefs);
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_switch_profile_persist_result",
                profile_name = %profile.name,
                profile_id = %profile.id,
                profile_backend = ?profile.backend,
                persisted = persist_result.is_ok(),
            );

            if let Err(error) = persist_result {
                return DispatchOutcome::error(
                    crate::action_helpers::ERROR_ACTION_FAILED,
                    profile_action.persist_failure_message(&profile.name, error),
                );
            }

            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_switch_profile_requested",
                profile_name = %profile.name,
                profile_id = %profile.id,
                profile_backend = ?profile.backend,
            );

            self.close_tab_ai_harness_terminal(cx);
            self.embedded_agent_chat = None;
            self.open_tab_ai_agent_chat_with_entry_intent(None, cx);

            let mut outcome = DispatchOutcome::success();
            outcome.user_message = Some(profile_action.relaunch_message(&profile.name));
            return outcome;
        }

        if let Some(request_id) =
            crate::actions::agent_chat_receipt_history_request_id_from_action(action_id)
        {
            let Some(entry) =
                crate::agentic_protocol_bus::find_protocol_response_by_request_id(request_id)
            else {
                return DispatchOutcome::error(
                    crate::action_helpers::ERROR_ACTION_FAILED,
                    format!("Receipt {request_id} was not found"),
                );
            };

            let json = serde_json::to_string_pretty(&entry).unwrap_or_else(|_| "{}".to_string());
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(json));
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_receipt_history_copied",
                request_id,
                response_type = %entry.response_type,
                "Copied protocol receipt history entry"
            );

            let mut outcome = DispatchOutcome::success();
            outcome.user_message = Some("Copied receipt history to clipboard".to_string());
            return outcome;
        }

        if let Some(thread_id) = crate::actions::agent_chat_switch_thread_id_from_action(action_id)
        {
            let thread_id = thread_id.to_string();
            let entity = entity.clone();
            entity.update(cx, |chat, cx| {
                chat.switch_to_thread(&thread_id, cx);
            });
            return DispatchOutcome::success();
        }

        if let Some(entry_id) = crate::actions::agent_chat_fork_edit_entry_from_action(action_id) {
            let entry_id = entry_id.to_string();
            let entity = entity.clone();
            entity.update(cx, |chat, cx| {
                if let Some(thread) = chat.thread() {
                    thread.update(cx, |thread, cx| {
                        thread.fork_to_message(&entry_id, cx);
                    });
                }
            });
            return DispatchOutcome::success();
        }

        match action_id {
            "agent_chat_send" | "agent_chat_stop" => {
                let command = if action_id == "agent_chat_send" {
                    crate::components::conversation_actions::AgentChatConversationCommand::Send
                } else {
                    crate::components::conversation_actions::AgentChatConversationCommand::Stop
                };
                let executed = entity.update(cx, |chat, cx| {
                    chat.execute_conversation_command(command, cx)
                });
                if executed {
                    DispatchOutcome::success()
                } else {
                    DispatchOutcome::not_handled()
                }
            }
            "agent_chat_new_thread" => {
                let entity = entity.clone();
                entity.update(cx, |chat, cx| chat.start_new_thread(cx));
                DispatchOutcome::success()
            }
            "agent_chat_review_approvals" => {
                let entity = entity.clone();
                entity.update(cx, |chat, cx| {
                    if let Some(thread) = chat.thread() {
                        thread.update(cx, |thread, cx| thread.review_standing_approvals(cx));
                    }
                });
                DispatchOutcome::success()
            }
            "agent_chat_copy_last_response" => {
                let Some(last_response_action) =
                    AgentChatLastResponseHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let entity = entity.clone();
                let last_response = {
                    let view = entity.read(cx);
                    view.thread().and_then(|thread| {
                        thread
                            .read(cx)
                            .messages
                            .iter()
                            .rev()
                            .find(|msg| {
                                matches!(
                                    msg.role,
                                    crate::ai::agent_chat::ui::thread::AgentChatThreadMessageRole::Assistant
                                )
                            })
                            .map(|msg| msg.body.to_string())
                    })
                };

                if let Some(text) = last_response {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                    let mut outcome = DispatchOutcome::success();
                    outcome.user_message = Some(last_response_action.success_message().to_string());
                    outcome
                } else {
                    DispatchOutcome::not_handled()
                }
            }
            "agent_chat_append_last_response_to_today" => {
                let entity = entity.clone();
                let last_response = {
                    let view = entity.read(cx);
                    view.thread().and_then(|thread| {
                        thread
                            .read(cx)
                            .messages
                            .iter()
                            .rev()
                            .find(|msg| {
                                matches!(
                                    msg.role,
                                    crate::ai::agent_chat::ui::thread::AgentChatThreadMessageRole::Assistant
                                )
                            })
                            .map(|msg| msg.body.to_string())
                    })
                };
                let Some(last_response) = last_response.filter(|text| !text.trim().is_empty())
                else {
                    let mut outcome = DispatchOutcome::success();
                    outcome.user_message =
                        Some("No Agent Chat response to append to Today".to_string());
                    return outcome;
                };

                let Some(AppView::DayPage { entity: day_page }) =
                    self.tab_ai_harness_return_view.clone()
                else {
                    tracing::warn!(
                        target: "script_kit::day_page",
                        event = "agent_chat_append_to_today_blocked",
                        reason = "return_origin_not_day_page",
                    );
                    return DispatchOutcome::not_handled();
                };

                match day_page.update(cx, |view, _cx| {
                    view.append_agent_chat_response_to_today_file(&last_response)
                }) {
                    Ok(()) => {
                        tracing::info!(
                            target: "script_kit::day_page",
                            event = "agent_chat_append_to_today_succeeded",
                            char_count = last_response.chars().count(),
                        );
                        self.clear_actions_popup_state();
                        if crate::actions::is_actions_window_open() {
                            crate::actions::close_actions_window(cx);
                        }
                        self.close_tab_ai_harness_terminal(cx);
                        let mut outcome = DispatchOutcome::success();
                        outcome.user_message =
                            Some("Appended last Agent Chat response to Today".to_string());
                        outcome
                    }
                    Err(error) => {
                        tracing::warn!(
                            target: "script_kit::day_page",
                            event = "agent_chat_append_to_today_failed",
                            error = %error,
                        );
                        DispatchOutcome::error(
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            format!("Failed to append to Today: {error}"),
                        )
                    }
                }
            }
            "agent_chat_new_conversation" | "agent_chat_clear_conversation" => {
                let Some(session_action) =
                    AgentChatConversationSessionHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                if !session_action.preserves_session() {
                    // Close and reopen the Agent Chat chat for a fresh session
                    self.close_tab_ai_harness_terminal(cx);
                    self.open_tab_ai_agent_chat_with_entry_intent(None, cx);
                    return DispatchOutcome::success();
                }

                // Clear messages but keep the session alive through the same
                // descriptor-backed transaction used by ⌘L.
                let entity = entity.clone();
                let changed = entity.update(cx, |chat, cx| chat.start_new_conversation(cx));
                if !changed {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        "Stop the current response first.",
                    );
                }
                DispatchOutcome::success()
            }
            "agent_chat_paste_to_frontmost" => {
                let Some(last_response_action) =
                    AgentChatLastResponseHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let last_response = {
                    let view = entity.read(cx);
                    view.thread().and_then(|thread| {
                        thread
                            .read(cx)
                            .messages
                            .iter()
                            .rev()
                            .find(|msg| {
                                matches!(
                                    msg.role,
                                    crate::ai::agent_chat::ui::thread::AgentChatThreadMessageRole::Assistant
                                )
                            })
                            .map(|msg| msg.body.to_string())
                    })
                };

                if let Some(text) = last_response {
                    // Hide the window so the frontmost app regains focus
                    crate::platform::defer_hide_main_window(cx);
                    // Spawn a background thread to paste after a short delay
                    let text_for_paste = text.clone();
                    std::thread::spawn(move || {
                        // Small delay to let the frontmost app regain focus
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        let injector = crate::text_injector::TextInjector::new();
                        if let Err(e) = injector.paste_text(&text_for_paste) {
                            tracing::warn!(%e, "agent_chat_paste_to_frontmost_failed");
                        }
                    });
                    let mut outcome = DispatchOutcome::success();
                    outcome.user_message = Some(last_response_action.success_message().to_string());
                    outcome
                } else {
                    DispatchOutcome::not_handled()
                }
            }
            "agent_chat_copy_all_code" => {
                let Some(code_copy_action) =
                    AgentChatCodeCopyHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let entity = entity.clone();
                let messages = {
                    let view = entity.read(cx);
                    view.thread().map(|thread| thread.read(cx).messages.clone())
                };
                let mut all_code = String::new();
                if let Some(messages) = messages {
                    for msg in &messages {
                        if matches!(
                            msg.role,
                            crate::ai::agent_chat::ui::thread::AgentChatThreadMessageRole::Assistant
                        ) {
                            // Extract all code blocks from this message
                            let mut in_block = false;
                            let mut current = String::new();
                            for line in msg.body.lines() {
                                if line.trim_start().starts_with("```") {
                                    if in_block {
                                        if !current.is_empty() {
                                            if !all_code.is_empty() {
                                                all_code.push_str("\n\n");
                                            }
                                            all_code.push_str(&current);
                                        }
                                        current.clear();
                                        in_block = false;
                                    } else {
                                        in_block = true;
                                        current.clear();
                                    }
                                } else if in_block {
                                    if !current.is_empty() {
                                        current.push('\n');
                                    }
                                    current.push_str(line);
                                }
                            }
                        }
                    }
                }
                if all_code.is_empty() {
                    let mut o = DispatchOutcome::success();
                    o.user_message = Some(code_copy_action.result_message(false).to_string());
                    o
                } else {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(all_code));
                    let mut o = DispatchOutcome::success();
                    o.user_message = Some(code_copy_action.result_message(true).to_string());
                    o
                }
            }
            "agent_chat_retry_last" => {
                let Some(retry_action) = AgentChatRetryLastHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let entity = entity.clone();
                let last_user_msg = {
                    let view = entity.read(cx);
                    view.thread().and_then(|thread| {
                        thread
                            .read(cx)
                            .messages
                            .iter()
                            .rev()
                            .find(|m| {
                                matches!(m.role, crate::ai::agent_chat::ui::thread::AgentChatThreadMessageRole::User)
                            })
                            .map(|m| m.body.to_string())
                    })
                };

                if let Some(text) = last_user_msg {
                    entity.update(cx, |chat, cx| {
                        if let Some(thread) = chat.thread() {
                            thread.update(cx, |thread, cx| {
                                thread.set_input(text, cx);
                                let _ = thread.submit_input(cx);
                            });
                        }
                    });
                    DispatchOutcome::success()
                } else {
                    let mut o = DispatchOutcome::success();
                    o.user_message = Some(retry_action.missing_user_message().to_string());
                    o
                }
            }
            "agent_chat_save_as_script" => {
                let Some(code_block_action) =
                    AgentChatLastCodeBlockHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let entity = entity.clone();
                let last_response = {
                    let view = entity.read(cx);
                    view.thread().and_then(|thread| {
                        thread
                            .read(cx)
                            .messages
                            .iter()
                            .rev()
                            .find(|m| {
                                matches!(
                                    m.role,
                                    crate::ai::agent_chat::ui::thread::AgentChatThreadMessageRole::Assistant
                                )
                            })
                            .map(|m| m.body.to_string())
                    })
                };

                if let Some(text) = last_response {
                    let block = extract_last_code_block_with_lang(&text);
                    if let Some(block) = block {
                        let code = block.code;
                        let ext = match block.language.as_deref() {
                            Some("typescript" | "ts") => "ts",
                            Some("javascript" | "js") => "js",
                            Some("python" | "py") => "py",
                            Some("rust" | "rs") => "rs",
                            Some("bash" | "sh" | "zsh") => "sh",
                            _ => "ts", // Default to TypeScript for Script Kit
                        };
                        // Generate a script name from the first line
                        let name = code
                            .lines()
                            .find(|l| !l.trim().is_empty())
                            .and_then(|l| {
                                let trimmed = l.trim().trim_start_matches("//").trim();
                                if trimmed.len() > 3 && trimmed.len() < 50 {
                                    Some(
                                        trimmed
                                            .to_lowercase()
                                            .replace(' ', "-")
                                            .chars()
                                            .filter(|c| c.is_alphanumeric() || *c == '-')
                                            .collect::<String>(),
                                    )
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| {
                                format!("ai-script-{}", chrono::Utc::now().format("%H%M%S"))
                            });

                        let path = crate::plugins::plugin_scripts_dir("main")
                            .join(format!("{name}.{ext}"));

                        if let Err(e) = std::fs::write(&path, &code) {
                            tracing::warn!(%e, "agent_chat_save_as_script_failed");
                        } else {
                            let mut o = DispatchOutcome::success();
                            o.user_message = code_block_action.saved_script_message(&name, ext);
                            return o;
                        }
                    }
                }
                let mut o = DispatchOutcome::success();
                o.user_message = Some(code_block_action.missing_code_message().to_string());
                o
            }
            "agent_chat_run_last_code" => {
                let Some(code_block_action) =
                    AgentChatLastCodeBlockHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let entity = entity.clone();
                let last_response = {
                    let view = entity.read(cx);
                    view.thread().and_then(|thread| {
                        thread
                            .read(cx)
                            .messages
                            .iter()
                            .rev()
                            .find(|m| {
                                matches!(
                                    m.role,
                                    crate::ai::agent_chat::ui::thread::AgentChatThreadMessageRole::Assistant
                                )
                            })
                            .map(|m| m.body.to_string())
                    })
                };

                if let Some(text) = last_response {
                    if let Some(block) = extract_last_code_block_with_lang(&text) {
                        let lang = block
                            .language
                            .as_deref()
                            .unwrap_or("typescript")
                            .to_lowercase();

                        // Write to temp file
                        let ext = match lang.as_str() {
                            "typescript" | "ts" => "ts",
                            "javascript" | "js" => "js",
                            "python" | "py" => "py",
                            "bash" | "sh" | "zsh" | "shell" => "sh",
                            _ => "ts",
                        };
                        let name = format!("ai-run-{}.{ext}", chrono::Utc::now().format("%H%M%S"));
                        let tmp_dir = std::env::temp_dir().join("scriptkit-runs");
                        let _ = std::fs::create_dir_all(&tmp_dir);
                        let path = tmp_dir.join(&name);

                        if let Err(e) = std::fs::write(&path, &block.code) {
                            tracing::warn!(%e, "agent_chat_run_last_code_write_failed");
                            let mut o = DispatchOutcome::success();
                            o.user_message = code_block_action.temp_write_failure_message(e);
                            return o;
                        }

                        // Pick the runner
                        let path_str = path.to_string_lossy().to_string();
                        let (cmd, args): (&str, Vec<String>) = match ext {
                            "ts" => ("bun", vec!["run".into(), path_str.clone()]),
                            "js" => ("node", vec![path_str.clone()]),
                            "py" => ("python3", vec![path_str.clone()]),
                            "sh" => ("bash", vec![path_str.clone()]),
                            _ => ("bun", vec!["run".into(), path_str.clone()]),
                        };
                        let cmd = cmd.to_string();

                        // Show "running..." message immediately
                        let Some(thread) = entity.read(cx).thread() else {
                            return DispatchOutcome::not_handled();
                        };
                        thread.update(cx, |t, cx| {
                            if let Some(message) = code_block_action.running_message(&name) {
                                t.push_system_message(message, cx);
                            }
                        });

                        // Spawn async execution to avoid blocking the UI
                        let thread_for_result = thread.clone();
                        let path_clone = path.clone();
                        cx.spawn(async move |_this, cx| {
                            let result = cx
                                .background_executor()
                                .spawn(async move {
                                    std::process::Command::new(&cmd)
                                        .args(&args)
                                        .current_dir(std::env::temp_dir())
                                        .output()
                                })
                                .await;

                            // Clean up temp file
                            let _ = std::fs::remove_file(&path_clone);

                            let message = match result {
                                Ok(output) => {
                                    let stdout =
                                        String::from_utf8_lossy(&output.stdout).trim().to_string();
                                    let stderr =
                                        String::from_utf8_lossy(&output.stderr).trim().to_string();
                                    if output.status.success() {
                                        code_block_action
                                            .run_success_message(&stdout)
                                            .unwrap_or_default()
                                    } else {
                                        let out = if stderr.is_empty() { stdout } else { stderr };
                                        code_block_action
                                            .run_failure_message(output.status, &out)
                                            .unwrap_or_default()
                                    }
                                }
                                Err(e) => code_block_action
                                    .run_spawn_failure_message(e)
                                    .unwrap_or_default(),
                            };

                            cx.update(|cx| {
                                thread_for_result.update(cx, |t, cx| {
                                    t.push_system_message(message, cx);
                                });
                            });
                        })
                        .detach();

                        return DispatchOutcome::success();
                    }
                }
                let mut o = DispatchOutcome::success();
                o.user_message = Some(code_block_action.missing_code_message().to_string());
                o
            }
            "agent_chat_open_in_editor" => {
                let kit_path = crate::setup::get_kit_path();
                if let Err(e) = open::that(&kit_path) {
                    tracing::warn!(%e, "agent_chat_open_in_editor_failed");
                }
                DispatchOutcome::success()
            }
            "agent_chat_export_markdown" => {
                let Some(markdown_action) =
                    AgentChatConversationMarkdownHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let entity = entity.clone();
                let markdown = {
                    let view = entity.read(cx);
                    view.thread().and_then(|thread| {
                        build_agent_chat_conversation_markdown_from_thread(thread.read(cx))
                    })
                };
                let message_count = {
                    let view = entity.read(cx);
                    view.thread()
                        .map(|thread| thread.read(cx).messages.len())
                        .unwrap_or(0)
                };

                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_export_markdown_started",
                    message_count,
                    "Starting Agent Chat export-as-markdown"
                );

                let Some(markdown) = markdown else {
                    let reason = markdown_action.blocked_reason(message_count);
                    tracing::warn!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_export_markdown_blocked",
                        reason = %reason.trace_value(),
                        message_count,
                        "Agent Chat export-as-markdown blocked"
                    );
                    let mut outcome = DispatchOutcome::success();
                    outcome.user_message = Some(markdown_action.empty_message().to_string());
                    return outcome;
                };

                let char_count = markdown.chars().count();
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(markdown));

                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_export_markdown_succeeded",
                    message_count,
                    char_count,
                    "Agent Chat export-as-markdown completed"
                );

                let mut outcome = DispatchOutcome::success();
                outcome.user_message = Some(markdown_action.success_message().to_string());
                outcome
            }
            "agent_chat_save_as_note" => {
                let Some(markdown_action) =
                    AgentChatConversationMarkdownHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let entity = entity.clone();
                let markdown = {
                    let view = entity.read(cx);
                    view.thread().and_then(|thread| {
                        build_agent_chat_conversation_markdown_from_thread(thread.read(cx))
                    })
                };
                let message_count = {
                    let view = entity.read(cx);
                    view.thread()
                        .map(|thread| thread.read(cx).messages.len())
                        .unwrap_or(0)
                };

                let thread_source = {
                    let view = entity.read(cx);
                    view.thread().map(|thread| {
                        crate::notes::agent_chat_thread_source(thread.read(cx).ui_thread_id())
                    })
                };

                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_save_as_note_started",
                    message_count,
                    "Starting Agent Chat save-as-note"
                );

                let Some(markdown) = markdown else {
                    let reason = markdown_action.blocked_reason(message_count);
                    tracing::warn!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_save_as_note_blocked",
                        reason = %reason.trace_value(),
                        message_count,
                        "Agent Chat save-as-note blocked"
                    );
                    let mut o = DispatchOutcome::success();
                    o.user_message = Some(markdown_action.empty_message().to_string());
                    return o;
                };

                let char_count = markdown.chars().count();
                match crate::notes::save_note_with_content_and_source(cx, markdown, thread_source) {
                    Ok(_) => {
                        self.close_agent_chat_to_script_list(false, cx);
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "agent_chat_save_as_note_succeeded",
                            message_count,
                            char_count,
                            handoff = "notes_window",
                            "Agent Chat save-as-note completed"
                        );
                        let mut o = DispatchOutcome::success();
                        o.user_message = Some(markdown_action.success_message().to_string());
                        o
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: "script_kit::tab_ai",
                            event = "agent_chat_save_as_note_failed",
                            message_count,
                            char_count,
                            error = %e,
                            "Agent Chat save-as-note failed"
                        );
                        let message = markdown_action
                            .failure_message(e)
                            .unwrap_or_else(|| "Failed to handle Agent Chat markdown".to_string());
                        DispatchOutcome::error(crate::action_helpers::ERROR_ACTION_FAILED, message)
                    }
                }
            }
            "agent_chat_show_history" => {
                let Some(panel_action) =
                    AgentChatPanelWindowHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(
                    event = "agent_chat_history_action_invoked",
                    action = "openHistory"
                );
                self.open_builtin_filterable_view(
                    AppView::AgentChatHistoryView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    panel_action
                        .history_search_placeholder()
                        .unwrap_or_default(),
                    true,
                    cx,
                );
                let mut outcome = DispatchOutcome::success();
                outcome.user_message = panel_action.success_message().map(String::from);
                outcome
            }
            "agent_chat_clear_history" => {
                let Some(history_action) =
                    AgentChatHistoryMutationHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                // Delete history index and conversations directory
                let kit = crate::setup::get_kit_path();
                let _ = std::fs::remove_file(history_action.history_index_path(&kit));
                let _ = std::fs::remove_dir_all(history_action.conversations_dir(&kit));
                let mut o = DispatchOutcome::success();
                o.user_message = Some(history_action.success_message().to_string());
                o
            }
            "agent_chat_scroll_to_top" => {
                let entity = entity.clone();
                entity.update(cx, |chat, cx| {
                    if let Some(transcript) = &chat.transcript {
                        transcript.read(cx).scroll_to(gpui::ListOffset {
                            item_ix: 0,
                            offset_in_item: gpui::px(0.),
                        });
                    }
                    cx.notify();
                });
                DispatchOutcome::success()
            }
            "agent_chat_scroll_to_bottom" => {
                let entity = entity.clone();
                entity.update(cx, |chat, cx| {
                    if let Some(transcript) = &chat.transcript {
                        transcript.read(cx).scroll_to_end();
                    }
                    cx.notify();
                });
                DispatchOutcome::success()
            }
            "agent_chat_expand_all" => {
                let entity = entity.clone();
                entity.update(cx, |chat, cx| {
                    // Add all collapsible message IDs to collapsed_ids (which means expanded)
                    let ids: Vec<u64> = chat.thread().map_or_else(Vec::new, |thread| {
                        thread
                            .read(cx)
                            .messages
                            .iter()
                            .filter(|m| {
                                matches!(
                                    m.role,
                                    crate::ai::agent_chat::ui::thread::AgentChatThreadMessageRole::Thought
                                        | crate::ai::agent_chat::ui::thread::AgentChatThreadMessageRole::Tool
                                )
                            })
                            .map(|m| m.id)
                            .collect()
                    });
                    if let Some(transcript) = &chat.transcript {
                        transcript.update(cx, |t, cx| t.expand_ids(ids, cx));
                    }
                    cx.notify();
                });
                DispatchOutcome::success()
            }
            "agent_chat_collapse_all" => {
                let entity = entity.clone();
                entity.update(cx, |chat, cx| {
                    if let Some(transcript) = &chat.transcript {
                        transcript.update(cx, |t, cx| t.clear_collapsed_ids(cx));
                    }
                    cx.notify();
                });
                DispatchOutcome::success()
            }
            "agent_chat_detach_window" => {
                let Some(panel_action) =
                    AgentChatPanelWindowHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let Some(thread) = entity.read(cx).thread() else {
                    return DispatchOutcome::not_handled();
                };
                let inherit_bounds = match window.window_bounds() {
                    gpui::WindowBounds::Windowed(bounds) => Some(bounds),
                    _ => Some(window.bounds()),
                };
                tracing::info!(
                    event = "actions_detach_agent_chat_requested",
                    has_inherited_bounds = true,
                );
                if let Err(e) = crate::ai::agent_chat::ui::chat_window::open_chat_window_with_thread(
                    thread,
                    inherit_bounds,
                    cx,
                ) {
                    tracing::warn!(%e, "agent_chat_detach_window_failed");
                    DispatchOutcome::success()
                } else {
                    // Activation is handled inside open_chat_window_with_thread.
                    self.close_agent_chat_to_script_list(false, cx);
                    // The chat just popped out into its own window; dismiss the
                    // always-on-top launcher panel so it doesn't linger behind
                    // the detached window (focus-loss auto-hide is disabled
                    // while a detached chat exists).
                    self.dispatch_window_event(
                        crate::window_orchestrator::WindowEvent::DismissMain,
                        cx,
                    );
                    tracing::info!(
                        event = "actions_detach_agent_chat_completed",
                        detached_window_activated = true,
                        main_window_dismissed = true,
                    );
                    let mut o = DispatchOutcome::success();
                    o.user_message = panel_action.success_message().map(String::from);
                    o
                }
            }
            "agent_chat_reattach_panel" => {
                let Some(panel_action) =
                    AgentChatPanelWindowHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                // The reattach helper reads the live thread out of the
                // detached view BEFORE closing the window — do not pre-close
                // here or the conversation hand-off is lost.
                self.reattach_embedded_agent_chat_from_detached(cx);
                let mut o = DispatchOutcome::success();
                o.user_message = panel_action.success_message().map(String::from);
                o
            }
            "agent_chat_close" => {
                self.close_tab_ai_harness_terminal(cx);
                DispatchOutcome::success()
            }
            _ => DispatchOutcome::not_handled(),
        }
    }

    fn handle_action(&mut self, action_id: String, window: &mut Window, cx: &mut Context<Self>) {
        let start = std::time::Instant::now();

        let action_id_stripped = action_id
            .strip_prefix("clip:")
            .or_else(|| action_id.strip_prefix("file:"))
            .or_else(|| action_id.strip_prefix("chat:"))
            .unwrap_or(action_id.as_str())
            .to_string();

        let dctx = DispatchContext::for_action(&action_id_stripped);

        tracing::info!(
            category = "UI",
            action = %action_id_stripped,
            trace_id = %dctx.trace_id,
            surface = %dctx.surface,
            "Action dispatch started"
        );

        let is_prompt_action =
            crate::ai::agent_prompt_handoff::is_prompt_action_id(&action_id_stripped);
        let should_transition_to_script_list =
            should_transition_to_script_list_after_action(&self.current_view) && !is_prompt_action;

        let selected_clipboard_entry = if action_id_stripped.starts_with("clipboard_") {
            self.selected_clipboard_entry()
        } else {
            None
        };
        // Clipboard actions handle their own transitions and notifications.
        let clipboard_outcome =
            self.handle_clipboard_action(&action_id_stripped, selected_clipboard_entry, &dctx, cx);
        if clipboard_outcome.was_handled() {
            log_dispatch_outcome(
                &action_id_stripped,
                &dctx.trace_id,
                "clipboard",
                &clipboard_outcome,
                &start,
            );
            self.show_outcome_feedback(&clipboard_outcome, cx);
            return;
        }

        let selected_dictation_entry = if action_id_stripped.starts_with("dictation_history_") {
            self.selected_dictation_history_entry()
        } else {
            None
        };
        let dictation_outcome = self.handle_dictation_history_action(
            &action_id_stripped,
            selected_dictation_entry,
            &dctx,
            window,
            cx,
        );
        if dictation_outcome.was_handled() {
            log_dispatch_outcome(
                &action_id_stripped,
                &dctx.trace_id,
                "dictation_history",
                &dictation_outcome,
                &start,
            );
            self.show_outcome_feedback(&dictation_outcome, cx);
            return;
        }

        let favorites_outcome =
            self.handle_favorites_action(&action_id_stripped, &dctx, window, cx);
        if favorites_outcome.was_handled() {
            log_dispatch_outcome(
                &action_id_stripped,
                &dctx.trace_id,
                "favorites",
                &favorites_outcome,
                &start,
            );
            self.show_outcome_feedback(&favorites_outcome, cx);
            return;
        }

        // Only script-list-hosted actions should force a ScriptList transition.
        if should_transition_to_script_list {
            self.transition_to_script_list_after_action(cx);
        }

        // Dispatch through handler chain, collecting the final outcome.
        let (handler, outcome) = {
            let o = self.handle_shortcut_alias_action(&action_id_stripped, &dctx, window, cx);
            if o.was_handled() {
                ("shortcut_alias", o)
            } else {
                let o = self.handle_script_action(&action_id_stripped, &dctx, window, cx);
                if o.was_handled() {
                    ("script", o)
                } else {
                    let o = self.handle_file_action(&action_id_stripped, &dctx, cx);
                    if o.was_handled() {
                        ("file", o)
                    } else {
                        let o = self.handle_app_action(&action_id_stripped, &dctx, window, cx);
                        if o.was_handled() {
                            ("app", o)
                        } else {
                            let o = self.handle_scriptlet_action(&action_id_stripped, &dctx, cx);
                            if o.was_handled() {
                                ("scriptlet", o)
                            } else {
                                let o =
                                    self.handle_main_prompt_handoff_action(&action_id_stripped, cx);
                                if o.was_handled() {
                                    ("main_prompt_handoff", o)
                                } else {
                                    let o = self
                                        .handle_main_prompt_export_action(&action_id_stripped, cx);
                                    if o.was_handled() {
                                        ("main_prompt_export", o)
                                    } else {
                                        let o = self.handle_agent_chat_action(
                                            &action_id_stripped,
                                            window,
                                            cx,
                                        );
                                        if o.was_handled() {
                                            ("agent_chat", o)
                                        } else {
                                            // SDK actions as final fallback — thread trace_id from dctx
                                            (
                                                "sdk_fallback",
                                                self.trigger_sdk_action_with_trace(
                                                    &action_id_stripped,
                                                    &dctx.trace_id,
                                                ),
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };

        log_dispatch_outcome(
            &action_id_stripped,
            &dctx.trace_id,
            handler,
            &outcome,
            &start,
        );
        self.show_outcome_feedback(&outcome, cx);
        cx.notify();
    }
}

/// Log structured outcome at the end of action dispatch.
fn log_dispatch_outcome(
    action_id: &str,
    trace_id: &str,
    handler: &str,
    outcome: &DispatchOutcome,
    start: &std::time::Instant,
) {
    tracing::info!(
        category = "UI",
        action = %action_id,
        trace_id = %trace_id,
        handler = handler,
        status = %outcome.status,
        error_code = outcome.error_code,
        duration_ms = start.elapsed().as_millis() as u64,
        "Action dispatch completed"
    );
}

// Include semantic submodules — each adds `impl ScriptListApp` methods.
include!("clipboard.rs");
include!("paste.rs");
include!("dictation_history.rs");
include!("favorites.rs");
include!("scripts.rs");
include!("shortcuts.rs");
include!("files.rs");
include!("apps.rs");
include!("scriptlets.rs");
