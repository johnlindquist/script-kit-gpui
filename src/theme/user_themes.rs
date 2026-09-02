//! User-authored themes stored under `~/.scriptkit/themes/<slug>.json`.
//!
//! Each JSON file holds a `name`, `appearance`, and the same token shape the
//! bundled presets and the user-owned `~/.scriptkit/theme.json` override
//! use. Saving a theme first runs the hover/selected validation in
//! [`crate::theme::validation`] so the row-state opacity contract cannot be
//! broken by a user-authored file.
//!
//! The launcher-facing theme chooser merges the list returned by
//! [`list_user_themes`] into the built-in preset catalog so user themes show
//! up alongside the bundled ones and can be selected, applied, or duplicated
//! without leaving the app.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::theme::Theme;

/// Returns the directory that holds user-authored themes.
///
/// The directory lives at `~/.scriptkit/themes/`. It is seeded at app
/// startup so listing it never needs to create it on the hot path.
pub fn user_themes_dir() -> PathBuf {
    crate::setup::themes_dir()
}

/// Ensures the user-themes directory exists. Call once during setup.
pub fn ensure_user_themes_dir() -> Result<()> {
    let dir = user_themes_dir();
    require_theme_path(&dir)?;
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .with_context(|| format!("creating user themes dir {}", dir.display()))?;
    }
    Ok(())
}

/// Metadata describing a single user-authored theme on disk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserTheme {
    /// Slug derived from the file stem (lowercase, `-`-separated).
    pub slug: String,
    /// Human-readable name stored inside the JSON payload.
    pub name: String,
    /// Absolute path to the theme file.
    pub path: PathBuf,
}

/// Preview of the slug/display-name a save-copy operation will use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserThemeNameResolution {
    pub requested_name: String,
    pub display_name: String,
    pub slug: String,
    pub path: PathBuf,
    pub collision_count: usize,
}

/// Backup returned by a staged delete so Theme Designer can offer a one-step
/// restore during the same session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedUserThemeBackup {
    pub slug: String,
    pub name: String,
    pub contents: String,
    pub path: PathBuf,
}

/// Lists every user theme under [`user_themes_dir`]. Corrupt or unreadable
/// files are skipped so one bad file cannot hide the rest.
pub fn list_user_themes() -> Vec<UserTheme> {
    let dir = user_themes_dir();
    if require_theme_path(&dir).is_err() {
        return Vec::new();
    }
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut themes: Vec<UserTheme> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            require_theme_path(&path).ok()?;
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                return None;
            }
            let slug = path.file_stem()?.to_string_lossy().into_owned();
            let contents = fs::read_to_string(&path).ok()?;
            let value: Value = serde_json::from_str(&contents).ok()?;
            let name = value
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(slug.as_str())
                .to_string();
            Some(UserTheme { slug, name, path })
        })
        .collect();

    themes.sort_by_cached_key(|a| a.name.to_lowercase());
    themes
}

/// Looks up a user theme by slug. Returns `None` if the theme file is
/// missing or cannot be parsed.
pub fn find_user_theme(slug: &str) -> Option<UserTheme> {
    list_user_themes().into_iter().find(|t| t.slug == slug)
}

/// Loads a user-authored theme by slug. Invalid files are ignored, matching
/// list behavior so one broken custom theme cannot break Theme Designer.
pub fn load_user_theme(slug: &str) -> Option<Theme> {
    ensure_safe_user_theme_slug(slug).ok()?;
    let path = user_themes_dir().join(format!("{slug}.json"));
    require_theme_path(&path).ok()?;
    let contents = fs::read_to_string(path).ok()?;
    super::types::decode_theme_json(&contents, super::types::detect_system_appearance()).ok()
}

/// Normalizes a display name into a filesystem-safe slug.
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !slug.is_empty() {
            slug.push('-');
            prev_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        slug.push_str("theme");
    }
    slug
}

