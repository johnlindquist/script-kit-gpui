//! Configuration loading from file system
//!
//! Handles loading and parsing the config.ts file using bun.

use anyhow::Context;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{LazyLock, Mutex};
use std::time::UNIX_EPOCH;
use tempfile::Builder;
use tracing::{info, instrument, warn};

use super::types::{Config, ScriptKitUserPreferences};

fn config_ts_path() -> PathBuf {
    crate::setup::get_kit_path().join("kit").join("config.ts")
}

// ---------------------------------------------------------------------------
// Config cache — skip Bun on warm launches when config.ts hasn't changed
// ---------------------------------------------------------------------------

const CONFIG_JSON_CACHE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ConfigSourceFingerprint {
    len: u64,
    modified_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ConfigJsonCache {
    schema_version: u32,
    config_path: String,
    len: u64,
    modified_ms: u64,
    json: String,
}

fn config_json_cache_path() -> PathBuf {
    crate::setup::get_kit_path()
        .join("cache")
        .join("config-loader-cache.v1.json")
}

fn fingerprint_config_file(path: &Path) -> Option<ConfigSourceFingerprint> {
    let metadata = fs::metadata(path).ok()?;
    let modified_ms: u64 = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis()
        .try_into()
        .ok()?;
    Some(ConfigSourceFingerprint {
        len: metadata.len(),
        modified_ms,
    })
}

fn try_load_cached_config(
    config_path: &Path,
    fingerprint: ConfigSourceFingerprint,
    correlation_id: &str,
) -> Option<Config> {
    let cache_path = config_json_cache_path();
    let cache_text = match fs::read_to_string(&cache_path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            info!(
                correlation_id = %correlation_id,
                path = %cache_path.display(),
                "CONFIG_CACHE_MISS reason=cache_file_missing"
            );
            return None;
        }
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                path = %cache_path.display(),
                error = %error,
                "CONFIG_CACHE_MISS reason=cache_read_failed"
            );
            return None;
        }
    };

    let cache = match serde_json::from_str::<ConfigJsonCache>(&cache_text) {
        Ok(cache) => cache,
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                path = %cache_path.display(),
                error = %error,
                "CONFIG_CACHE_MISS reason=cache_json_invalid"
            );
            return None;
        }
    };

    let expected_path = config_path.to_string_lossy();
    if cache.schema_version != CONFIG_JSON_CACHE_SCHEMA_VERSION
        || cache.config_path != expected_path
        || cache.len != fingerprint.len
        || cache.modified_ms != fingerprint.modified_ms
    {
        info!(
            correlation_id = %correlation_id,
            path = %cache_path.display(),
            source = %config_path.display(),
            cache_schema_version = cache.schema_version,
            cache_len = cache.len,
            cache_modified_ms = cache.modified_ms,
            source_len = fingerprint.len,
            source_modified_ms = fingerprint.modified_ms,
            "CONFIG_CACHE_MISS reason=fingerprint_mismatch"
        );
        return None;
    }

    let parsed_json = match serde_json::from_str::<Value>(cache.json.trim()) {
        Ok(value) => value,
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                path = %cache_path.display(),
                source = %config_path.display(),
                error = %error,
                "CONFIG_CACHE_MISS reason=cached_payload_invalid_json"
            );
            return None;
        }
    };

    let config = match serde_json::from_value::<Config>(parsed_json.clone()) {
        Ok(config) => config,
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                path = %cache_path.display(),
                source = %config_path.display(),
                error = %error,
                "Cached config parse failed; recovering valid fields"
            );
            recover_config_fields(parsed_json, correlation_id)
        }
    };

    info!(
        correlation_id = %correlation_id,
        path = %cache_path.display(),
        source = %config_path.display(),
        len = fingerprint.len,
        modified_ms = fingerprint.modified_ms,
        "CONFIG_CACHE_HIT"
    );
    Some(config)
}

