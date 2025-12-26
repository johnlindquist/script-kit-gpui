//! JSONL Protocol for Script Kit GPUI
//!
//! Defines message types for bidirectional communication between scripts and the GPUI app.
//! Messages are exchanged as newline-delimited JSON (JSONL).
//!
//! Message kinds:
//! - 'arg': Script sends prompt with choices, app responds with selected value
//! - 'div': Script sends HTML content, app responds with acknowledgment
//! - 'submit': App sends selected value or submission
//! - 'update': App sends live updates to script
//! - 'exit': Script or app signals termination

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read};
use tracing::{debug, warn};

/// A choice option for arg() prompts
///
/// Supports Script Kit API: name, value, and optional description
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Choice {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A field definition for form/fields prompts
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub field_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

impl Field {
    pub fn new(name: String) -> Self {
        Field {
            name,
            label: None,
            field_type: None,
            placeholder: None,
            value: None,
        }
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.label = Some(label);
        self
    }

    pub fn with_type(mut self, field_type: String) -> Self {
        self.field_type = Some(field_type);
        self
    }

    pub fn with_placeholder(mut self, placeholder: String) -> Self {
        self.placeholder = Some(placeholder);
        self
    }
}

/// Clipboard action type
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardAction {
    Read,
    Write,
}

/// Clipboard format type
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardFormat {
    Text,
    Image,
}

/// Keyboard action type
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum KeyboardAction {
    Type,
    Tap,
}

/// Mouse action type
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum MouseAction {
    Move,
    Click,
    SetPosition,
}

impl Choice {
    pub fn new(name: String, value: String) -> Self {
        Choice {
            name,
            value,
            description: None,
        }
    }

    pub fn with_description(name: String, value: String, description: String) -> Self {
        Choice {
            name,
            value,
            description: Some(description),
        }
    }
}

/// Protocol message with type discrimination via serde tag
///
/// This enum uses the "type" field to discriminate between message kinds.
/// Each variant corresponds to a message kind in the Script Kit v1 API.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    // ============================================================
    // CORE PROMPTS (existing)
    // ============================================================

    /// Script sends arg prompt with choices
    #[serde(rename = "arg")]
    Arg {
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
    },

    /// Script sends div (HTML display)
    #[serde(rename = "div")]
    Div {
        id: String,
        html: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tailwind: Option<String>,
    },

    /// App responds with submission (selected value or null)
    #[serde(rename = "submit")]
    Submit { id: String, value: Option<String> },

    /// App sends live update
    #[serde(rename = "update")]
    Update {
        id: String,
        #[serde(flatten)]
        data: serde_json::Value,
    },

    /// Signal termination
    #[serde(rename = "exit")]
    Exit {
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },

    // ============================================================
    // TEXT INPUT PROMPTS
    // ============================================================

    /// Code/text editor with syntax highlighting
    #[serde(rename = "editor")]
    Editor {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        language: Option<String>,
        #[serde(rename = "onInit", skip_serializing_if = "Option::is_none")]
        on_init: Option<String>,
        #[serde(rename = "onSubmit", skip_serializing_if = "Option::is_none")]
        on_submit: Option<String>,
    },

    /// Compact arg prompt (same as Arg but compact display)
    #[serde(rename = "mini")]
    Mini {
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
    },

    /// Tiny arg prompt (same as Arg but tiny display)
    #[serde(rename = "micro")]
    Micro {
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
    },

    // ============================================================
    // SELECTION PROMPTS
    // ============================================================

    /// Select from choices with optional multiple selection
    #[serde(rename = "select")]
    Select {
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
        #[serde(skip_serializing_if = "Option::is_none")]
        multiple: Option<bool>,
    },

    // ============================================================
    // FORM PROMPTS
    // ============================================================

    /// Multiple input fields
    #[serde(rename = "fields")]
    Fields {
        id: String,
        fields: Vec<Field>,
    },

    /// Custom HTML form
    #[serde(rename = "form")]
    Form {
        id: String,
        html: String,
    },

    // ============================================================
    // FILE/PATH PROMPTS
    // ============================================================

    /// File/folder path picker
    #[serde(rename = "path")]
    Path {
        id: String,
        #[serde(rename = "startPath", skip_serializing_if = "Option::is_none")]
        start_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
    },

    /// File drop zone
    #[serde(rename = "drop")]
    Drop { id: String },

    // ============================================================
    // INPUT CAPTURE PROMPTS
    // ============================================================

    /// Hotkey capture
    #[serde(rename = "hotkey")]
    Hotkey {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        placeholder: Option<String>,
    },

    // ============================================================
    // TEMPLATE/TEXT PROMPTS
    // ============================================================

    /// Template string with placeholders
    #[serde(rename = "template")]
    Template {
        id: String,
        template: String,
    },

    /// Environment variable prompt
    #[serde(rename = "env")]
    Env {
        id: String,
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        secret: Option<bool>,
    },

    // ============================================================
    // MEDIA PROMPTS
    // ============================================================

    /// Chat interface
    #[serde(rename = "chat")]
    Chat { id: String },

    /// Terminal emulator
    #[serde(rename = "term")]
    Term {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        command: Option<String>,
    },

    /// Custom widget with HTML
    #[serde(rename = "widget")]
    Widget {
        id: String,
        html: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        options: Option<serde_json::Value>,
    },

    /// Webcam capture
    #[serde(rename = "webcam")]
    Webcam { id: String },

    /// Microphone recording
    #[serde(rename = "mic")]
    Mic { id: String },

    // ============================================================
    // NOTIFICATION/FEEDBACK MESSAGES
    // ============================================================

    /// System notification
    #[serde(rename = "notify")]
    Notify {
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        body: Option<String>,
    },

    /// System beep sound
    #[serde(rename = "beep")]
    Beep {},

    /// Text-to-speech
    #[serde(rename = "say")]
    Say {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        voice: Option<String>,
    },

    /// Status bar update
    #[serde(rename = "setStatus")]
    SetStatus {
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },

    // ============================================================
    // SYSTEM CONTROL MESSAGES
    // ============================================================

    /// Menu bar icon/scripts
    #[serde(rename = "menu")]
    Menu {
        #[serde(skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        scripts: Option<Vec<String>>,
    },

    /// Clipboard operations
    #[serde(rename = "clipboard")]
    Clipboard {
        action: ClipboardAction,
        #[serde(skip_serializing_if = "Option::is_none")]
        format: Option<ClipboardFormat>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
    },

    /// Keyboard simulation
    #[serde(rename = "keyboard")]
    Keyboard {
        action: KeyboardAction,
        #[serde(skip_serializing_if = "Option::is_none")]
        keys: Option<String>,
    },

    /// Mouse control
    #[serde(rename = "mouse")]
    Mouse {
        action: MouseAction,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
    },

    /// Show window
    #[serde(rename = "show")]
    Show {},

    /// Hide window
    #[serde(rename = "hide")]
    Hide {},

    /// Open URL in default browser
    #[serde(rename = "browse")]
    Browse { url: String },

    /// Execute shell command
    #[serde(rename = "exec")]
    Exec {
        command: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        options: Option<serde_json::Value>,
    },

    // ============================================================
    // UI UPDATE MESSAGES
    // ============================================================

    /// Set panel HTML content
    #[serde(rename = "setPanel")]
    SetPanel { html: String },

    /// Set preview HTML content
    #[serde(rename = "setPreview")]
    SetPreview { html: String },

    /// Set prompt HTML content
    #[serde(rename = "setPrompt")]
    SetPrompt { html: String },
}

