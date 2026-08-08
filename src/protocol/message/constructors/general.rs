use super::*;

impl Message {
    // ============================================================
    // Constructor methods for new message types
    // ============================================================

    /// Create an editor prompt message
    pub fn editor(id: String) -> Self {
        Message::Editor {
            id,
            content: None,
            language: None,
            template: None,
            on_init: None,
            on_submit: None,
            actions: None,
        }
    }

    /// Create an editor with content and language
    pub fn editor_with_content(id: String, content: String, language: Option<String>) -> Self {
        Message::Editor {
            id,
            content: Some(content),
            language,
            template: None,
            on_init: None,
            on_submit: None,
            actions: None,
        }
    }

    /// Create an editor with a VSCode-style snippet template
    pub fn editor_with_template(id: String, template: String, language: Option<String>) -> Self {
        Message::Editor {
            id,
            content: None,
            language,
            template: Some(template),
            on_init: None,
            on_submit: None,
            actions: None,
        }
    }

    /// Create a mini prompt message
    pub fn mini(id: String, placeholder: String, choices: Vec<Choice>) -> Self {
        Message::Mini {
            id,
            placeholder,
            choices,
        }
    }

    /// Create a micro prompt message
    pub fn micro(id: String, placeholder: String, choices: Vec<Choice>) -> Self {
        Message::Micro {
            id,
            placeholder,
            choices,
        }
    }

    /// Create a select prompt message
    pub fn select(id: String, placeholder: String, choices: Vec<Choice>, multiple: bool) -> Self {
        Message::Select {
            id,
            placeholder,
            choices,
            multiple: if multiple { Some(true) } else { None },
        }
    }

    /// Create a fields prompt message
    pub fn fields(id: String, fields: Vec<Field>) -> Self {
        Message::Fields {
            id,
            fields,
            actions: None,
        }
    }

    /// Create a form prompt message
    pub fn form(id: String, html: String) -> Self {
        Message::Form {
            id,
            html,
            actions: None,
        }
    }

    /// Create a path prompt message
    pub fn path(id: String, start_path: Option<String>) -> Self {
        Message::Path {
            id,
            start_path,
            hint: None,
        }
    }

    /// Create a drop zone message
    pub fn drop(id: String) -> Self {
        Message::Drop { id }
    }

    /// Create a hotkey prompt message
    pub fn hotkey(id: String) -> Self {
        Message::Hotkey {
            id,
            placeholder: None,
        }
    }

    /// Create a template prompt message
    pub fn template(id: String, template: String) -> Self {
        Message::Template { id, template }
    }

    /// Create an env prompt message
    pub fn env(id: String, key: String, secret: bool) -> Self {
        Message::Env {
            id,
            key,
            prompt: None,
            title: None,
            secret: if secret { Some(true) } else { None },
        }
    }

    /// Create a chat prompt message
    pub fn chat(id: String) -> Self {
        Message::Chat {
            id,
            placeholder: None,
            messages: Vec::new(),
            hint: None,
            footer: None,
            actions: None,
            model: None,
            models: Vec::new(),
            save_history: true,
            use_builtin_ai: false,
        }
    }

    /// Create a chat prompt message with placeholder
    pub fn chat_with_placeholder(id: String, placeholder: impl Into<String>) -> Self {
        Message::Chat {
            id,
            placeholder: Some(placeholder.into()),
            messages: Vec::new(),
            hint: None,
            footer: None,
            actions: None,
            model: None,
            models: Vec::new(),
            save_history: true,
            use_builtin_ai: false,
        }
    }

    /// Create a chat prompt message with configuration
    pub fn chat_with_config(id: String, config: ChatPromptConfig) -> Self {
        Message::Chat {
            id,
            placeholder: config.placeholder,
            messages: config.messages,
            hint: config.hint,
            footer: config.footer,
            actions: if config.actions.is_empty() {
                None
            } else {
                Some(config.actions)
            },
            model: config.model,
            models: config.models,
            save_history: config.save_history,
            use_builtin_ai: config.use_builtin_ai,
        }
    }

    /// Create a chat message to add to the chat
    pub fn chat_message(id: String, message: ChatPromptMessage) -> Self {
        Message::ChatMessage { id, message }
    }

    /// Create a chat stream start message
    pub fn chat_stream_start(
        id: String,
        message_id: String,
        position: ChatMessagePosition,
    ) -> Self {
        Message::ChatStreamStart {
            id,
            message_id,
            position,
        }
    }

    /// Create a chat stream chunk message
    pub fn chat_stream_chunk(id: String, message_id: String, chunk: String) -> Self {
        Message::ChatStreamChunk {
            id,
            message_id,
            chunk,
        }
    }

    /// Create a chat stream complete message
    pub fn chat_stream_complete(id: String, message_id: String) -> Self {
        Message::ChatStreamComplete { id, message_id }
    }