fn write_config_cache(
    config_path: &Path,
    fingerprint: ConfigSourceFingerprint,
    json_str: &str,
    correlation_id: &str,
) {
    let cache_path = config_json_cache_path();
    let Some(parent) = cache_path.parent() else {
        return;
    };
    if let Err(error) = fs::create_dir_all(parent) {
        warn!(
            correlation_id = %correlation_id,
            path = %parent.display(),
            error = %error,
            "CONFIG_CACHE_WRITE_SKIPPED reason=create_parent_failed"
        );
        return;
    }

    let cache = ConfigJsonCache {
        schema_version: CONFIG_JSON_CACHE_SCHEMA_VERSION,
        config_path: config_path.to_string_lossy().into_owned(),
        len: fingerprint.len,
        modified_ms: fingerprint.modified_ms,
        json: json_str.trim().to_string(),
    };

    let encoded = match serde_json::to_vec_pretty(&cache) {
        Ok(encoded) => encoded,
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                path = %cache_path.display(),
                error = %error,
                "CONFIG_CACHE_WRITE_SKIPPED reason=serialize_failed"
            );
            return;
        }
    };

    if let Err(error) = fs::write(&cache_path, encoded) {
        warn!(
            correlation_id = %correlation_id,
            path = %cache_path.display(),
            error = %error,
            "CONFIG_CACHE_WRITE_SKIPPED reason=write_failed"
        );
        return;
    }

    info!(
        correlation_id = %correlation_id,
        path = %cache_path.display(),
        source = %config_path.display(),
        len = fingerprint.len,
        modified_ms = fingerprint.modified_ms,
        bytes = json_str.len(),
        "CONFIG_CACHE_WRITE"
    );
}

fn settings_json_path() -> PathBuf {
    crate::setup::get_kit_path()
        .join("kit")
        .join("settings.json")
}

static CONFIG_PREFERENCE_WRITE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Build a Bun command that imports a module by URL and writes its default export as JSON to stdout.
///
/// Uses `pathToFileURL` so paths with special characters are handled safely.
fn build_bun_extract_command(module_path: &Path) -> Result<Command, serde_json::Error> {
    let module_path_json = serde_json::to_string(&module_path.to_string_lossy().into_owned())?;
    let script = format!(
        r#"const {{ pathToFileURL }} = await import("node:url");
const moduleUrl = pathToFileURL({module_path_json}).href;
const loaded = await import(moduleUrl);
process.stdout.write(JSON.stringify(loaded?.default ?? {{}}));"#
    );
    let mut command = Command::new("bun");
    command.arg("--eval").arg(script);
    Ok(command)
}

fn parse_optional_field<T>(
    object: &Map<String, Value>,
    field: &'static str,
    correlation_id: &str,
) -> Option<T>
where
    T: DeserializeOwned,
{
    let raw = object.get(field)?;

    match serde_json::from_value::<Option<T>>(raw.clone()) {
        Ok(parsed) => parsed,
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                field,
                error = %error,
                "Config field failed validation; using default for this field"
            );
            None
        }
    }
}

fn parse_required_field<T>(
    object: &Map<String, Value>,
    field: &'static str,
    fallback: T,
    correlation_id: &str,
) -> T
where
    T: DeserializeOwned,
{
    let Some(raw) = object.get(field) else {
        return fallback;
    };

    match serde_json::from_value::<T>(raw.clone()) {
        Ok(parsed) => parsed,
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                field,
                error = %error,
                "Required config field failed validation; using default"
            );
            fallback
        }
    }
}

