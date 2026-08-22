//! Database worker thread
//!
//! Single-threaded SQLite access via message passing.
//! Eliminates global Mutex contention and enables proper WAL concurrency.
//!
//! This module provides infrastructure for migrating from the global
//! `Arc<Mutex<Connection>>` pattern to a dedicated DB worker thread.
//! The migration will be done incrementally - currently this is unused
//! but provides the architecture for the fix.

#![allow(dead_code)] // Infrastructure module - wired up incrementally

mod db_impl;

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::OnceLock;
use std::thread::{self, JoinHandle};
use tracing::{debug, error, info, warn};

use super::types::{ClipboardEntry, ClipboardEntryMeta, ContentType};
use db_impl::*;

/// Global sender to the DB worker thread
static DB_SENDER: OnceLock<Sender<DbRequest>> = OnceLock::new();

/// Guard to ensure worker is started only once
static WORKER_STARTED: OnceLock<JoinHandle<()>> = OnceLock::new();

/// Request types for the DB worker
pub enum DbRequest {
    /// Add or update an entry (dedup by content hash)
    AddOrTouch {
        content: String,
        content_type: ContentType,
        content_hash: String,
        text_preview: Option<String>,
        image_width: Option<u32>,
        image_height: Option<u32>,
        byte_size: usize,
        reply: SyncSender<Result<String>>,
    },
    /// Get entry content by ID
    GetContent {
        id: String,
        reply: SyncSender<Option<String>>,
    },
    /// Get entry by ID (full entry including content)
    GetEntry {
        id: String,
        reply: SyncSender<Option<ClipboardEntry>>,
    },
    /// Get paginated entry metadata (no content payload)
    GetMeta {
        limit: usize,
        offset: usize,
        reply: SyncSender<Vec<ClipboardEntryMeta>>,
    },
    /// Get paginated full entries
    GetPage {
        limit: usize,
        offset: usize,
        reply: SyncSender<Vec<ClipboardEntry>>,
    },
    /// Get total entry count
    GetCount { reply: SyncSender<usize> },
    /// Pin an entry
    Pin {
        id: String,
        reply: SyncSender<Result<()>>,
    },
    /// Unpin an entry
    Unpin {
        id: String,
        reply: SyncSender<Result<()>>,
    },
    /// Remove an entry
    Remove {
        id: String,
        reply: SyncSender<Result<()>>,
    },
    /// Clear all history
    Clear { reply: SyncSender<Result<()>> },
    /// Prune old entries (returns count deleted)
    Prune {
        cutoff_timestamp_ms: i64,
        reply: SyncSender<Result<usize>>,
    },
    /// Trim oversized text entries (returns count deleted)
    TrimOversized {
        max_len: usize,
        reply: SyncSender<Result<usize>>,
    },
    /// Update OCR text for an entry
    UpdateOcr {
        id: String,
        text: String,
        reply: SyncSender<Result<()>>,
    },
    /// Run incremental vacuum
    IncrementalVacuum { reply: SyncSender<Result<()>> },
    /// Run WAL checkpoint
    WalCheckpoint { reply: SyncSender<Result<()>> },
    /// Shutdown the worker
    Shutdown,
}

/// Get the database path (~/.scriptkit/db/clipboard-history.sqlite)
pub fn get_db_path() -> Result<PathBuf> {
    let kit_dir = PathBuf::from(shellexpand::tilde("~/.scriptkit").as_ref());
    let db_dir = kit_dir.join("db");
    if !db_dir.exists() {
        std::fs::create_dir_all(&db_dir).context("Failed to create ~/.scriptkit/db directory")?;
    }
    Ok(db_dir.join("clipboard-history.sqlite"))
}

/// Start the database worker thread
pub fn start_db_worker() -> Result<()> {
    if WORKER_STARTED.get().is_some() {
        debug!("DB worker already started");
        return Ok(());
    }

    let (tx, rx): (Sender<DbRequest>, Receiver<DbRequest>) = mpsc::channel();
    if DB_SENDER.set(tx).is_err() {
        debug!("DB sender already set");
        return Ok(());
    }

    let handle = thread::spawn(move || match init_connection() {
        Ok(conn) => db_worker_loop(conn, rx),
        Err(error) => error!(
            diagnostic_fingerprint = %crate::ai::reliability::redacted_fingerprint(
                &error.to_string()
            ),
            "Failed to initialize private clipboard DB worker connection"
        ),
    });

    let _ = WORKER_STARTED.set(handle);
    info!("DB worker thread started");
    Ok(())
}

