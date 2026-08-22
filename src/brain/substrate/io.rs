//! Atomic filesystem writes for the brain substrate.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context as _, Result};

/// Process-wide serialization for every mutation of files under the brain
/// substrate. Day/note/fragment files have multiple concurrent writers (editor
/// autosave, `;todo` capture, clipboard sediment, dictation, agent traces);
/// several perform an unlocked read-modify-write. Without one lock a background
/// append can land between a save's disk read and its overwrite, silently
/// dropping the appended line.
static BRAIN_FILE_WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Run `f` while holding the process-wide brain file write lock. All mutations
/// of files under the brain substrate (writes, appends, editor saves) MUST go
/// through this so read-modify-write appends can never interleave with saves.
///
/// The lock is NOT reentrant: never call a `with_brain_write_lock`-wrapped
/// function from inside another wrapped closure. `atomic_write` deliberately
/// does not take the lock so it can be used as the write primitive inside a
/// wrapped read-modify-write scope.
pub fn with_brain_write_lock<T>(f: impl FnOnce() -> T) -> T {
    let _guard = BRAIN_FILE_WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f()
}

/// Prepare the private substrate root and its owned document directory before
/// opening either existing user text or a new atomic-write sibling. The
/// generic append helper also has isolated callers outside the substrate, so
/// only recognized Brain child directories cause their parent to be hardened.
fn prepare_private_document_directory(path: &Path) -> Result<&Path> {
    let parent = path
        .parent()
        .with_context(|| format!("brain path has no parent: {}", path.display()))?;

    if matches!(
        parent.file_name().and_then(|name| name.to_str()),
        Some("days" | "fragments" | "notes" | "trash")
    ) {
        if let Some(brain_root) = parent.parent() {
            crate::atomic_file::ensure_private_directory(brain_root).with_context(|| {
                format!("preparing private brain root {}", brain_root.display())
            })?;
        }
    }

    crate::atomic_file::ensure_private_directory(parent)
        .with_context(|| format!("preparing private brain dir {}", parent.display()))?;
    Ok(parent)
}

/// Read canonical Notes, day-page, or fragment text only after its existing
/// directory and opened no-follow file descriptor are repaired to `0700` and
/// `0600`. A planted parent/final symlink fails without exposing foreign bytes.
pub(crate) fn read_private_document(path: &Path) -> Result<String> {
    prepare_private_document_directory(path)?;
    crate::atomic_file::read_private_file(path)
        .with_context(|| format!("reading private brain document {}", path.display()))
}

/// Distinguish a genuinely absent document from an unsafe target. Callers must
/// not collapse symlinks, directories, or failed permission repair into a
/// fabricated empty document and then overwrite unrelated content.
pub(crate) fn read_private_document_if_present(path: &Path) -> Result<Option<String>> {
    prepare_private_document_directory(path)?;
    if !crate::atomic_file::inspect_private_file(path)
        .with_context(|| format!("inspecting private brain document {}", path.display()))?
    {
        return Ok(None);
    }
    read_private_document(path).map(Some)
}

/// Write `contents` to `path` atomically via a temp file in the same directory.
pub fn atomic_write(path: &Path, contents: &str) -> Result<()> {
    prepare_private_document_directory(path)?;
    crate::atomic_file::write_private_atomic(path, contents.as_bytes()).with_context(|| {
        format!(
            "atomically writing private brain document {}",
            path.display()
        )
    })
}

/// Append `line` to an existing file atomically (read-modify-write). The entire
/// read-modify-write runs under [`with_brain_write_lock`] so a concurrent append
/// or editor save cannot interleave between the disk read and the rewrite.
pub fn atomic_append_line(path: &Path, line: &str) -> Result<()> {
    with_brain_write_lock(|| {
        let mut contents = read_private_document_if_present(path)?.unwrap_or_default();

        if !contents.is_empty() && !contents.ends_with('\n') {
            contents.push('\n');
        }
        contents.push_str(line);
        if !line.ends_with('\n') {
            contents.push('\n');
        }

        atomic_write(path, &contents)
    })
}