fn recover_config_fields(value: Value, correlation_id: &str) -> Config {
    let Some(object) = value.as_object() else {
        warn!(
            correlation_id = %correlation_id,
            "Config root is not an object; using defaults"
        );
        return Config::default();
    };

    let defaults = Config::default();
    Config {
        hotkey: parse_required_field(object, "hotkey", defaults.hotkey.clone(), correlation_id),
        bun_path: parse_optional_field(object, "bun_path", correlation_id),
        editor: parse_optional_field(object, "editor", correlation_id),
        padding: parse_optional_field(object, "padding", correlation_id),
        editor_font_size: parse_optional_field(object, "editorFontSize", correlation_id),
        terminal_font_size: parse_optional_field(object, "terminalFontSize", correlation_id),
        ui_scale: parse_optional_field(object, "uiScale", correlation_id),
        built_ins: parse_optional_field(object, "builtIns", correlation_id),
        process_limits: parse_optional_field(object, "processLimits", correlation_id),
        clipboard_history_max_text_length: parse_optional_field(
            object,
            "clipboardHistoryMaxTextLength",
            correlation_id,
        ),
        suggested: parse_optional_field(object, "suggested", correlation_id),
        notes_hotkey: parse_optional_field(object, "notesHotkey", correlation_id),
        ai_hotkey: parse_optional_field(object, "aiHotkey", correlation_id),
        ai_hotkey_enabled: parse_optional_field(object, "aiHotkeyEnabled", correlation_id),
        logs_hotkey: parse_optional_field(object, "logsHotkey", correlation_id),
        logs_hotkey_enabled: parse_optional_field(object, "logsHotkeyEnabled", correlation_id),
        dictation_hotkey: parse_optional_field(object, "dictationHotkey", correlation_id),
        dictation_hotkey_enabled: parse_optional_field(
            object,
            "dictationHotkeyEnabled",
            correlation_id,
        ),
        watcher: parse_optional_field(object, "watcher", correlation_id),
        layout: parse_optional_field(object, "layout", correlation_id),
        theme: parse_optional_field(object, "theme", correlation_id),
        dictation: parse_optional_field(object, "dictation", correlation_id),
        ai: parse_optional_field(object, "ai", correlation_id),
        window_management: parse_optional_field(object, "windowManagement", correlation_id),
        commands: parse_optional_field(object, "commands", correlation_id),
        claude_code: parse_optional_field(object, "claudeCode", correlation_id),
        mcp: parse_optional_field(object, "mcp", correlation_id),
    }
}

fn parse_config_json(json_str: &str, correlation_id: &str) -> Config {
    let parsed_json = match serde_json::from_str::<Value>(json_str.trim()) {
        Ok(value) => value,
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                error = %error,
                "Config output was not valid JSON; using defaults"
            );
            return Config::default();
        }
    };

    match serde_json::from_value::<Config>(parsed_json.clone()) {
        Ok(config) => config,
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                error = %error,
                "Config parse failed; recovering valid fields"
            );
            recover_config_fields(parsed_json, correlation_id)
        }
    }
}

fn recover_user_preferences_fields(value: Value, correlation_id: &str) -> ScriptKitUserPreferences {
    let Some(object) = value.as_object() else {
        warn!(
            correlation_id = %correlation_id,
            "Legacy preference root is not an object; using defaults"
        );
        return ScriptKitUserPreferences::default();
    };

    let defaults = ScriptKitUserPreferences::default();
    ScriptKitUserPreferences {
        layout: parse_required_field(object, "layout", defaults.layout, correlation_id),
        theme: parse_required_field(object, "theme", defaults.theme, correlation_id),
        dictation: parse_required_field(object, "dictation", defaults.dictation, correlation_id),
        ai: parse_required_field(object, "ai", defaults.ai, correlation_id),
        window_management: parse_required_field(
            object,
            "windowManagement",
            defaults.window_management,
            correlation_id,
        ),
    }
}

fn parse_user_preferences_json(json_str: &str, correlation_id: &str) -> ScriptKitUserPreferences {
    let parsed_json = match serde_json::from_str::<Value>(json_str.trim()) {
        Ok(value) => value,
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                error = %error,
                "Legacy preference JSON was invalid; using defaults"
            );
            return ScriptKitUserPreferences::default();
        }
    };

    match serde_json::from_value::<ScriptKitUserPreferences>(parsed_json.clone()) {
        Ok(preferences) => preferences,
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                error = %error,
                "Legacy preference parse failed; recovering valid fields"
            );
            recover_user_preferences_fields(parsed_json, correlation_id)
        }
    }
}

fn preferences_from_config(config: &Config) -> ScriptKitUserPreferences {
    ScriptKitUserPreferences {
        layout: config.layout.clone().unwrap_or_default(),
        theme: config.get_theme_selection(),
        dictation: config.get_dictation_preferences(),
        ai: config.get_ai_preferences(),
        window_management: config.get_window_management_preferences(),
    }
}