/// Get the sender to the DB worker
pub fn get_db_sender() -> Option<&'static Sender<DbRequest>> {
    DB_SENDER.get()
}

fn init_connection() -> Result<Connection> {
    let db_path = get_db_path()?;
    init_connection_at(&db_path)
}

fn init_connection_at(db_path: &Path) -> Result<Connection> {
    let conn = crate::utils::db_permissions::open_private_sqlite(db_path)
        .context("Failed to open private clipboard-worker database")?;

    conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; \
         PRAGMA busy_timeout = 5000; PRAGMA auto_vacuum = INCREMENTAL;",
    )
    .context("Failed to set database pragmas")?;

    create_schema(&conn)?;
    run_migrations(&conn)?;
    create_indexes(&conn)?;
    crate::utils::db_permissions::harden_sqlite_permissions(db_path)
        .context("Failed to protect private clipboard-worker SQLite sidecars")?;

    info!("Database worker initialized");
    Ok(conn)
}

fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS history (
            id TEXT PRIMARY KEY, content TEXT NOT NULL, content_hash TEXT,
            content_type TEXT NOT NULL DEFAULT 'text', timestamp INTEGER NOT NULL,
            pinned INTEGER DEFAULT 0, ocr_text TEXT
        )",
        [],
    )
    .context("Failed to create history table")?;
    Ok(())
}

fn run_migrations(conn: &Connection) -> Result<()> {
    add_column_if_missing(conn, "ocr_text", "TEXT")?;
    add_column_if_missing(conn, "content_hash", "TEXT")?;
    add_column_if_missing(conn, "text_preview", "TEXT")?;
    add_column_if_missing(conn, "image_width", "INTEGER")?;
    add_column_if_missing(conn, "image_height", "INTEGER")?;
    add_column_if_missing(conn, "byte_size", "INTEGER DEFAULT 0")?;

    let needs_ts: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM history WHERE timestamp < 100000000000 AND timestamp > 0",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if needs_ts > 0 {
        conn.execute(
            "UPDATE history SET timestamp = timestamp * 1000 WHERE timestamp < 100000000000 AND timestamp > 0",
            [],
        )?;
        info!(count = needs_ts, "Migrated timestamps to milliseconds");
    }
    Ok(())
}

fn add_column_if_missing(conn: &Connection, name: &str, col_type: &str) -> Result<()> {
    let has: bool = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM pragma_table_info('history') WHERE name='{}'",
                name
            ),
            [],
            |row| row.get::<_, i32>(0),
        )
        .map(|c| c > 0)
        .unwrap_or(false);

    if !has {
        conn.execute(
            &format!("ALTER TABLE history ADD COLUMN {} {}", name, col_type),
            [],
        )?;
        info!(column = name, "Added column to history table");
    }
    Ok(())
}

fn create_indexes(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_timestamp ON history(timestamp DESC)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_pinned_timestamp ON history(pinned DESC, timestamp DESC)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_dedup ON history(content_type, content_hash)",
        [],
    )?;
    Ok(())
}

fn db_worker_loop(conn: Connection, rx: Receiver<DbRequest>) {
    info!("DB worker loop started");
    for request in rx {
        if !handle_request(&conn, request) {
            break;
        }
    }
    info!("DB worker loop ended");
}

