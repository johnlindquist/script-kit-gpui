use super::*;

impl Message {
    // ============================================================
    // PROMPT CONSTRUCTORS
    // ============================================================

    /// Create an arg prompt message
    pub fn arg(id: String, placeholder: String, choices: Vec<Choice>) -> Self {
        Message::Arg {
            id,
            placeholder,
            choices,
            actions: None,
        }
    }

    /// Create an arg prompt message with actions
    pub fn arg_with_actions(
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
        actions: Vec<ProtocolAction>,
    ) -> Self {
        Message::Arg {
            id,
            placeholder,
            choices,
            actions: if actions.is_empty() {
                None
            } else {
                Some(actions)
            },
        }
    }

    /// Create a div (HTML display) message
    pub fn div(id: String, html: String) -> Self {
        Message::Div {
            id,
            html,
            container_classes: None,
            actions: None,
            placeholder: None,
            hint: None,
            footer: None,
            container_bg: None,
            container_padding: None,
            opacity: None,
        }
    }

    /// Create a div message with container classes
    pub fn div_with_classes(id: String, html: String, container_classes: String) -> Self {
        Message::Div {
            id,
            html,
            container_classes: Some(container_classes),
            actions: None,
            placeholder: None,
            hint: None,
            footer: None,
            container_bg: None,
            container_padding: None,
            opacity: None,
        }
    }

    /// Create a submit response message
    pub fn submit(id: String, value: Option<String>) -> Self {
        Message::Submit { id, value }
    }

    /// Create an exit message
    pub fn exit(code: Option<i32>, message: Option<String>) -> Self {
        Message::Exit { code, message }
    }

    /// Get the prompt ID for prompt-type messages (arg, div, editor, etc.)
    ///
    /// These messages have an `id` field that identifies the prompt session.
    /// Returns None for non-prompt messages.
    pub fn prompt_id(&self) -> Option<&str> {
        match self {
            // Core prompts
            Message::Arg { id, .. }
            | Message::Div { id, .. }
            | Message::Submit { id, .. }
            | Message::Update { id, .. }
            // Text input prompts
            | Message::Editor { id, .. }
            | Message::Mini { id, .. }
            | Message::Micro { id, .. }
            // Selection prompts
            | Message::Select { id, .. }
            // Form prompts
            | Message::Fields { id, .. }
            | Message::Form { id, .. }
            // File/path prompts
            | Message::Path { id, .. }
            | Message::Drop { id, .. }
            // Input capture prompts
            | Message::Hotkey { id, .. }
            // Template/text prompts
            | Message::Template { id, .. }
            | Message::Env { id, .. }
            // Media prompts
            | Message::Chat { id, .. }
            | Message::ChatMessage { id, .. }
            | Message::ChatStreamStart { id, .. }
            | Message::ChatStreamChunk { id, .. }
            | Message::ChatStreamComplete { id, .. }
            | Message::ChatClear { id, .. }
            | Message::ChatSubmit { id, .. }
            | Message::Term { id, .. }
            | Message::Widget { id, .. }
            | Message::Webcam { id, .. }
            | Message::Mic { id, .. } => Some(id),
            // Clipboard has optional id
            Message::Clipboard { id, .. } => id.as_deref(),
            // All other messages don't have prompt IDs
            _ => None,
        }
    }