fn overlay_legacy_preferences_if_missing(
    mut prefs: ScriptKitUserPreferences,
    legacy: ScriptKitUserPreferences,
    config: &Config,
) -> ScriptKitUserPreferences {
    if config.layout.is_none() {
        prefs.layout = legacy.layout;
    }
    if config.theme.is_none() {
        prefs.theme = legacy.theme;
    }
    if config.dictation.is_none() {
        prefs.dictation = legacy.dictation;
    }
    if config.ai.is_none() {
        prefs.ai = legacy.ai;
    }
    if config.window_management.is_none() {
        prefs.window_management = legacy.window_management;
    }
    prefs
}

fn maybe_load_legacy_user_preferences(correlation_id: &str) -> Option<ScriptKitUserPreferences> {
    let settings_path = settings_json_path();
    if !settings_path.exists() {
        return None;
    }

    match std::fs::read_to_string(&settings_path) {
        Ok(contents) => Some(parse_user_preferences_json(&contents, correlation_id)),
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                path = %settings_path.display(),
                error = %error,
                "Failed to read legacy settings.json"
            );
            None
        }
    }
}

/// Load runtime preference groups from `config.ts`, with a legacy `settings.json`
/// fallback for users who have not migrated yet.
pub fn load_user_preferences() -> ScriptKitUserPreferences {
    let correlation_id = format!("config_prefs_load:{}", uuid::Uuid::new_v4());
    let config = load_config();
    let preferences = preferences_from_config(&config);
    if let Some(legacy) = maybe_load_legacy_user_preferences(&correlation_id) {
        return overlay_legacy_preferences_if_missing(preferences, legacy, &config);
    }
    preferences
}

fn cleanup_legacy_settings_file_if_safe(correlation_id: &str) {
    let path = settings_json_path();
    if !path.exists() {
        return;
    }

    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                path = %path.display(),
                error = %error,
                "Failed to inspect legacy settings.json for cleanup"
            );
            return;
        }
    };

    let parsed = match serde_json::from_str::<Value>(&contents) {
        Ok(parsed) => parsed,
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                path = %path.display(),
                error = %error,
                "Legacy settings.json is invalid JSON; leaving it in place"
            );
            return;
        }
    };

    let Some(object) = parsed.as_object() else {
        return;
    };

    let known_keys = ["layout", "theme", "dictation", "ai", "windowManagement"];
    if object.keys().all(|key| known_keys.contains(&key.as_str())) {
        if let Err(error) = fs::remove_file(&path) {
            warn!(
                correlation_id = %correlation_id,
                path = %path.display(),
                error = %error,
                "Failed to remove legacy settings.json after config migration"
            );
        } else {
            info!(
                correlation_id = %correlation_id,
                path = %path.display(),
                "Removed legacy settings.json after migrating preferences to config.ts"
            );
        }
    }
}

fn write_preference_group<T: Serialize + PartialEq>(
    config_path: &Path,
    property_name: &str,
    current_exists: bool,
    value: &T,
    default_value: &T,
) -> anyhow::Result<()> {
    if value == default_value && !current_exists {
        return Ok(());
    }

    let property = if value == default_value {
        super::editor::ConfigProperty::new(property_name, "undefined")
    } else {
        super::editor::ConfigProperty::new(
            property_name,
            serde_json::to_string(value)
                .with_context(|| format!("failed to serialize {}", property_name))?,
        )
    };

    super::editor::write_config_safely(config_path, &property, None)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(())
}

