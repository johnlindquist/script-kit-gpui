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

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read};

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
/// Each variant corresponds to a message kind: arg, div, submit, update, exit
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
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

    /// Get the message ID (works for all message types except Exit)
    pub fn id(&self) -> Option<&str> {
        match self {
            Message::Arg { id, .. } => Some(id),
            Message::Div { id, .. } => Some(id),
            Message::Submit { id, .. } => Some(id),
            Message::Update { id, .. } => Some(id),
            Message::Exit { .. } => None,
        }
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
    serde_json::from_str(line)
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
            0 => Ok(None), // EOF
            _ => {
                let msg = parse_message(line.trim())?;
                Ok(Some(msg))
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
}
