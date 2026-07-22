//! AI Preset Persistence Layer
//!
//! Manages saving and loading custom AI presets to `~/.scriptkit/ai-presets.json`.
//! Presets include a name, system prompt, and optional preferred model.

use anyhow::{bail, Context, Result};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn};

/// A user-created AI preset stored on disk.
///
/// Uses camelCase for JSON serialization per protocol conventions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SavedAiPreset {
    /// Unique identifier (kebab-case slug derived from name)
    pub id: String,
    /// Display name
    pub name: String,
    /// Description shown in lists
    pub description: String,
    /// System prompt to prepend to chats
    pub system_prompt: String,
    /// Icon identifier (maps to LocalIconName variants)
    #[serde(default = "default_icon")]
    pub icon: String,
    /// Optional preferred model ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_model: Option<String>,
}

fn default_icon() -> String {
    "star".to_string()
}

/// Get the path to the AI presets file (`~/.scriptkit/ai-presets.json`).
pub fn get_presets_path() -> PathBuf {
    let kit_dir = crate::setup::get_kit_path();
    kit_dir.join("ai-presets.json")
}

/// Load saved presets from disk.
///
/// Returns an empty vec if the file doesn't exist yet.
/// Handles corrupt files gracefully by logging and returning empty.
pub fn load_presets() -> Result<Vec<SavedAiPreset>> {
    let path = get_presets_path();

    if !path.exists() {
        info!(path = %path.display(), "No presets file found, returning empty list");
        return Ok(Vec::new());
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read presets file: {}", path.display()))?;

    let presets: Vec<SavedAiPreset> = serde_json::from_str(&contents).with_context(|| {
        format!(
            "Failed to parse presets file: {} — file may be corrupt",
            path.display()
        )
    })?;

    info!(
        count = presets.len(),
        path = %path.display(),
        "Loaded AI presets from disk"
    );

    Ok(presets)
}

/// Save presets to disk (overwrites existing file).
///
/// Uses atomic write-to-temp-then-rename to prevent corruption on partial failure.
pub fn save_presets(presets: &[SavedAiPreset]) -> Result<()> {
    let path = get_presets_path();

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create presets directory: {}", parent.display()))?;
    }

    let json =
        serde_json::to_string_pretty(presets).context("Failed to serialize presets to JSON")?;

    // Atomic write via a UNIQUE temp file + rename (see src/atomic_file.rs): a
    // fixed temp path let concurrent savers corrupt the file.
    crate::atomic_file::write_atomic(&path, json.as_bytes()).with_context(|| {
        format!(
            "Failed to atomically write presets file: {}",
            path.display()
        )
    })?;

    info!(
        count = presets.len(),
        path = %path.display(),
        "Saved AI presets to disk"
    );

    Ok(())
}

/// Create a new preset and save it to disk.
///
/// Validates that the name is non-empty and generates a unique ID.
/// Returns the created preset.
pub fn create_preset(
    name: &str,
    system_prompt: &str,
    preferred_model: Option<&str>,
) -> Result<SavedAiPreset> {
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        bail!("Preset name cannot be empty");
    }

    let id = slug_from_name(trimmed_name);

    let preset = SavedAiPreset {
        id,
        name: trimmed_name.to_string(),
        description: truncate_for_description(system_prompt),
        system_prompt: system_prompt.to_string(),
        icon: default_icon(),
        preferred_model: preferred_model.map(String::from),
    };

    let mut existing = load_presets().unwrap_or_default();

    // Deduplicate by ID (update if exists)
    if let Some(pos) = existing.iter().position(|p| p.id == preset.id) {
        existing[pos] = preset.clone();
        info!(id = %preset.id, action = "update_preset", "Updated existing preset");
    } else {
        existing.push(preset.clone());
        info!(id = %preset.id, action = "create_preset", "Created new preset");
    }

    save_presets(&existing)?;
    Ok(preset)
}

/// Import presets from a JSON file, merging with existing presets.
///
/// Presets with the same ID are updated (import wins).
/// Returns the total count after merge.
pub fn import_presets_from_file(path: &std::path::Path) -> Result<usize> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read import file: {}", path.display()))?;

    let imported = validate_presets_json(&contents)
        .with_context(|| format!("Invalid import file: {}", path.display()))?;

    let mut existing = load_presets().unwrap_or_default();
    let import_count = imported.len();

    for import_preset in imported {
        if let Some(pos) = existing.iter().position(|p| p.id == import_preset.id) {
            existing[pos] = import_preset;
        } else {
            existing.push(import_preset);
        }
    }

    save_presets(&existing)?;

    info!(
        imported = import_count,
        total = existing.len(),
        action = "import_presets",
        "Imported AI presets"
    );

    Ok(existing.len())
}