/// Persist runtime preference groups back into `config.ts`.
pub fn save_user_preferences(prefs: &ScriptKitUserPreferences) -> anyhow::Result<()> {
    let _write_guard = CONFIG_PREFERENCE_WRITE_LOCK
        .lock()
        .expect("config preference write lock poisoned");
    let config_path = config_ts_path();
    let current = load_config();

    write_preference_group(
        &config_path,
        "layout",
        current.layout.is_some(),
        &prefs.layout,
        &super::types::LayoutConfig::default(),
    )?;
    write_preference_group(
        &config_path,
        "theme",
        current.theme.is_some(),
        &prefs.theme,
        &super::types::ThemeSelectionPreferences::default(),
    )?;
    write_preference_group(
        &config_path,
        "dictation",
        current.dictation.is_some(),
        &prefs.dictation,
        &super::types::DictationPreferences::default(),
    )?;
    write_preference_group(
        &config_path,
        "ai",
        current.ai.is_some(),
        &prefs.ai,
        &super::types::AiPreferences::default(),
    )?;
    write_preference_group(
        &config_path,
        "windowManagement",
        current.window_management.is_some(),
        &prefs.window_management,
        &super::types::WindowManagementPreferences::default(),
    )?;

    let correlation_id = format!("config_prefs_save:{}", uuid::Uuid::new_v4());
    cleanup_legacy_settings_file_if_safe(&correlation_id);
    Ok(())
}