fn normalize_user_theme_display_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        "Custom Theme".to_string()
    } else {
        trimmed.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

fn ensure_safe_user_theme_slug(slug: &str) -> Result<()> {
    if slug.is_empty()
        || slug.contains('/')
        || slug.contains('\\')
        || slug.contains("..")
        || slug != slugify(slug)
    {
        anyhow::bail!("unsafe user theme slug: {slug}");
    }
    Ok(())
}

/// Resolves a human name into the unique display name and slug that will be
/// written if the user saves a copy right now.
pub fn resolve_user_theme_name(name: &str) -> UserThemeNameResolution {
    let requested_name = normalize_user_theme_display_name(name);
    let base_slug = slugify(&requested_name);
    let mut slug = base_slug.clone();
    let mut collision_count = 0usize;
    while user_themes_dir().join(format!("{slug}.json")).exists() {
        collision_count += 1;
        slug = format!("{base_slug}-{}", collision_count + 1);
    }
    let display_name = if collision_count == 0 {
        requested_name.clone()
    } else {
        format!("{} {}", requested_name, collision_count + 1)
    };

    UserThemeNameResolution {
        requested_name,
        display_name,
        path: user_themes_dir().join(format!("{slug}.json")),
        slug,
        collision_count,
    }
}

/// Saves a user theme JSON payload to disk after validating the row-state
/// opacity contract in [`crate::theme::validation`]. Returns the saved
/// [`UserTheme`] metadata on success.
///
/// The payload must be a serializable JSON object that carries a `name` key
/// and a tokens object matching the shape of the bundled presets. The caller
/// is responsible for producing that shape — this helper only owns storage,
/// name-based slugging, and validation.
pub fn save_user_theme(name: &str, payload: &Value) -> Result<UserTheme> {
    ensure_user_themes_dir()?;

    validate_payload(payload).context("validating theme before save")?;

    let slug = slugify(name);
    let path = user_themes_dir().join(format!("{slug}.json"));

    let mut enriched = payload.clone();
    if let Value::Object(map) = &mut enriched {
        map.insert("name".to_string(), Value::String(name.to_string()));
    }

    let serialized =
        serde_json::to_string_pretty(&enriched).context("serializing user theme json")?;
    atomic_write(&path, &serialized)?;

    Ok(UserTheme {
        slug,
        name: name.to_string(),
        path,
    })
}

/// Saves a user theme without overwriting an existing slug.
pub fn save_user_theme_unique(name: &str, payload: &Value) -> Result<UserTheme> {
    ensure_user_themes_dir()?;
    let resolution = resolve_user_theme_name(name);
    let display_name = resolution.display_name.clone();

    validate_payload(payload).context("validating theme before save")?;
    let path = resolution.path.clone();
    let mut enriched = payload.clone();
    if let Value::Object(map) = &mut enriched {
        map.insert("name".to_string(), Value::String(display_name.clone()));
    }
    let serialized =
        serde_json::to_string_pretty(&enriched).context("serializing user theme json")?;
    atomic_write(&path, &serialized)?;

    Ok(UserTheme {
        slug: resolution.slug,
        name: display_name,
        path,
    })
}

/// Saves the current in-memory theme as a user-authored preset.
pub fn save_theme_as_user_theme(name: &str, theme: &Theme) -> Result<UserTheme> {
    let payload = serde_json::to_value(theme).context("serializing current theme")?;
    save_user_theme_unique(name, &payload)
}

/// Overwrites a specific user theme slug with the provided in-memory theme.
pub fn save_theme_to_user_theme_slug(slug: &str, name: &str, theme: &Theme) -> Result<UserTheme> {
    ensure_safe_user_theme_slug(slug)?;
    ensure_user_themes_dir()?;
    let payload = serde_json::to_value(theme).context("serializing current theme")?;
    validate_payload(&payload).context("validating theme before save")?;
    let path = user_themes_dir().join(format!("{slug}.json"));
    let mut enriched = payload;
    if let Value::Object(map) = &mut enriched {
        map.insert("name".to_string(), Value::String(name.to_string()));
    }
    let serialized =
        serde_json::to_string_pretty(&enriched).context("serializing user theme json")?;
    atomic_write(&path, &serialized)?;

    Ok(UserTheme {
        slug: slug.to_string(),
        name: name.to_string(),
        path,
    })
}

/// Deletes a user theme by slug. Missing files are treated as success so the
/// caller does not need to guard against pre-existing deletion races.
pub fn delete_user_theme(slug: &str) -> Result<()> {
    ensure_safe_user_theme_slug(slug)?;
    let path = user_themes_dir().join(format!("{slug}.json"));
    require_theme_path(&path)?;
    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("deleting user theme {}", path.display()))?;
    }
    Ok(())
}

