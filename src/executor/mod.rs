//! Script execution module
//!
//! This module handles all aspects of script execution including:
//! - Interactive script sessions with bidirectional JSONL communication
//! - Scriptlet execution (embedded scripts in markdown)
//! - Error parsing and suggestions
//! - Selected text operations
//! - Auto-submit mode for autonomous testing

mod auto_submit;
mod errors;
pub(crate) mod runner;
mod scriptlet;
mod selected_text;
mod stderr_buffer;
pub(crate) mod telemetry;

// Re-export public items for external use
// Allow unused imports - these are public API exports that may be used by external code
// or will be used in the future (marked #[allow(dead_code)] in their source files)
#[allow(unused_imports)]
pub use auto_submit::{
    get_auto_submit_config, get_auto_submit_delay, get_auto_submit_index, get_auto_submit_value,
    is_auto_submit_enabled, AutoSubmitConfig,
};

pub use errors::{extract_error_message, generate_suggestions, parse_stack_trace};

pub use runner::{execute_script_interactive_with_env_and_args, ScriptSession};

#[cfg(test)]
pub(crate) use runner::{
    find_executable, find_sdk_path, is_javascript, is_typescript, spawn_script, ProcessHandle,
    SplitSession,
};

pub use scriptlet::{run_scriptlet, ScriptletExecOptions};

// Additional scriptlet exports
#[allow(unused_imports)]
pub use scriptlet::{
    build_final_content, execute_applescript, execute_edit, execute_open, execute_paste,
    execute_shell_scriptlet, execute_submit, execute_transform, execute_type, execute_typescript,
    execute_with_interpreter, shell_not_found_suggestions, ScriptletResult,
};

pub use selected_text::{handle_selected_text_message, SelectedTextHandleResult};

// Allow unused - these are public API exports for future use
#[allow(unused_imports)]
pub use stderr_buffer::{spawn_stderr_reader, StderrBuffer, StderrCapture};

// Re-export tool_extension only for tests
#[cfg(test)]
pub use scriptlet::tool_extension;

#[cfg(test)]
#[test]
fn test_runner_helpers_available_for_executor_tests() {
    let _ = find_executable;
    let _ = find_sdk_path;
    let _ = is_javascript;
    let _ = is_typescript;
    let _ = spawn_script;
    let _ = std::mem::size_of::<ProcessHandle>();
    let _ = std::mem::size_of::<SplitSession>();
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod runner_integration_tests;

#[cfg(all(test, target_os = "macos", feature = "owned-ui-evaluation"))]
mod owned_effect_tests;