/// Load configuration from `<SK_PATH>/kit/config.ts` (or `~/.scriptkit/kit/config.ts`)
///
/// Cache path: checks a fingerprinted JSON cache keyed by config path, size, and mtime.
/// Fast path: imports config.ts directly with a single Bun process.
/// Fallback: transpiles to a temp `.mjs` file then extracts (two Bun processes).
///
/// Returns Config::default() if any step fails.
#[instrument(name = "load_config")]
pub fn load_config() -> Config {
    let correlation_id = format!("config_load:{}", uuid::Uuid::new_v4());
    let config_path = config_ts_path();

    if !config_path.exists() {
        info!(
            correlation_id = %correlation_id,
            path = %config_path.display(),
            "Config file not found, using defaults"
        );
        return Config::default();
    }

    // Fingerprint the source file for cache lookup and later cache write.
    let fingerprint = fingerprint_config_file(&config_path);

    // Cache path: try to serve from a previous Bun evaluation.
    if let Some(fp) = fingerprint {
        if let Some(config) = try_load_cached_config(&config_path, fp, &correlation_id) {
            return config;
        }
    } else {
        warn!(
            correlation_id = %correlation_id,
            path = %config_path.display(),
            "Failed to fingerprint config.ts; skipping config cache"
        );
    }

    // Fast path: import config.ts directly with one Bun process.
    let direct_start = std::time::Instant::now();
    let mut direct_command = match build_bun_extract_command(&config_path) {
        Ok(command) => command,
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                error = %error,
                "Failed to prepare direct bun config import, using defaults"
            );
            return Config::default();
        }
    };

    match direct_command.output() {
        Ok(output) if output.status.success() => {
            let json_str = String::from_utf8_lossy(&output.stdout);
            let config = parse_config_json(&json_str, &correlation_id);
            if let Some(fp) = fingerprint {
                write_config_cache(&config_path, fp, &json_str, &correlation_id);
            }
            info!(
                correlation_id = %correlation_id,
                mode = "direct_import",
                cache = "miss",
                elapsed_ms = direct_start.elapsed().as_secs_f64() * 1000.0,
                path = %config_path.display(),
                "Loaded config"
            );
            return config;
        }
        Ok(output) => {
            warn!(
                correlation_id = %correlation_id,
                mode = "direct_import",
                elapsed_ms = direct_start.elapsed().as_secs_f64() * 1000.0,
                stderr = %String::from_utf8_lossy(&output.stderr),
                "Direct bun config import failed, falling back to transpile path"
            );
        }
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                mode = "direct_import",
                elapsed_ms = direct_start.elapsed().as_secs_f64() * 1000.0,
                error = %error,
                "Direct bun config import failed to start, falling back to transpile path"
            );
        }
    }

    // Fallback: preserve the existing two-step transpile behavior for edge cases.
    let fallback_start = std::time::Instant::now();
    let tmp_js = match Builder::new().suffix(".mjs").tempfile() {
        Ok(file) => file,
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                error = %error,
                "Failed to create temporary file for config transpile fallback, using defaults"
            );
            return Config::default();
        }
    };

    let build_output = Command::new("bun")
        .arg("build")
        .arg("--target=bun")
        .arg(config_path.to_string_lossy().to_string())
        .arg(format!("--outfile={}", tmp_js.path().display()))
        .output();

    match build_output {
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                mode = "transpile_fallback",
                error = %error,
                "Failed to transpile config with bun, using defaults"
            );
            return Config::default();
        }
        Ok(output) if !output.status.success() => {
            warn!(
                correlation_id = %correlation_id,
                mode = "transpile_fallback",
                stderr = %String::from_utf8_lossy(&output.stderr),
                "bun build failed, using defaults"
            );
            return Config::default();
        }
        Ok(_) => {}
    }

    let mut fallback_command = match build_bun_extract_command(tmp_js.path()) {
        Ok(command) => command,
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                mode = "transpile_fallback",
                error = %error,
                "Failed to prepare fallback bun config extraction, using defaults"
            );
            return Config::default();
        }
    };

    match fallback_command.output() {
        Err(error) => {
            warn!(
                correlation_id = %correlation_id,
                mode = "transpile_fallback",
                error = %error,
                "Failed to execute bun fallback extraction, using defaults"
            );
            Config::default()
        }
        Ok(output) if !output.status.success() => {
            warn!(
                correlation_id = %correlation_id,
                mode = "transpile_fallback",
                stderr = %String::from_utf8_lossy(&output.stderr),
                "bun fallback extraction failed, using defaults"
            );
            Config::default()
        }
        Ok(output) => {
            let json_str = String::from_utf8_lossy(&output.stdout);
            let config = parse_config_json(&json_str, &correlation_id);
            if let Some(fp) = fingerprint {
                write_config_cache(&config_path, fp, &json_str, &correlation_id);
            }
            info!(
                correlation_id = %correlation_id,
                mode = "transpile_fallback",
                cache = "miss",
                elapsed_ms = fallback_start.elapsed().as_secs_f64() * 1000.0,
                path = %config_path.display(),
                "Loaded config"
            );
            config
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_bun_extract_command, load_user_preferences, parse_config_json,
        parse_user_preferences_json, save_user_preferences,
    };
    use crate::config::HotkeyConfig;
    use std::fs;
    use std::path::Path;

    /// Code audit test: Verify all config.ts path references use the authoritative path.
    ///
    /// AUTHORITATIVE PATH: ~/.scriptkit/kit/config.ts
    ///
    /// This test ensures no code accidentally references the wrong path like:
    /// - ~/.scriptkit/config.ts (missing /kit/)
    ///
    /// The correct pattern is always: ~/.scriptkit/kit/config.ts
    #[test]
    fn test_config_path_consistency() {
        // Files to audit for config path references
        // Note: We exclude this file (loader.rs) since it contains the test pattern strings
        let files_to_check = [
            "src/main.rs",
            "src/app_impl/mod.rs",
            "src/config/mod.rs",
            "src/config/types.rs",
            "src/watcher.rs",
            "src/setup.rs",
            "src/lib.rs",
        ];

        // The wrong pattern we're looking for (split to avoid self-detection)
        let wrong_pattern = format!(".scriptkit{}config.ts", "/");

        let mut violations = Vec::new();

        for file in files_to_check {
            let content = match fs::read_to_string(file) {
                Ok(c) => c,
                Err(_) => continue, // Skip files that don't exist
            };

            for (line_num, line) in content.lines().enumerate() {
                // Skip comments (documentation)
                if line.trim_start().starts_with("//") || line.trim_start().starts_with("///") {
                    continue;
                }

                // Detect WRONG pattern: .scriptkit/config.ts (missing /kit/)
                // This catches: "~/.scriptkit/config.ts" or ".scriptkit/config.ts"
                // But NOT: "~/.scriptkit/kit/config.ts" (the correct path)
                if line.contains(&wrong_pattern) {
                    violations.push(format!(
                        "{}:{}: {}\n  Found: {} (missing /kit/)\n  Expected: .scriptkit/kit/config.ts",
                        file,
                        line_num + 1,
                        line.trim(),
                        wrong_pattern
                    ));
                }
            }
        }

        if !violations.is_empty() {
            panic!(
                "Found {} inconsistent config path reference(s):\n\n{}\n\n\
                AUTHORITATIVE PATH: ~/.scriptkit/kit/config.ts",
                violations.len(),
                violations.join("\n\n")
            );
        }
    }

    #[test]
    fn test_config_loader_preserves_valid_fields_when_one_field_invalid() {
        let json = r#"{
            "hotkey": { "modifiers": ["meta"], "key": "Semicolon" },
            "editor": "nvim",
            "watcher": { "debounceMs": "bad-type", "stormThreshold": 321 }
        }"#;

        let config = parse_config_json(json, "test-correlation-id");

        // Valid fields remain intact
        assert_eq!(config.editor.as_deref(), Some("nvim"));
        assert_eq!(config.hotkey.key, "Semicolon");
        // Invalid watcher object should fall back to watcher defaults.
        let watcher = config.get_watcher();
        assert_eq!(
            watcher.storm_threshold,
            super::super::defaults::DEFAULT_WATCHER_STORM_THRESHOLD
        );
        assert_eq!(
            watcher.debounce_ms,
            super::super::defaults::DEFAULT_WATCHER_DEBOUNCE_MS
        );
    }

    #[test]
    fn test_config_loader_uses_default_hotkey_when_hotkey_missing_or_invalid() {
        let missing_hotkey = r#"{
            "editor": "vim"
        }"#;
        let missing_config = parse_config_json(missing_hotkey, "test-correlation-id");
        assert_eq!(
            missing_config.hotkey,
            HotkeyConfig {
                modifiers: vec!["meta".to_string()],
                key: "Semicolon".to_string(),
            }
        );

        let invalid_hotkey = r#"{
            "hotkey": { "modifiers": "meta", "key": 7 },
            "editor": "hx"
        }"#;
        let invalid_config = parse_config_json(invalid_hotkey, "test-correlation-id");
        assert_eq!(
            invalid_config.hotkey,
            HotkeyConfig {
                modifiers: vec!["meta".to_string()],
                key: "Semicolon".to_string(),
            }
        );
        assert_eq!(invalid_config.editor.as_deref(), Some("hx"));
    }

    #[test]
    fn test_config_loader_parses_runtime_preference_groups() {
        let json = r#"{
            "layout": { "standardHeight": 640, "maxHeight": 920 },
            "theme": { "presetId": "catppuccin-mocha" },
            "dictation": { "selectedDeviceId": "usb-mic" },
            "ai": { "selectedModelId": "gpt-5.4", "selectedAcpAgentId": "codex-acp" },
            "windowManagement": { "snapMode": "precision" }
        }"#;

        let config = parse_config_json(json, "test-correlation-id");

        assert_eq!(config.layout.as_ref().unwrap().standard_height, 640.0);
        assert_eq!(config.layout.as_ref().unwrap().max_height, 920.0);
        assert_eq!(
            config.theme.as_ref().unwrap().preset_id.as_deref(),
            Some("catppuccin-mocha")
        );
        assert_eq!(
            config
                .dictation
                .as_ref()
                .unwrap()
                .selected_device_id
                .as_deref(),
            Some("usb-mic")
        );
        assert_eq!(
            config.ai.as_ref().unwrap().selected_model_id.as_deref(),
            Some("gpt-5.4")
        );
        assert_eq!(
            config.ai.as_ref().unwrap().selected_acp_agent_id.as_deref(),
            Some("codex-acp")
        );
        assert_eq!(
            config.window_management.as_ref().unwrap().snap_mode,
            Some(crate::window_control::SnapMode::Precision)
        );
    }

    #[test]
    fn test_legacy_settings_loader_parses_layout_and_theme_preset() {
        let json = r#"{
            "layout": { "standardHeight": 640, "maxHeight": 920 },
            "theme": { "presetId": "catppuccin-mocha" }
        }"#;

        let preferences = parse_user_preferences_json(json, "test-correlation-id");

        assert_eq!(preferences.layout.standard_height, 640.0);
        assert_eq!(preferences.layout.max_height, 920.0);
        assert_eq!(
            preferences.theme.preset_id.as_deref(),
            Some("catppuccin-mocha")
        );
    }

    #[test]
    fn test_legacy_settings_loader_recovers_from_invalid_layout_field() {
        let json = r#"{
            "layout": { "standardHeight": "bad", "maxHeight": 920 },
            "theme": { "presetId": "nord" }
        }"#;

        let preferences = parse_user_preferences_json(json, "test-correlation-id");

        assert_eq!(
            preferences.layout.standard_height,
            super::super::defaults::DEFAULT_LAYOUT_STANDARD_HEIGHT
        );
        assert_eq!(
            preferences.layout.max_height,
            super::super::defaults::DEFAULT_LAYOUT_MAX_HEIGHT
        );
        assert_eq!(preferences.theme.preset_id.as_deref(), Some("nord"));
    }

    #[test]
    fn test_load_and_save_user_preferences_migrate_known_legacy_settings_into_config() {
        let _lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|error| error.into_inner());

        let temp_dir = tempfile::tempdir().expect("create temp Script Kit dir");
        let kit_dir = temp_dir.path().join("kit");
        fs::create_dir_all(&kit_dir).expect("create kit dir");

        let settings_path = kit_dir.join("settings.json");
        fs::write(
            &settings_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "layout": { "standardHeight": 640, "maxHeight": 920 },
                "theme": { "presetId": "nord" },
                "dictation": { "selectedDeviceId": "usb-mic" },
                "ai": {
                    "selectedModelId": "gpt-5.4",
                    "selectedAcpAgentId": "codex-acp"
                },
                "windowManagement": { "snapMode": "precision" }
            }))
            .expect("serialize legacy settings"),
        )
        .expect("write legacy settings");

        let previous_sk_path = std::env::var(crate::setup::SK_PATH_ENV).ok();
        std::env::set_var(crate::setup::SK_PATH_ENV, temp_dir.path());

        let loaded = load_user_preferences();
        assert_eq!(loaded.layout.standard_height, 640.0);
        assert_eq!(loaded.layout.max_height, 920.0);
        assert_eq!(loaded.theme.preset_id.as_deref(), Some("nord"));
        assert_eq!(
            loaded.dictation.selected_device_id.as_deref(),
            Some("usb-mic")
        );
        assert_eq!(loaded.ai.selected_model_id.as_deref(), Some("gpt-5.4"));
        assert_eq!(
            loaded.ai.selected_acp_agent_id.as_deref(),
            Some("codex-acp")
        );
        assert_eq!(
            loaded.window_management.snap_mode,
            Some(crate::window_control::SnapMode::Precision)
        );

        save_user_preferences(&loaded).expect("migrate loaded preferences into config.ts");

        let config_contents =
            fs::read_to_string(kit_dir.join("config.ts")).expect("read migrated config.ts");
        assert!(config_contents.contains("layout"));
        assert!(config_contents.contains("theme"));
        assert!(config_contents.contains("dictation"));
        assert!(config_contents.contains("ai"));
        assert!(config_contents.contains("windowManagement"));
        assert!(config_contents.contains("selectedAcpAgentId"));
        assert!(config_contents.contains("snapMode"));
        assert!(
            !settings_path.exists(),
            "known legacy settings file should be removed after migration"
        );

        match previous_sk_path {
            Some(path) => std::env::set_var(crate::setup::SK_PATH_ENV, path),
            None => std::env::remove_var(crate::setup::SK_PATH_ENV),
        }
    }

    #[test]
    fn test_build_bun_extract_command_uses_eval_with_json_escaped_path() {
        let module_path = Path::new("/tmp/config-with-'quote'.js");
        let command = build_bun_extract_command(module_path).expect("should build command");

        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        assert_eq!(args[0], "--eval");
        // The eval script should contain the JSON-escaped path, not a raw interpolation
        assert!(
            args[1].contains("pathToFileURL"),
            "script should use pathToFileURL for safe path handling"
        );
        assert!(
            args[1].contains(r#"/tmp/config-with-'quote'.js"#),
            "script should contain the JSON-escaped module path"
        );
    }
}
