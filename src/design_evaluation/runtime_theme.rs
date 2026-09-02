use anyhow::{ensure, Context as _, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const MAX_THEME_FILE_BYTES: u64 = 1024 * 1024;
const MALFORMED_THEME: &[u8] = b"{\"colors\": [owned malformed theme fixture";

fn owned_theme_path() -> Result<PathBuf> {
    let policy =
        crate::runtime_policy::owned_evaluation().context("owned_theme_fixture_required")?;
    let path = crate::setup::theme_json_path();
    policy.require_owned_path(&path)?;
    Ok(path)
}

fn read_theme_file(path: &Path) -> Result<Option<Vec<u8>>> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "theme_fixture_regular_file_required"
            );
            ensure!(
                metadata.len() <= MAX_THEME_FILE_BYTES,
                "theme_fixture_file_too_large"
            );
            Ok(Some(std::fs::read(path)?))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn file_hash(bytes: Option<&[u8]>) -> Option<String> {
    bytes.map(|bytes| format!("{:x}", Sha256::digest(bytes)))
}

struct ThemeFileRestore {
    path: PathBuf,
    original: Option<Vec<u8>>,
    active: bool,
}

impl ThemeFileRestore {
    fn capture() -> Result<Self> {
        let path = owned_theme_path()?;
        Ok(Self {
            original: read_theme_file(&path)?,
            path,
            active: true,
        })
    }

    fn restore(&mut self) -> Result<Option<String>> {
        crate::runtime_policy::owned_evaluation()
            .context("owned_theme_fixture_required")?
            .require_owned_path(&self.path)?;
        match std::fs::symlink_metadata(&self.path) {
            Ok(metadata) if metadata.is_dir() => std::fs::remove_dir(&self.path)?,
            Ok(metadata) => {
                ensure!(
                    metadata.is_file() && !metadata.file_type().is_symlink(),
                    "theme_fixture_restore_path_changed"
                );
                let current = read_theme_file(&self.path)?;
                ensure!(
                    current.as_deref() == Some(MALFORMED_THEME) || current == self.original,
                    "theme_fixture_restore_content_changed"
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        match &self.original {
            Some(bytes) => crate::atomic_file::write_atomic(&self.path, bytes)?,
            None if self.path.exists() => std::fs::remove_file(&self.path)?,
            None => {}
        }
        let restored = read_theme_file(&self.path)?;
        ensure!(restored == self.original, "theme_fixture_restore_mismatch");
        self.active = false;
        Ok(file_hash(restored.as_deref()))
    }
}

impl Drop for ThemeFileRestore {
    fn drop(&mut self) {
        if self.active {
            if let Err(error) = self.restore() {
                tracing::error!(%error, "Owned theme fixture restoration failed");
            }
        }
    }
}

/// Holds only a real owned filesystem obstruction, never a mocked save/reload
/// result or an assigned Theme Chooser status. Normal shutdown restores it.
#[derive(Default)]
pub(super) struct ThemeFixtureState {
    save_blocker: Option<ThemeFileRestore>,
}

impl ThemeFixtureState {
    pub(super) fn restore(&mut self) -> Result<()> {
        if let Some(guard) = self.save_blocker.as_mut() {
            guard.restore()?;
        }
        self.save_blocker = None;
        Ok(())
    }

    pub(super) fn control(
        &mut self,
        command: crate::protocol::ThemeFixtureCommand,
        cx: &mut gpui::App,
    ) -> Result<Value> {
        use crate::protocol::ThemeFixtureCommand;
        owned_theme_path()?;
        match command {
            ThemeFixtureCommand::ArmSaveFailure {} => {
                ensure!(
                    self.save_blocker.is_none(),
                    "theme_save_failure_already_armed"
                );
                let guard = ThemeFileRestore::capture()?;
                if guard.original.is_some() {
                    std::fs::remove_file(&guard.path)?;
                }
                std::fs::create_dir(&guard.path)?;
                let original_hash = file_hash(guard.original.as_deref());
                let blocker_present = std::fs::symlink_metadata(&guard.path)?.is_dir();
                self.save_blocker = Some(guard);
                Ok(
                    json!({"family":"theme","operation":"armSaveFailure","path":"theme.json",
                    "revision":crate::theme::service::theme_revision(),"originalFileSha256":original_hash,
                    "blockerPresent":blocker_present,"ordinaryRestartProven":false}),
                )
            }
            ThemeFixtureCommand::ClearSaveFailure {} => {
                let guard = self
                    .save_blocker
                    .as_mut()
                    .context("theme_save_failure_not_armed")?;
                let original_hash = file_hash(guard.original.as_deref());
                let restored_hash = guard.restore()?;
                self.save_blocker = None;
                Ok(
                    json!({"family":"theme","operation":"clearSaveFailure","path":"theme.json",
                    "revision":crate::theme::service::theme_revision(),"originalFileSha256":original_hash,
                    "restoredFileSha256":restored_hash,"blockerPresent":false,"ordinaryRestartProven":false}),
                )
            }
            ThemeFixtureCommand::MalformedReload {} => {
                ensure!(
                    self.save_blocker.is_none(),
                    "theme_save_failure_still_armed"
                );
                let mut guard = ThemeFileRestore::capture()?;
                let original_hash = file_hash(guard.original.as_deref());
                let before = crate::theme::get_theme_snapshot();
                let before_hash = format!(
                    "{:x}",
                    Sha256::digest(serde_json::to_vec(before.theme.as_ref())?)
                );
                crate::atomic_file::write_atomic(&guard.path, MALFORMED_THEME)?;
                let malformed_hash = file_hash(read_theme_file(&guard.path)?.as_deref());
                let reload = crate::theme::service::reload_theme(
                    cx,
                    crate::theme::service::ThemePublicationSource::FileReload,
                );
                let after = crate::theme::get_theme_snapshot();
                let after_hash = format!(
                    "{:x}",
                    Sha256::digest(serde_json::to_vec(after.theme.as_ref())?)
                );
                let restored_hash = guard.restore()?;
                Ok(
                    json!({"family":"theme","operation":"malformedReload","path":"theme.json",
                    "reloadError":reload.err().map(|error| error.to_string()),
                    "beforeRevision":before.revision,"afterRevision":after.revision,
                    "beforeThemeSha256":before_hash,"afterThemeSha256":after_hash,
                    "malformedFileSha256":malformed_hash,"originalFileSha256":original_hash,
                    "restoredFileSha256":restored_hash,"ordinaryRestartProven":false}),
                )
            }
        }
    }
}
