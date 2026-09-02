use anyhow::{Context, Result};
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::instrument;

use crate::scriptlets as scriptlet_parser;
use crate::setup::get_kit_path;

use super::super::types::Scriptlet;

/// Parsed source records remain local until their publication owner accepts
/// the complete catalogue. Dropping a failed/stale worker changes no capability cache.
pub struct ScriptletCatalogue {
    entries: Vec<(
        Arc<Scriptlet>,
        Option<crate::metadata_parser::TypedMetadata>,
    )>,
    source: Option<PathBuf>,
}

impl ScriptletCatalogue {
    pub fn from_scriptlets(scriptlets: Vec<Arc<Scriptlet>>) -> Self {
        Self {
            entries: scriptlets
                .into_iter()
                .map(|scriptlet| (scriptlet, None))
                .collect(),
            source: None,
        }
    }

    pub fn empty_source(path: &Path) -> Self {
        Self {
            entries: Vec::new(),
            source: Some(path.to_owned()),
        }
    }

    pub fn into_scriptlets(self) -> Vec<Arc<Scriptlet>> {
        self.entries
            .into_iter()
            .map(|(scriptlet, _)| scriptlet)
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn scriptlets(&self) -> impl Iterator<Item = &Arc<Scriptlet>> {
        self.entries.iter().map(|(scriptlet, _)| scriptlet)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn publish(self) -> Vec<Arc<Scriptlet>> {
        super::super::validation::publish_scriptlet_capability_snapshot(
            self.source.as_deref(),
            self.entries,
        )
    }
}

fn parse_catalogue_file(
    path: &Path,
    content: &str,
    plugin_id: &str,
    plugin_title: Option<&str>,
) -> Vec<(
    Arc<Scriptlet>,
    Option<crate::metadata_parser::TypedMetadata>,
)> {
    let path_str = path.to_string_lossy();
    let bundle_icon = scriptlet_parser::parse_bundle_frontmatter(content)
        .and_then(|frontmatter| frontmatter.icon);
    scriptlet_parser::parse_markdown_as_scriptlets(content, Some(&path_str))
        .into_iter()
        .map(|parsed| {
            let metadata = super::super::validation::merge_scriptlet_capability_metadata(
                parsed.typed_metadata.as_ref(),
                &parsed.metadata.extra,
            );
            let file_path = build_scriptlet_file_path(path, &parsed.command);
            let scriptlet = Scriptlet {
                name: parsed.name,
                description: parsed.metadata.description,
                code: parsed.scriptlet_content,
                tool: parsed.tool,
                shortcut: parsed.metadata.shortcut,
                keyword: parsed
                    .typed_metadata
                    .as_ref()
                    .and_then(|metadata| metadata.keyword.clone())
                    .or(parsed.metadata.keyword),
                group: (!parsed.group.is_empty()).then_some(parsed.group),
                plugin_id: plugin_id.to_owned(),
                plugin_title: plugin_title.map(str::to_owned),
                file_path: Some(file_path),
                command: Some(parsed.command),
                alias: parsed.metadata.alias,
                icon: parsed
                    .metadata
                    .extra
                    .get("icon")
                    .cloned()
                    .or_else(|| bundle_icon.clone()),
            };
            (Arc::new(scriptlet), metadata)
        })
        .collect()
}

/// Check if a path is a companion `.actions.md` file.
///
/// These files define shared actions for a parent bundle (e.g., `main.actions.md`
/// provides actions for `main.md`). They contain template variables like `{{content}}`
/// that are substituted at runtime when triggered from the parent context.
/// Loading them as standalone scriptlets would register broken commands with
/// unsubstituted templates and leak their shortcuts as global hotkeys.
fn is_actions_file(path: &Path) -> bool {
    path.to_string_lossy().ends_with(".actions.md")
}

/// Load scriptlets from markdown files using the comprehensive parser.
///
/// Consumes `discover_plugins()` so every loaded scriptlet carries explicit
/// `plugin_id` and `plugin_title` from the owning plugin manifest.
///
/// Scans `<plugin_root>/scriptlets/*.md` for each discovered plugin.
///
/// Uses `crate::scriptlets::parse_markdown_as_scriptlets` for parsing.
/// Returns Arc-wrapped scriptlets sorted by group then by name.
///
/// H1 Optimization: Returns Arc<Scriptlet> to avoid expensive clones during filter operations.
#[instrument(level = "debug", skip_all)]
pub fn load_scriptlets() -> Result<ScriptletCatalogue> {
    let index = crate::plugins::discover_plugins()?;
    let mut scriptlets = Vec::new();
    for plugin in &index.plugins {
        let scriptlets_dir = plugin.root.join("scriptlets");
        let entries = match fs::read_dir(&scriptlets_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to read scriptlets directory: {}",
                        scriptlets_dir.display()
                    )
                })
            }
        };
        for entry in entries {
            let entry = entry.with_context(|| {
                format!(
                    "Failed to enumerate scriptlets directory: {}",
                    scriptlets_dir.display()
                )
            })?;
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("md")
                || is_actions_file(&path)
                || !fs::metadata(&path)
                    .with_context(|| {
                        format!("Failed to inspect scriptlet entry: {}", path.display())
                    })?
                    .is_file()
            {
                continue;
            }
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read scriptlet source: {}", path.display()))?;
            scriptlets.extend(parse_catalogue_file(
                &path,
                &content,
                &plugin.id,
                Some(&plugin.manifest.title),
            ));
        }
    }
    scriptlets.sort_by(|(a, _), (b, _)| match (&a.group, &b.group) {
        (Some(left), Some(right)) => left.cmp(right).then_with(|| a.name.cmp(&b.name)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.name.cmp(&b.name),
    });
    Ok(ScriptletCatalogue {
        entries: scriptlets,
        source: None,
    })
}

