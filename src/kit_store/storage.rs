//! Persistent storage for installed kit metadata.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::setup::get_kit_path;

use super::InstalledKit;

const KIT_STORE_REGISTRY_FILE: &str = "kit-store.json";

fn registry_path() -> PathBuf {
    get_kit_path().join(KIT_STORE_REGISTRY_FILE)
}

/// List all installed kits from `~/.scriptkit/kit-store.json`.
pub fn list_installed_kits() -> Result<Vec<InstalledKit>> {
    list_installed_kits_from_path(&registry_path())
}

/// Get a single installed kit by kit name.
pub fn get_kit(name: &str) -> Result<Option<InstalledKit>> {
    get_kit_from_path(name, &registry_path())
}

/// Save the complete installed kit registry to `~/.scriptkit/kit-store.json`.
pub fn save_kit_registry(kits: &[InstalledKit]) -> Result<()> {
    save_kit_registry_to_path(kits, &registry_path())
}

/// Remove a kit from the installed kit registry by name.
pub fn remove_kit(name: &str) -> Result<()> {
    remove_kit_from_path(name, &registry_path())
}

fn list_installed_kits_from_path(path: &Path) -> Result<Vec<InstalledKit>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read kit store registry: {}", path.display()))?;

    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    serde_json::from_str(&content).with_context(|| {
        format!(
            "Failed to parse kit store registry JSON: {}",
            path.display()
        )
    })
}

fn save_kit_registry_to_path(kits: &[InstalledKit], path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create parent directory for kit store registry: {}",
                parent.display()
            )
        })?;
    }

    let content = serde_json::to_string_pretty(kits).with_context(|| {
        format!(
            "Failed to serialize {} kit entries for kit store registry",
            kits.len()
        )
    })?;

    // Atomic write (see src/atomic_file.rs): a raw fs::write left the installed-kit
    // registry corruptible by a crash mid-write or a concurrent writer.
    crate::atomic_file::write_atomic(path, content.as_bytes())
        .with_context(|| format!("Failed to write kit store registry: {}", path.display()))
}

fn get_kit_from_path(name: &str, path: &Path) -> Result<Option<InstalledKit>> {
    let kits = list_installed_kits_from_path(path)?;
    Ok(kits.into_iter().find(|kit| kit.name == name))
}

fn remove_kit_from_path(name: &str, path: &Path) -> Result<()> {
    let mut kits = list_installed_kits_from_path(path)?;
    kits.retain(|kit| kit.name != name);
    save_kit_registry_to_path(&kits, path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn sample_kits() -> Vec<InstalledKit> {
        vec![
            InstalledKit {
                name: "main".to_string(),
                path: PathBuf::from("/tmp/main"),
                repo_url: "https://github.com/script-kit/main".to_string(),
                git_hash: "abc123".to_string(),
                installed_at: "2026-02-07T21:30:00Z".to_string(),
            },
            InstalledKit {
                name: "tools".to_string(),
                path: PathBuf::from("/tmp/tools"),
                repo_url: "https://github.com/script-kit/tools".to_string(),
                git_hash: "def456".to_string(),
                installed_at: "2026-02-07T21:31:00Z".to_string(),
            },
        ]
    }

    #[test]
    fn test_list_installed_kits_returns_empty_when_registry_missing() {
        let temp_dir = tempfile::tempdir().expect("temp dir should create");
        let registry_path = temp_dir.path().join("kit-store.json");

        let kits = list_installed_kits_from_path(&registry_path).expect("load should succeed");
        assert!(kits.is_empty());
    }

    #[test]
    fn test_save_kit_registry_to_path_persists_entries_when_registry_is_new() {
        let temp_dir = tempfile::tempdir().expect("temp dir should create");
        let registry_path = temp_dir.path().join("kit-store.json");
        let expected = sample_kits();

        save_kit_registry_to_path(&expected, &registry_path).expect("save should succeed");
        let loaded =
            list_installed_kits_from_path(&registry_path).expect("load after save should succeed");

        assert_eq!(loaded, expected);
    }

    #[test]
    fn test_remove_kit_persists_filtered_registry_when_target_exists() {
        let temp_dir = tempfile::tempdir().expect("temp dir should create");
        let registry_path = temp_dir.path().join("kit-store.json");
        save_kit_registry_to_path(&sample_kits(), &registry_path)
            .expect("initial save should succeed");

        remove_kit_from_path("main", &registry_path).expect("remove should succeed");
        let loaded =
            list_installed_kits_from_path(&registry_path).expect("filtered load should succeed");

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "tools");
    }

    #[test]
    fn test_get_kit_from_path_returns_match_when_name_exists() {
        let temp_dir = tempfile::tempdir().expect("temp dir should create");
        let registry_path = temp_dir.path().join("kit-store.json");
        save_kit_registry_to_path(&sample_kits(), &registry_path).expect("save should succeed");

        let kit = get_kit_from_path("tools", &registry_path)
            .expect("lookup should succeed")
            .expect("kit should exist");
        assert_eq!(kit.repo_url, "https://github.com/script-kit/tools");
    }

    #[test]
    fn test_list_installed_kits_returns_empty_when_registry_is_blank() {
        let temp_dir = tempfile::tempdir().expect("temp dir should create");
        let registry_path = temp_dir.path().join("kit-store.json");

        fs::write(&registry_path, "   \n").expect("blank file write should succeed");
        let loaded =
            list_installed_kits_from_path(&registry_path).expect("blank file load should succeed");

        assert!(loaded.is_empty());
    }
}
