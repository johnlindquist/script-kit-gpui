use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

use super::types::PluginManifest;

const PLUGIN_MANIFEST_FILE: &str = "plugin.json";

/// Read a plugin manifest from `plugin.json`, falling back to `package.json`
/// or the directory name when no manifest file exists.
pub fn read_plugin_manifest(plugin_root: &Path) -> Result<PluginManifest> {
    let path = plugin_root.join(PLUGIN_MANIFEST_FILE);
    if path
        .try_exists()
        .with_context(|| format!("Failed to inspect plugin manifest: {}", path.display()))?
    {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read plugin manifest: {}", path.display()))?;
        let manifest: PluginManifest = serde_json::from_str(&text)
            .with_context(|| format!("Failed to parse plugin manifest: {}", path.display()))?;
        return Ok(manifest);
    }

    synthesize_plugin_manifest(plugin_root)
}

/// Synthesize a plugin manifest from `package.json` fields or the directory name.
pub fn synthesize_plugin_manifest(plugin_root: &Path) -> Result<PluginManifest> {
    let fallback_id = plugin_root
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let package_json = plugin_root.join("package.json");
    if package_json
        .try_exists()
        .with_context(|| format!("Failed to inspect package.json: {}", package_json.display()))?
    {
        let text = fs::read_to_string(&package_json)
            .with_context(|| format!("Failed to read package.json: {}", package_json.display()))?;
        let value: Value = serde_json::from_str(&text)
            .with_context(|| format!("Failed to parse package.json: {}", package_json.display()))?;

        return Ok(PluginManifest {
            id: value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(&fallback_id)
                .to_string(),
            title: value
                .get("title")
                .and_then(Value::as_str)
                .or_else(|| value.get("name").and_then(Value::as_str))
                .unwrap_or(&fallback_id)
                .to_string(),
            description: value
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            version: value
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            author: match value.get("author") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Object(map)) => map
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                _ => String::new(),
            },
            repo_url: match value.get("repository") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Object(map)) => map
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                _ => String::new(),
            },
        });
    }

    Ok(PluginManifest {
        id: fallback_id.clone(),
        title: fallback_id,
        ..PluginManifest::default()
    })
}
