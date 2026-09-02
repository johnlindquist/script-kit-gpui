use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::info;

use super::manifest::read_plugin_manifest;
use super::types::{PluginIndex, PluginRoot};

/// Return the container directory for all plugins: `<kit_path>/plugins/`.
pub fn plugins_container_dir() -> PathBuf {
    crate::setup::plugins_path()
}

/// Return the scripts directory for a given plugin.
pub fn plugin_scripts_dir(plugin_id: &str) -> PathBuf {
    plugins_container_dir().join(plugin_id).join("scripts")
}

/// Return the scriptlets directory for a given plugin.
pub fn plugin_scriptlets_dir(plugin_id: &str) -> PathBuf {
    plugins_container_dir().join(plugin_id).join("scriptlets")
}

/// Return the skills directory for a given plugin.
pub fn plugin_skills_dir(plugin_id: &str) -> PathBuf {
    plugins_container_dir().join(plugin_id).join("skills")
}

/// Return the agents directory for a given plugin.
pub fn plugin_agents_dir(plugin_id: &str) -> PathBuf {
    plugins_container_dir().join(plugin_id).join("agents")
}

/// Discover all plugins under a given container directory.
///
/// Each child directory is treated as a plugin root. Results are sorted by
/// plugin id for deterministic ordering.
pub fn discover_plugins_in(container: &Path) -> Result<PluginIndex> {
    let mut plugins = Vec::new();

    if !container.try_exists().with_context(|| {
        format!(
            "Failed to inspect plugin container: {}",
            container.display()
        )
    })? {
        return Ok(PluginIndex { plugins });
    }

    let entries = fs::read_dir(container)
        .with_context(|| format!("Failed to read plugin container: {}", container.display()))?;

    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "Failed to enumerate plugin container: {}",
                container.display()
            )
        })?;
        let path = entry.path();
        if !fs::metadata(&path)
            .with_context(|| format!("Failed to inspect plugin root: {}", path.display()))?
            .is_dir()
        {
            continue;
        }

        let manifest = read_plugin_manifest(&path)?;
        let plugin = PluginRoot {
            id: manifest.id.clone(),
            root: path.clone(),
            manifest,
        };

        info!(
            plugin_id = %plugin.id,
            root = %plugin.root.display(),
            "plugin_discovered"
        );

        plugins.push(plugin);
    }

    plugins.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(PluginIndex { plugins })
}

/// Discover all plugins under the default container (`<kit_path>/plugins/`).
pub fn discover_plugins() -> Result<PluginIndex> {
    discover_plugins_in(&plugins_container_dir())
}