/// Deletes a user theme and returns the removed JSON so callers can restore it.
pub fn delete_user_theme_with_backup(slug: &str) -> Result<Option<DeletedUserThemeBackup>> {
    ensure_safe_user_theme_slug(slug)?;
    let path = user_themes_dir().join(format!("{slug}.json"));
    require_theme_path(&path)?;
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("reading user theme before delete {}", path.display()))?;
    let name = serde_json::from_str::<Value>(&contents)
        .ok()
        .and_then(|value| {
            value
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| slug.to_string());
    fs::remove_file(&path).with_context(|| format!("deleting user theme {}", path.display()))?;
    Ok(Some(DeletedUserThemeBackup {
        slug: slug.to_string(),
        name,
        contents,
        path,
    }))
}

/// Restores a previously deleted user theme backup if the slug is still free.
pub fn restore_user_theme_backup(backup: &DeletedUserThemeBackup) -> Result<UserTheme> {
    ensure_safe_user_theme_slug(&backup.slug)?;
    ensure_user_themes_dir()?;
    let value: Value =
        serde_json::from_str(&backup.contents).context("parsing deleted user theme backup")?;
    validate_payload(&value).context("validating deleted user theme backup")?;
    let path = user_themes_dir().join(format!("{}.json", backup.slug));
    if path.exists() {
        anyhow::bail!(
            "cannot restore user theme {}; slug already exists",
            backup.slug
        );
    }
    atomic_write(&path, &backup.contents)?;
    Ok(UserTheme {
        slug: backup.slug.clone(),
        name: backup.name.clone(),
        path,
    })
}

fn atomic_write(path: &Path, contents: &str) -> Result<()> {
    require_theme_path(path)?;
    crate::atomic_file::write_atomic(path, contents.as_bytes())?;
    if crate::runtime_policy::is_owned_evaluation() {
        crate::runtime_policy::record_completed_fixture_effect();
    }
    Ok(())
}

fn require_theme_path(path: &Path) -> Result<()> {
    if let Some(policy) = crate::runtime_policy::owned_evaluation() {
        policy.require_owned_path(path)?;
    }
    Ok(())
}

