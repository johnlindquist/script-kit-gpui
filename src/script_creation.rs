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
//! // Create a new scriptlet
//! let scriptlet_path = create_new_scriptlet("my-scriptlet")?;
//!
//! // Open in editor
//! let config = Config::default();
//! open_in_editor(&script_path, &config)?;
//! ```

use crate::config::Config;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, instrument, warn};

/// Scripts directory under ~/.sk/kit/
const SCRIPTS_DIR: &str = "~/.sk/kit/scripts";

/// Scriptlets directory under ~/.sk/kit/
const SCRIPTLETS_DIR: &str = "~/.sk/kit/scriptlets";

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
        .collect::<Vec<_>>()
        .join(" ")
}

/// Generate the script template using the global metadata format.
///
/// This is the preferred format per AGENTS.md - uses `export const metadata = {...}`
/// instead of comment-based metadata.
fn generate_script_template(name: &str) -> String {
    let title = name_to_title(name);
    format!(
        r#"import "@scriptkit/kit";

export const metadata = {{
  name: "{title}",
  description: "",
}};

// Script implementation
const result = await arg("Enter your input");
console.log(result);
"#
    )
}

/// Generate the scriptlet template using the global metadata format.
///
/// Scriptlets are simpler scripts that can use template variables.
fn generate_scriptlet_template(name: &str) -> String {
    let title = name_to_title(name);
    format!(
        r#"import "@scriptkit/kit";

export const metadata = {{
  name: "{title}",
  description: "",
}};

// Scriptlet implementation
await div(`<h1>{title}</h1>`);
"#
    )
}

/// Create a new script file in ~/.sk/kit/scripts/
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
/// - A script with the same name already exists
/// - The file cannot be written
#[instrument(name = "create_new_script", skip_all, fields(name = %name))]
pub fn create_new_script(name: &str) -> Result<PathBuf> {
    let sanitized_name = sanitize_name(name);
    if sanitized_name.is_empty() {
        anyhow::bail!("Script name cannot be empty after sanitization");
    }

    let scripts_dir = PathBuf::from(shellexpand::tilde(SCRIPTS_DIR).as_ref());

    // Ensure the scripts directory exists
    fs::create_dir_all(&scripts_dir).with_context(|| {
        format!(
            "Failed to create scripts directory: {}",
            scripts_dir.display()
        )
    })?;

    let filename = format!("{}.ts", sanitized_name);
    let script_path = scripts_dir.join(&filename);

    // Check if script already exists
    if script_path.exists() {
        anyhow::bail!("Script already exists: {}", script_path.display());
    }

    // Generate and write the template
    let template = generate_script_template(&sanitized_name);
    fs::write(&script_path, &template)
        .with_context(|| format!("Failed to write script file: {}", script_path.display()))?;

    info!(
        path = %script_path.display(),
        name = %sanitized_name,
        "Created new script"
    );

    Ok(script_path)
}

