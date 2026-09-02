//! AI Preset Persistence Layer
//!
//! Manages saving and loading custom AI presets to `~/.scriptkit/ai-presets.json`.
//! Presets include a name, system prompt, and optional preferred model.

use anyhow::{bail, Context, Result};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::info;

static AI_PRESET_WRITE_LOCK: Mutex<()> = Mutex::new(());

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
    if let Some(policy) = crate::runtime_policy::owned_evaluation() {
        return policy.root().join("ai-presets.json");
    }
    let kit_dir = crate::setup::get_kit_path();
    kit_dir.join("ai-presets.json")
}

/// Load saved presets from disk.
///
/// Returns an empty vec if the file doesn't exist yet.
/// Corrupt, unsafe, or unreadable stores fail closed so later mutations cannot
/// silently replace recoverable private system prompts with an empty list.
pub fn load_presets() -> Result<Vec<SavedAiPreset>> {
    load_presets_at(&get_presets_path())
}

fn load_presets_at(path: &Path) -> Result<Vec<SavedAiPreset>> {
    if let Some(policy) = crate::runtime_policy::owned_evaluation() {
        policy.require_owned_path(path)?;
    }
    if !crate::atomic_file::inspect_private_file(path).context("Inspect private AI preset store")? {
        info!("No AI preset store found, returning empty list");
        return Ok(Vec::new());
    }

    let contents =
        crate::atomic_file::read_private_file(path).context("Read owner-only AI preset store")?;

    let presets: Vec<SavedAiPreset> =
        serde_json::from_str(&contents).context("Parse private AI preset store")?;

    info!(count = presets.len(), "Loaded private AI presets from disk");

    Ok(presets)
}

/// Save presets to disk (overwrites existing file).
///
/// Uses atomic write-to-temp-then-rename to prevent corruption on partial failure.
pub fn save_presets(presets: &[SavedAiPreset]) -> Result<()> {
    let _owner = AI_PRESET_WRITE_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("AI preset persistence lock poisoned"))?;
    save_presets_at(&get_presets_path(), presets)
}

fn save_presets_at(path: &Path, presets: &[SavedAiPreset]) -> Result<()> {
    if let Some(policy) = crate::runtime_policy::owned_evaluation() {
        policy.require_owned_path(path)?;
    }
    let json = serde_json::to_vec_pretty(presets).context("Serialize private AI presets")?;

    crate::atomic_file::write_private_atomic(path, &json)
        .context("Atomically write owner-only AI presets")?;
    crate::runtime_policy::record_completed_fixture_effect();

    info!(count = presets.len(), "Saved private AI presets to disk");

    Ok(())
}

fn mutate_presets_at<T>(
    path: &Path,
    mutation: impl FnOnce(&mut Vec<SavedAiPreset>) -> Result<(T, bool)>,
) -> Result<T> {
    let _owner = AI_PRESET_WRITE_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("AI preset persistence lock poisoned"))?;
    let mut presets = load_presets_at(path)?;
    let (result, changed) = mutation(&mut presets)?;
    if changed {
        save_presets_at(path, &presets)?;
    }
    Ok(result)
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
    create_preset_at(&get_presets_path(), name, system_prompt, preferred_model)
}

fn create_preset_at(
    path: &Path,
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

    mutate_presets_at(path, |existing| {
        let preset_fingerprint = crate::ai::reliability::redacted_fingerprint(&preset.id);
        if let Some(pos) = existing
            .iter()
            .position(|existing| existing.id == preset.id)
        {
            existing[pos] = preset.clone();
            info!(
                preset_fingerprint,
                action = "update_preset",
                "Updated existing AI preset"
            );
        } else {
            existing.push(preset.clone());
            info!(
                preset_fingerprint,
                action = "create_preset",
                "Created new AI preset"
            );
        }
        Ok((preset, true))
    })
}

/// Import presets from a JSON file, merging with existing presets.
///
/// Presets with the same ID are updated (import wins).
/// Returns the total count after merge.
pub fn import_presets_from_file(path: &Path) -> Result<usize> {
    import_presets_from_file_at(&get_presets_path(), path)
}

