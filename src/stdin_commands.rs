//! External command handling via stdin.
//!
//! This module provides the ability to control the Script Kit app via stdin JSONL commands.
//! This is primarily used for testing and automation.
//!
//! # Protocol
//!
//! Commands are sent as JSON objects, one per line (JSONL format):
//!
//! ```json
//! {"type": "run", "path": "/path/to/script.ts"}
//! {"type": "show"}
//! {"type": "hide"}
//! {"type": "setFilter", "text": "search term"}
//! {"type": "triggerBuiltin", "name": "clipboardHistory"}
//! {"type": "simulateKey", "key": "enter", "modifiers": ["cmd"]}
//! ```
//!
//! # Example Usage
//!
//! ```bash
//! # Run a script via stdin
//! echo '{"type": "run", "path": "/path/to/script.ts"}' | ./script-kit-gpui
//!
//! # Show/hide the window
//! echo '{"type": "show"}' | ./script-kit-gpui
//! echo '{"type": "hide"}' | ./script-kit-gpui
//!
//! # Filter the script list (for testing)
//! echo '{"type": "setFilter", "text": "hello"}' | ./script-kit-gpui
//! ```

use crate::logging;
use crate::protocol::GridDepthOption;

/// Default grid size for ShowGrid command
fn default_grid_size() -> u32 {
    8
}

/// External commands that can be sent to the app via stdin
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ExternalCommand {
    /// Run a script by path
    Run { path: String },
    /// Show the window
    Show,
    /// Hide the window
    Hide,
    /// Set the filter text (for testing)
    SetFilter { text: String },
    /// Trigger a built-in feature by name (for testing)
    TriggerBuiltin { name: String },
    /// Simulate a key press (for testing)
    /// key: Key name like "enter", "escape", "up", "down", "k", etc.
    /// modifiers: Optional array of modifiers ["cmd", "shift", "alt", "ctrl"]
    SimulateKey {
        key: String,
        #[serde(default)]
        modifiers: Vec<String>,
    },
    /// Open the Notes window (for testing)
    OpenNotes,
    /// Open the AI Chat window (for testing)
    OpenAi,
    /// Open the AI Chat window with mock data (for visual testing)
    /// This inserts sample conversations to test the UI layout
    OpenAiWithMockData,
    /// Capture a screenshot of a window by title pattern and save to file (for testing)
    /// title: Title pattern to match (e.g., "Script Kit AI" for the AI window)
    /// path: File path to save the PNG screenshot
    CaptureWindow { title: String, path: String },
    /// Set the AI window search filter (for testing chat search)
    /// text: Search query to filter chats
    SetAiSearch { text: String },
    /// Set the AI window input text and optionally submit (for testing streaming)
    /// text: Message text to set in the input field
    /// submit: If true, submit the message after setting (triggers streaming)
    SetAiInput {
        text: String,
        #[serde(default)]
        submit: bool,
    },
    /// Show the debug grid overlay with options (for visual testing)
    ShowGrid {
        #[serde(default = "default_grid_size", rename = "gridSize")]
        grid_size: u32,
        #[serde(default, rename = "showBounds")]
        show_bounds: bool,
        #[serde(default, rename = "showBoxModel")]
        show_box_model: bool,
        #[serde(default, rename = "showAlignmentGuides")]
        show_alignment_guides: bool,
        #[serde(default)]
        depth: GridDepthOption,
    },
    /// Hide the debug grid overlay
    HideGrid,
}

