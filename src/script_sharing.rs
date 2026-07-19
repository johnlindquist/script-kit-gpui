use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use arboard::Clipboard;
use base64::Engine as _;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::clipboard_history::change_detection::ClipboardChangeDetector;

pub const SHARE_URI_PREFIX: &str = "scriptkit-share://v1/";
const RECENT_EXPORT_TTL: Duration = Duration::from_secs(5);
const RECENT_PROMPT_TTL: Duration = Duration::from_secs(10);
const WATCHER_POLL_INTERVAL: Duration = Duration::from_millis(350);

static RECENTLY_EXPORTED_SHARE: std::sync::LazyLock<Mutex<Option<RecentShareRecord>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));
static RECENTLY_PROMPTED_SHARE: std::sync::LazyLock<Mutex<Option<RecentShareRecord>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

#[derive(Debug, Clone, Copy)]
struct RecentShareRecord {
    hash: u64,
    recorded_at: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptShareBundle {
    pub version: u8,
    pub kind: ShareKind,
    pub title: String,
    pub plugin: crate::plugins::PluginManifest,
    pub entry_path: String,
    #[serde(default)]
    pub files: Vec<ShareFile>,
}

impl ScriptShareBundle {
    pub fn prompt_title(&self) -> String {
        format!("Install Shared {}?", self.kind.display_name())
    }

