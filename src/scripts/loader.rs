//! Script loading from file system
//!
//! This module provides functions for loading scripts from the
//! ~/.scriptkit/plugins/*/scripts/ directories.

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, instrument};

use rayon::prelude::*;

use crate::setup::get_kit_path;

use super::metadata::extract_full_metadata;
use super::scriptlet_loader::extract_kit_from_path;
use super::types::Script;
use super::validation::{validate_script_catalog, ScriptCatalogReport};

/// Reads scripts from all discovered plugin roots.
///
/// Consumes `discover_plugins()` so every loaded script carries explicit
/// `plugin_id` and `plugin_title` from the owning plugin manifest.
///
/// Returns a sorted list of Arc-wrapped Script structs for .ts and .js files.
/// Missing optional directories return an empty catalogue; failed reads return an error.
///
/// H1 Optimization: Returns Arc<Script> to avoid expensive clones during filter operations.
/// Uses rayon for parallel file scanning across plugin directories.
#[instrument(level = "debug", skip_all)]
pub fn read_scripts() -> Result<Vec<Arc<Script>>> {
    let index = crate::plugins::discover_plugins()?;
    if index.plugins.is_empty() {
        debug!("No plugins discovered — no scripts to load");
        return Ok(Vec::new());
    }

    let kit_path = get_kit_path();
    let load_started = std::time::Instant::now();

    // Read scripts from each plugin's scripts directory in parallel
    let mut scripts: Vec<Arc<Script>> = index
        .plugins
        .par_iter()
        .map(|plugin| -> Result<Vec<Arc<Script>>> {
            let scripts_dir = plugin.root.join("scripts");
            info!(plugin_id = %plugin.id, path = %scripts_dir.display(), "plugin_scripts_loading");
            let mut scripts = read_scripts_from_dir(&scripts_dir, &kit_path)?;
            for script in &mut scripts {
                let script = Arc::make_mut(script);
                script.plugin_id = plugin.id.clone();
                script.plugin_title = Some(plugin.manifest.title.clone());
                script.kit_name = Some(plugin.id.clone());
            }
            Ok(scripts)
        })
        .try_reduce(Vec::new, |mut all, scripts| {
            all.extend(scripts);
            Ok(all)
        })?;

    // Sort by name for deterministic ordering
    scripts.sort_by(|a, b| a.name.cmp(&b.name));

    crate::logging::log(
        "FILTER_PERF",
        &format!(
            "[SCRIPT_BODY_INDEX] scripts={} plugins={} parallel=true elapsed_ms={:.2}",
            scripts.len(),
            index.plugins.len(),
            load_started.elapsed().as_secs_f64() * 1000.0
        ),
    );

    debug!(
        count = scripts.len(),
        plugins = index.plugins.len(),
        elapsed_ms = load_started.elapsed().as_secs_f64() * 1000.0,
        "Loaded scripts from all plugins with parallel body indexing"
    );
    Ok(scripts)
}

/// Load scripts and run the startup-time validation pass, returning an
/// immutable [`ScriptCatalogReport`] that pairs the kept catalog with a
/// [`super::validation::ValidationReport`] for the MCP resource + menu-bar
/// badge.
#[instrument(level = "debug", skip_all)]
pub fn read_scripts_report() -> Result<Arc<ScriptCatalogReport>> {
    let scripts = read_scripts()?;
    Ok(Arc::new(validate_script_catalog(scripts)))
}

/// Read scripts from a single directory.
/// Returns a Vec of loaded scripts for parallel collection.
///
/// H1 Optimization: Creates Arc-wrapped Scripts for cheap cloning.
///
/// # Arguments
/// * `scripts_dir` - Path to the scripts directory (e.g., ~/.scriptkit/plugins/main/scripts)
/// * `kit_path` - Root kit path for extracting kit name (e.g., ~/.scriptkit)
pub(crate) fn read_scripts_from_dir(
    scripts_dir: &Path,
    kit_path: &Path,
) -> Result<Vec<Arc<Script>>> {
    let entries = match std::fs::read_dir(scripts_dir) {
        Ok(entries) => entries
            .collect::<std::io::Result<Vec<_>>>()
            .with_context(|| {
                format!(
                    "Failed to enumerate scripts directory: {}",
                    scripts_dir.display()
                )
            })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "Failed to read scripts directory: {}",
                    scripts_dir.display()
                )
            })
        }
    };
    entries
        .into_par_iter()
        .map(|entry| load_script_entry(entry, kit_path))
        .collect::<Result<Vec<_>>>()
        .map(|scripts| scripts.into_iter().flatten().collect())
}