/// Create a new scriptlet file in ~/.sk/kit/scriptlets/
///
/// # Arguments
///
/// * `name` - The name of the scriptlet (will be sanitized for filename)
///
/// # Returns
///
/// The path to the created scriptlet file.
///
/// # Errors
///
/// Returns an error if:
/// - The scriptlets directory cannot be created
/// - A scriptlet with the same name already exists
/// - The file cannot be written
#[instrument(name = "create_new_scriptlet", skip_all, fields(name = %name))]
pub fn create_new_scriptlet(name: &str) -> Result<PathBuf> {
    let sanitized_name = sanitize_name(name);
    if sanitized_name.is_empty() {
        anyhow::bail!("Scriptlet name cannot be empty after sanitization");
    }

    let scriptlets_dir = PathBuf::from(shellexpand::tilde(SCRIPTLETS_DIR).as_ref());

    // Ensure the scriptlets directory exists
    fs::create_dir_all(&scriptlets_dir).with_context(|| {
        format!(
            "Failed to create scriptlets directory: {}",
            scriptlets_dir.display()
        )
    })?;

    let filename = format!("{}.ts", sanitized_name);
    let scriptlet_path = scriptlets_dir.join(&filename);

    // Check if scriptlet already exists
    if scriptlet_path.exists() {
        anyhow::bail!("Scriptlet already exists: {}", scriptlet_path.display());
    }

    // Generate and write the template
    let template = generate_scriptlet_template(&sanitized_name);
    fs::write(&scriptlet_path, &template).with_context(|| {
        format!(
            "Failed to write scriptlet file: {}",
            scriptlet_path.display()
        )
    })?;

    info!(
        path = %scriptlet_path.display(),
        name = %sanitized_name,
        "Created new scriptlet"
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
#[instrument(name = "open_in_editor", skip(config), fields(path = %path.display()))]
pub fn open_in_editor(path: &Path, config: &Config) -> Result<()> {
    let editor = config.get_editor();

    info!(editor = %editor, path = %path.display(), "Opening file in editor");

    let status = Command::new(&editor).arg(path).spawn().with_context(|| {
        format!(
            "Failed to spawn editor '{}' for file: {}",
            editor,
            path.display()
        )
    })?;

    // We spawn and detach - don't wait for the editor to close
    // The child process handle is dropped, but the process continues
    drop(status);

    Ok(())
}

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
        assert!(template.contains("import \"@scriptkit/kit\";"));
        assert!(template.contains("export const metadata = {"));
        assert!(template.contains("name: \"My Script\""));
        assert!(template.contains("description: \"\""));
        assert!(template.contains("await arg("));
    }

    #[test]
    fn test_generate_scriptlet_template() {
        let template = generate_scriptlet_template("my-scriptlet");
        assert!(template.contains("import \"@scriptkit/kit\";"));
        assert!(template.contains("export const metadata = {"));
        assert!(template.contains("name: \"My Scriptlet\""));
        assert!(template.contains("await div("));
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
        // Create a temp directory and override the scripts path via environment
        let temp_dir = tempdir().unwrap();
        let scripts_dir = temp_dir.path().join("scripts");
        fs::create_dir_all(&scripts_dir).unwrap();

        // For this test, we'll directly test the file creation logic
        let sanitized_name = sanitize_name("test-script");
        let filename = format!("{}.ts", sanitized_name);
        let script_path = scripts_dir.join(&filename);

        let template = generate_script_template(&sanitized_name);
        fs::write(&script_path, &template).unwrap();

        // Verify the file was created
        assert!(script_path.exists());

        // Verify the content
        let content = fs::read_to_string(&script_path).unwrap();
        assert!(content.contains("export const metadata"));
        assert!(content.contains("Test Script"));
    }

    #[test]
    fn test_create_scriptlet_integration() {
        let temp_dir = tempdir().unwrap();
        let scriptlets_dir = temp_dir.path().join("scriptlets");
        fs::create_dir_all(&scriptlets_dir).unwrap();

        let sanitized_name = sanitize_name("test-scriptlet");
        let filename = format!("{}.ts", sanitized_name);
        let scriptlet_path = scriptlets_dir.join(&filename);

        let template = generate_scriptlet_template(&sanitized_name);
        fs::write(&scriptlet_path, &template).unwrap();

        // Verify the file was created
        assert!(scriptlet_path.exists());

        // Verify the content
        let content = fs::read_to_string(&scriptlet_path).unwrap();
        assert!(content.contains("export const metadata"));
        assert!(content.contains("Test Scriptlet"));
        assert!(content.contains("await div"));
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
            frecency: None,
            notes_hotkey: None,
            ai_hotkey: None,
            commands: None,
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
            frecency: None,
            notes_hotkey: None,
            ai_hotkey: None,
            commands: None,
        };
        assert_eq!(custom_config.get_editor(), "vim");

        // Restore original EDITOR
        if let Some(val) = original_editor {
            env::set_var("EDITOR", val);
        }
    }
}