impl Message {
    /// Create an arg prompt message
    pub fn arg(id: String, placeholder: String, choices: Vec<Choice>) -> Self {
        Message::Arg {
            id,
            placeholder,
            choices,
        }
    }

    /// Create a div (HTML display) message
    pub fn div(id: String, html: String) -> Self {
        Message::Div {
            id,
            html,
            tailwind: None,
        }
    }

    /// Create a div message with tailwind classes
    pub fn div_with_tailwind(id: String, html: String, tailwind: String) -> Self {
        Message::Div {
            id,
            html,
            tailwind: Some(tailwind),
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

    /// Get the message ID (works for message types that have an ID)
    pub fn id(&self) -> Option<&str> {
        match self {
            // Core prompts
            Message::Arg { id, .. } => Some(id),
            Message::Div { id, .. } => Some(id),
            Message::Submit { id, .. } => Some(id),
            Message::Update { id, .. } => Some(id),
            Message::Exit { .. } => None,
            // Text input prompts
            Message::Editor { id, .. } => Some(id),
            Message::Mini { id, .. } => Some(id),
            Message::Micro { id, .. } => Some(id),
            // Selection prompts
            Message::Select { id, .. } => Some(id),
            // Form prompts
            Message::Fields { id, .. } => Some(id),
            Message::Form { id, .. } => Some(id),
            // File/path prompts
            Message::Path { id, .. } => Some(id),
            Message::Drop { id, .. } => Some(id),
            // Input capture prompts
            Message::Hotkey { id, .. } => Some(id),
            // Template/text prompts
            Message::Template { id, .. } => Some(id),
            Message::Env { id, .. } => Some(id),
            // Media prompts
            Message::Chat { id, .. } => Some(id),
            Message::Term { id, .. } => Some(id),
            Message::Widget { id, .. } => Some(id),
            Message::Webcam { id, .. } => Some(id),
            Message::Mic { id, .. } => Some(id),
            // Notification/feedback (no ID)
            Message::Notify { .. } => None,
            Message::Beep {} => None,
            Message::Say { .. } => None,
            Message::SetStatus { .. } => None,
            // System control (no ID)
            Message::Menu { .. } => None,
            Message::Clipboard { .. } => None,
            Message::Keyboard { .. } => None,
            Message::Mouse { .. } => None,
            Message::Show {} => None,
            Message::Hide {} => None,
            Message::Browse { .. } => None,
            Message::Exec { .. } => None,
            // UI update (no ID)
            Message::SetPanel { .. } => None,
            Message::SetPreview { .. } => None,
            Message::SetPrompt { .. } => None,
        }
    }

    // ============================================================
    // Constructor methods for new message types
    // ============================================================

    /// Create an editor prompt message
    pub fn editor(id: String) -> Self {
        Message::Editor {
            id,
            content: None,
            language: None,
            on_init: None,
            on_submit: None,
        }
    }

    /// Create an editor with content and language
    pub fn editor_with_content(id: String, content: String, language: Option<String>) -> Self {
        Message::Editor {
            id,
            content: Some(content),
            language,
            on_init: None,
            on_submit: None,
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
        Message::Fields { id, fields }
    }

    /// Create a form prompt message
    pub fn form(id: String, html: String) -> Self {
        Message::Form { id, html }
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
            secret: if secret { Some(true) } else { None },
        }
    }

    /// Create a chat prompt message
    pub fn chat(id: String) -> Self {
        Message::Chat { id }
    }

    /// Create a term prompt message
    pub fn term(id: String, command: Option<String>) -> Self {
        Message::Term { id, command }
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
        Message::Notify { title, body }
    }

    /// Create a beep message
    pub fn beep() -> Self {
        Message::Beep {}
    }

    /// Create a say message
    pub fn say(text: String, voice: Option<String>) -> Self {
        Message::Say { text, voice }
    }

    /// Create a set status message
    pub fn set_status(status: String, message: Option<String>) -> Self {
        Message::SetStatus { status, message }
    }

    /// Create a menu message
    pub fn menu(icon: Option<String>, scripts: Option<Vec<String>>) -> Self {
        Message::Menu { icon, scripts }
    }

    /// Create a clipboard read message
    pub fn clipboard_read(format: Option<ClipboardFormat>) -> Self {
        Message::Clipboard {
            action: ClipboardAction::Read,
            format,
            content: None,
        }
    }

    /// Create a clipboard write message
    pub fn clipboard_write(content: String, format: Option<ClipboardFormat>) -> Self {
        Message::Clipboard {
            action: ClipboardAction::Write,
            format,
            content: Some(content),
        }
    }

    /// Create a keyboard type message
    pub fn keyboard_type(keys: String) -> Self {
        Message::Keyboard {
            action: KeyboardAction::Type,
            keys: Some(keys),
        }
    }

    /// Create a keyboard tap message
    pub fn keyboard_tap(keys: String) -> Self {
        Message::Keyboard {
            action: KeyboardAction::Tap,
            keys: Some(keys),
        }
    }

    /// Create a mouse message
    pub fn mouse(action: MouseAction, data: Option<serde_json::Value>) -> Self {
        Message::Mouse { action, data }
    }

    /// Create a show message
    pub fn show() -> Self {
        Message::Show {}
    }

    /// Create a hide message
    pub fn hide() -> Self {
        Message::Hide {}
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
}

/// Parse a single JSONL message from a string
///
/// # Arguments
/// * `line` - A JSON string (typically one line from JSONL)
///
/// # Returns
/// * `Result<Message, serde_json::Error>` - Parsed message or deserialization error
pub fn parse_message(line: &str) -> Result<Message, serde_json::Error> {
    serde_json::from_str(line).map_err(|e| {
        // Log the raw input and error for debugging
        warn!(
            raw_input = %line,
            error = %e,
            "Failed to parse JSONL message"
        );
        e
    })
}

/// Result type for graceful message parsing
#[derive(Debug)]
pub enum ParseResult {
    /// Successfully parsed a known message type
    Ok(Message),
    /// Unknown message type - contains the type name and raw JSON
    UnknownType { message_type: String, raw: String },
    /// JSON parsing failed entirely
    ParseError(serde_json::Error),
}

/// Parse a message with graceful handling of unknown types
///
/// Unlike `parse_message`, this function handles unknown message types
/// gracefully by logging a warning and returning `ParseResult::UnknownType`
/// instead of failing.
///
/// # Arguments
/// * `line` - A JSON string (typically one line from JSONL)
///
/// # Returns
/// * `ParseResult` - Either a parsed message, unknown type info, or parse error
pub fn parse_message_graceful(line: &str) -> ParseResult {
    // First, try to parse as a known Message
    match serde_json::from_str::<Message>(line) {
        Ok(msg) => {
            debug!(message_id = ?msg.id(), "Successfully parsed message");
            ParseResult::Ok(msg)
        }
        Err(e) => {
            // Check if this is an unknown message type
            // Try parsing as generic JSON to extract the "type" field
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(msg_type) = value.get("type").and_then(|t| t.as_str()) {
                    // Valid JSON with "type" field, but unknown type
                    warn!(
                        message_type = %msg_type,
                        raw_input = %line,
                        "Unknown message type received - ignoring"
                    );
                    return ParseResult::UnknownType {
                        message_type: msg_type.to_string(),
                        raw: line.to_string(),
                    };
                }
            }
            
            // Not valid JSON or missing type field
            warn!(
                raw_input = %line,
                error = %e,
                "Failed to parse JSONL message - invalid JSON"
            );
            ParseResult::ParseError(e)
        }
    }
}

/// Serialize a message to JSONL format
///
/// # Arguments
/// * `msg` - The message to serialize
///
/// # Returns
/// * `Result<String, serde_json::Error>` - JSON string (without newline)
pub fn serialize_message(msg: &Message) -> Result<String, serde_json::Error> {
    serde_json::to_string(msg)
}

/// JSONL reader for streaming/chunked message reads
///
/// Provides utilities to read messages one at a time from a reader.
pub struct JsonlReader<R: Read> {
    reader: BufReader<R>,
}

impl<R: Read> JsonlReader<R> {
    /// Create a new JSONL reader
    pub fn new(reader: R) -> Self {
        JsonlReader {
            reader: BufReader::new(reader),
        }
    }

    /// Read the next message from the stream
    ///
    /// # Returns
    /// * `Ok(Some(Message))` - Successfully parsed message
    /// * `Ok(None)` - End of stream
    /// * `Err(e)` - Parse error
    pub fn next_message(&mut self) -> Result<Option<Message>, Box<dyn std::error::Error>> {
        let mut line = String::new();
        match self.reader.read_line(&mut line)? {
            0 => {
                debug!("Reached end of JSONL stream");
                Ok(None)
            }
            bytes_read => {
                debug!(bytes_read, "Read line from JSONL stream");
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    debug!("Skipping empty line in JSONL stream");
                    return self.next_message(); // Skip empty lines
                }
                let msg = parse_message(trimmed)?;
                Ok(Some(msg))
            }
        }
    }

    /// Read the next message with graceful unknown type handling
    ///
    /// Unlike `next_message`, this method uses `parse_message_graceful` to
    /// handle unknown message types without errors. Unknown types are logged
    /// and skipped, continuing to read the next message.
    ///
    /// # Returns
    /// * `Ok(Some(Message))` - Successfully parsed known message
    /// * `Ok(None)` - End of stream
    /// * `Err(e)` - IO error (not parse errors for unknown types)
    pub fn next_message_graceful(&mut self) -> Result<Option<Message>, std::io::Error> {
        loop {
            let mut line = String::new();
            match self.reader.read_line(&mut line)? {
                0 => {
                    debug!("Reached end of JSONL stream");
                    return Ok(None);
                }
                _ => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        debug!("Skipping empty line in JSONL stream");
                        continue;
                    }

                    match parse_message_graceful(trimmed) {
                        ParseResult::Ok(msg) => return Ok(Some(msg)),
                        ParseResult::UnknownType { message_type, .. } => {
                            // Already logged in parse_message_graceful
                            debug!(
                                message_type = %message_type,
                                "Skipping unknown message type, continuing to next message"
                            );
                            continue;
                        }
                        ParseResult::ParseError(e) => {
                            // Log but continue - graceful degradation
                            warn!(
                                error = %e,
                                "Skipping malformed message, continuing to next message"
                            );
                            continue;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_choice_creation() {
        let choice = Choice::new("Apple".to_string(), "apple".to_string());
        assert_eq!(choice.name, "Apple");
        assert_eq!(choice.value, "apple");
        assert_eq!(choice.description, None);
    }

    #[test]
    fn test_choice_with_description() {
        let choice = Choice::with_description(
            "Apple".to_string(),
            "apple".to_string(),
            "A red fruit".to_string(),
        );
        assert_eq!(choice.name, "Apple");
        assert_eq!(choice.value, "apple");
        assert_eq!(choice.description, Some("A red fruit".to_string()));
    }

    #[test]
    fn test_serialize_arg_message() {
        let choices = vec![
            Choice::new("Apple".to_string(), "apple".to_string()),
            Choice::new("Banana".to_string(), "banana".to_string()),
        ];
        let msg = Message::arg(
            "1".to_string(),
            "Pick one".to_string(),
            choices,
        );

        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"arg\""));
        assert!(json.contains("\"id\":\"1\""));
        assert!(json.contains("\"placeholder\":\"Pick one\""));
        assert!(json.contains("\"Apple\""));
    }

    #[test]
    fn test_parse_arg_message() {
        let json = r#"{"type":"arg","id":"1","placeholder":"Pick one","choices":[{"name":"Apple","value":"apple"},{"name":"Banana","value":"banana"}]}"#;
        let msg = parse_message(json).unwrap();

        match msg {
            Message::Arg {
                id,
                placeholder,
                choices,
            } => {
                assert_eq!(id, "1");
                assert_eq!(placeholder, "Pick one");
                assert_eq!(choices.len(), 2);
                assert_eq!(choices[0].name, "Apple");
                assert_eq!(choices[0].value, "apple");
            }
            _ => panic!("Expected Arg message"),
        }
    }

    #[test]
    fn test_parse_div_message() {
        let json = r#"{"type":"div","id":"2","html":"<h1>Hello</h1>"}"#;
        let msg = parse_message(json).unwrap();

        match msg {
            Message::Div { id, html, tailwind } => {
                assert_eq!(id, "2");
                assert_eq!(html, "<h1>Hello</h1>");
                assert_eq!(tailwind, None);
            }
            _ => panic!("Expected Div message"),
        }
    }

    #[test]
    fn test_parse_div_with_tailwind() {
        let json = r#"{"type":"div","id":"2","html":"<h1>Hello</h1>","tailwind":"text-2xl font-bold"}"#;
        let msg = parse_message(json).unwrap();

        match msg {
            Message::Div { id, html, tailwind } => {
                assert_eq!(id, "2");
                assert_eq!(html, "<h1>Hello</h1>");
                assert_eq!(tailwind, Some("text-2xl font-bold".to_string()));
            }
            _ => panic!("Expected Div message"),
        }
    }

    #[test]
    fn test_parse_submit_message() {
        let json = r#"{"type":"submit","id":"1","value":"apple"}"#;
        let msg = parse_message(json).unwrap();

        match msg {
            Message::Submit { id, value } => {
                assert_eq!(id, "1");
                assert_eq!(value, Some("apple".to_string()));
            }
            _ => panic!("Expected Submit message"),
        }
    }

    #[test]
    fn test_parse_submit_null_value() {
        let json = r#"{"type":"submit","id":"2","value":null}"#;
        let msg = parse_message(json).unwrap();

        match msg {
            Message::Submit { id, value } => {
                assert_eq!(id, "2");
                assert_eq!(value, None);
            }
            _ => panic!("Expected Submit message"),
        }
    }

    #[test]
    fn test_parse_exit_message() {
        let json = r#"{"type":"exit","code":0,"message":"Success"}"#;
        let msg = parse_message(json).unwrap();

        match msg {
            Message::Exit { code, message } => {
                assert_eq!(code, Some(0));
                assert_eq!(message, Some("Success".to_string()));
            }
            _ => panic!("Expected Exit message"),
        }
    }

    #[test]
    fn test_message_id() {
        let arg_msg = Message::arg("1".to_string(), "Pick".to_string(), vec![]);
        assert_eq!(arg_msg.id(), Some("1"));

        let div_msg = Message::div("2".to_string(), "<h1>Hi</h1>".to_string());
        assert_eq!(div_msg.id(), Some("2"));

        let exit_msg = Message::exit(None, None);
        assert_eq!(exit_msg.id(), None);
    }

    #[test]
    fn test_jsonl_reader() {
        let _jsonl = "\"type\":\"arg\",\"id\":\"1\",\"placeholder\":\"Pick\",\"choices\":[]}\n{\"type\":\"submit\",\"id\":\"1\",\"value\":\"apple\"}";
        // Note: This test uses a partial JSON to ensure line-by-line reading
        // A real test would need complete valid JSON lines
    }

    // ============================================================
    // FIELD STRUCT TESTS
    // ============================================================

    #[test]
    fn test_field_creation() {
        let field = Field::new("username".to_string());
        assert_eq!(field.name, "username");
        assert_eq!(field.label, None);
        assert_eq!(field.field_type, None);
    }

    #[test]
    fn test_field_builder() {
        let field = Field::new("email".to_string())
            .with_label("Email Address".to_string())
            .with_type("email".to_string())
            .with_placeholder("Enter your email".to_string());
        
        assert_eq!(field.name, "email");
        assert_eq!(field.label, Some("Email Address".to_string()));
        assert_eq!(field.field_type, Some("email".to_string()));
        assert_eq!(field.placeholder, Some("Enter your email".to_string()));
    }

    // ============================================================
    // TEXT INPUT PROMPT TESTS
    // ============================================================

    #[test]
    fn test_serialize_editor_message() {
        let msg = Message::editor("1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"editor\""));
        assert!(json.contains("\"id\":\"1\""));
        // Optional fields should not be present when None
        assert!(!json.contains("\"content\""));
    }

    #[test]
    fn test_parse_editor_message() {
        let json = r#"{"type":"editor","id":"1","content":"hello","language":"javascript"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Editor { id, content, language, .. } => {
                assert_eq!(id, "1");
                assert_eq!(content, Some("hello".to_string()));
                assert_eq!(language, Some("javascript".to_string()));
            }
            _ => panic!("Expected Editor message"),
        }
    }