/// Export presets to a user-chosen file path.
///
/// Uses atomic write (temp file + rename) to prevent corruption.
/// Returns the number of presets written.
pub fn export_presets_to_file(path: &std::path::Path) -> Result<usize> {
    let presets = load_presets()?;
    let count = presets.len();

    let json =
        serde_json::to_string_pretty(&presets).context("Failed to serialize presets to JSON")?;

    // Atomic write via a UNIQUE temp file + rename (see src/atomic_file.rs).
    crate::atomic_file::write_atomic(path, json.as_bytes())
        .with_context(|| format!("Failed to atomically write export file: {}", path.display()))?;

    info!(
        count = count,
        path = %path.display(),
        action = "export_presets",
        "Exported AI presets to file"
    );

    Ok(count)
}

/// Validate that a JSON string contains a valid preset array.
///
/// Returns the parsed presets on success or an error describing what's wrong.
pub fn validate_presets_json(contents: &str) -> Result<Vec<SavedAiPreset>> {
    let presets: Vec<SavedAiPreset> = serde_json::from_str(contents)
        .context("Invalid JSON: expected an array of AI preset objects")?;

    for preset in &presets {
        if preset.name.trim().is_empty() {
            bail!("Preset with id '{}' has an empty name", preset.id);
        }
        if preset.id.trim().is_empty() {
            bail!("Found a preset with an empty id");
        }
    }

    Ok(presets)
}

/// Delete a preset by ID.
pub fn delete_preset(id: &str) -> Result<bool> {
    let mut existing = load_presets().unwrap_or_default();
    let original_len = existing.len();
    existing.retain(|p| p.id != id);

    if existing.len() < original_len {
        save_presets(&existing)?;
        info!(id = %id, action = "delete_preset", "Deleted AI preset");
        Ok(true)
    } else {
        Ok(false)
    }
}

/// A saved AI preset resolved for application to the current Agent Chat surface.
///
/// This is the handoff payload `AgentChatView::apply_preset_by_id` consumes:
/// the preset's system prompt is staged in the composer and the preferred
/// model is selected through the thread's model-picker mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentChatPresetPlan {
    /// System prompt staged into the Agent Chat composer.
    pub system_prompt: String,
    /// Preferred model to select on the current thread, when the preset has one.
    pub preferred_model: Option<String>,
}

/// Resolve a saved preset by ID for the current Agent Chat preset handoff.
///
/// Returns the preset-specific failure message on load errors or unknown IDs
/// so the deferred handoff can surface `Failed to apply AI preset: {error}`.
pub fn resolve_agent_chat_preset(preset_id: &str) -> Result<AgentChatPresetPlan, String> {
    let presets = load_presets().map_err(|error| format!("Failed to load AI presets: {error}"))?;
    let preset = presets
        .into_iter()
        .find(|preset| preset.id == preset_id)
        .ok_or_else(|| format!("Unknown AI preset: {preset_id}"))?;
    Ok(AgentChatPresetPlan {
        system_prompt: preset.system_prompt,
        preferred_model: preset.preferred_model,
    })
}

/// Generate a kebab-case slug from a preset name.
fn slug_from_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .join("-")
}