/// Start a thread that listens on stdin for external JSONL commands.
/// Returns an async_channel::Receiver that can be awaited without polling.
///
/// # Channel Capacity
///
/// Uses a bounded channel with capacity of 100 to prevent unbounded memory growth.
/// This is generous for stdin commands which typically arrive at < 10/sec.
///
/// # Thread Safety
///
/// Spawns a background thread that reads stdin line-by-line. When the channel
/// is closed (receiver dropped), the thread will exit gracefully.
pub fn start_stdin_listener() -> async_channel::Receiver<ExternalCommand> {
    use std::io::BufRead;

    // P1-6: Use bounded channel to prevent unbounded memory growth
    // Capacity of 100 is generous for stdin commands (typically < 10/sec)
    let (tx, rx) = async_channel::bounded(100);

    std::thread::spawn(move || {
        logging::log("STDIN", "External command listener started");
        let stdin = std::io::stdin();
        let reader = stdin.lock();

        for line in reader.lines() {
            match line {
                Ok(line) if !line.trim().is_empty() => {
                    logging::log("STDIN", &format!("Received: {}", line));
                    match serde_json::from_str::<ExternalCommand>(&line) {
                        Ok(cmd) => {
                            logging::log("STDIN", &format!("Parsed command: {:?}", cmd));
                            // send_blocking is used since we're in a sync thread
                            if tx.send_blocking(cmd).is_err() {
                                logging::log("STDIN", "Command channel closed, exiting");
                                break;
                            }
                        }
                        Err(e) => {
                            logging::log("STDIN", &format!("Failed to parse command: {}", e));
                        }
                    }
                }
                Ok(_) => {} // Empty line, ignore
                Err(e) => {
                    logging::log("STDIN", &format!("Error reading stdin: {}", e));
                    break;
                }
            }
        }
        logging::log("STDIN", "External command listener exiting");
    });

    rx
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_external_command_run_deserialization() {
        let json = r#"{"type": "run", "path": "/path/to/script.ts"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        match cmd {
            ExternalCommand::Run { path } => assert_eq!(path, "/path/to/script.ts"),
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_external_command_show_deserialization() {
        let json = r#"{"type": "show"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        assert!(matches!(cmd, ExternalCommand::Show));
    }

    #[test]
    fn test_external_command_hide_deserialization() {
        let json = r#"{"type": "hide"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        assert!(matches!(cmd, ExternalCommand::Hide));
    }

    #[test]
    fn test_external_command_set_filter_deserialization() {
        let json = r#"{"type": "setFilter", "text": "hello world"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        match cmd {
            ExternalCommand::SetFilter { text } => assert_eq!(text, "hello world"),
            _ => panic!("Expected SetFilter command"),
        }
    }

    #[test]
    fn test_external_command_trigger_builtin_deserialization() {
        let json = r#"{"type": "triggerBuiltin", "name": "clipboardHistory"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        match cmd {
            ExternalCommand::TriggerBuiltin { name } => assert_eq!(name, "clipboardHistory"),
            _ => panic!("Expected TriggerBuiltin command"),
        }
    }

    #[test]
    fn test_external_command_simulate_key_deserialization() {
        let json = r#"{"type": "simulateKey", "key": "enter", "modifiers": ["cmd", "shift"]}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        match cmd {
            ExternalCommand::SimulateKey { key, modifiers } => {
                assert_eq!(key, "enter");
                assert_eq!(modifiers, vec!["cmd", "shift"]);
            }
            _ => panic!("Expected SimulateKey command"),
        }
    }

    #[test]
    fn test_external_command_simulate_key_no_modifiers() {
        let json = r#"{"type": "simulateKey", "key": "escape"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        match cmd {
            ExternalCommand::SimulateKey { key, modifiers } => {
                assert_eq!(key, "escape");
                assert!(modifiers.is_empty());
            }
            _ => panic!("Expected SimulateKey command"),
        }
    }

    #[test]
    fn test_external_command_invalid_json_fails() {
        let json = r#"{"type": "unknown"}"#;
        let result = serde_json::from_str::<ExternalCommand>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_external_command_missing_required_field_fails() {
        // Run command requires path field
        let json = r#"{"type": "run"}"#;
        let result = serde_json::from_str::<ExternalCommand>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_external_command_clone() {
        let cmd = ExternalCommand::Run {
            path: "/test".to_string(),
        };
        let cloned = cmd.clone();
        match cloned {
            ExternalCommand::Run { path } => assert_eq!(path, "/test"),
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_external_command_debug() {
        let cmd = ExternalCommand::Show;
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("Show"));
    }

    #[test]
    fn test_external_command_open_notes_deserialization() {
        let json = r#"{"type": "openNotes"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        assert!(matches!(cmd, ExternalCommand::OpenNotes));
    }

    #[test]
    fn test_external_command_open_ai_deserialization() {
        let json = r#"{"type": "openAi"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        assert!(matches!(cmd, ExternalCommand::OpenAi));
    }

    #[test]
    fn test_external_command_open_ai_with_mock_data_deserialization() {
        let json = r#"{"type": "openAiWithMockData"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        assert!(matches!(cmd, ExternalCommand::OpenAiWithMockData));
    }

    #[test]
    fn test_external_command_capture_window_deserialization() {
        let json =
            r#"{"type": "captureWindow", "title": "Script Kit AI", "path": "/tmp/screenshot.png"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        match cmd {
            ExternalCommand::CaptureWindow { title, path } => {
                assert_eq!(title, "Script Kit AI");
                assert_eq!(path, "/tmp/screenshot.png");
            }
            _ => panic!("Expected CaptureWindow command"),
        }
    }

    #[test]
    fn test_external_command_show_grid_defaults() {
        let json = r#"{"type": "showGrid"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        match cmd {
            ExternalCommand::ShowGrid {
                grid_size,
                show_bounds,
                show_box_model,
                show_alignment_guides,
                depth,
            } => {
                assert_eq!(grid_size, 8); // default
                assert!(!show_bounds); // default false
                assert!(!show_box_model); // default false
                assert!(!show_alignment_guides); // default false
                assert!(matches!(depth, GridDepthOption::Preset(_))); // default
            }
            _ => panic!("Expected ShowGrid command"),
        }
    }

    #[test]
    fn test_external_command_show_grid_with_options() {
        let json = r#"{"type": "showGrid", "gridSize": 16, "showBounds": true, "showBoxModel": true, "showAlignmentGuides": true, "depth": "all"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        match cmd {
            ExternalCommand::ShowGrid {
                grid_size,
                show_bounds,
                show_box_model,
                show_alignment_guides,
                depth,
            } => {
                assert_eq!(grid_size, 16);
                assert!(show_bounds);
                assert!(show_box_model);
                assert!(show_alignment_guides);
                match depth {
                    GridDepthOption::Preset(s) => assert_eq!(s, "all"),
                    _ => panic!("Expected Preset depth"),
                }
            }
            _ => panic!("Expected ShowGrid command"),
        }
    }

    #[test]
    fn test_external_command_show_grid_with_components() {
        let json = r#"{"type": "showGrid", "depth": ["header", "footer"]}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        match cmd {
            ExternalCommand::ShowGrid { depth, .. } => match depth {
                GridDepthOption::Components(components) => {
                    assert_eq!(components, vec!["header", "footer"]);
                }
                _ => panic!("Expected Components depth"),
            },
            _ => panic!("Expected ShowGrid command"),
        }
    }

    #[test]
    fn test_external_command_hide_grid_deserialization() {
        let json = r#"{"type": "hideGrid"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        assert!(matches!(cmd, ExternalCommand::HideGrid));
    }
}
