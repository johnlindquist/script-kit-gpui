use std::cmp::Ordering;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};

use glob::glob;

use crate::scriptlets as scriptlet_parser;
use crate::setup::get_kit_path;

use super::super::types::Scriptlet;
use super::parse_scriptlet_section;

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

pub fn read_scriptlets() -> Vec<Arc<Scriptlet>> {
    // Default to the main plugin.
    let scriptlets_dir = crate::plugins::plugin_scriptlets_dir("main");

    // Check if directory exists
    if !scriptlets_dir.exists() {
        debug!(path = %scriptlets_dir.display(), "Scriptlets directory does not exist");
        return vec![];
    }

    let mut scriptlets = Vec::new();

    // Read all .md files in the scriptlets directory
    match fs::read_dir(&scriptlets_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();

                // Only process .md files
                if path.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }

                // Skip companion .actions.md files (they define shared actions
                // with {{content}} templates, not standalone scriptlets)
                if is_actions_file(&path) {
                    continue;
                }

                // Skip if not a file
                if !path.is_file() {
                    continue;
                }

                debug!(path = %path.display(), "Reading scriptlets file");

                match fs::read_to_string(&path) {
                    Ok(content) => {
                        // Split by ## headings
                        let mut current_section = String::new();
                        for line in content.lines() {
                            if line.starts_with("##") && !current_section.is_empty() {
                                // Parse previous section
                                if let Some(scriptlet) =
                                    parse_scriptlet_section(&current_section, Some(&path))
                                {
                                    scriptlets.push(Arc::new(scriptlet));
                                }
                                current_section = line.to_string();
                            } else {
                                if !current_section.is_empty() {
                                    current_section.push('\n');
                                }
                                current_section.push_str(line);
                            }
                        }

                        // Parse the last section
                        if !current_section.is_empty() {
                            if let Some(scriptlet) =
                                parse_scriptlet_section(&current_section, Some(&path))
                            {
                                scriptlets.push(Arc::new(scriptlet));
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            error = %e,
                            path = %path.display(),
                            "Failed to read scriptlets file"
                        );
                    }
                }
            }
        }
        Err(e) => {
            warn!(
                error = %e,
                path = %scriptlets_dir.display(),
                "Failed to read scriptlets directory"
            );
            return vec![];
        }
    }

    // Sort by name
    scriptlets.sort_by(|a, b| a.name.cmp(&b.name));

    debug!(
        count = scriptlets.len(),
        "Loaded scriptlets from all .md files"
    );
    scriptlets
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
pub fn load_scriptlets() -> Vec<Arc<Scriptlet>> {
    let index = match crate::plugins::discover_plugins() {
        Ok(index) => index,
        Err(error) => {
            warn!(error = %error, "Failed to discover plugins for scriptlet loading");
            return Vec::new();
        }
    };

    let capability_generation = super::super::validation::begin_scriptlet_capability_generation();
    let mut scriptlets = Vec::new();

    for plugin in &index.plugins {
        let scriptlets_dir = plugin.root.join("scriptlets");
        if !scriptlets_dir.exists() {
            continue;
        }

        let pattern = scriptlets_dir.join("*.md");
        let pattern_str = pattern.to_string_lossy().to_string();

        info!(
            plugin_id = %plugin.id,
            path = %scriptlets_dir.display(),
            "plugin_scriptlet_loading"
        );

        match glob(&pattern_str) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    // Skip companion .actions.md files (they define shared actions
                    // with {{content}} templates, not standalone scriptlets)
                    if is_actions_file(&entry) {
                        debug!(path = %entry.display(), "Skipping .actions.md file");
                        continue;
                    }

                    debug!(path = %entry.display(), "Parsing scriptlet file");

                    match fs::read_to_string(&entry) {
                        Ok(content) => {
                            let path_str = entry.to_string_lossy().to_string();
                            // Bundle-level default icon from YAML frontmatter (e.g. `icon: layout-grid`)
                            let bundle_icon = scriptlet_parser::parse_bundle_frontmatter(&content)
                                .and_then(|fm| fm.icon);
                            let parsed = scriptlet_parser::parse_markdown_as_scriptlets(
                                &content,
                                Some(&path_str),
                            );

                            for parsed_scriptlet in parsed {
                                let capability_metadata =
                                    super::super::validation::merge_scriptlet_capability_metadata(
                                        parsed_scriptlet.typed_metadata.as_ref(),
                                        &parsed_scriptlet.metadata.extra,
                                    );
                                let file_path =
                                    build_scriptlet_file_path(&entry, &parsed_scriptlet.command);

                                let scriptlet = Scriptlet {
                                    name: parsed_scriptlet.name,
                                    description: parsed_scriptlet.metadata.description,
                                    code: parsed_scriptlet.scriptlet_content,
                                    tool: parsed_scriptlet.tool,
                                    shortcut: parsed_scriptlet.metadata.shortcut,
                                    keyword: parsed_scriptlet
                                        .typed_metadata
                                        .as_ref()
                                        .and_then(|t| t.keyword.clone())
                                        .or(parsed_scriptlet.metadata.keyword.clone()),
                                    group: if parsed_scriptlet.group.is_empty() {
                                        None
                                    } else {
                                        Some(parsed_scriptlet.group)
                                    },
                                    plugin_id: plugin.id.clone(),
                                    plugin_title: Some(plugin.manifest.title.clone()),
                                    file_path: Some(file_path),
                                    command: Some(parsed_scriptlet.command),
                                    alias: parsed_scriptlet.metadata.alias,
                                    icon: parsed_scriptlet
                                        .metadata
                                        .extra
                                        .get("icon")
                                        .cloned()
                                        .or_else(|| bundle_icon.clone()),
                                };
                                super::super::validation::register_scriptlet_capabilities_for_generation(
                                    &scriptlet,
                                    capability_metadata.as_ref(),
                                    capability_generation,
                                );
                                scriptlets.push(Arc::new(scriptlet));
                            }
                        }
                        Err(e) => {
                            warn!(
                                error = %e,
                                path = %entry.display(),
                                "Failed to read scriptlet file"
                            );
                        }
                    }
                }
            }
            Err(e) => {
                warn!(
                    error = %e,
                    pattern = %pattern_str,
                    "Failed to glob scriptlet files"
                );
            }
        }
    }

    // Sort by group first (None last), then by name
    scriptlets.sort_by(|a, b| match (&a.group, &b.group) {
        (Some(g1), Some(g2)) => match g1.cmp(g2) {
            Ordering::Equal => a.name.cmp(&b.name),
            other => other,
        },
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.name.cmp(&b.name),
    });

    if !super::super::validation::complete_scriptlet_capability_generation(capability_generation) {
        debug!(
            generation = capability_generation,
            "Discarded stale full-load scriptlet capability staging"
        );
    }

    debug!(
        count = scriptlets.len(),
        plugins = index.plugins.len(),
        "Loaded scriptlets from all plugins via parser"
    );
    scriptlets
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
/// Vector of Arc-wrapped Scriptlet structs parsed from the file, or empty vec on error
#[instrument(level = "debug", skip_all, fields(path = %path.display()))]
pub fn read_scriptlets_from_file(path: &Path) -> Vec<Arc<Scriptlet>> {
    // Verify it's a markdown file
    if path.extension().and_then(|e| e.to_str()) != Some("md") {
        debug!(path = %path.display(), "Not a markdown file, skipping");
        return vec![];
    }

    // Skip companion .actions.md files (they define shared actions
    // with {{content}} templates, not standalone scriptlets)
    if is_actions_file(path) {
        debug!(path = %path.display(), "Skipping .actions.md file");
        return vec![];
    }

    super::super::validation::clear_scriptlet_capabilities_for_source(path);

    // Get kit path for plugin resolution
    let kit_path = get_kit_path();

    // Read file content
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                error = %e,
                path = %path.display(),
                "Failed to read scriptlet file"
            );
            return vec![];
        }
    };

    let path_str = path.to_string_lossy().to_string();
    // Bundle-level default icon from YAML frontmatter (e.g. `icon: layout-grid`)
    let bundle_icon = scriptlet_parser::parse_bundle_frontmatter(&content).and_then(|fm| fm.icon);
    let parsed = scriptlet_parser::parse_markdown_as_scriptlets(&content, Some(&path_str));

    // Resolve plugin identity from the file path.
    // Path structure: <kit_path>/plugins/<plugin_id>/scriptlets/<file>.md
    let (plugin_id, plugin_title) = resolve_plugin_from_path(path, &kit_path);

    // Convert parsed scriptlets to our Arc-wrapped Scriptlet format
    let scriptlets: Vec<Arc<Scriptlet>> = parsed
        .into_iter()
        .map(|parsed_scriptlet| {
            let capability_metadata = super::super::validation::merge_scriptlet_capability_metadata(
                parsed_scriptlet.typed_metadata.as_ref(),
                &parsed_scriptlet.metadata.extra,
            );
            let file_path = build_scriptlet_file_path(path, &parsed_scriptlet.command);

            let scriptlet = Scriptlet {
                name: parsed_scriptlet.name,
                description: parsed_scriptlet.metadata.description,
                code: parsed_scriptlet.scriptlet_content,
                tool: parsed_scriptlet.tool,
                shortcut: parsed_scriptlet.metadata.shortcut,
                keyword: parsed_scriptlet
                    .typed_metadata
                    .as_ref()
                    .and_then(|t| t.keyword.clone())
                    .or(parsed_scriptlet.metadata.keyword.clone()),
                group: if parsed_scriptlet.group.is_empty() {
                    None
                } else {
                    Some(parsed_scriptlet.group)
                },
                plugin_id: plugin_id.clone(),
                plugin_title: plugin_title.clone(),
                file_path: Some(file_path),
                command: Some(parsed_scriptlet.command),
                alias: parsed_scriptlet.metadata.alias,
                icon: parsed_scriptlet
                    .metadata
                    .extra
                    .get("icon")
                    .cloned()
                    .or_else(|| bundle_icon.clone()),
            };
            super::super::validation::register_scriptlet_capabilities(
                &scriptlet,
                capability_metadata.as_ref(),
            );
            Arc::new(scriptlet)
        })
        .collect();

    debug!(
        count = scriptlets.len(),
        path = %path.display(),
        "Parsed scriptlets from file"
    );

    scriptlets
}

/// Resolve plugin identity from a file path under the plugins container.
///
/// Path structure: `<kit_path>/plugins/<plugin_id>/scriptlets/<file>.md`
/// Returns `(plugin_id, plugin_title)` — reads the manifest if possible.
fn resolve_plugin_from_path(path: &Path, kit_path: &Path) -> (String, Option<String>) {
    let container = kit_path.join("plugins");
    let container_str = format!("{}/", container.display());
    let path_str = path.to_string_lossy();

    if let Some(relative) = path_str.strip_prefix(&container_str) {
        if let Some(plugin_id) = relative.split('/').next() {
            let plugin_root = container.join(plugin_id);
            let title = crate::plugins::read_plugin_manifest(&plugin_root)
                .ok()
                .map(|m| m.title)
                .filter(|t| !t.is_empty());
            return (plugin_id.to_string(), title);
        }
    }

    (String::new(), None)
}