/// Truncate system prompt to a short description.
fn truncate_for_description(system_prompt: &str) -> String {
    let first_line = system_prompt.lines().next().unwrap_or(system_prompt);
    let char_count = first_line.chars().count();
    if char_count > 80 {
        let truncated: String = first_line.chars().take(77).collect();
        format!("{}…", truncated)
    } else {
        first_line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slug_from_name_converts_spaces_and_special_chars() {
        assert_eq!(slug_from_name("My Cool Preset"), "my-cool-preset");
        assert_eq!(slug_from_name("  spaces  "), "spaces");
        assert_eq!(slug_from_name("Code & Debug!"), "code-debug");
    }

    #[test]
    fn test_truncate_for_description_handles_short_strings() {
        assert_eq!(truncate_for_description("Short prompt"), "Short prompt");
    }

    #[test]
    fn test_truncate_for_description_truncates_long_strings() {
        let long = "a".repeat(100);
        let desc = truncate_for_description(&long);
        assert!(desc.chars().count() <= 80);
        assert!(desc.ends_with('…'));
    }

    #[test]
    fn test_truncate_for_description_handles_multibyte_chars() {
        // 100 emoji characters — each is 4 bytes in UTF-8
        let long: String = "🔥".repeat(100);
        let desc = truncate_for_description(&long);
        assert!(desc.chars().count() <= 80);
        assert!(desc.ends_with('…'));
        // Must not panic on multi-byte boundary
    }

    #[test]
    fn test_truncate_for_description_uses_first_line() {
        let multi = "First line\nSecond line\nThird line";
        assert_eq!(truncate_for_description(multi), "First line");
    }

    #[test]
    fn test_saved_preset_serde_roundtrip() {
        let preset = SavedAiPreset {
            id: "test-preset".to_string(),
            name: "Test Preset".to_string(),
            description: "A test preset".to_string(),
            system_prompt: "You are a test assistant.".to_string(),
            icon: "star".to_string(),
            preferred_model: Some("claude-3-5-sonnet".to_string()),
        };

        let json = serde_json::to_string(&preset).expect("serialize");
        let parsed: SavedAiPreset = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(preset, parsed);

        // Verify camelCase in JSON
        assert!(json.contains("systemPrompt"));
        assert!(json.contains("preferredModel"));
        assert!(!json.contains("system_prompt"));
    }

    #[test]
    fn test_saved_preset_deserialize_missing_optional_fields() {
        let json = r#"{"id":"x","name":"X","description":"d","systemPrompt":"sp"}"#;
        let preset: SavedAiPreset = serde_json::from_str(json).expect("deserialize");
        assert_eq!(preset.icon, "star"); // default
        assert!(preset.preferred_model.is_none());
    }

    #[test]
    fn test_create_preset_rejects_empty_name() {
        // This test doesn't touch disk since it fails before load
        let result = create_preset("", "prompt", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_create_preset_rejects_whitespace_name() {
        let result = create_preset("   ", "prompt", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_presets_json_accepts_valid_array() {
        let json = r#"[{"id":"a","name":"A","description":"d","systemPrompt":"sp"}]"#;
        let presets = validate_presets_json(json).expect("should parse");
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].id, "a");
    }

    #[test]
    fn test_validate_presets_json_rejects_empty_name() {
        let json = r#"[{"id":"a","name":"  ","description":"d","systemPrompt":"sp"}]"#;
        let result = validate_presets_json(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty name"));
    }

    #[test]
    fn test_validate_presets_json_rejects_empty_id() {
        let json = r#"[{"id":"","name":"A","description":"d","systemPrompt":"sp"}]"#;
        let result = validate_presets_json(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty id"));
    }

    #[test]
    fn test_validate_presets_json_rejects_invalid_json() {
        let result = validate_presets_json("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn test_export_import_roundtrip() {
        let dir = std::env::temp_dir().join("scriptkit-test-presets-roundtrip");
        let _ = std::fs::create_dir_all(&dir);
        let export_path = dir.join("test-export.json");

        // Write some presets directly so we don't depend on the real presets file
        let presets = vec![
            SavedAiPreset {
                id: "roundtrip-a".to_string(),
                name: "Roundtrip A".to_string(),
                description: "Test A".to_string(),
                system_prompt: "You are test A.".to_string(),
                icon: "star".to_string(),
                preferred_model: Some("gpt-4".to_string()),
            },
            SavedAiPreset {
                id: "roundtrip-b".to_string(),
                name: "Roundtrip B".to_string(),
                description: "Test B".to_string(),
                system_prompt: "You are test B.".to_string(),
                icon: "bolt".to_string(),
                preferred_model: None,
            },
        ];

        let json = serde_json::to_string_pretty(&presets).expect("serialize");
        std::fs::write(&export_path, &json).expect("write");

        // Re-read and validate
        let contents = std::fs::read_to_string(&export_path).expect("read");
        let imported = validate_presets_json(&contents).expect("validate");

        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0], presets[0]);
        assert_eq!(imported[1], presets[1]);

        // Clean up
        let _ = std::fs::remove_file(&export_path);
        let _ = std::fs::remove_dir(&dir);
    }
}