fn handle_request(conn: &Connection, req: DbRequest) -> bool {
    match req {
        DbRequest::AddOrTouch {
            content,
            content_type,
            content_hash,
            text_preview,
            image_width,
            image_height,
            byte_size,
            reply,
        } => {
            let result = add_or_touch_impl(
                conn,
                &content,
                content_type,
                &content_hash,
                text_preview,
                image_width,
                image_height,
                byte_size,
            );
            if reply.send(result).is_err() {
                warn!("DbRequest::AddOrTouch reply dropped");
            }
        }
        DbRequest::GetContent { id, reply } => {
            if reply.send(get_content_impl(conn, &id)).is_err() {
                warn!("DbRequest::GetContent reply dropped");
            }
        }
        DbRequest::GetEntry { id, reply } => {
            if reply.send(get_entry_impl(conn, &id)).is_err() {
                warn!("DbRequest::GetEntry reply dropped");
            }
        }
        DbRequest::GetMeta {
            limit,
            offset,
            reply,
        } => {
            if reply.send(get_meta_impl(conn, limit, offset)).is_err() {
                warn!("DbRequest::GetMeta reply dropped");
            }
        }
        DbRequest::GetPage {
            limit,
            offset,
            reply,
        } => {
            if reply.send(get_page_impl(conn, limit, offset)).is_err() {
                warn!("DbRequest::GetPage reply dropped");
            }
        }
        DbRequest::GetCount { reply } => {
            if reply.send(get_count_impl(conn)).is_err() {
                warn!("DbRequest::GetCount reply dropped");
            }
        }
        DbRequest::Pin { id, reply } => {
            if reply.send(pin_impl(conn, &id)).is_err() {
                warn!("DbRequest::Pin reply dropped");
            }
        }
        DbRequest::Unpin { id, reply } => {
            if reply.send(unpin_impl(conn, &id)).is_err() {
                warn!("DbRequest::Unpin reply dropped");
            }
        }
        DbRequest::Remove { id, reply } => {
            if reply.send(remove_impl(conn, &id)).is_err() {
                warn!("DbRequest::Remove reply dropped");
            }
        }
        DbRequest::Clear { reply } => {
            if reply.send(clear_impl(conn)).is_err() {
                warn!("DbRequest::Clear reply dropped");
            }
        }
        DbRequest::Prune {
            cutoff_timestamp_ms,
            reply,
        } => {
            if reply.send(prune_impl(conn, cutoff_timestamp_ms)).is_err() {
                warn!("DbRequest::Prune reply dropped");
            }
        }
        DbRequest::TrimOversized { max_len, reply } => {
            if reply.send(trim_oversized_impl(conn, max_len)).is_err() {
                warn!("DbRequest::TrimOversized reply dropped");
            }
        }
        DbRequest::UpdateOcr { id, text, reply } => {
            if reply.send(update_ocr_impl(conn, &id, &text)).is_err() {
                warn!("DbRequest::UpdateOcr reply dropped");
            }
        }
        DbRequest::IncrementalVacuum { reply } => {
            if reply.send(vacuum_impl(conn)).is_err() {
                warn!("DbRequest::IncrementalVacuum reply dropped");
            }
        }
        DbRequest::WalCheckpoint { reply } => {
            if reply.send(checkpoint_impl(conn)).is_err() {
                warn!("DbRequest::WalCheckpoint reply dropped");
            }
        }
        DbRequest::Shutdown => {
            info!("DB worker shutdown");
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn clipboard_worker_database_and_sidecars_are_owner_only_from_initialization() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("isolated clipboard worker fixture");
        let path = directory.path().join("clipboard.sqlite");
        let connection = init_connection_at(&path).expect("initialize private clipboard worker");

        for suffix in ["", "-wal", "-shm"] {
            let mut candidate = path.as_os_str().to_owned();
            candidate.push(suffix);
            let candidate = PathBuf::from(candidate);
            if candidate.exists() {
                assert_eq!(
                    std::fs::metadata(candidate)
                        .expect("private clipboard SQLite metadata")
                        .permissions()
                        .mode()
                        & 0o777,
                    0o600
                );
            }
        }
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM history", [], |row| row
                    .get::<_, i64>(0))
                .expect("read isolated clipboard worker history"),
            0
        );
    }

    #[cfg(unix)]
    #[test]
    fn clipboard_worker_refuses_database_symlinks_before_exposing_foreign_history() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("isolated clipboard worker fixture");
        let path = directory.path().join("clipboard.sqlite");
        let foreign = directory.path().join("foreign.sqlite");
        std::fs::write(&foreign, b"foreign clipboard tokens and one-time codes")
            .expect("seed foreign clipboard owner");
        symlink(&foreign, &path).expect("plant clipboard database symlink");

        assert!(init_connection_at(&path).is_err());
        assert_eq!(
            std::fs::read(&foreign).expect("foreign clipboard owner remains untouched"),
            b"foreign clipboard tokens and one-time codes"
        );
    }

    #[test]
    fn test_db_path_format() {
        let path = get_db_path().unwrap();
        assert!(path.to_string_lossy().contains("clipboard-history.sqlite"));
    }
}
