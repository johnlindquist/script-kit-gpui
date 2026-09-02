//! Atomic filesystem writes for the brain substrate.

use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context as _, Result};

use super::paths::BrainPaths;

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
    if let Some(policy) = crate::runtime_policy::owned_evaluation() {
        policy.require_owned_path(path)?;
    }
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

/// Preserve a conflicting on-disk document under the same Brain owner's
/// private trash without overwriting another conflict created this second.
/// Callers already hold the Brain write lock, so this primitive deliberately
/// does not reacquire it.
pub(crate) fn preserve_private_conflict_copy(path: &Path, contents: &str) -> Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    preserve_private_conflict_copy_at(path, contents, timestamp)
}

pub(crate) fn preserve_private_conflict_copy_at(
    path: &Path,
    contents: &str,
    timestamp: u64,
) -> Result<PathBuf> {
    let parent = path
        .parent()
        .with_context(|| format!("private conflict source has no parent: {}", path.display()))?;
    let root = parent.parent().with_context(|| {
        format!(
            "private conflict source has no Brain root: {}",
            path.display()
        )
    })?;
    let paths = BrainPaths::new(root);
    let trash = paths.trash_dir();
    if !paths.contains(path) || parent == trash.as_path() {
        bail!(
            "private conflict source must be an owned Brain document: {}",
            path.display()
        );
    }

    prepare_private_document_directory(path)?;
    if !crate::atomic_file::inspect_private_file(path)? {
        bail!("private conflict source is missing: {}", path.display());
    }
    crate::atomic_file::ensure_private_directory(&trash)
        .with_context(|| format!("preparing private Brain trash {}", trash.display()))?;

    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .context("private conflict source has no valid filename stem")?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("md");

    for attempt in 1..=1024 {
        let suffix = if attempt == 1 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let destination = trash.join(format!("{stem}.conflict-{timestamp}{suffix}.{extension}"));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }

        match options.open(&destination) {
            Ok(mut file) => {
                if let Err(error) = file
                    .write_all(contents.as_bytes())
                    .and_then(|_| file.sync_all())
                {
                    let _ = std::fs::remove_file(&destination);
                    return Err(error).with_context(|| {
                        format!("writing private Brain conflict {}", destination.display())
                    });
                }
                return Ok(destination);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("creating private Brain conflict {}", destination.display())
                });
            }
        }
    }

    bail!("no unused private Brain conflict filename remained")
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