/// Extract kit name from a kit path
/// e.g., ~/.scriptkit/plugins/my-kit/scriptlets/file.md -> Some("my-kit")
pub(crate) fn extract_kit_from_path(path: &Path, kit_root: &Path) -> Option<String> {
    let plugins_root = kit_root.join("plugins");
    let kit_prefix = format!("{}/", plugins_root.to_string_lossy());
    let path_str = path.to_string_lossy().to_string();

    if path_str.starts_with(&kit_prefix) {
        // Extract the kit name from the path
        // Path structure is: plugins/<kit-name>/scriptlets/...
        let relative = &path_str[kit_prefix.len()..];
        let parts: Vec<&str> = relative.split('/').collect();

        if !parts.is_empty() {
            return Some(parts[0].to_string());
        }
    }
    None
}

/// Build the file path with anchor for scriptlet execution
/// Format: /path/to/file.md#slug
pub(crate) fn build_scriptlet_file_path(md_path: &Path, command: &str) -> String {
    format!("{}#{}", md_path.display(), command)
}

/// Read scriptlets from a single markdown file
///
/// This function parses a single .md file and returns all scriptlets found in it.
/// Used for incremental updates when a scriptlet file changes.
///
/// H1 Optimization: Returns Arc<Scriptlet> to avoid expensive clones during filter operations.
///
/// # Arguments
/// * `path` - Path to the markdown file
///
/// # Returns
/// An immutable parsed snapshot, or an I/O/manifest error without cache mutation.
#[instrument(level = "debug", skip_all, fields(path = %path.display()))]
pub fn read_scriptlets_from_file(path: &Path) -> Result<ScriptletCatalogue> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("md")
        || is_actions_file(path)
    {
        return Ok(ScriptletCatalogue::empty_source(path));
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read scriptlet source: {}", path.display()))?;
    let (plugin_id, plugin_title) = resolve_plugin_from_path(path, &get_kit_path())?;
    Ok(ScriptletCatalogue {
        entries: parse_catalogue_file(path, &content, &plugin_id, plugin_title.as_deref()),
        source: Some(path.to_owned()),
    })
}

/// Resolve plugin identity from a file path under the plugins container.
///
/// Path structure: `<kit_path>/plugins/<plugin_id>/scriptlets/<file>.md`
/// Returns `(plugin_id, plugin_title)` — reads the manifest if possible.
fn resolve_plugin_from_path(path: &Path, kit_path: &Path) -> Result<(String, Option<String>)> {
    let container = kit_path.join("plugins");
    let container_str = format!("{}/", container.display());
    let path_str = path.to_string_lossy();

    if let Some(relative) = path_str.strip_prefix(&container_str) {
        if let Some(plugin_id) = relative.split('/').next() {
            let plugin_root = container.join(plugin_id);
            let manifest = crate::plugins::read_plugin_manifest(&plugin_root)?;
            return Ok((
                manifest.id,
                (!manifest.title.is_empty()).then_some(manifest.title),
            ));
        }
    }

    Ok((String::new(), None))
}
