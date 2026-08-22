//! Shared fail-closed ownership for private SQLite files and WAL/SHM sidecars.
//!
//! Clipboard history, notes, Brain, and AI chats contain full private user
//! content. Their main file must be `0600` before SQLite can read or write it;
//! existing sidecars must be repaired before WAL initialization; and SQLite
//! itself must reject symbolic links when it reopens the prepared filename.

use anyhow::Context as _;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

fn sqlite_path_with_suffix(db_path: &Path, suffix: &str) -> std::path::PathBuf {
    if suffix.is_empty() {
        db_path.to_path_buf()
    } else {
        let mut path = db_path.as_os_str().to_owned();
        path.push(suffix);
        std::path::PathBuf::from(path)
    }
}

fn repair_private_sqlite_file(path: &Path, create: bool) -> std::io::Result<()> {
    let exists = crate::atomic_file::inspect_private_file(path)?;
    if !exists && !create {
        return Ok(());
    }

    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(create);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "private SQLite target must be a regular, non-symlink file",
        ));
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

/// Prepare private SQLite ownership before SQLite can expose private bytes.
/// Existing hostile sidecars fail closed before a missing primary is created.
pub(crate) fn prepare_private_sqlite(db_path: &Path) -> std::io::Result<()> {
    if let Some(parent) = db_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
        let parent_metadata = std::fs::symlink_metadata(parent)?;
        if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "private SQLite parent must be a non-symlink directory",
            ));
        }
    }

    for suffix in ["-wal", "-shm"] {
        repair_private_sqlite_file(&sqlite_path_with_suffix(db_path, suffix), false)?;
    }
    repair_private_sqlite_file(db_path, true)
}

/// Open through SQLite's own `SQLITE_OPEN_NOFOLLOW` flag after preparing a
/// private `0600` primary and inspecting both existing private sidecars.
pub(crate) fn open_private_sqlite(db_path: &Path) -> anyhow::Result<Connection> {
    prepare_private_sqlite(db_path).context("Prepare owner-only private SQLite files")?;
    let file_name = db_path
        .file_name()
        .context("Private SQLite path must name a database file")?;
    let parent = db_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    // SQLite's NOFOLLOW rejects macOS's ordinary `/var` -> `/private/var`
    // ancestor alias too. Resolve only the already-verified parent; the
    // actual private database filename still receives no-follow protection.
    let sqlite_path = parent
        .canonicalize()
        .context("Resolve verified private SQLite parent")?
        .join(file_name);
    Connection::open_with_flags(
        sqlite_path,
        OpenFlags::default() | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .context("Open private SQLite database without following symbolic links")
}

/// Recheck the main database and any sidecars materialized by WAL mode.
/// Permission failures, hostile links, and non-file targets are never ignored.
pub(crate) fn harden_sqlite_permissions(db_path: &Path) -> std::io::Result<()> {
    for suffix in ["", "-wal", "-shm"] {
        repair_private_sqlite_file(&sqlite_path_with_suffix(db_path, suffix), suffix.is_empty())?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn hardens_db_and_sidecars_to_0600() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("secret.sqlite");
        std::fs::write(&db, b"data").expect("write db");
        std::fs::write(dir.path().join("secret.sqlite-wal"), b"wal").expect("write wal");
        // -shm intentionally absent: the helper must tolerate a missing sidecar.

        // Loosen first so we can prove the helper tightens it.
        std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o644)).unwrap();

        harden_sqlite_permissions(&db).expect("private SQLite permissions must fail closed");

        let mode = std::fs::metadata(&db).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "db file should be owner-only");
        let wal_mode = std::fs::metadata(dir.path().join("secret.sqlite-wal"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(wal_mode, 0o600, "wal sidecar should be owner-only");
    }

    #[test]
    fn private_sqlite_is_owner_only_before_first_schema_or_private_content() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("private.sqlite");

        let connection = open_private_sqlite(&db).expect("open owner-only SQLite database");
        assert_eq!(
            std::fs::metadata(&db).unwrap().permissions().mode() & 0o777,
            0o600
        );
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL; \
                 CREATE TABLE private_content (body TEXT NOT NULL); \
                 INSERT INTO private_content VALUES ('private user content');",
            )
            .expect("write private content only after private preparation");
        harden_sqlite_permissions(&db).expect("verify private WAL sidecars");

        for suffix in ["", "-wal", "-shm"] {
            let path = sqlite_path_with_suffix(&db, suffix);
            if path.exists() {
                assert_eq!(
                    std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
        }
    }

    #[test]
    fn private_sqlite_repairs_legacy_permissions_before_reopening_private_rows() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("legacy.sqlite");
        {
            let connection = Connection::open(&db).expect("seed permissive legacy database");
            connection
                .execute_batch(
                    "CREATE TABLE private_content (body TEXT NOT NULL); \
                     INSERT INTO private_content VALUES ('legacy private prompt');",
                )
                .expect("seed private legacy row");
        }
        std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o644))
            .expect("seed permissive legacy database");

        let connection = open_private_sqlite(&db).expect("repair before reopening private rows");
        assert_eq!(
            std::fs::metadata(&db).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let body: String = connection
            .query_row("SELECT body FROM private_content", [], |row| row.get(0))
            .expect("read private row only after repair");
        assert_eq!(body, "legacy private prompt");
    }

    #[test]
    fn private_sqlite_rejects_primary_symlinks_without_exposing_foreign_content() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("private.sqlite");
        let foreign = dir.path().join("foreign.sqlite");
        let original = b"foreign private SQLite contents";
        std::fs::write(&foreign, original).expect("seed foreign private owner");
        std::fs::set_permissions(&foreign, std::fs::Permissions::from_mode(0o644))
            .expect("seed foreign permission canary");
        symlink(&foreign, &db).expect("plant private SQLite symlink");

        assert!(open_private_sqlite(&db).is_err());
        assert!(harden_sqlite_permissions(&db).is_err());
        assert_eq!(std::fs::read(&foreign).unwrap(), original);
        assert_eq!(
            std::fs::metadata(&foreign).unwrap().permissions().mode() & 0o777,
            0o644
        );
    }

    #[test]
    fn private_sqlite_rejects_hostile_sidecar_before_creating_primary_database() {
        use std::os::unix::fs::symlink;

        for suffix in ["-wal", "-shm"] {
            let dir = tempfile::tempdir().expect("tempdir");
            let db = dir.path().join("private.sqlite");
            let foreign = dir.path().join("foreign.txt");
            let sidecar = sqlite_path_with_suffix(&db, suffix);
            std::fs::write(&foreign, b"foreign private sidecar owner")
                .expect("seed unrelated private owner");
            symlink(&foreign, &sidecar).expect("plant hostile SQLite sidecar");

            assert!(open_private_sqlite(&db).is_err());
            assert!(
                !db.exists(),
                "hostile sidecar must prevent primary creation"
            );
            assert_eq!(
                std::fs::read(&foreign).unwrap(),
                b"foreign private sidecar owner"
            );
        }
    }

    #[test]
    fn private_sqlite_rejects_symlinked_parent_without_modifying_foreign_directory() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().expect("tempdir");
        let foreign = dir.path().join("foreign");
        let planted = dir.path().join("planted");
        std::fs::create_dir(&foreign).expect("create foreign parent");
        symlink(&foreign, &planted).expect("plant SQLite parent symlink");

        assert!(open_private_sqlite(&planted.join("private.sqlite")).is_err());
        assert!(!foreign.join("private.sqlite").exists());
    }
}