    /// Create a chat clear message
    pub fn chat_clear(id: String) -> Self {
        Message::ChatClear { id }
    }

    /// Create a chat submit message (App → SDK)
    pub fn chat_submit(id: String, text: String) -> Self {
        Message::ChatSubmit { id, text }
    }

    /// Create a term prompt message
    pub fn term(id: String, command: Option<String>) -> Self {
        Message::Term {
            id,
            command,
            actions: None,
        }
    }

    /// Create a widget message
    pub fn widget(id: String, html: String) -> Self {
        Message::Widget {
            id,
            html,
            options: None,
        }
    }

    /// Create a webcam prompt message
    pub fn webcam(id: String) -> Self {
        Message::Webcam { id }
    }

    /// Create a mic prompt message
    pub fn mic(id: String) -> Self {
        Message::Mic { id }
    }

    /// Create a notify message
    pub fn notify(title: Option<String>, body: Option<String>) -> Self {
        Message::Notify {
            request_id: None,
            title,
            body,
        }
    }

    /// Create a beep message
    pub fn beep() -> Self {
        Message::Beep { request_id: None }
    }

    /// Create a say message
    pub fn say(text: String, voice: Option<String>) -> Self {
        Message::Say {
            request_id: None,
            text,
            voice,
        }
    }

    /// Create a set status message
    pub fn set_status(status: String, message: Option<String>) -> Self {
        Message::SetStatus { status, message }
    }

    /// Create a HUD overlay message
    pub fn hud(text: String, duration_ms: Option<u64>) -> Self {
        Message::Hud { text, duration_ms }
    }

    /// Create a menu message
    pub fn menu(icon: Option<String>, scripts: Option<Vec<String>>) -> Self {
        Message::Menu { icon, scripts }
    }

    /// Create a clipboard read message
    pub fn clipboard_read(format: Option<ClipboardFormat>) -> Self {
        Message::Clipboard {
            id: None,
            action: ClipboardAction::Read,
            format,
            content: None,
        }
    }

    /// Create a clipboard write message
    pub fn clipboard_write(content: String, format: Option<ClipboardFormat>) -> Self {
        Message::Clipboard {
            id: None,
            action: ClipboardAction::Write,
            format,
            content: Some(content),
        }
    }

    /// Create a reserved keyboard type message.
    ///
    /// This constructs the protocol shape only; GPUI does not currently provide
    /// receipt-backed native keyboard input.
    pub fn keyboard_type(keys: String) -> Self {
        Message::Keyboard {
            action: KeyboardAction::Type,
            keys: Some(keys),
        }
    }

    /// Create a reserved keyboard tap message.
    ///
    /// This constructs the protocol shape only; GPUI does not currently provide
    /// receipt-backed native keyboard input.
    pub fn keyboard_tap(keys: String) -> Self {
        Message::Keyboard {
            action: KeyboardAction::Tap,
            keys: Some(keys),
        }
    }

    /// Create a reserved mouse message.
    ///
    /// This constructs the protocol shape only; GPUI does not currently provide
    /// receipt-backed native mouse input.
    pub fn mouse(action: MouseAction, data: Option<MouseData>) -> Self {
        Message::Mouse { action, data }
    }

    /// Create a reserved mouse move message.
    ///
    /// This constructs the protocol shape only; GPUI does not currently provide
    /// receipt-backed native mouse input.
    pub fn mouse_move(x: f64, y: f64) -> Self {
        Message::Mouse {
            action: MouseAction::Move,
            data: Some(MouseData::new(x, y)),
        }
    }

    /// Create a reserved mouse click message.
    ///
    /// This constructs the protocol shape only; GPUI does not currently provide
    /// receipt-backed native mouse input.
    pub fn mouse_click(x: f64, y: f64, button: Option<String>) -> Self {
        Message::Mouse {
            action: MouseAction::Click,
            data: Some(MouseData { x, y, button }),
        }
    }

    /// Create a reserved mouse set-position message.
    ///
    /// This constructs the protocol shape only; GPUI does not currently provide
    /// receipt-backed native mouse input.
    pub fn mouse_set_position(x: f64, y: f64) -> Self {
        Message::Mouse {
            action: MouseAction::SetPosition,
            data: Some(MouseData::new(x, y)),
        }
    }

    /// Create a show message
    pub fn show() -> Self {
        Message::Show {}
    }

    /// Create a hide message
    pub fn hide() -> Self {
        Message::Hide {}
    }

