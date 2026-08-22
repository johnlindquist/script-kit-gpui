//! Crash- and concurrency-safe file writes.
//!
//! The naive "write to `<path>.tmp`, then rename" idiom is atomic against a
//! crash, but NOT against concurrent writers: two savers that share one FIXED
//! temp path (`file.json.tmp`) interleave their `write` calls into the same
//! file, so the renamed result can be a torn mix of both payloads even though
//! each rename is itself atomic. That is reachable across processes (two app
//! instances sharing `~/.scriptkit/`, or a crash-relaunch overlap — no in-process
//! mutex spans processes) and, for un-serialized writers like the window-state
//! saver, within a process too. A reproduction of the fixed-temp pattern under
//! 8 concurrent writers produced thousands of torn reads.
//!
//! [`write_atomic`] avoids this by giving every write its own UNIQUE temp file
//! (via `tempfile`) in the destination directory, then persisting (renaming) it
//! over the target. Concurrent writers therefore never share a temp file, so the
//! final file is always exactly one writer's complete output (clean
//! last-writer-wins), never a torn mix.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Atomically write `bytes` to `path` using a unique temp file + rename.
///
/// Creates the parent directory if needed. On Unix the rename is atomic; a
/// concurrent reader always sees either the old file or one writer's complete
/// new file, never a partial/torn one.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    if let Some(parent) = parent {
        std::fs::create_dir_all(parent)?;
    }
    let dir = parent.unwrap_or_else(|| Path::new("."));

    let mut temp = tempfile::Builder::new()
        .prefix(".sk-atomic-")
        .suffix(".tmp")
        .tempfile_in(dir)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    // `persist` renames the unique temp over `path`; on failure the temp is
    // cleaned up by `TempPath`'s drop rather than left as litter.
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

fn unsafe_private_file_target() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "private file target must be a regular, non-symlink file",
    )
}

fn unsafe_private_directory_target() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "private directory target must be a non-symlink directory",
    )
}

/// Create private directories at `0700` from their first appearance. Existing
/// directories are opened with `O_NOFOLLOW | O_DIRECTORY` and repaired through
/// that same descriptor before any caller writes sensitive child files.
pub(crate) fn ensure_private_directory(path: &Path) -> std::io::Result<()> {
    if path.as_os_str().is_empty() {
        return Err(unsafe_private_directory_target());
    }

    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_dir() => {}
        Ok(_) => return Err(unsafe_private_directory_target()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = std::fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt as _;
                builder.mode(0o700);
            }
            builder.create(path)?;
        }
        Err(error) => return Err(error),
    }

    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY);
    }
    let directory = options.open(path)?;
    let metadata = directory.metadata()?;
    if !metadata.is_dir() {
        return Err(unsafe_private_directory_target());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o777 != 0o700 {
            directory.set_permissions(std::fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}

/// Inspect a private destination without following a planted symbolic link.
/// Missing files are legitimate; directories, devices, and links fail closed.
pub(crate) fn inspect_private_file(path: &Path) -> std::io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_file() => Ok(true),
        Ok(_) => Err(unsafe_private_file_target()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn make_opened_private_file_owner_only(file: &std::fs::File) -> std::io::Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(unsafe_private_file_target());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o777 != 0o600 {
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
    }
    Ok(())
}

/// Read sensitive content only after its opened no-follow descriptor has been
/// repaired to owner-only permissions. A legacy `0644` file is fixed before
/// any private bytes are exposed to the caller.
pub(crate) fn read_private_file(path: &Path) -> std::io::Result<String> {
    if !inspect_private_file(path)? {
        return Err(std::io::Error::from(std::io::ErrorKind::NotFound));
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    make_opened_private_file_owner_only(&file)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

fn open_private_file_for_append(path: &Path, readable: bool) -> std::io::Result<std::fs::File> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let _ = inspect_private_file(path)?;
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true).read(readable);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    make_opened_private_file_owner_only(&file)?;
    Ok(file)
}

/// Append sensitive bytes through one opened, owner-only, no-follow handle.
/// Existing legacy permissions are repaired before any new private bytes are
/// written; newly created files are `0600` from their first creation.
pub(crate) fn append_private_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = open_private_file_for_append(path, false)?;
    file.write_all(bytes)?;
    file.sync_data()
}

/// Append one JSONL record through the exact secure descriptor used to inspect
/// its existing boundary. Legacy files missing a terminal newline are repaired
/// without reading their complete private contents into foreground memory.
pub(crate) fn append_private_jsonl_record(path: &Path, record: &[u8]) -> std::io::Result<()> {
    append_private_jsonl_record_with_durability(path, record, true)
}

/// Append one private, crash-optional observability record without an fsync.
/// Trace files remain `0600`/no-follow and record writes stay single-buffer
/// atomic, but lifecycle instrumentation must not distort measured latency.
pub(crate) fn append_private_observability_record(
    path: &Path,
    record: &[u8],
) -> std::io::Result<()> {
    append_private_jsonl_record_with_durability(path, record, false)
}

fn append_private_jsonl_record_with_durability(
    path: &Path,
    record: &[u8],
    durable: bool,
) -> std::io::Result<()> {
    if record.contains(&b'\n') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "private JSONL record must not contain a newline",
        ));
    }

    let mut file = open_private_file_for_append(path, true)?;
    let needs_boundary = if file.metadata()?.len() == 0 {
        false
    } else {
        file.seek(SeekFrom::End(-1))?;
        let mut last = [0_u8; 1];
        file.read_exact(&mut last)?;
        last[0] != b'\n'
    };

    let mut append = Vec::with_capacity(record.len() + usize::from(needs_boundary) + 1);
    if needs_boundary {
        append.push(b'\n');
    }
    append.extend_from_slice(record);
    append.push(b'\n');
    file.write_all(&append)?;
    if durable {
        file.sync_data()?;
    }
    Ok(())
}