    /// Get the request ID for request/response type messages
    ///
    /// These messages have a `request_id` field for correlating requests with responses.
    /// Returns None for non-request messages.
    pub fn request_id(&self) -> Option<&str> {
        match self {
            // Selected text operations
            Message::GetSelectedText { request_id, .. }
            | Message::SetSelectedText { request_id, .. }
            | Message::CaptureFocusedText { request_id, .. }
            | Message::ReplaceFocusedText { request_id, .. }
            | Message::AppendFocusedText { request_id, .. }
            | Message::CopyFocusedTextOutput { request_id, .. }
            | Message::CheckAccessibility { request_id, .. }
            | Message::RequestAccessibility { request_id, .. }
            | Message::SelectedText { request_id, .. }
            | Message::TextSet { request_id, .. }
            | Message::FocusedTextSnapshot { request_id, .. }
            | Message::FocusedTextMutation { request_id, .. }
            | Message::AccessibilityStatus { request_id, .. }
            // Window information
            | Message::GetWindowBounds { request_id, .. }
            | Message::WindowBounds { request_id, .. }
            // Clipboard history
            | Message::ClipboardHistory { request_id, .. }
            | Message::ClipboardHistoryEntry { request_id, .. }
            | Message::ClipboardHistoryList { request_id, .. }
            | Message::ClipboardHistoryResult { request_id, .. }
            // Window management
            | Message::WindowList { request_id, .. }
            | Message::WindowAction { request_id, .. }
            | Message::WindowListResult { request_id, .. }
            | Message::WindowActionResult { request_id, .. }
            // File search
            | Message::FileSearch { request_id, .. }
            | Message::FileSearchResult { request_id, .. }
            | Message::FocusedTextAgentChatFixtureOpenResult { request_id, .. }
            | Message::ExternalCommandResult { request_id, .. }
            | Message::TriggerActionResult { request_id, .. }
            // Screenshot capture
            | Message::CaptureScreenshot { request_id, .. }
            | Message::ScreenshotResult { request_id, .. }
            // State query
            | Message::GetState { request_id, .. }
            | Message::StateResult { request_id, .. }
            // Element query
            | Message::GetElements { request_id, .. }
            | Message::ElementsResult { request_id, .. }
            // Layout info
            | Message::GetLayoutInfo { request_id, .. }
            | Message::LayoutInfoResult { request_id, .. }
            // Exact-window inspection
            | Message::InspectAutomationWindow { request_id, .. }
            | Message::AutomationInspectResult { request_id, .. }
            // Scriptlet operations
            | Message::RunScriptlet { request_id, .. }
            | Message::GetScriptlets { request_id, .. }
            | Message::ScriptletList { request_id, .. }
            | Message::ScriptletResult { request_id, .. }
            // Test infrastructure
            | Message::SimulateClick { request_id, .. }
            | Message::SimulateClickResult { request_id, .. }
            // Menu bar
            | Message::GetMenuBar { request_id, .. }
            | Message::MenuBarResult { request_id, .. }
            | Message::ExecuteMenuAction { request_id, .. }
            | Message::MenuActionResult { request_id, .. }
            // AI SDK API
            | Message::AiIsOpen { request_id, .. }
            | Message::AiIsOpenResult { request_id, .. }
            | Message::AiGetActiveChat { request_id, .. }
            | Message::AiActiveChatResult { request_id, .. }
            | Message::AiListChats { request_id, .. }
            | Message::AiChatListResult { request_id, .. }
            | Message::AiGetConversation { request_id, .. }
            | Message::AiConversationResult { request_id, .. }
            | Message::AiStartChat { request_id, .. }
            | Message::AiChatCreated { request_id, .. }
            | Message::AiAppendMessage { request_id, .. }
            | Message::AiMessageAppended { request_id, .. }
            | Message::AiSendMessage { request_id, .. }
            | Message::AiMessageSent { request_id, .. }
            | Message::AiSetSystemPrompt { request_id, .. }
            | Message::AiSystemPromptSet { request_id, .. }
            | Message::AiFocus { request_id, .. }
            | Message::AiFocusResult { request_id, .. }
            | Message::AiGetStreamingStatus { request_id, .. }
            | Message::AiStreamingStatusResult { request_id, .. }
            | Message::AiDeleteChat { request_id, .. }
            | Message::AiChatDeleted { request_id, .. }
            | Message::AiSubscribe { request_id, .. }
            | Message::AiSubscribed { request_id, .. }
            | Message::AiUnsubscribe { request_id, .. }
            | Message::SystemFeedbackResult { request_id, .. }
            | Message::WindowVisibilityAck { request_id, .. }
            | Message::DisplayList { request_id, .. }
            | Message::DisplayListResult { request_id, .. }
            | Message::FrontmostWindow { request_id, .. }
            | Message::FrontmostWindowResult { request_id, .. }
            | Message::GetAgentChatState { request_id, .. }
            | Message::AgentChatStateResult { request_id, .. }
            | Message::GetAiReliabilityState { request_id, .. }
            | Message::AiReliabilityStateResult { request_id, .. }
            | Message::SetAiReliabilityTestFixture { request_id, .. }
            | Message::AiReliabilityTestFixtureResult { request_id, .. }
            | Message::InspectContextPreparation { request_id, .. }
            | Message::ContextPreparationProbeResult { request_id, .. }
            | Message::ResetAgentChatTestProbe { request_id, .. }
            | Message::GetAgentChatTestProbe { request_id, .. }
            | Message::AgentChatTestProbeResult { request_id, .. }
            | Message::PerformAgentChatSetupAction { request_id, .. }
            | Message::AgentChatSetupActionResult { request_id, .. }
            | Message::WaitFor { request_id, .. }
            | Message::WaitForResult { request_id, .. }
            | Message::Batch { request_id, .. }
            | Message::BatchResult { request_id, .. }
            | Message::ListAutomationWindows { request_id, .. }
            | Message::AutomationWindowListResult { request_id, .. }
            | Message::SimulateGpuiEvent { request_id, .. }
            | Message::CancelGpuiEvent { request_id, .. }
            | Message::SimulateGpuiEventResult { request_id, .. }
            | Message::GetLogs { request_id, .. }
            | Message::LogsResult { request_id, .. }
            | Message::AiUnsubscribed { request_id, .. } => Some(request_id),
            Message::Notify { request_id, .. }
            | Message::Beep { request_id }
            | Message::Say { request_id, .. } => request_id.as_deref(),
            // AiError has optional request_id
            Message::AiError { request_id, .. } => request_id.as_deref(),
            // All other messages don't have request IDs
            _ => None,
        }
    }

    /// Get the message ID (works for message types that have an ID)
    ///
    /// This is a unified accessor that returns either prompt_id or request_id,
    /// whichever is applicable for the message type.
    pub fn id(&self) -> Option<&str> {
        // Try prompt_id first, then request_id
        self.prompt_id().or_else(|| self.request_id())
    }
}