    /// Create a windowVisibilityAck reply for a show/hide RPC.
    pub fn window_visibility_ack(request_id: String, window_visible: bool) -> Self {
        Message::WindowVisibilityAck {
            request_id,
            window_visible,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn focused_text_agent_chat_fixture_open_result(
        request_id: String,
        mode: String,
        ok: bool,
        submitted: bool,
        text_length: usize,
        instruction_length: usize,
        error_code: Option<String>,
        error_message: Option<String>,
    ) -> Self {
        Message::FocusedTextAgentChatFixtureOpenResult {
            request_id,
            mode,
            ok,
            submitted,
            target_id: "agent_chat-chat".to_string(),
            target_kind: "main".to_string(),
            text_length,
            instruction_length,
            error_code,
            error_message,
        }
    }

    pub fn external_command_result(
        request_id: String,
        command: String,
        ok: bool,
        error_code: Option<String>,
        error_message: Option<String>,
    ) -> Self {
        Message::ExternalCommandResult {
            request_id,
            command,
            ok,
            error_code,
            error_message,
        }
    }

    pub fn trigger_action_result(
        request_id: String,
        action_id: String,
        host: Option<String>,
        ok: bool,
        popup_closed: bool,
        error_code: Option<String>,
        native_footer_activation: Option<serde_json::Value>,
    ) -> Self {
        Message::TriggerActionResult {
            request_id,
            action_id,
            host,
            ok,
            popup_closed,
            error_code,
            native_footer_activation,
        }
    }

    /// Create a browse message to open URL in default browser
    pub fn browse(url: String) -> Self {
        Message::Browse { url }
    }

    /// Create an exec message
    pub fn exec(command: String, options: Option<serde_json::Value>) -> Self {
        Message::Exec { command, options }
    }

    /// Create a set panel message
    pub fn set_panel(html: String) -> Self {
        Message::SetPanel { html }
    }

    /// Create a set preview message
    pub fn set_preview(html: String) -> Self {
        Message::SetPreview { html }
    }

    /// Create a set prompt message
    pub fn set_prompt(html: String) -> Self {
        Message::SetPrompt { html }
    }

    // ============================================================
    // Constructor methods for selected text operations
    // ============================================================

    /// Create a get selected text request
    pub fn get_selected_text(request_id: String) -> Self {
        Message::GetSelectedText { request_id }
    }

    /// Create a set selected text request
    pub fn set_selected_text_msg(text: String, request_id: String) -> Self {
        Message::SetSelectedText { text, request_id }
    }

    /// Create a whole-field focused text capture request.
    pub fn capture_focused_text(request_id: String) -> Self {
        Message::CaptureFocusedText { request_id }
    }

    /// Create a focused text replace request.
    pub fn replace_focused_text(session_id: String, text: String, request_id: String) -> Self {
        Message::ReplaceFocusedText {
            session_id,
            text,
            request_id,
        }
    }

    /// Create a focused text append request.
    pub fn append_focused_text(session_id: String, text: String, request_id: String) -> Self {
        Message::AppendFocusedText {
            session_id,
            text,
            request_id,
        }
    }

    /// Create a copy-only focused-text Agent Chat output request.
    pub fn copy_focused_text_output(text: String, request_id: String) -> Self {
        Message::CopyFocusedTextOutput { text, request_id }
    }

    /// Create a check accessibility request
    pub fn check_accessibility(request_id: String) -> Self {
        Message::CheckAccessibility { request_id }
    }

    /// Create a request accessibility request
    pub fn request_accessibility(request_id: String) -> Self {
        Message::RequestAccessibility { request_id }
    }

    /// Create a selected text response
    pub fn selected_text_response(text: String, request_id: String) -> Self {
        Message::SelectedText { text, request_id }
    }

    /// Create a text set response (success)
    pub fn text_set_success(request_id: String) -> Self {
        Message::TextSet {
            success: true,
            error: None,
            request_id,
        }
    }

    /// Create a text set response (error)
    pub fn text_set_error(error: String, request_id: String) -> Self {
        Message::TextSet {
            success: false,
            error: Some(error),
            request_id,
        }
    }

    /// Create a focused text snapshot response.
    pub fn focused_text_snapshot_response(snapshot: serde_json::Value, request_id: String) -> Self {
        Message::FocusedTextSnapshot {
            success: true,
            snapshot: Some(snapshot),
            error: None,
            request_id,
        }
    }

    /// Create a focused text snapshot error response.
    pub fn focused_text_snapshot_error(error: String, request_id: String) -> Self {
        Message::FocusedTextSnapshot {
            success: false,
            snapshot: None,
            error: Some(error),
            request_id,
        }
    }

    /// Create a focused text mutation response.
    pub fn focused_text_mutation_response(action: String, request_id: String) -> Self {
        Message::FocusedTextMutation {
            success: true,
            action,
            error: None,
            request_id,
        }
    }

    /// Create a focused text mutation error response.
    pub fn focused_text_mutation_error(action: String, error: String, request_id: String) -> Self {
        Message::FocusedTextMutation {
            success: false,
            action,
            error: Some(error),
            request_id,
        }
    }

    /// Create an accessibility status response
    pub fn accessibility_status(granted: bool, request_id: String) -> Self {
        Message::AccessibilityStatus {
            granted,
            request_id,
        }
    }
}
