//! Script and Scriptlet Creation Module
//!
//! This module provides functions to create new scripts and scriptlets
//! in the Script Kit environment, as well as opening files in the configured editor.
//!
//! # Usage
//!
//! ```rust,ignore
//! use script_kit_gpui::script_creation::{create_new_script, create_new_scriptlet, open_in_editor};
//! use script_kit_gpui::config::Config;
//!
//! // Create a new script
//! let script_path = create_new_script("my-script")?;
//!
//! // Create a new scriptlet bundle
//! let scriptlet_path = create_new_scriptlet("my-scriptlet")?;
//!
//! // Open in editor
//! let config = Config::default();
//! open_in_editor(&script_path, &config)?;
//! ```

// --- merged from part_000.rs ---
use crate::config::Config;
use anyhow::{Context, Result};
use itertools::Itertools;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, instrument, warn};
/// Maximum filename size on most filesystems (bytes, not chars).
const MAX_FILENAME_BYTES: usize = 255;
/// Reserved filenames on Windows that are invalid even with an extension.
const WINDOWS_RESERVED_FILENAMES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];
/// Root directory for a given plugin under the active Script Kit workspace.
pub fn plugin_dir(plugin_id: &str) -> PathBuf {
    crate::setup::get_kit_path().join("kit").join(plugin_id)
}
/// Scripts directory for a given plugin.
pub fn scripts_dir_for_plugin(plugin_id: &str) -> PathBuf {
    plugin_dir(plugin_id).join("scripts")
}
/// Scriptlets directory for a given plugin.
pub fn scriptlets_dir_for_plugin(plugin_id: &str) -> PathBuf {
    plugin_dir(plugin_id).join("scriptlets")
}
/// Agents directory for a given plugin.
pub fn agents_dir_for_plugin(plugin_id: &str) -> PathBuf {
    plugin_dir(plugin_id).join("agents")
}
/// Skills directory for a given plugin.
pub fn skills_dir_for_plugin(plugin_id: &str) -> PathBuf {
    plugin_dir(plugin_id).join("skills")
}
/// Scripts directory under the active Script Kit workspace (default: `kit/main/scripts`).
pub fn scripts_dir() -> PathBuf {
    scripts_dir_for_plugin("main")
}
/// Scriptlets directory under the active Script Kit workspace (default: `kit/main/scriptlets`).
pub fn scriptlets_dir() -> PathBuf {
    scriptlets_dir_for_plugin("main")
}
/// Create a new script in a specific plugin's scripts directory.
pub fn create_new_script_in_plugin(plugin_id: &str, name: &str) -> Result<PathBuf> {
    create_new_script_in_dir(name, &scripts_dir_for_plugin(plugin_id))
}
/// Create a new scriptlet in a specific plugin's scriptlets directory.
pub fn create_new_scriptlet_in_plugin(plugin_id: &str, name: &str) -> Result<PathBuf> {
    create_new_scriptlet_in_dir(name, &scriptlets_dir_for_plugin(plugin_id))
}
fn validate_sanitized_name(
    original_name: &str,
    sanitized_name: &str,
    extension: &str,
    item_label: &str,
) -> Result<()> {
    if sanitized_name.is_empty() {
        anyhow::bail!(
            "{item_label} name cannot be empty after sanitization (original='{original_name}', sanitized='{sanitized_name}')"
        );
    }

    let normalized = sanitized_name.to_ascii_lowercase();
    if WINDOWS_RESERVED_FILENAMES.contains(&normalized.as_str()) {
        anyhow::bail!(
            "{item_label} name is reserved on Windows (original='{original_name}', sanitized='{sanitized_name}')"
        );
    }

    let filename = format!("{sanitized_name}.{extension}");
    if filename.len() > MAX_FILENAME_BYTES {
        anyhow::bail!(
            "{item_label} filename too long after sanitization (original='{original_name}', sanitized='{sanitized_name}', max_bytes={MAX_FILENAME_BYTES})"
        );
    }

    Ok(())
}
fn create_new_text_file(path: &Path, content: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}
fn create_unique_templated_file<F>(
    original_name: &str,
    sanitized_name: &str,
    dir: &Path,
    extension: &str,
    item_label: &str,
    template_for_name: F,
) -> Result<(PathBuf, String)>
where
    F: Fn(&str) -> String,
{
    for suffix in 0usize.. {
        let candidate_name = if suffix == 0 {
            sanitized_name.to_string()
        } else {
            format!("{sanitized_name}-{suffix}")
        };
        validate_sanitized_name(original_name, &candidate_name, extension, item_label)?;

        let filename = format!("{candidate_name}.{extension}");
        let path = dir.join(&filename);
        let template = template_for_name(&candidate_name);

        match create_new_text_file(&path, &template) {
            Ok(()) => return Ok((path, candidate_name)),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                warn!(
                    candidate = %candidate_name,
                    path = %path.display(),
                    "script_creation_collision_retry"
                );
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "Failed to write {item_label} file '{}' (sanitized='{}')",
                        path.display(),
                        candidate_name
                    )
                });
            }
        }
    }

    unreachable!("infinite suffix loop should always return or error")
}
/// Sanitize a script name for use as a filename.
///
/// - Converts to lowercase
/// - Replaces spaces and underscores with hyphens
/// - Removes special characters (keeps only alphanumeric and hyphens)
/// - Removes leading/trailing hyphens
/// - Collapses multiple consecutive hyphens into one
fn sanitize_name(name: &str) -> String {
    let sanitized: String = name
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c == ' ' || c == '_' || c == '-' {
                '-'
            } else {
                // Skip other special characters
                '\0'
            }
        })
        .filter(|&c| c != '\0')
        .collect();

    // Collapse multiple consecutive hyphens and trim
    let mut result = String::new();
    let mut last_was_hyphen = false;

    for c in sanitized.chars() {
        if c == '-' {
            if !last_was_hyphen && !result.is_empty() {
                result.push(c);
                last_was_hyphen = true;
            }
        } else {
            result.push(c);
            last_was_hyphen = false;
        }
    }

    // Remove trailing hyphen
    if result.ends_with('-') {
        result.pop();
    }

    result
}
/// Convert a sanitized filename to a human-readable title.
///
/// - Replaces hyphens with spaces
/// - Capitalizes first letter of each word
fn name_to_title(name: &str) -> String {
    name.split('-')
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .join(" ")
}
/// Generate the script template using the global metadata format.
///
/// This is the preferred format per AGENTS.md - uses `export const metadata = {...}`
/// instead of comment-based metadata.
fn generate_script_template(name: &str) -> String {
    let title = name_to_title(name);
    format!(
        r#"import "@scriptkit/sdk";

export const metadata = {{
  name: "{title}",
  description: "",
}};

/*
Template Guide
=============
A) Input Prompt
   Ask for a value with `arg()` and continue with that input.

B) Selection List
   Return an array from `arg()` to let users pick one option.

C) Background Task
   Use async functions and status updates for longer work.
*/

// Quick starter: input prompt → show result in a HUD
const result = await arg("What should this script do?");
await div(md(`## ${{result}}`));
"#
    )
}
/// Generate the scriptlet template as markdown with embedded code.
///
/// Scriptlets are markdown files with code blocks that can be executed.
fn generate_scriptlet_template(name: &str) -> String {
    let title = name_to_title(name);
    format!(
        r#"---
name: {title}
description: "Grouped lightweight helpers"
icon: sparkles
---

## Hello Snippet

```metadata
keyword: !hello
description: Quick greeting
```

```paste
Hello from {title}!
```

## Quick Note

```metadata
description: Save a quick note
```

```tool:quick-note
import "@scriptkit/sdk";

const note = await arg("Note");
await Bun.write(`${{env.HOME}}/quick-note.txt`, note);
await notify("Saved");
```
"#
    )
}
/// Create a new script file in ~/.scriptkit/kit/main/scripts/
///
/// # Arguments
///
/// * `name` - The name of the script (will be sanitized for filename)
///
/// # Returns
///
/// The path to the created script file.
///
/// # Errors
///
/// Returns an error if:
/// - The scripts directory cannot be created
/// - A valid filename cannot be derived from the provided name
/// - The file cannot be written
#[instrument(name = "create_new_script", skip_all, fields(name = %name))]
pub fn create_new_script(name: &str) -> Result<PathBuf> {
    create_new_script_in_dir(name, &scripts_dir())
}
fn create_new_script_in_dir(name: &str, scripts_dir: &Path) -> Result<PathBuf> {
    let sanitized_name = sanitize_name(name);
    validate_sanitized_name(name, &sanitized_name, "ts", "Script")?;

    // Ensure the scripts directory exists
    fs::create_dir_all(scripts_dir).with_context(|| {
        format!(
            "Failed to create scripts directory: {}",
            scripts_dir.display()
        )
    })?;

    let (script_path, created_name) = create_unique_templated_file(
        name,
        &sanitized_name,
        scripts_dir,
        "ts",
        "script",
        generate_script_template,
    )?;

    info!(
        path = %script_path.display(),
        name = %created_name,
        requested_name = %name,
        "Created new script"
    );

    Ok(script_path)
}
/// Create a new scriptlet file in ~/.scriptkit/kit/main/scriptlets/
///
/// # Arguments
///
/// * `name` - The name of the scriptlet bundle (will be sanitized for filename)
///
/// # Returns
///
/// The path to the created scriptlet file.
///
/// # Errors
///
/// Returns an error if:
/// - The scriptlets directory cannot be created
/// - A valid filename cannot be derived from the provided name
/// - The file cannot be written
#[instrument(name = "create_new_scriptlet", skip_all, fields(name = %name))]
pub fn create_new_scriptlet(name: &str) -> Result<PathBuf> {
    create_new_scriptlet_in_dir(name, &scriptlets_dir())
}
fn create_new_scriptlet_in_dir(name: &str, scriptlets_dir: &Path) -> Result<PathBuf> {
    let sanitized_name = sanitize_name(name);
    validate_sanitized_name(name, &sanitized_name, "md", "Scriptlet bundle")?;

    // Ensure the scriptlets directory exists
    fs::create_dir_all(scriptlets_dir).with_context(|| {
        format!(
            "Failed to create scriptlets directory: {}",
            scriptlets_dir.display()
        )
    })?;

    let (scriptlet_path, created_name) = create_unique_templated_file(
        name,
        &sanitized_name,
        scriptlets_dir,
        "md",
        "scriptlet bundle",
        generate_scriptlet_template,
    )?;

    info!(
        path = %scriptlet_path.display(),
        name = %created_name,
        requested_name = %name,
        "Created new scriptlet bundle"
    );

    Ok(scriptlet_path)
}
/// Open a file in the configured editor.
///
/// Uses the editor from config, falling back to $EDITOR env var,
/// then to "code" (VS Code) as the final default.
///
/// # Arguments
///
/// * `path` - The path to the file to open
/// * `config` - The application configuration
///
/// # Errors
///
/// Returns an error if the editor command fails to spawn.
fn parse_editor_command(editor: &str) -> Result<Vec<String>> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = editor.chars().peekable();

    while let Some(ch) = chars.next() {
        match quote {
            Some(quote_char) => {
                if ch == quote_char {
                    quote = None;
                } else if ch == '\\' && quote_char == '"' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    } else {
                        current.push(ch);
                    }
                } else {
                    current.push(ch);
                }
            }
            None => match ch {
                '\'' | '"' => quote = Some(ch),
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    } else {
                        current.push(ch);
                    }
                }
                c if c.is_whitespace() => {
                    if !current.is_empty() {
                        parts.push(std::mem::take(&mut current));
                    }
                }
                _ => current.push(ch),
            },
        }
    }

    if quote.is_some() {
        anyhow::bail!("Unterminated quote in editor command: {editor}");
    }

    if !current.is_empty() {
        parts.push(current);
    }

    if parts.is_empty() {
        anyhow::bail!("Editor command is empty");
    }

    Ok(parts)
}
#[instrument(name = "open_in_editor", skip(config), fields(path = %path.display()))]
pub fn open_in_editor(path: &Path, config: &Config) -> Result<()> {
    let editor = config.get_editor();
    let command_parts = parse_editor_command(&editor)?;
    let executable = &command_parts[0];
    let args = &command_parts[1..];

    info!(editor = %editor, path = %path.display(), "Opening file in editor");

    let mut command = Command::new(executable);
    command.args(args);
    command.arg(path);

    let status = command.spawn().with_context(|| {
        format!(
            "Failed to spawn editor command for file {} (raw='{}', executable='{}', args={:?})",
            path.display(),
            editor,
            executable,
            args
        )
    })?;

    // We spawn and detach - don't wait for the editor to close
    // The child process handle is dropped, but the process continues
    drop(status);

    Ok(())
}