/// Minimal structural validation. Rejects payloads where the hover opacity is
/// not strictly less than the selected opacity, matching the row-state
/// contract documented in `removed-docs`.
fn validate_payload(payload: &Value) -> Result<()> {
    let Some(opacity) = payload
        .get("opacity")
        .or_else(|| payload.get("tokens").and_then(|t| t.get("opacity")))
    else {
        return Ok(());
    };

    let hover = opacity
        .get("hover")
        .or_else(|| opacity.get("row_state_hover"))
        .or_else(|| opacity.get("rowStateHover"))
        .and_then(|v| v.as_f64());
    let selected = opacity
        .get("selected")
        .or_else(|| opacity.get("row_state_selected"))
        .or_else(|| opacity.get("rowStateSelected"))
        .and_then(|v| v.as_f64());

    if let (Some(h), Some(s)) = (hover, selected) {
        if h >= s {
            anyhow::bail!(
                "invalid theme: hover opacity ({h}) must be strictly less than selected opacity ({s})"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn with_temp_themes_dir(test: impl FnOnce()) {
        let _lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp = tempfile::tempdir().expect("tempdir");
        let previous = std::env::var_os(crate::setup::SK_PATH_ENV);
        std::env::set_var(crate::setup::SK_PATH_ENV, temp.path());
        test();
        if let Some(previous) = previous {
            std::env::set_var(crate::setup::SK_PATH_ENV, previous);
        } else {
            std::env::remove_var(crate::setup::SK_PATH_ENV);
        }
    }

    #[test]
    fn slugify_normalizes_names() {
        assert_eq!(slugify("My Theme"), "my-theme");
        assert_eq!(slugify("  Extra   Spaces  "), "extra-spaces");
        assert_eq!(slugify("!!!"), "theme");
        assert_eq!(slugify("Foo/Bar Baz"), "foo-bar-baz");
    }

    #[test]
    fn validate_payload_rejects_hover_ge_selected() {
        let bad = json!({
            "opacity": { "row_state_hover": 0.25, "row_state_selected": 0.23 }
        });
        assert!(validate_payload(&bad).is_err());
    }

    #[test]
    fn validate_payload_accepts_hover_below_selected() {
        let good = json!({
            "opacity": { "row_state_hover": 0.06, "row_state_selected": 0.23 }
        });
        assert!(validate_payload(&good).is_ok());
    }

    #[test]
    fn validate_payload_checks_current_hover_selected_shape() {
        let bad = json!({
            "opacity": { "hover": 0.25, "selected": 0.23 }
        });
        assert!(validate_payload(&bad).is_err());
    }

    #[test]
    fn resolve_user_theme_name_reports_collisions() {
        with_temp_themes_dir(|| {
            ensure_user_themes_dir().expect("themes dir");
            let first = resolve_user_theme_name("Night Work");
            assert_eq!(first.display_name, "Night Work");
            assert_eq!(first.slug, "night-work");

            fs::write(user_themes_dir().join("night-work.json"), "{}").expect("write first");
            let second = resolve_user_theme_name(" Night   Work ");
            assert_eq!(second.display_name, "Night Work 2");
            assert_eq!(second.slug, "night-work-2");
            assert_eq!(second.collision_count, 1);

            fs::write(user_themes_dir().join("night-work-2.json"), "{}").expect("write second");
            let third = resolve_user_theme_name("Night Work");
            assert_eq!(third.display_name, "Night Work 3");
            assert_eq!(third.slug, "night-work-3");
        });
    }

    #[test]
    fn update_delete_and_restore_user_theme_by_slug() {
        with_temp_themes_dir(|| {
            let mut theme = Theme::default();
            theme.colors.accent.selected = 0x123456;
            let saved = save_theme_to_user_theme_slug("night-work", "Night Work", &theme)
                .expect("save theme");
            assert_eq!(saved.slug, "night-work");

            theme.colors.accent.selected = 0x654321;
            let updated = save_theme_to_user_theme_slug("night-work", "Night Work", &theme)
                .expect("update theme");
            assert_eq!(updated.name, "Night Work");
            let loaded = load_user_theme("night-work").expect("load updated");
            assert_eq!(loaded.colors.accent.selected, 0x654321);

            assert!(save_theme_to_user_theme_slug("../bad", "Bad", &theme).is_err());

            let backup = delete_user_theme_with_backup("night-work")
                .expect("delete")
                .expect("backup");
            assert!(!user_themes_dir().join("night-work.json").exists());
            let restored = restore_user_theme_backup(&backup).expect("restore");
            assert_eq!(restored.slug, "night-work");
            assert!(user_themes_dir().join("night-work.json").exists());
            assert!(restore_user_theme_backup(&backup).is_err());
        });
    }
}