/// Load a single script entry from a directory entry.
fn load_script_entry(entry: std::fs::DirEntry, kit_path: &Path) -> Result<Option<Arc<Script>>> {
    let path = entry.path();
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return Ok(None);
    };
    if !matches!(extension, "ts" | "js") {
        return Ok(None);
    }
    let Some(filename) = path.file_stem().and_then(|value| value.to_str()) else {
        return Ok(None);
    };
    if !std::fs::metadata(&path)
        .with_context(|| format!("Failed to inspect script entry: {}", path.display()))?
        .is_file()
    {
        return Ok(None);
    }
    let body = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read script: {}", path.display()))?;
    let (script_metadata, typed_metadata, schema) = extract_full_metadata(&body);
    let name = script_metadata.name.unwrap_or_else(|| filename.to_owned());
    let extension = extension.to_owned();
    let kit_name = extract_kit_from_path(&path, kit_path);
    Ok(Some(Arc::new(Script {
        name,
        path,
        extension,
        description: script_metadata.description,
        icon: script_metadata.icon,
        alias: script_metadata.alias,
        shortcut: script_metadata.shortcut,
        typed_metadata,
        schema,
        plugin_id: String::new(),
        plugin_title: None,
        kit_name,
        body: Some(body),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after UNIX epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("script-kit-gpui-{}-{}", label, nonce))
    }

    #[test]
    fn catalogue_read_distinguishes_optional_absence_from_failed_source_content() {
        let root = tempfile::tempdir().expect("create root");
        let source = root.path().join("scripts");
        assert!(read_scripts_from_dir(&source, root.path())
            .expect("optional absence")
            .is_empty());
        fs::create_dir(&source).expect("create source directory");
        fs::write(
            source.join("valid.ts"),
            "// Name: Retained\nconsole.log('good');",
        )
        .expect("write valid source");
        fs::write(source.join("invalid.ts"), [0xff, 0xfe]).expect("write unreadable UTF-8 source");
        assert!(
            read_scripts_from_dir(&source, root.path()).is_err(),
            "partial source snapshots cannot replace last-good catalogue"
        );
        fs::remove_file(source.join("invalid.ts")).expect("repair source");
        let scripts = read_scripts_from_dir(&source, root.path()).expect("read repaired source");
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].name, "Retained");
    }

    #[cfg(unix)]
    #[test]
    fn catalogue_read_follows_script_symlinks() {
        let root = tempfile::tempdir().expect("create source root");
        let scripts = root.path().join("scripts");
        fs::create_dir(&scripts).expect("create scripts directory");
        let target = root.path().join("target.ts");
        fs::write(&target, "// Name: Linked Command\nconsole.log('linked');")
            .expect("write target");
        std::os::unix::fs::symlink(&target, scripts.join("linked.ts")).expect("link source");
        let loaded = read_scripts_from_dir(&scripts, root.path()).expect("read linked source");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Linked Command");
    }
    #[test]
    fn full_catalogue_preserves_discovered_plugin_identity() {
        let _lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        struct RestorePath(Option<std::ffi::OsString>);
        impl Drop for RestorePath {
            fn drop(&mut self) {
                match &self.0 {
                    Some(path) => std::env::set_var(crate::setup::SK_PATH_ENV, path),
                    None => std::env::remove_var(crate::setup::SK_PATH_ENV),
                }
            }
        }
        let root = tempfile::tempdir().expect("create plugin root");
        let plugin = root.path().join("plugins/plugin-folder");
        fs::create_dir_all(plugin.join("scripts")).expect("create scripts directory");
        fs::write(
            plugin.join("package.json"),
            r#"{"name":"stable-plugin","title":"Plugin Title"}"#,
        )
        .expect("write manifest");
        fs::write(
            plugin.join("scripts/demo.ts"),
            "// Name: Demo\nconsole.log('body');",
        )
        .expect("write script");
        let _restore = RestorePath(std::env::var_os(crate::setup::SK_PATH_ENV));
        std::env::set_var(crate::setup::SK_PATH_ENV, root.path());
        let scripts = read_scripts().expect("read plugin catalogue");
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].plugin_id, "stable-plugin");
        assert_eq!(scripts[0].plugin_title.as_deref(), Some("Plugin Title"));
    }

    #[test]
    fn read_scripts_from_dir_reloads_updated_body_content() {
        let root = unique_test_dir("loader-body-reload");
        let scripts_dir = root.join("plugins").join("main").join("scripts");
        fs::create_dir_all(&scripts_dir).expect("scripts dir should be created for test");

        let script_path = scripts_dir.join("demo.ts");
        fs::write(&script_path, "console.log('alphaUniqueToken');\n")
            .expect("first write should succeed");

        let first =
            read_scripts_from_dir(&scripts_dir, &root).expect("initial catalogue should load");
        assert_eq!(first.len(), 1);
        assert_eq!(
            first[0].body.as_deref(),
            Some("console.log('alphaUniqueToken');\n")
        );

        fs::write(&script_path, "console.log('betaUniqueToken');\n")
            .expect("second write should succeed");

        let second =
            read_scripts_from_dir(&scripts_dir, &root).expect("updated catalogue should load");
        assert_eq!(second.len(), 1);
        assert_eq!(
            second[0].body.as_deref(),
            Some("console.log('betaUniqueToken');\n")
        );

        let _ = fs::remove_dir_all(&root);
    }
}
