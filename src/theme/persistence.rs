//! Recoverable two-file save using the existing AST editor and atomic storage.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};

static SAVE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeSave {
    schema_version: u8,
    rollback: bool,
    theme_before: Option<String>,
    config_before: Option<String>,
    theme_after: String,
    config_after: String,
}

struct SavePaths {
    theme: PathBuf,
    config: PathBuf,
    journal: PathBuf,
}
impl SavePaths {
    fn current() -> Result<Self> {
        let theme = crate::setup::theme_json_path();
        let root = theme.parent().context("theme path has no parent")?;
        let paths = Self {
            config: root.join("config.ts"),
            journal: root.join(".theme-save-transaction.json"),
            theme,
        };
        if let Some(policy) = crate::runtime_policy::owned_evaluation() {
            for path in [&paths.theme, &paths.config, &paths.journal] {
                policy.require_owned_path(path)?;
            }
        }
        Ok(paths)
    }
}

fn read_optional(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn replace_owned_version(
    path: &Path,
    before: Option<&str>,
    after: &str,
    rollback: bool,
) -> Result<()> {
    let current = read_optional(path)?;
    ensure!(
        current.as_deref() == before || current.as_deref() == Some(after),
        "theme_save_concurrent_change: {}",
        path.display()
    );
    let desired = if rollback { before } else { Some(after) };
    if current.as_deref() == desired {
        return Ok(());
    }
    if let Some(desired) = desired {
        crate::atomic_file::write_atomic(path, desired.as_bytes())?;
    } else if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn finish(paths: &SavePaths, transaction: &ThemeSave) -> Result<()> {
    ensure!(
        transaction.schema_version == 1,
        "unsupported_theme_save_transaction"
    );
    for (path, before, after) in [
        (
            &paths.theme,
            transaction.theme_before.as_deref(),
            transaction.theme_after.as_str(),
        ),
        (
            &paths.config,
            transaction.config_before.as_deref(),
            transaction.config_after.as_str(),
        ),
    ] {
        let current = read_optional(path)?;
        ensure!(
            current.as_deref() == before || current.as_deref() == Some(after),
            "theme_save_concurrent_change: {}",
            path.display()
        );
    }
    replace_owned_version(
        &paths.theme,
        transaction.theme_before.as_deref(),
        &transaction.theme_after,
        transaction.rollback,
    )?;
    replace_owned_version(
        &paths.config,
        transaction.config_before.as_deref(),
        &transaction.config_after,
        transaction.rollback,
    )?;
    std::fs::remove_file(&paths.journal)?;
    Ok(())
}

fn recover(paths: &SavePaths) -> Result<()> {
    if let Some(journal) = read_optional(&paths.journal)? {
        finish(paths, &serde_json::from_str::<ThemeSave>(&journal)?)?;
    }
    Ok(())
}
pub(super) fn recover_theme_save() -> Result<()> {
    let _guard = SAVE_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("theme save lock poisoned"))?;
    crate::config::with_user_preference_write_lock(|| recover(&SavePaths::current()?))
}
pub(super) fn write_theme_to_disk(theme: &super::Theme) -> Result<()> {
    let _guard = SAVE_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("theme save lock poisoned"))?;
    crate::config::with_user_preference_write_lock(|| save_at(&SavePaths::current()?, theme))
}

fn save_at(paths: &SavePaths, theme: &super::Theme) -> Result<()> {
    recover(paths)?;
    let theme_before = read_optional(&paths.theme)?;
    let config_before = read_optional(&paths.config)?;
    // An explicit empty selection also defeats the legacy settings fallback.
    let property = crate::config::editor::ConfigProperty::new("theme", "{ presetId: null }");
    let config_after = crate::config::editor::prepare_config_property(
        config_before.as_deref().unwrap_or(""),
        &property,
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let mut transaction = ThemeSave {
        schema_version: 1,
        rollback: false,
        theme_before,
        config_before,
        theme_after: serde_json::to_string_pretty(theme)?,
        config_after,
    };
    crate::atomic_file::write_atomic(&paths.journal, &serde_json::to_vec(&transaction)?)?;
    if let Err(error) = finish(paths, &transaction) {
        // Persist rollback intent before restoring either file; a subsequent
        // load resumes that intent, and never overwrites an unrelated edit.
        transaction.rollback = true;
        crate::atomic_file::write_atomic(&paths.journal, &serde_json::to_vec(&transaction)?)?;
        finish(paths, &transaction).context(format!("save failed ({error}); rollback failed"))?;
        return Err(error);
    }
    if crate::runtime_policy::is_owned_evaluation() {
        crate::runtime_policy::record_completed_fixture_effect();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn paths(root: &Path) -> SavePaths {
        SavePaths {
            theme: root.join("theme.json"),
            config: root.join("config.ts"),
            journal: root.join("journal.json"),
        }
    }

    #[test]
    fn custom_save_clears_preset_but_preserves_other_config() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        std::fs::write(&paths.config, "import type { Config } from '@scriptkit/sdk';\nexport default { theme: { presetId: 'nord' }, custom: 'keep' } satisfies Config;\n").unwrap();
        let theme = super::super::Theme::light_default();
        save_at(&paths, &theme).unwrap();
        let config = std::fs::read_to_string(&paths.config).unwrap();
        assert!(config.contains("presetId: null"));
        assert!(config.contains("custom: 'keep'"));
        let reloaded = super::super::types::decode_theme_json(
            &std::fs::read_to_string(&paths.theme).unwrap(),
            true,
        )
        .unwrap();
        assert_eq!(
            reloaded.colors.background.main,
            theme.colors.background.main
        );
        assert!(!paths.journal.exists());
    }

    #[test]
    fn interrupted_save_recovers_and_foreign_edits_are_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        let transaction = ThemeSave {
            schema_version: 1,
            rollback: false,
            theme_before: None,
            config_before: None,
            theme_after: "theme-new".into(),
            config_after: "config-new".into(),
        };
        std::fs::write(&paths.journal, serde_json::to_vec(&transaction).unwrap()).unwrap();
        std::fs::write(&paths.theme, &transaction.theme_after).unwrap();
        recover(&paths).unwrap();
        assert_eq!(
            std::fs::read_to_string(&paths.config).unwrap(),
            "config-new"
        );
        std::fs::write(&paths.journal, serde_json::to_vec(&transaction).unwrap()).unwrap();
        std::fs::write(&paths.config, "foreign").unwrap();
        assert!(recover(&paths).is_err());
        assert_eq!(std::fs::read_to_string(&paths.config).unwrap(), "foreign");
        assert!(paths.journal.exists());
    }

    #[test]
    fn invalid_config_refuses_before_writing_either_file() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        std::fs::write(&paths.config, "not a config").unwrap();
        std::fs::write(&paths.theme, "old").unwrap();
        assert!(save_at(&paths, &super::super::Theme::dark_default()).is_err());
        assert_eq!(std::fs::read_to_string(&paths.theme).unwrap(), "old");
        assert!(!paths.journal.exists());
    }
}