/// Atomically replace sensitive content via a unique, exclusive `0600`
/// sibling. Existing symlinks are rejected instead of silently replaced; no
/// private bytes are ever written through a caller-reopenable path.
pub(crate) fn write_private_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        std::fs::create_dir_all(parent)?;
    }
    let directory = parent.unwrap_or_else(|| Path::new("."));
    let _ = inspect_private_file(path)?;
    let temporary = directory.join(format!(".sk-private-{}.tmp", uuid::Uuid::new_v4().simple()));

    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options.open(&temporary)?;
        make_opened_private_file_owner_only(&file)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        let _ = inspect_private_file(path)?;
        std::fs::rename(&temporary, path)
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{
        append_private_file, append_private_jsonl_record, append_private_observability_record,
        ensure_private_directory, inspect_private_file, read_private_file, write_atomic,
        write_private_atomic,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[cfg(unix)]
    #[test]
    fn private_directory_creates_owner_only_and_repairs_legacy_permissions() {
        use std::os::unix::fs::PermissionsExt as _;

        let fixture = tempfile::tempdir().expect("isolated private directory fixture");
        let directory = fixture.path().join("nested").join("private");
        ensure_private_directory(&directory).expect("create owner-only directory");
        assert_eq!(
            std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );

        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
        ensure_private_directory(&directory).expect("repair older permissive directory");
        assert_eq!(
            std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_directory_rejects_symlinks_and_foreign_targets_without_mutation() {
        use std::os::unix::fs::{symlink, PermissionsExt as _};

        let fixture = tempfile::tempdir().expect("isolated private directory symlink fixture");
        let external = fixture.path().join("foreign");
        let planted = fixture.path().join("private");
        std::fs::create_dir(&external).unwrap();
        std::fs::set_permissions(&external, std::fs::Permissions::from_mode(0o755)).unwrap();
        symlink(&external, &planted).unwrap();

        assert!(ensure_private_directory(&planted).is_err());
        assert_eq!(
            std::fs::metadata(&external).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert!(std::fs::symlink_metadata(planted)
            .unwrap()
            .file_type()
            .is_symlink());

        let regular = fixture.path().join("regular.txt");
        std::fs::write(&regular, "do not replace").unwrap();
        assert!(ensure_private_directory(&regular).is_err());
        assert_eq!(std::fs::read_to_string(regular).unwrap(), "do not replace");
    }

    #[cfg(unix)]
    #[test]
    fn private_file_create_append_and_atomic_replace_are_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("isolated private file fixture");
        let path = directory.path().join("private.jsonl");
        append_private_file(&path, b"first private transcript\n").expect("private append");
        append_private_file(&path, b"second private transcript\n").expect("second append");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            read_private_file(&path).unwrap(),
            "first private transcript\nsecond private transcript\n"
        );

        write_private_atomic(&path, b"complete replacement").expect("atomic private replacement");
        assert_eq!(read_private_file(&path).unwrap(), "complete replacement");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_file_repairs_legacy_permissions_before_read_and_append() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("isolated legacy privacy fixture");
        let path = directory.path().join("legacy.jsonl");
        std::fs::write(&path, "legacy private transcript\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            read_private_file(&path).unwrap(),
            "legacy private transcript\n"
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();
        append_private_file(&path, b"another private transcript\n").unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_file_jsonl_append_repairs_legacy_boundary_and_owner_permissions() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("isolated private JSONL fixture");
        let path = directory.path().join("private-audits.jsonl");
        std::fs::write(&path, br#"{"private":"legacy prompt"}"#).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        append_private_jsonl_record(&path, br#"{"private":"new prompt"}"#).unwrap();
        append_private_jsonl_record(&path, br#"{"private":"third prompt"}"#).unwrap();
        assert_eq!(
            read_private_file(&path).unwrap(),
            "{\"private\":\"legacy prompt\"}\n{\"private\":\"new prompt\"}\n{\"private\":\"third prompt\"}\n"
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(append_private_jsonl_record(&path, b"invalid\nrecord").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn private_file_observability_records_are_owner_only_and_single_line_without_fsync() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("isolated private observability fixture");
        let path = directory.path().join("private-trace.ndjson");
        append_private_observability_record(&path, br#"{"event":"first"}"#).unwrap();
        append_private_observability_record(&path, br#"{"event":"second"}"#).unwrap();
        assert_eq!(
            read_private_file(&path).unwrap(),
            "{\"event\":\"first\"}\n{\"event\":\"second\"}\n"
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(append_private_observability_record(&path, b"invalid\ntrace").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn private_file_rejects_symlinks_before_read_append_or_atomic_replacement() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("isolated symlink privacy fixture");
        let external = directory.path().join("external-private.txt");
        let planted = directory.path().join("history.jsonl");
        std::fs::write(&external, "never read or modify this target").unwrap();
        symlink(&external, &planted).expect("planted symlink");

        assert!(inspect_private_file(&planted).is_err());
        assert!(read_private_file(&planted).is_err());
        assert!(append_private_file(&planted, b"private append").is_err());
        assert!(append_private_jsonl_record(&planted, br#"{"private":true}"#).is_err());
        assert!(append_private_observability_record(&planted, br#"{"trace":true}"#).is_err());
        assert!(write_private_atomic(&planted, b"private replacement").is_err());
        assert_eq!(
            std::fs::read_to_string(external).unwrap(),
            "never read or modify this target"
        );
        assert!(std::fs::symlink_metadata(planted)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn private_file_rejects_directory_targets_without_leaving_temporary_files() {
        let directory = tempfile::tempdir().expect("isolated unsafe target fixture");
        let target = directory.path().join("directory.jsonl");
        std::fs::create_dir(&target).unwrap();

        assert!(read_private_file(&target).is_err());
        assert!(append_private_file(&target, b"private append").is_err());
        assert!(write_private_atomic(&target, b"private replacement").is_err());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn write_atomic_never_tears_under_concurrent_writers() {
        // Regression: the previous fixed-temp-path pattern
        // (`path.with_extension("json.tmp")` + write + rename), shared by
        // input_history / frecency / window_state, corrupted the destination
        // when >1 saver ran at once (a standalone repro of that exact pattern
        // produced thousands of torn reads). `write_atomic` uses a unique temp
        // per call, so a concurrent reader must always see one writer's COMPLETE
        // payload — never a mix.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = Arc::new(dir.path().join("state.json"));
        // Two distinct, differently-sized valid payloads (large enough to need
        // multiple write syscalls, which is where tearing happened).
        let a: Arc<String> = Arc::new(format!(
            "{{\"who\":\"A\",\"v\":[{}]}}",
            (0..1500)
                .map(|i| format!("\"aaaaaaaaaa-{i}\""))
                .collect::<Vec<_>>()
                .join(",")
        ));
        let b: Arc<String> = Arc::new(format!(
            "{{\"who\":\"B\",\"v\":[{}]}}",
            (0..800)
                .map(|i| format!("\"bbbbbbbbbbbbbbbbbbbb-{i}\""))
                .collect::<Vec<_>>()
                .join(",")
        ));
        let torn = Arc::new(AtomicUsize::new(0));
        let iters = 1500usize;

        let mut handles = Vec::new();
        for t in 0..8 {
            let (path, a, b) = (path.clone(), a.clone(), b.clone());
            handles.push(std::thread::spawn(move || {
                for _ in 0..iters {
                    let payload = if t % 2 == 0 { &*a } else { &*b };
                    write_atomic(&path, payload.as_bytes()).expect("write_atomic");
                }
            }));
        }
        let reader = {
            let (path, a, b, torn) = (path.clone(), a.clone(), b.clone(), torn.clone());
            std::thread::spawn(move || {
                for _ in 0..(iters * 6) {
                    if let Ok(s) = std::fs::read_to_string(&*path) {
                        if !s.is_empty() && s != *a && s != *b {
                            torn.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            })
        };
        for h in handles {
            h.join().unwrap();
        }
        reader.join().unwrap();

        assert_eq!(
            torn.load(Ordering::Relaxed),
            0,
            "write_atomic produced a torn/mixed file under concurrent writers"
        );
        // And the final file is one complete, parseable payload.
        let final_contents = std::fs::read_to_string(&*path).expect("final read");
        assert!(final_contents == *a || final_contents == *b);
    }
}