// --- merged from part_001.rs ---
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::tempdir;

    #[test]
    fn test_sanitize_name_basic() {
        assert_eq!(sanitize_name("hello"), "hello");
        assert_eq!(sanitize_name("Hello World"), "hello-world");
        assert_eq!(sanitize_name("my_script_name"), "my-script-name");
    }

    #[test]
    fn test_sanitize_name_special_chars() {
        assert_eq!(sanitize_name("hello@world!"), "helloworld");
        assert_eq!(sanitize_name("test#$%script"), "testscript");
        assert_eq!(sanitize_name("foo & bar"), "foo-bar");
    }

    #[test]
    fn test_sanitize_name_multiple_hyphens() {
        assert_eq!(sanitize_name("hello---world"), "hello-world");
        assert_eq!(sanitize_name("a - b - c"), "a-b-c");
        assert_eq!(sanitize_name("  spaces  "), "spaces");
    }

    #[test]
    fn test_sanitize_name_leading_trailing() {
        assert_eq!(sanitize_name("-hello-"), "hello");
        assert_eq!(sanitize_name("---test---"), "test");
        assert_eq!(sanitize_name(" - hello - "), "hello");
    }

    #[test]
    fn test_sanitize_name_empty() {
        assert_eq!(sanitize_name(""), "");
        assert_eq!(sanitize_name("   "), "");
        assert_eq!(sanitize_name("@#$%"), "");
    }

    #[test]
    fn test_name_to_title_basic() {
        assert_eq!(name_to_title("hello"), "Hello");
        assert_eq!(name_to_title("hello-world"), "Hello World");
        assert_eq!(name_to_title("my-awesome-script"), "My Awesome Script");
    }

    #[test]
    fn test_name_to_title_edge_cases() {
        assert_eq!(name_to_title(""), "");
        assert_eq!(name_to_title("a"), "A");
        assert_eq!(name_to_title("a-b-c"), "A B C");
    }

    #[test]
    fn test_generate_script_template() {
        let template = generate_script_template("my-script");
        assert!(template.contains("import \"@scriptkit/sdk\";"));
        assert!(template.contains("export const metadata = {"));
        assert!(template.contains("name: \"My Script\""));
        assert!(template.contains("description: \"\""));
        assert!(template.contains("await arg("));
        assert!(template.contains("Template Guide"));
        assert!(template.contains("A) Input Prompt"));
        assert!(template.contains("B) Selection List"));
        assert!(template.contains("C) Background Task"));
    }

    #[test]
    fn test_generate_scriptlet_template() {
        let template = generate_scriptlet_template("my-extension");
        assert!(template.starts_with("---"));
        assert!(template.contains("name: My Extension"));
        assert!(template.contains("description: \"Grouped lightweight helpers\""));
        assert!(template.contains("icon: sparkles"));
        assert!(template.contains("## Hello Snippet"));
        assert!(template.contains("```metadata"));
        assert!(template.contains("```paste"));
        assert!(template.contains("```tool:quick-note"));
        assert!(template.contains("import \"@scriptkit/sdk\";"));
    }

    #[test]
    fn test_create_new_script_empty_name() {
        let result = create_new_script("");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("empty after sanitization"));
    }

    #[test]
    fn test_create_new_script_special_chars_only() {
        let result = create_new_script("@#$%^&*");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("empty after sanitization"));
    }

    #[test]
    fn test_create_new_scriptlet_empty_name() {
        let result = create_new_scriptlet("");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("empty after sanitization"));
    }

    // Integration tests that actually create files
    // These use tempdir to avoid polluting the real scripts directory

    #[test]
    fn test_create_script_integration() {
        let temp_dir = tempdir().unwrap();
        let scripts_dir = temp_dir.path().join("scripts");
        let script_path = create_new_script_in_dir("test-script", &scripts_dir).unwrap();

        // Verify the file was created
        assert!(script_path.exists());
        assert_eq!(script_path.file_name().unwrap(), "test-script.ts");

        // Verify the content
        let content = fs::read_to_string(&script_path).unwrap();
        assert!(content.contains("export const metadata"));
        assert!(content.contains("Test Script"));
    }

    #[test]
    fn test_create_scriptlet_integration() {
        let temp_dir = tempdir().unwrap();
        let scriptlets_dir = temp_dir.path().join("scriptlets");
        let scriptlet_path =
            create_new_scriptlet_in_dir("test-extension", &scriptlets_dir).unwrap();

        // Verify the file was created
        assert!(scriptlet_path.exists());
        assert_eq!(scriptlet_path.file_name().unwrap(), "test-extension.md");

        // Verify the content is markdown with metadata fences
        let content = fs::read_to_string(&scriptlet_path).unwrap();
        assert!(content.contains("name: Test Extension"));
        assert!(content.contains("```metadata"));
    }

    #[test]
    fn test_create_new_script_in_dir_generates_unique_name_when_base_exists() {
        let temp_dir = tempdir().unwrap();
        let scripts_dir = temp_dir.path().join("scripts");

        let first = create_new_script_in_dir("untitled", &scripts_dir).unwrap();
        let second = create_new_script_in_dir("untitled", &scripts_dir).unwrap();

        assert_eq!(first.file_name().unwrap(), "untitled.ts");
        assert_eq!(second.file_name().unwrap(), "untitled-1.ts");
        assert!(first.exists());
        assert!(second.exists());
    }

    #[test]
    fn test_create_new_scriptlet_in_dir_generates_unique_name_when_base_exists() {
        let temp_dir = tempdir().unwrap();
        let scriptlets_dir = temp_dir.path().join("scriptlets");

        let first = create_new_scriptlet_in_dir("my-extension", &scriptlets_dir).unwrap();
        let second = create_new_scriptlet_in_dir("my-extension", &scriptlets_dir).unwrap();

        assert_eq!(first.file_name().unwrap(), "my-extension.md");
        assert_eq!(second.file_name().unwrap(), "my-extension-1.md");
        assert!(first.exists());
        assert!(second.exists());
    }

    #[test]
    fn test_validate_sanitized_name_rejects_windows_reserved_name() {
        let err = validate_sanitized_name("CON", "con", "ts", "Script").unwrap_err();
        assert!(err.to_string().contains("reserved"));
    }

    #[test]
    fn test_create_new_script_in_dir_rejects_windows_reserved_name_after_sanitization() {
        let temp_dir = tempdir().unwrap();
        let scripts_dir = temp_dir.path().join("scripts");

        let err = create_new_script_in_dir("CON!!!", &scripts_dir).unwrap_err();
        assert!(err.to_string().contains("reserved on Windows"));
    }

    #[test]
    fn test_validate_sanitized_name_rejects_overlong_filename() {
        let long_name = "a".repeat(253);
        let err = validate_sanitized_name(&long_name, &long_name, "ts", "Script").unwrap_err();
        assert!(err.to_string().contains("too long"));
    }

    #[test]
    fn test_parse_editor_command_splits_flags_and_quotes() {
        let parts = parse_editor_command(r#"code --reuse-window --goto "src/main.rs:10""#).unwrap();
        assert_eq!(
            parts,
            vec![
                "code".to_string(),
                "--reuse-window".to_string(),
                "--goto".to_string(),
                "src/main.rs:10".to_string(),
            ]
        );
    }

    #[test]
    fn test_create_new_text_file_does_not_overwrite_existing_file() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("atomic-test.txt");

        create_new_text_file(&path, "first").unwrap();
        let err = create_new_text_file(&path, "second").unwrap_err();

        assert_eq!(err.kind(), ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(path).unwrap(), "first");
    }

    #[test]
    fn test_config_get_editor() {
        // Test that Config::get_editor works as expected
        let config = Config::default();

        // Save and clear EDITOR env var for predictable test
        let original_editor = env::var("EDITOR").ok();
        env::remove_var("EDITOR");

        // With no config editor and no EDITOR env, should return "code"
        let default_config = Config {
            hotkey: config.hotkey.clone(),
            bun_path: None,
            editor: None,
            padding: None,
            editor_font_size: None,
            terminal_font_size: None,
            ui_scale: None,
            built_ins: None,
            process_limits: None,
            clipboard_history_max_text_length: None,
            suggested: None,
            notes_hotkey: None,
            ai_hotkey: None,
            logs_hotkey: None,
            ai_hotkey_enabled: None,
            logs_hotkey_enabled: None,
            dictation_hotkey: None,
            dictation_hotkey_enabled: None,
            watcher: None,
            layout: None,
            theme: None,
            dictation: None,
            ai: None,
            window_management: None,
            commands: None,
            claude_code: None,
            mcp: None,
        };
        assert_eq!(default_config.get_editor(), "code");

        // With config editor set, should use that
        let custom_config = Config {
            hotkey: config.hotkey.clone(),
            bun_path: None,
            editor: Some("vim".to_string()),
            padding: None,
            editor_font_size: None,
            terminal_font_size: None,
            ui_scale: None,
            built_ins: None,
            process_limits: None,
            clipboard_history_max_text_length: None,
            suggested: None,
            notes_hotkey: None,
            ai_hotkey: None,
            logs_hotkey: None,
            ai_hotkey_enabled: None,
            logs_hotkey_enabled: None,
            dictation_hotkey: None,
            dictation_hotkey_enabled: None,
            watcher: None,
            layout: None,
            theme: None,
            dictation: None,
            ai: None,
            window_management: None,
            commands: None,
            claude_code: None,
            mcp: None,
        };
        assert_eq!(custom_config.get_editor(), "vim");

        // Restore original EDITOR
        if let Some(val) = original_editor {
            env::set_var("EDITOR", val);
        }
    }
}