    #[test]
    fn test_serialize_mini_message() {
        let choices = vec![Choice::new("A".to_string(), "a".to_string())];
        let msg = Message::mini("1".to_string(), "Quick pick".to_string(), choices);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"mini\""));
    }

    #[test]
    fn test_parse_mini_message() {
        let json = r#"{"type":"mini","id":"1","placeholder":"Quick","choices":[]}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Mini { id, placeholder, choices } => {
                assert_eq!(id, "1");
                assert_eq!(placeholder, "Quick");
                assert!(choices.is_empty());
            }
            _ => panic!("Expected Mini message"),
        }
    }

    #[test]
    fn test_serialize_micro_message() {
        let msg = Message::micro("1".to_string(), "Tiny".to_string(), vec![]);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"micro\""));
    }

    #[test]
    fn test_parse_micro_message() {
        let json = r#"{"type":"micro","id":"1","placeholder":"Tiny","choices":[]}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Micro { id, placeholder, .. } => {
                assert_eq!(id, "1");
                assert_eq!(placeholder, "Tiny");
            }
            _ => panic!("Expected Micro message"),
        }
    }

    // ============================================================
    // SELECTION PROMPT TESTS
    // ============================================================

    #[test]
    fn test_serialize_select_message() {
        let choices = vec![Choice::new("A".to_string(), "a".to_string())];
        let msg = Message::select("1".to_string(), "Pick".to_string(), choices, true);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"select\""));
        assert!(json.contains("\"multiple\":true"));
    }

    #[test]
    fn test_parse_select_message() {
        let json = r#"{"type":"select","id":"1","placeholder":"Pick","choices":[],"multiple":true}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Select { id, multiple, .. } => {
                assert_eq!(id, "1");
                assert_eq!(multiple, Some(true));
            }
            _ => panic!("Expected Select message"),
        }
    }

    // ============================================================
    // FORM PROMPT TESTS
    // ============================================================

    #[test]
    fn test_serialize_fields_message() {
        let fields = vec![
            Field::new("name".to_string()).with_label("Name".to_string()),
            Field::new("email".to_string()).with_type("email".to_string()),
        ];
        let msg = Message::fields("1".to_string(), fields);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"fields\""));
        assert!(json.contains("\"name\""));
    }

    #[test]
    fn test_parse_fields_message() {
        let json = r#"{"type":"fields","id":"1","fields":[{"name":"username","label":"Username"}]}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Fields { id, fields } => {
                assert_eq!(id, "1");
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "username");
            }
            _ => panic!("Expected Fields message"),
        }
    }

    #[test]
    fn test_serialize_form_message() {
        let msg = Message::form("1".to_string(), "<form>...</form>".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"form\""));
        assert!(json.contains("\"html\""));
    }

    #[test]
    fn test_parse_form_message() {
        let json = r#"{"type":"form","id":"1","html":"<form></form>"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Form { id, html } => {
                assert_eq!(id, "1");
                assert_eq!(html, "<form></form>");
            }
            _ => panic!("Expected Form message"),
        }
    }

    // ============================================================
    // FILE/PATH PROMPT TESTS
    // ============================================================

    #[test]
    fn test_serialize_path_message() {
        let msg = Message::path("1".to_string(), Some("/home".to_string()));
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"path\""));
        assert!(json.contains("\"startPath\":\"/home\""));
    }

    #[test]
    fn test_parse_path_message() {
        let json = r#"{"type":"path","id":"1","startPath":"/home","hint":"Select folder"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Path { id, start_path, hint } => {
                assert_eq!(id, "1");
                assert_eq!(start_path, Some("/home".to_string()));
                assert_eq!(hint, Some("Select folder".to_string()));
            }
            _ => panic!("Expected Path message"),
        }
    }

    #[test]
    fn test_serialize_drop_message() {
        let msg = Message::drop("1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"drop\""));
    }

    #[test]
    fn test_parse_drop_message() {
        let json = r#"{"type":"drop","id":"1"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Drop { id } => {
                assert_eq!(id, "1");
            }
            _ => panic!("Expected Drop message"),
        }
    }

    // ============================================================
    // INPUT CAPTURE PROMPT TESTS
    // ============================================================

    #[test]
    fn test_serialize_hotkey_message() {
        let msg = Message::hotkey("1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"hotkey\""));
    }

    #[test]
    fn test_parse_hotkey_message() {
        let json = r#"{"type":"hotkey","id":"1","placeholder":"Press a key"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Hotkey { id, placeholder } => {
                assert_eq!(id, "1");
                assert_eq!(placeholder, Some("Press a key".to_string()));
            }
            _ => panic!("Expected Hotkey message"),
        }
    }

    // ============================================================
    // TEMPLATE/TEXT PROMPT TESTS
    // ============================================================

    #[test]
    fn test_serialize_template_message() {
        let msg = Message::template("1".to_string(), "Hello {{name}}!".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"template\""));
        assert!(json.contains("Hello {{name}}!"));
    }

    #[test]
    fn test_parse_template_message() {
        let json = r#"{"type":"template","id":"1","template":"Hi {{name}}"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Template { id, template } => {
                assert_eq!(id, "1");
                assert_eq!(template, "Hi {{name}}");
            }
            _ => panic!("Expected Template message"),
        }
    }

    #[test]
    fn test_serialize_env_message() {
        let msg = Message::env("1".to_string(), "API_KEY".to_string(), true);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"env\""));
        assert!(json.contains("\"key\":\"API_KEY\""));
        assert!(json.contains("\"secret\":true"));
    }

    #[test]
    fn test_parse_env_message() {
        let json = r#"{"type":"env","id":"1","key":"SECRET","secret":true}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Env { id, key, secret } => {
                assert_eq!(id, "1");
                assert_eq!(key, "SECRET");
                assert_eq!(secret, Some(true));
            }
            _ => panic!("Expected Env message"),
        }
    }

    // ============================================================
    // MEDIA PROMPT TESTS
    // ============================================================

    #[test]
    fn test_serialize_chat_message() {
        let msg = Message::chat("1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"chat\""));
    }

    #[test]
    fn test_parse_chat_message() {
        let json = r#"{"type":"chat","id":"1"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Chat { id } => {
                assert_eq!(id, "1");
            }
            _ => panic!("Expected Chat message"),
        }
    }

    #[test]
    fn test_serialize_term_message() {
        let msg = Message::term("1".to_string(), Some("ls -la".to_string()));
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"term\""));
        assert!(json.contains("\"command\":\"ls -la\""));
    }

    #[test]
    fn test_parse_term_message() {
        let json = r#"{"type":"term","id":"1","command":"pwd"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Term { id, command } => {
                assert_eq!(id, "1");
                assert_eq!(command, Some("pwd".to_string()));
            }
            _ => panic!("Expected Term message"),
        }
    }

    #[test]
    fn test_serialize_widget_message() {
        let msg = Message::widget("1".to_string(), "<div>Widget</div>".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"widget\""));
    }

    #[test]
    fn test_parse_widget_message() {
        let json = r#"{"type":"widget","id":"1","html":"<div></div>"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Widget { id, html, options } => {
                assert_eq!(id, "1");
                assert_eq!(html, "<div></div>");
                assert_eq!(options, None);
            }
            _ => panic!("Expected Widget message"),
        }
    }

    #[test]
    fn test_serialize_webcam_message() {
        let msg = Message::webcam("1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"webcam\""));
    }

    #[test]
    fn test_parse_webcam_message() {
        let json = r#"{"type":"webcam","id":"1"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Webcam { id } => {
                assert_eq!(id, "1");
            }
            _ => panic!("Expected Webcam message"),
        }
    }

    #[test]
    fn test_serialize_mic_message() {
        let msg = Message::mic("1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"mic\""));
    }

    #[test]
    fn test_parse_mic_message() {
        let json = r#"{"type":"mic","id":"1"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Mic { id } => {
                assert_eq!(id, "1");
            }
            _ => panic!("Expected Mic message"),
        }
    }

    // ============================================================
    // NOTIFICATION/FEEDBACK MESSAGE TESTS
    // ============================================================

    #[test]
    fn test_serialize_notify_message() {
        let msg = Message::notify(Some("Title".to_string()), Some("Body".to_string()));
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"notify\""));
        assert!(json.contains("\"title\":\"Title\""));
        assert!(json.contains("\"body\":\"Body\""));
    }

    #[test]
    fn test_parse_notify_message() {
        let json = r#"{"type":"notify","title":"Alert","body":"Something happened"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Notify { title, body } => {
                assert_eq!(title, Some("Alert".to_string()));
                assert_eq!(body, Some("Something happened".to_string()));
            }
            _ => panic!("Expected Notify message"),
        }
    }

    #[test]
    fn test_serialize_beep_message() {
        let msg = Message::beep();
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"beep\""));
    }

    #[test]
    fn test_parse_beep_message() {
        let json = r#"{"type":"beep"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Beep {} => {}
            _ => panic!("Expected Beep message"),
        }
    }

    #[test]
    fn test_serialize_say_message() {
        let msg = Message::say("Hello".to_string(), Some("Alex".to_string()));
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"say\""));
        assert!(json.contains("\"text\":\"Hello\""));
        assert!(json.contains("\"voice\":\"Alex\""));
    }

    #[test]
    fn test_parse_say_message() {
        let json = r#"{"type":"say","text":"Hi there","voice":"Samantha"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Say { text, voice } => {
                assert_eq!(text, "Hi there");
                assert_eq!(voice, Some("Samantha".to_string()));
            }
            _ => panic!("Expected Say message"),
        }
    }

    #[test]
    fn test_serialize_set_status_message() {
        let msg = Message::set_status("busy".to_string(), Some("Working...".to_string()));
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"setStatus\""));
    }

    #[test]
    fn test_parse_set_status_message() {
        let json = r#"{"type":"setStatus","status":"idle","message":"Ready"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SetStatus { status, message } => {
                assert_eq!(status, "idle");
                assert_eq!(message, Some("Ready".to_string()));
            }
            _ => panic!("Expected SetStatus message"),
        }
    }

    // ============================================================
    // SYSTEM CONTROL MESSAGE TESTS
    // ============================================================

    #[test]
    fn test_serialize_menu_message() {
        let msg = Message::menu(Some("🚀".to_string()), Some(vec!["script1".to_string()]));
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"menu\""));
    }

    #[test]
    fn test_parse_menu_message() {
        let json = r#"{"type":"menu","icon":"⚡","scripts":["a","b"]}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Menu { icon, scripts } => {
                assert_eq!(icon, Some("⚡".to_string()));
                assert_eq!(scripts, Some(vec!["a".to_string(), "b".to_string()]));
            }
            _ => panic!("Expected Menu message"),
        }
    }

    #[test]
    fn test_serialize_clipboard_read_message() {
        let msg = Message::clipboard_read(Some(ClipboardFormat::Text));
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"clipboard\""));
        assert!(json.contains("\"action\":\"read\""));
        assert!(json.contains("\"format\":\"text\""));
    }

    #[test]
    fn test_serialize_clipboard_write_message() {
        let msg = Message::clipboard_write("content".to_string(), None);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"action\":\"write\""));
        assert!(json.contains("\"content\":\"content\""));
    }

    #[test]
    fn test_parse_clipboard_message() {
        let json = r#"{"type":"clipboard","action":"read","format":"image"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Clipboard { action, format, content } => {
                assert_eq!(action, ClipboardAction::Read);
                assert_eq!(format, Some(ClipboardFormat::Image));
                assert_eq!(content, None);
            }
            _ => panic!("Expected Clipboard message"),
        }
    }

    #[test]
    fn test_serialize_keyboard_type_message() {
        let msg = Message::keyboard_type("Hello".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"keyboard\""));
        assert!(json.contains("\"action\":\"type\""));
        assert!(json.contains("\"keys\":\"Hello\""));
    }

    #[test]
    fn test_serialize_keyboard_tap_message() {
        let msg = Message::keyboard_tap("cmd+c".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"action\":\"tap\""));
    }

    #[test]
    fn test_parse_keyboard_message() {
        let json = r#"{"type":"keyboard","action":"tap","keys":"cmd+v"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Keyboard { action, keys } => {
                assert_eq!(action, KeyboardAction::Tap);
                assert_eq!(keys, Some("cmd+v".to_string()));
            }
            _ => panic!("Expected Keyboard message"),
        }
    }

    #[test]
    fn test_serialize_mouse_message() {
        let msg = Message::mouse(MouseAction::Click, None);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"mouse\""));
        assert!(json.contains("\"action\":\"click\""));
    }

    #[test]
    fn test_parse_mouse_message() {
        let json = r#"{"type":"mouse","action":"move","data":{"x":100,"y":200}}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Mouse { action, data } => {
                assert_eq!(action, MouseAction::Move);
                assert!(data.is_some());
            }
            _ => panic!("Expected Mouse message"),
        }
    }

    #[test]
    fn test_serialize_show_message() {
        let msg = Message::show();
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"show\""));
    }

    #[test]
    fn test_parse_show_message() {
        let json = r#"{"type":"show"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Show {} => {}
            _ => panic!("Expected Show message"),
        }
    }

    #[test]
    fn test_serialize_hide_message() {
        let msg = Message::hide();
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"hide\""));
    }

    #[test]
    fn test_parse_hide_message() {
        let json = r#"{"type":"hide"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Hide {} => {}
            _ => panic!("Expected Hide message"),
        }
    }

    #[test]
    fn test_serialize_exec_message() {
        let msg = Message::exec("ls -la".to_string(), None);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"exec\""));
        assert!(json.contains("\"command\":\"ls -la\""));
    }

    #[test]
    fn test_parse_exec_message() {
        let json = r#"{"type":"exec","command":"pwd","options":{"cwd":"/home"}}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Exec { command, options } => {
                assert_eq!(command, "pwd");
                assert!(options.is_some());
            }
            _ => panic!("Expected Exec message"),
        }
    }

    // ============================================================
    // UI UPDATE MESSAGE TESTS
    // ============================================================

    #[test]
    fn test_serialize_set_panel_message() {
        let msg = Message::set_panel("<div>Panel</div>".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"setPanel\""));
        assert!(json.contains("\"html\":\"<div>Panel</div>\""));
    }

    #[test]
    fn test_parse_set_panel_message() {
        let json = r#"{"type":"setPanel","html":"<p>Info</p>"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SetPanel { html } => {
                assert_eq!(html, "<p>Info</p>");
            }
            _ => panic!("Expected SetPanel message"),
        }
    }

    #[test]
    fn test_serialize_set_preview_message() {
        let msg = Message::set_preview("<div>Preview</div>".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"setPreview\""));
    }

    #[test]
    fn test_parse_set_preview_message() {
        let json = r#"{"type":"setPreview","html":"<img src=\"x\">"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SetPreview { html } => {
                assert!(html.contains("img"));
            }
            _ => panic!("Expected SetPreview message"),
        }
    }

    #[test]
    fn test_serialize_set_prompt_message() {
        let msg = Message::set_prompt("<span>Prompt</span>".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"setPrompt\""));
    }

    #[test]
    fn test_parse_set_prompt_message() {
        let json = r#"{"type":"setPrompt","html":"<b>Enter:</b>"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SetPrompt { html } => {
                assert!(html.contains("Enter"));
            }
            _ => panic!("Expected SetPrompt message"),
        }
    }

    // ============================================================
    // MESSAGE ID TESTS FOR NEW TYPES
    // ============================================================

    #[test]
    fn test_new_message_ids() {
        // Messages with IDs
        assert_eq!(Message::editor("1".to_string()).id(), Some("1"));
        assert_eq!(Message::mini("2".to_string(), "".to_string(), vec![]).id(), Some("2"));
        assert_eq!(Message::micro("3".to_string(), "".to_string(), vec![]).id(), Some("3"));
        assert_eq!(Message::select("4".to_string(), "".to_string(), vec![], false).id(), Some("4"));
        assert_eq!(Message::fields("5".to_string(), vec![]).id(), Some("5"));
        assert_eq!(Message::form("6".to_string(), "".to_string()).id(), Some("6"));
        assert_eq!(Message::path("7".to_string(), None).id(), Some("7"));
        assert_eq!(Message::drop("8".to_string()).id(), Some("8"));
        assert_eq!(Message::hotkey("9".to_string()).id(), Some("9"));
        assert_eq!(Message::template("10".to_string(), "".to_string()).id(), Some("10"));
        assert_eq!(Message::env("11".to_string(), "".to_string(), false).id(), Some("11"));
        assert_eq!(Message::chat("12".to_string()).id(), Some("12"));
        assert_eq!(Message::term("13".to_string(), None).id(), Some("13"));
        assert_eq!(Message::widget("14".to_string(), "".to_string()).id(), Some("14"));
        assert_eq!(Message::webcam("15".to_string()).id(), Some("15"));
        assert_eq!(Message::mic("16".to_string()).id(), Some("16"));

        // Messages without IDs
        assert_eq!(Message::notify(None, None).id(), None);
        assert_eq!(Message::beep().id(), None);
        assert_eq!(Message::say("".to_string(), None).id(), None);
        assert_eq!(Message::set_status("".to_string(), None).id(), None);
        assert_eq!(Message::menu(None, None).id(), None);
        assert_eq!(Message::clipboard_read(None).id(), None);
        assert_eq!(Message::keyboard_type("".to_string()).id(), None);
        assert_eq!(Message::mouse(MouseAction::Click, None).id(), None);
        assert_eq!(Message::show().id(), None);
        assert_eq!(Message::hide().id(), None);
        assert_eq!(Message::exec("".to_string(), None).id(), None);
        assert_eq!(Message::set_panel("".to_string()).id(), None);
        assert_eq!(Message::set_preview("".to_string()).id(), None);
        assert_eq!(Message::set_prompt("".to_string()).id(), None);
    }

    // ============================================================
    // ENUM TYPE TESTS
    // ============================================================

    #[test]
    fn test_clipboard_action_serialization() {
        assert_eq!(serde_json::to_string(&ClipboardAction::Read).unwrap(), "\"read\"");
        assert_eq!(serde_json::to_string(&ClipboardAction::Write).unwrap(), "\"write\"");
    }

    #[test]
    fn test_clipboard_format_serialization() {
        assert_eq!(serde_json::to_string(&ClipboardFormat::Text).unwrap(), "\"text\"");
        assert_eq!(serde_json::to_string(&ClipboardFormat::Image).unwrap(), "\"image\"");
    }

    #[test]
    fn test_keyboard_action_serialization() {
        assert_eq!(serde_json::to_string(&KeyboardAction::Type).unwrap(), "\"type\"");
        assert_eq!(serde_json::to_string(&KeyboardAction::Tap).unwrap(), "\"tap\"");
    }

    #[test]
    fn test_mouse_action_serialization() {
        // camelCase applies to all variants
        assert_eq!(serde_json::to_string(&MouseAction::Move).unwrap(), "\"move\"");
        assert_eq!(serde_json::to_string(&MouseAction::Click).unwrap(), "\"click\"");
        assert_eq!(serde_json::to_string(&MouseAction::SetPosition).unwrap(), "\"setPosition\"");
    }

    // ============================================================
    // GRACEFUL PARSING TESTS
    // ============================================================

    #[test]
    fn test_parse_message_graceful_known_type() {
        let json = r#"{"type":"arg","id":"1","placeholder":"Pick","choices":[]}"#;
        match parse_message_graceful(json) {
            ParseResult::Ok(Message::Arg { id, .. }) => {
                assert_eq!(id, "1");
            }
            _ => panic!("Expected ParseResult::Ok with Arg message"),
        }
    }

    #[test]
    fn test_parse_message_graceful_unknown_type() {
        let json = r#"{"type":"futureFeature","id":"1","data":"test"}"#;
        match parse_message_graceful(json) {
            ParseResult::UnknownType { message_type, raw } => {
                assert_eq!(message_type, "futureFeature");
                assert_eq!(raw, json);
            }
            _ => panic!("Expected ParseResult::UnknownType"),
        }
    }

    #[test]
    fn test_parse_message_graceful_invalid_json() {
        let json = "not valid json at all";
        match parse_message_graceful(json) {
            ParseResult::ParseError(_) => {}
            _ => panic!("Expected ParseResult::ParseError"),
        }
    }

    #[test]
    fn test_parse_message_graceful_missing_type_field() {
        let json = r#"{"id":"1","data":"test"}"#;
        match parse_message_graceful(json) {
            ParseResult::ParseError(_) => {}
            _ => panic!("Expected ParseResult::ParseError for missing type field"),
        }
    }

    #[test]
    fn test_jsonl_reader_skips_empty_lines() {
        use std::io::Cursor;
        
        let jsonl = "\n{\"type\":\"beep\"}\n\n{\"type\":\"show\"}\n";
        let cursor = Cursor::new(jsonl);
        let mut reader = JsonlReader::new(cursor);
        
        // First message should be beep (skipping initial empty line)
        let msg1 = reader.next_message().unwrap();
        assert!(matches!(msg1, Some(Message::Beep {})));
        
        // Second message should be show (skipping intermediate empty lines)
        let msg2 = reader.next_message().unwrap();
        assert!(matches!(msg2, Some(Message::Show {})));
        
        // Should be EOF
        let msg3 = reader.next_message().unwrap();
        assert!(msg3.is_none());
    }

    #[test]
    fn test_jsonl_reader_graceful_skips_unknown() {
        use std::io::Cursor;
        
        let jsonl = r#"{"type":"unknownType","id":"1"}
{"type":"beep"}
{"type":"anotherUnknown","data":"test"}
{"type":"show"}
"#;
        let cursor = Cursor::new(jsonl);
        let mut reader = JsonlReader::new(cursor);
        
        // Should skip unknownType and return beep
        let msg1 = reader.next_message_graceful().unwrap();
        assert!(matches!(msg1, Some(Message::Beep {})));
        
        // Should skip anotherUnknown and return show
        let msg2 = reader.next_message_graceful().unwrap();
        assert!(matches!(msg2, Some(Message::Show {})));
        
        // Should be EOF
        let msg3 = reader.next_message_graceful().unwrap();
        assert!(msg3.is_none());
    }
}
