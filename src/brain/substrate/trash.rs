//! Trash and restore semantics for brain files.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context as _, Result};

use super::io::read_private_document_if_present;
use super::paths::BrainPaths;

/// Move `source` into `brain/trash/`, preserving the filename. On collision,
/// append a unix-timestamp suffix before the extension.
pub fn trash_file(paths: &BrainPaths, source: &Path) -> Result<PathBuf> {
    if !paths.contains(source) {
        bail!(
            "refusing to trash path outside brain tree: {}",
            source.display()
        );
    }
    if read_private_document_if_present(source)?.is_none() {
        bail!("cannot trash missing file: {}", source.display());
    }

    let filename = source
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("invalid trash source filename: {}", source.display()))?;

    let trash_dir = paths.trash_dir();
    crate::atomic_file::ensure_private_directory(paths.base())
        .with_context(|| format!("preparing private brain root {}", paths.base().display()))?;
    crate::atomic_file::ensure_private_directory(&trash_dir)
        .with_context(|| format!("preparing private trash dir {}", trash_dir.display()))?;

    let mut destination = trash_dir.join(filename);
    if crate::atomic_file::inspect_private_file(&destination)
        .with_context(|| format!("inspecting private trash target {}", destination.display()))?
    {
        let stem = source
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("file");
        let extension = source
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| format!(".{ext}"))
            .unwrap_or_default();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let mut available = None;
        for attempt in 1..=1024 {
            let ordinal = if attempt == 1 {
                String::new()
            } else {
                format!("-{attempt}")
            };
            let candidate = trash_dir.join(format!("{stem}-{ts}{ordinal}{extension}"));
            if !crate::atomic_file::inspect_private_file(&candidate).with_context(|| {
                format!("inspecting private trash target {}", candidate.display())
            })? {
                available = Some(candidate);
                break;
            }
        }
        destination = available.context("no unused private brain trash filename remained")?;
    }

    fs::rename(source, &destination).with_context(|| {
        format!(
            "moving {} to trash at {}",
            source.display(),
            destination.display()
        )
    })?;

    Ok(destination)
}

/// Move a file from `brain/trash/` back to `destination`.
pub fn restore_file(paths: &BrainPaths, trashed: &Path, destination: &Path) -> Result<()> {
    let trash_dir = paths.trash_dir();
    if !paths.contains(trashed) || trashed.parent() != Some(trash_dir.as_path()) {
        bail!(
            "restore source must live in trash dir: {}",
            trashed.display()
        );
    }
    if !paths.contains(destination) || destination.parent() == Some(trash_dir.as_path()) {
        bail!(
            "restore destination must live in an owned brain document directory: {}",
            destination.display()
        );
    }
    if read_private_document_if_present(trashed)?.is_none() {
        bail!("cannot restore missing trash entry: {}", trashed.display());
    }
    if read_private_document_if_present(destination)?.is_some() {
        bail!(
            "restore destination already exists: {}",
            destination.display()
        );
    }

    fs::rename(trashed, destination).with_context(|| {
        format!(
            "restoring {} to {}",
            trashed.display(),
            destination.display()
        )
    })?;

    Ok(())
}