    pub fn prompt_body(&self) -> String {
        let plugin_label = if self.plugin.title.trim().is_empty() {
            self.plugin.id.trim()
        } else {
            self.plugin.title.trim()
        };
        format!(
            "Script Kit found a shared {} on your clipboard.\n\nTitle: {}\nPlugin: {}\nFiles: {}\n\nOnly install it if you trust the sender.",
            self.kind.display_name().to_lowercase(),
            self.title.trim(),
            plugin_label,
            self.files.len(),
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ShareKind {
    Script,
    Scriptlet,
    Skill,
    Agent,
}

impl ShareKind {
    pub fn display_name(self) -> &'static str {
        match self {
            ShareKind::Script => "Script",
            ShareKind::Scriptlet => "Snippet",
            ShareKind::Skill => "Skill",
            ShareKind::Agent => "Agent",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShareFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ShareInstallOutcome {
    pub plugin_id: String,
    pub plugin_root: PathBuf,
    pub entry_path: PathBuf,
    pub files_written: usize,
}

#[derive(Debug, Clone)]
pub struct ClipboardShareImport {
    pub uri: String,
    pub bundle: ScriptShareBundle,
}

pub fn is_shareable_result(result: &crate::scripts::SearchResult) -> bool {
    matches!(
        result,
        crate::scripts::SearchResult::Script(_)
            | crate::scripts::SearchResult::Scriptlet(_)
            | crate::scripts::SearchResult::Skill(_)
            | crate::scripts::SearchResult::Agent(_)
    )
}

pub fn encode_share_bundle(bundle: &ScriptShareBundle) -> Result<String> {
    let json = serde_json::to_vec(bundle).context("Failed to serialize share bundle")?;
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json);
    Ok(format!("{SHARE_URI_PREFIX}{payload}"))
}

pub fn decode_share_text(text: &str) -> Result<Option<ScriptShareBundle>> {
    let Some(uri) = extract_share_uri(text) else {
        return Ok(None);
    };
    decode_share_uri(&uri).map(Some)
}

pub fn mark_recently_exported_share(uri: &str) {
    remember_recent_share(&RECENTLY_EXPORTED_SHARE, uri);
}

pub fn should_ignore_recently_exported_share(uri: &str) -> bool {
    has_recent_share(&RECENTLY_EXPORTED_SHARE, uri, RECENT_EXPORT_TTL)
}

pub fn bundle_from_search_result(
    result: &crate::scripts::SearchResult,
) -> Result<ScriptShareBundle> {
    match result {
        crate::scripts::SearchResult::Script(sm) => {
            let path = &sm.script.path;
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .ok_or_else(|| anyhow::anyhow!("Script path is missing a file name"))?;
            let plugin = resolve_plugin_manifest(
                Some(sm.script.plugin_id.as_str()),
                sm.script.plugin_title.as_deref(),
                sm.script.kit_name.as_deref(),
                &sm.script.name,
            );
            build_single_file_bundle(
                ShareKind::Script,
                sm.script.name.clone(),
                plugin,
                PathBuf::from("scripts").join(file_name),
                path,
            )
        }
        crate::scripts::SearchResult::Scriptlet(sm) => {
            let raw_path = sm
                .scriptlet
                .file_path
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("Snippet is missing a source markdown path"))?;
            let markdown_path = raw_path.split('#').next().unwrap_or(raw_path);
            let file_name = Path::new(markdown_path)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .ok_or_else(|| anyhow::anyhow!("Snippet path is missing a file name"))?;
            let plugin = resolve_plugin_manifest(
                Some(sm.scriptlet.plugin_id.as_str()),
                sm.scriptlet.plugin_title.as_deref(),
                sm.scriptlet.group.as_deref(),
                &sm.scriptlet.name,
            );
            build_single_file_bundle(
                ShareKind::Scriptlet,
                sm.scriptlet.name.clone(),
                plugin,
                PathBuf::from("scriptlets").join(file_name),
                Path::new(markdown_path),
            )
        }
        crate::scripts::SearchResult::Skill(sm) => {
            let skill_doc = &sm.skill.path;
            let skill_root = skill_doc
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Skill path is missing its root directory"))?;
            let plugin = resolve_plugin_manifest(
                Some(sm.skill.plugin_id.as_str()),
                Some(sm.skill.plugin_title.as_str()),
                Some(sm.skill.plugin_id.as_str()),
                &sm.skill.title,
            );
            let skill_id = skill_root
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| slugify_text(&sm.skill.title, "shared-skill"));
            let relative_skill_root = PathBuf::from("skills").join(&skill_id);
            let files = collect_directory_files(skill_root, &relative_skill_root)?;
            if files.is_empty() {
                anyhow::bail!("Skill folder does not contain any files to share");
            }
            Ok(ScriptShareBundle {
                version: 1,
                kind: ShareKind::Skill,
                title: sm.skill.title.clone(),
                plugin,
                entry_path: relative_skill_root
                    .join("SKILL.md")
                    .to_string_lossy()
                    .to_string(),
                files,
            })
        }
        crate::scripts::SearchResult::Agent(am) => {
            let path = &am.agent.path;
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .ok_or_else(|| anyhow::anyhow!("Agent path is missing a file name"))?;
            let plugin = resolve_plugin_manifest(
                am.agent.kit.as_deref(),
                am.agent.kit.as_deref(),
                am.agent.kit.as_deref(),
                &am.agent.name,
            );
            build_single_file_bundle(
                ShareKind::Agent,
                am.agent.name.clone(),
                plugin,
                PathBuf::from("agents").join(file_name),
                path,
            )
        }
        _ => anyhow::bail!("This item only supports launcher deeplinks, not clipboard sharing"),
    }
}

pub fn install_share_bundle(bundle: &ScriptShareBundle) -> Result<ShareInstallOutcome> {
    if bundle.version != 1 {
        anyhow::bail!("Unsupported share bundle version: {}", bundle.version);
    }
    if bundle.files.is_empty() {
        anyhow::bail!("Shared item does not contain any files");
    }

    let requested_plugin_id = normalize_plugin_id(&bundle.plugin.id, &bundle.title);
    let plugin_root = unique_plugin_root(&requested_plugin_id);
    fs::create_dir_all(&plugin_root)
        .with_context(|| format!("Failed to create plugin root {}", plugin_root.display()))?;

    for directory in ["scripts", "scriptlets", "agents", "skills"] {
        fs::create_dir_all(plugin_root.join(directory)).with_context(|| {
            format!(
                "Failed to create plugin subdirectory {}",
                plugin_root.join(directory).display()
            )
        })?;
    }

    let final_plugin_id = plugin_root
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| requested_plugin_id.clone());
    let plugin_manifest = crate::plugins::PluginManifest {
        id: final_plugin_id.clone(),
        title: first_non_empty(&bundle.plugin.title, &bundle.title),
        description: bundle.plugin.description.trim().to_string(),
        version: bundle.plugin.version.trim().to_string(),
        author: bundle.plugin.author.trim().to_string(),
        repo_url: bundle.plugin.repo_url.trim().to_string(),
    };
    let manifest_json = serde_json::to_string_pretty(&plugin_manifest)
        .context("Failed to serialize plugin manifest")?;
    crate::atomic_file::write_atomic(&plugin_root.join("plugin.json"), manifest_json.as_bytes())
        .with_context(|| {
            format!(
                "Failed to write plugin manifest {}",
                plugin_root.join("plugin.json").display()
            )
        })?;

    for file in &bundle.files {
        let relative_path = validate_share_relative_path(&file.path)?;
        let destination = plugin_root.join(&relative_path);
        let parent = destination
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Share file path is missing a parent directory"))?;
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
        fs::write(&destination, &file.content)
            .with_context(|| format!("Failed to write {}", destination.display()))?;
    }

    let entry_path = plugin_root.join(validate_share_relative_path(&bundle.entry_path)?);
    Ok(ShareInstallOutcome {
        plugin_id: final_plugin_id,
        plugin_root,
        entry_path,
        files_written: bundle.files.len(),
    })
}

pub fn spawn_clipboard_share_watcher() -> async_channel::Receiver<ClipboardShareImport> {
    let (tx, rx) = async_channel::bounded(8);
    std::thread::spawn(move || {
        let mut clipboard: Option<Clipboard> = None;
        let mut detector = ClipboardChangeDetector::new();

        loop {
            std::thread::sleep(WATCHER_POLL_INTERVAL);

            let changed = detector.has_changed().unwrap_or(true);
            if !changed {
                continue;
            }

            if clipboard.is_none() {
                clipboard = Clipboard::new().ok();
                if clipboard.is_none() {
                    continue;
                }
            }

            let Some(clipboard_ref) = clipboard.as_mut() else {
                continue;
            };
            let text = match clipboard_ref.get_text() {
                Ok(text) => text,
                Err(error) => {
                    tracing::debug!(?error, "clipboard_share_watcher_read_failed");
                    clipboard = None;
                    continue;
                }
            };

            let Some(uri) = extract_share_uri(&text) else {
                continue;
            };
            if should_ignore_recently_exported_share(&uri)
                || has_recent_share(&RECENTLY_PROMPTED_SHARE, &uri, RECENT_PROMPT_TTL)
            {
                continue;
            }

            let bundle = match decode_share_uri(&uri) {
                Ok(bundle) => bundle,
                Err(error) => {
                    tracing::debug!(?error, "clipboard_share_watcher_decode_failed");
                    continue;
                }
            };

            remember_recent_share(&RECENTLY_PROMPTED_SHARE, &uri);
            if tx
                .send_blocking(ClipboardShareImport { uri, bundle })
                .is_err()
            {
                break;
            }
        }
    });
    rx
}

fn decode_share_uri(uri: &str) -> Result<ScriptShareBundle> {
    let payload = uri
        .strip_prefix(SHARE_URI_PREFIX)
        .ok_or_else(|| anyhow::anyhow!("Clipboard text is not a Script Kit share link"))?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .context("Failed to decode share bundle payload")?;
    let bundle: ScriptShareBundle =
        serde_json::from_slice(&decoded).context("Failed to parse share bundle JSON")?;
    Ok(bundle)
}

fn build_single_file_bundle(
    kind: ShareKind,
    title: String,
    plugin: crate::plugins::PluginManifest,
    relative_path: PathBuf,
    absolute_path: &Path,
) -> Result<ScriptShareBundle> {
    let content = fs::read_to_string(absolute_path)
        .with_context(|| format!("Failed to read {}", absolute_path.display()))?;
    let entry_path = relative_path.to_string_lossy().to_string();
    Ok(ScriptShareBundle {
        version: 1,
        kind,
        title,
        plugin,
        entry_path: entry_path.clone(),
        files: vec![ShareFile {
            path: entry_path,
            content,
        }],
    })
}

fn collect_directory_files(root: &Path, relative_root: &Path) -> Result<Vec<ShareFile>> {
    let mut files = Vec::new();
    collect_directory_files_recursive(root, root, relative_root, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn collect_directory_files_recursive(
    root: &Path,
    current_dir: &Path,
    relative_root: &Path,
    files: &mut Vec<ShareFile>,
) -> Result<()> {
    let entries = fs::read_dir(current_dir)
        .with_context(|| format!("Failed to read {}", current_dir.display()))?;

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            collect_directory_files_recursive(root, &path, relative_root, files)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let relative_path = path
            .strip_prefix(root)
            .with_context(|| format!("Failed to relativize {}", path.display()))?;
        let share_path = relative_root.join(relative_path);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        files.push(ShareFile {
            path: share_path.to_string_lossy().to_string(),
            content,
        });
    }

    Ok(())
}

fn resolve_plugin_manifest(
    plugin_id: Option<&str>,
    plugin_title: Option<&str>,
    fallback_plugin_id: Option<&str>,
    bundle_title: &str,
) -> crate::plugins::PluginManifest {
    let requested_plugin_id = first_non_empty(
        plugin_id.unwrap_or_default(),
        fallback_plugin_id.unwrap_or_default(),
    );
    if !requested_plugin_id.is_empty() {
        if let Ok(index) = crate::plugins::discover_plugins() {
            if let Some(plugin_root) = index
                .plugins
                .into_iter()
                .find(|plugin| plugin.id == requested_plugin_id)
            {
                return plugin_root.manifest;
            }
        }
    }

    let normalized_id = normalize_plugin_id(&requested_plugin_id, bundle_title);
    crate::plugins::PluginManifest {
        id: normalized_id,
        title: first_non_empty(plugin_title.unwrap_or_default(), bundle_title),
        ..crate::plugins::PluginManifest::default()
    }
}

fn normalize_plugin_id(plugin_id: &str, fallback_title: &str) -> String {
    let candidate = if plugin_id.trim().is_empty() {
        fallback_title
    } else {
        plugin_id
    };
    slugify_text(candidate, "shared")
}

fn unique_plugin_root(plugin_id: &str) -> PathBuf {
    let container = crate::plugins::plugins_container_dir();
    let direct_root = container.join(plugin_id);
    if !direct_root.exists() {
        return direct_root;
    }

    for suffix in 2..1000 {
        let candidate = container.join(format!("{plugin_id}-shared-{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    container.join(format!("{plugin_id}-shared-{}", share_hash(plugin_id)))
}

fn validate_share_relative_path(path: &str) -> Result<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Shared file path cannot be empty");
    }

    let parsed = Path::new(trimmed);
    if parsed.is_absolute() {
        anyhow::bail!("Shared file paths must be relative");
    }

    let mut normalized = PathBuf::new();
    for component in parsed.components() {
        match component {
            Component::Normal(segment) => normalized.push(segment),
            Component::CurDir => {}
            Component::ParentDir => anyhow::bail!("Shared file paths cannot contain '..'"),
            Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("Shared file paths cannot escape the plugin root")
            }
        }
    }

    let top_level = normalized
        .components()
        .next()
        .and_then(|component| match component {
            Component::Normal(segment) => Some(segment.to_string_lossy().to_string()),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("Shared file path is missing a top-level directory"))?;
    if !matches!(
        top_level.as_str(),
        "scripts" | "scriptlets" | "agents" | "skills"
    ) {
        anyhow::bail!(
            "Shared file path must live under scripts/, scriptlets/, agents/, or skills/"
        );
    }

    Ok(normalized)
}

fn extract_share_uri(text: &str) -> Option<String> {
    for token in text.split_whitespace() {
        let candidate = token.trim_matches(|ch: char| {
            matches!(
                ch,
                '`' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
            )
        });
        if candidate.starts_with(SHARE_URI_PREFIX) {
            return Some(candidate.to_string());
        }
    }

    let trimmed = text.trim();
    trimmed
        .starts_with(SHARE_URI_PREFIX)
        .then(|| trimmed.to_string())
}

fn remember_recent_share(
    slot: &std::sync::LazyLock<Mutex<Option<RecentShareRecord>>>,
    value: &str,
) {
    *slot.lock() = Some(RecentShareRecord {
        hash: share_hash(value),
        recorded_at: Instant::now(),
    });
}

fn has_recent_share(
    slot: &std::sync::LazyLock<Mutex<Option<RecentShareRecord>>>,
    value: &str,
    ttl: Duration,
) -> bool {
    let record = *slot.lock();
    match record {
        Some(record) => record.hash == share_hash(value) && record.recorded_at.elapsed() <= ttl,
        None => false,
    }
}

fn share_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn slugify_text(input: &str, fallback: &str) -> String {
    let mut slug = String::new();
    let mut previous_was_separator = false;

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            slug.push('-');
            previous_was_separator = true;
        }
    }

    let normalized = slug.trim_matches('-').to_string();
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized
    }
}

fn first_non_empty(primary: &str, fallback: &str) -> String {
    let trimmed = primary.trim();
    if trimmed.is_empty() {
        fallback.trim().to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_bundle_round_trips_through_uri_encoding() {
        let bundle = ScriptShareBundle {
            version: 1,
            kind: ShareKind::Skill,
            title: "Example Skill".to_string(),
            plugin: crate::plugins::PluginManifest {
                id: "main".to_string(),
                title: "Main".to_string(),
                ..crate::plugins::PluginManifest::default()
            },
            entry_path: "skills/example-skill/SKILL.md".to_string(),
            files: vec![ShareFile {
                path: "skills/example-skill/SKILL.md".to_string(),
                content: "# Example Skill".to_string(),
            }],
        };

        let uri = encode_share_bundle(&bundle).expect("share bundle should encode");
        let decoded = decode_share_text(&uri)
            .expect("share bundle should decode")
            .expect("share uri should be found");
        assert_eq!(decoded, bundle);
    }

    #[test]
    fn decode_share_text_finds_uri_inside_wrapped_text() {
        let bundle = ScriptShareBundle {
            version: 1,
            kind: ShareKind::Script,
            title: "Example".to_string(),
            plugin: crate::plugins::PluginManifest {
                id: "main".to_string(),
                ..crate::plugins::PluginManifest::default()
            },
            entry_path: "scripts/example.ts".to_string(),
            files: vec![ShareFile {
                path: "scripts/example.ts".to_string(),
                content: "console.log('hi')".to_string(),
            }],
        };
        let uri = encode_share_bundle(&bundle).expect("share bundle should encode");
        let wrapped = format!("Paste this into Script Kit: `{uri}`");

        let decoded = decode_share_text(&wrapped)
            .expect("wrapped share text should decode")
            .expect("wrapped share text should contain a uri");
        assert_eq!(decoded.title, "Example");
    }

    #[test]
    fn validate_share_relative_path_rejects_parent_dirs() {
        let error = validate_share_relative_path("../scripts/nope.ts")
            .expect_err("parent-dir paths should be rejected");
        assert!(error.to_string().contains("cannot contain '..'"));
    }

    #[test]
    fn validate_share_relative_path_requires_known_top_level_dir() {
        let error = validate_share_relative_path("assets/file.txt")
            .expect_err("unexpected top-level dirs should be rejected");
        assert!(error.to_string().contains("scripts/"));
    }

    #[test]
    fn install_share_bundle_writes_plugin_manifest_and_entry_file() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let original_home = std::env::var_os("HOME");
        std::env::set_var("HOME", temp.path());

        let bundle = ScriptShareBundle {
            version: 1,
            kind: ShareKind::Script,
            title: "Example".to_string(),
            plugin: crate::plugins::PluginManifest {
                id: "shared-example".to_string(),
                title: "Shared Example".to_string(),
                ..crate::plugins::PluginManifest::default()
            },
            entry_path: "scripts/example.ts".to_string(),
            files: vec![ShareFile {
                path: "scripts/example.ts".to_string(),
                content: "console.log('shared')".to_string(),
            }],
        };

        let outcome = install_share_bundle(&bundle).expect("share bundle should install");
        assert_eq!(outcome.plugin_id, "shared-example");
        assert_eq!(outcome.files_written, 1);
        assert!(outcome.plugin_root.join("plugin.json").exists());
        assert_eq!(
            fs::read_to_string(&outcome.entry_path).expect("entry file should exist"),
            "console.log('shared')"
        );

        match original_home {
            Some(home) => std::env::set_var("HOME", home),
            None => std::env::remove_var("HOME"),
        }
    }
}