fn import_presets_from_file_at(store_path: &Path, import_path: &Path) -> Result<usize> {
    let contents = crate::atomic_file::read_private_file(import_path)
        .context("Read owner-only AI preset import file")?;

    let imported = validate_presets_json(&contents).context("Validate AI preset import file")?;

    let import_count = imported.len();

    mutate_presets_at(store_path, |existing| {
        for import_preset in imported {
            if let Some(pos) = existing
                .iter()
                .position(|existing| existing.id == import_preset.id)
            {
                existing[pos] = import_preset;
            } else {
                existing.push(import_preset);
            }
        }

        info!(
            imported = import_count,
            total = existing.len(),
            action = "import_presets",
            "Imported private AI presets"
        );

        Ok((existing.len(), true))
    })
}

/// Export presets to a user-chosen file path.
///
/// Uses atomic write (temp file + rename) to prevent corruption.
/// Returns the number of presets written.
pub fn export_presets_to_file(path: &Path) -> Result<usize> {
    export_presets_to_file_at(&get_presets_path(), path)
}

fn export_presets_to_file_at(store_path: &Path, export_path: &Path) -> Result<usize> {
    let presets = load_presets_at(store_path)?;
    let count = presets.len();

    let json = serde_json::to_vec_pretty(&presets).context("Serialize private AI preset export")?;

    crate::atomic_file::write_private_atomic(export_path, &json)
        .context("Atomically write owner-only AI preset export")?;

    info!(
        count = count,
        action = "export_presets",
        "Exported private AI presets to an owner-only file"
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
    delete_preset_at(&get_presets_path(), id)
}

fn delete_preset_at(path: &Path, id: &str) -> Result<bool> {
    mutate_presets_at(path, |existing| {
        let original_len = existing.len();
        existing.retain(|preset| preset.id != id);
        let deleted = existing.len() < original_len;
        if deleted {
            info!(
                preset_fingerprint = %crate::ai::reliability::redacted_fingerprint(id),
                action = "delete_preset",
                "Deleted private AI preset"
            );
        }
        Ok((deleted, deleted))
    })
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

    fn private_preset(id: &str, system_prompt: &str) -> SavedAiPreset {
        SavedAiPreset {
            id: id.to_string(),
            name: id.to_string(),
            description: "Private preset fixture".to_string(),
            system_prompt: system_prompt.to_string(),
            icon: "star".to_string(),
            preferred_model: Some("private-custom-model".to_string()),
        }
    }

    #[test]
    #[cfg(unix)]
    fn private_ai_presets_store_system_prompts_owner_only_and_repair_legacy_permissions() {
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("ai-presets.json");
        let presets = vec![private_preset("private", "private customer system prompt")];

        save_presets_at(&path, &presets).expect("save private system prompt");
        assert_eq!(
            std::fs::metadata(&path)
                .expect("preset metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("make legacy preset readable");
        assert_eq!(
            load_presets_at(&path).expect("repair private preset before exposing its prompt"),
            presets
        );
        assert_eq!(
            std::fs::metadata(&path)
                .expect("repaired preset metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    #[cfg(unix)]
    fn private_ai_presets_reject_store_symlinks_without_reading_or_replacing_foreign_prompts() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("ai-presets.json");
        let foreign = temp.path().join("foreign.json");
        let original = serde_json::to_vec(&vec![private_preset(
            "foreign",
            "private system prompt belonging to another owner",
        )])
        .expect("serialize foreign private prompt");
        std::fs::write(&foreign, &original).expect("seed foreign private prompts");
        symlink(&foreign, &path).expect("plant preset store symlink");

        assert!(load_presets_at(&path).is_err());
        assert!(save_presets_at(&path, &[private_preset("new", "new private prompt")]).is_err());
        assert!(create_preset_at(&path, "New", "new private prompt", None).is_err());
        assert!(delete_preset_at(&path, "foreign").is_err());
        assert_eq!(
            std::fs::read(&foreign).expect("foreign private prompts remain untouched"),
            original
        );
    }

    #[test]
    fn private_ai_presets_malformed_store_is_never_erased_by_create_delete_or_import() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("ai-presets.json");
        let import_path = temp.path().join("import.json");
        let original = br#"[{"systemPrompt":"recoverable private system prompt""#;
        std::fs::write(&path, original).expect("seed malformed recoverable presets");
        save_presets_at(
            &import_path,
            &[private_preset("imported", "private imported prompt")],
        )
        .expect("seed private preset import");

        assert!(create_preset_at(&path, "New", "new private prompt", None).is_err());
        assert!(delete_preset_at(&path, "private").is_err());
        assert!(import_presets_from_file_at(&path, &import_path).is_err());
        assert_eq!(
            std::fs::read(&path).expect("malformed prompt bytes remain available for recovery"),
            original
        );
    }

    #[test]
    #[cfg(unix)]
    fn private_ai_preset_exports_are_owner_only_and_never_replace_symlink_targets() {
        use std::os::unix::fs::{symlink, PermissionsExt as _};

        let temp = tempfile::tempdir().expect("temp dir");
        let store_path = temp.path().join("ai-presets.json");
        let export_path = temp.path().join("private-export.json");
        let hostile_export = temp.path().join("hostile-export.json");
        let foreign = temp.path().join("foreign.json");
        let presets = vec![private_preset("private", "private exported system prompt")];
        save_presets_at(&store_path, &presets).expect("seed private prompt store");

        assert_eq!(
            export_presets_to_file_at(&store_path, &export_path)
                .expect("export owner-only private prompts"),
            1
        );
        assert_eq!(
            std::fs::metadata(&export_path)
                .expect("private export metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            load_presets_at(&export_path).expect("read private exported prompts"),
            presets
        );

        std::fs::write(&foreign, "another owner's private content")
            .expect("seed foreign export target");
        symlink(&foreign, &hostile_export).expect("plant export symlink");
        assert!(export_presets_to_file_at(&store_path, &hostile_export).is_err());
        assert_eq!(
            std::fs::read(&foreign).expect("foreign export target remains untouched"),
            b"another owner's private content"
        );
    }

    #[test]
    #[cfg(unix)]
    fn private_ai_preset_imports_repair_legacy_permissions_and_reject_symlinks() {
        use std::os::unix::fs::{symlink, PermissionsExt as _};

        let temp = tempfile::tempdir().expect("temp dir");
        let store_path = temp.path().join("ai-presets.json");
        let import_path = temp.path().join("private-import.json");
        let hostile_import = temp.path().join("hostile-import.json");
        let presets = vec![private_preset("imported", "private imported system prompt")];
        std::fs::write(
            &import_path,
            serde_json::to_vec(&presets).expect("serialize imported private prompt"),
        )
        .expect("seed legacy private import");
        std::fs::set_permissions(&import_path, std::fs::Permissions::from_mode(0o644))
            .expect("seed permissive legacy import");

        assert_eq!(
            import_presets_from_file_at(&store_path, &import_path)
                .expect("repair and import private prompts"),
            1
        );
        assert_eq!(
            std::fs::metadata(&import_path)
                .expect("repaired private import metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            load_presets_at(&store_path).expect("load imported private prompt"),
            presets
        );

        symlink(&import_path, &hostile_import).expect("plant import symlink");
        assert!(import_presets_from_file_at(&store_path, &hostile_import).is_err());
        assert_eq!(
            load_presets_at(&store_path).expect("private prompt import remains unchanged"),
            presets
        );
    }

    #[test]
    fn private_ai_preset_concurrent_creators_preserve_every_private_prompt() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("ai-presets.json");

        std::thread::scope(|scope| {
            for index in 0..8 {
                let preset_path = &path;
                scope.spawn(move || {
                    create_preset_at(
                        preset_path,
                        &format!("Private owner {index}"),
                        &format!("Private system prompt {index}"),
                        None,
                    )
                    .expect("serialize private owner read/merge/write");
                });
            }
        });

        let presets = load_presets_at(&path).expect("load every serialized private prompt");
        assert_eq!(presets.len(), 8);
        for index in 0..8 {
            assert!(presets.iter().any(|preset| {
                preset.id == format!("private-owner-{index}")
                    && preset.system_prompt == format!("Private system prompt {index}")
            }));
        }
    }

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
