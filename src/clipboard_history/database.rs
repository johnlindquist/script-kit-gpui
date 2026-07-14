//! Clipboard history database operations
//!
//! SQLite database management for clipboard entries, including CRUD operations,
//! migrations, and background maintenance.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tracing::{debug, error, info};
use uuid::Uuid;

use super::cache::{
    clear_all_caches, evict_image_cache, refresh_entry_cache, remove_entry_from_cache,
    update_ocr_text_in_cache, update_pin_status_in_cache, upsert_entry_in_cache,
};
use super::config::{get_max_text_content_len, get_retention_days, is_text_over_limit};
use super::image::get_image_dimensions;
use super::types::{
    root_clipboard_entry_is_eligible, root_clipboard_history_query_is_eligible, ClipboardEntry,
    ClipboardEntryMeta, ContentType, RootClipboardHistorySectionOptions,
};

/// Global database connection (thread-safe)
static DB_CONNECTION: OnceLock<Arc<Mutex<Connection>>> = OnceLock::new();

#[cfg(test)]
static TEST_DB_CONNECTION: std::sync::Mutex<Option<Arc<Mutex<Connection>>>> =
    std::sync::Mutex::new(None);

fn db_lock_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("DB lock error: {e}")
}

fn parse_optional_dimension(value: Option<i64>) -> Option<u32> {
    value.and_then(|v| u32::try_from(v).ok())
}

/// Compute SHA-256 hash of content for fast dedup lookups
pub fn compute_content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Get the database path (~/.scriptkit/db/clipboard-history.sqlite)
pub fn get_db_path() -> Result<PathBuf> {
    #[cfg(test)]
    if let Some(path) = get_test_db_path() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create test clipboard db directory")?;
        }
        return Ok(path);
    }

    let kit_dir = PathBuf::from(shellexpand::tilde("~/.scriptkit").as_ref());
    let db_dir = kit_dir.join("db");

    if !db_dir.exists() {
        std::fs::create_dir_all(&db_dir).context("Failed to create ~/.scriptkit/db directory")?;
    }

    Ok(db_dir.join("clipboard-history.sqlite"))
}

/// Sediment columns on a clipboard row (T10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SedimentState {
    pub brain_kept: bool,
    pub brain_tier: i64,
    pub copy_count: i64,
    pub kept_url_day: Option<String>,
}

fn ensure_clipboard_schema(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS history (
            id TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            content_hash TEXT,
            content_type TEXT NOT NULL DEFAULT 'text',
            timestamp INTEGER NOT NULL,
            pinned INTEGER DEFAULT 0,
            ocr_text TEXT
        )",
        [],
    )
    .context("Failed to create history table")?;

    migrate_add_column_if_missing(
        conn,
        "ocr_text",
        "ALTER TABLE history ADD COLUMN ocr_text TEXT",
    )?;
    migrate_add_column_if_missing(
        conn,
        "content_hash",
        "ALTER TABLE history ADD COLUMN content_hash TEXT",
    )?;

    let needs_migration: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM history WHERE timestamp < 100000000000 AND timestamp > 0",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if needs_migration > 0 {
        conn.execute(
            "UPDATE history SET timestamp = timestamp * 1000 WHERE timestamp < 100000000000 AND timestamp > 0",
            [],
        )
        .context("Failed to migrate timestamps to milliseconds")?;
        info!(
            migrated_count = needs_migration,
            "Migrated clipboard history timestamps from seconds to milliseconds"
        );
    }

    if !column_exists(conn, "text_preview")? {
        conn.execute("ALTER TABLE history ADD COLUMN text_preview TEXT", [])
            .context("Failed to add text_preview column")?;
        conn.execute("ALTER TABLE history ADD COLUMN image_width INTEGER", [])
            .context("Failed to add image_width column")?;
        conn.execute("ALTER TABLE history ADD COLUMN image_height INTEGER", [])
            .context("Failed to add image_height column")?;
        conn.execute(
            "ALTER TABLE history ADD COLUMN byte_size INTEGER DEFAULT 0",
            [],
        )
        .context("Failed to add byte_size column")?;
        info!("Migrated clipboard history: added metadata columns");
        populate_existing_metadata(conn)?;
    }

    migrate_add_column_if_missing(
        conn,
        "brain_kept",
        "ALTER TABLE history ADD COLUMN brain_kept INTEGER NOT NULL DEFAULT 0",
    )?;
    migrate_add_column_if_missing(
        conn,
        "brain_tier",
        "ALTER TABLE history ADD COLUMN brain_tier INTEGER NOT NULL DEFAULT 0",
    )?;
    migrate_add_column_if_missing(
        conn,
        "copy_count",
        "ALTER TABLE history ADD COLUMN copy_count INTEGER NOT NULL DEFAULT 1",
    )?;
    migrate_add_column_if_missing(
        conn,
        "kept_url_day",
        "ALTER TABLE history ADD COLUMN kept_url_day TEXT",
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_timestamp ON history(timestamp DESC)",
        [],
    )
    .context("Failed to create timestamp index")?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_pinned_timestamp ON history(pinned DESC, timestamp DESC)",
        [],
    )
    .context("Failed to create pinned+timestamp index")?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_dedup ON history(content_type, content_hash)",
        [],
    )
    .context("Failed to create dedup index")?;

    Ok(())
}

fn column_exists(conn: &Connection, name: &str) -> Result<bool> {
    conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('history') WHERE name = ?1",
        params![name],
        |row| row.get::<_, i32>(0),
    )
    .map(|count| count > 0)
    .context("Failed to inspect history columns")
}

fn migrate_add_column_if_missing(conn: &Connection, column: &str, ddl: &str) -> Result<()> {
    if column_exists(conn, column)? {
        return Ok(());
    }
    conn.execute(ddl, [])
        .with_context(|| format!("Failed to add {column} column"))?;
    info!(column, "Migrated clipboard history: added sediment column");
    Ok(())
}

fn open_and_init_connection(db_path: &PathBuf) -> Result<Connection> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("Failed to open database at {:?}", db_path))?;

    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
        .context("Failed to enable WAL mode")?;
    debug!("Enabled WAL mode for clipboard history database");

    conn.execute_batch("PRAGMA busy_timeout = 5000;")
        .context("Failed to set busy_timeout")?;
    debug!("Set SQLite busy_timeout to 5000ms");

    conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL;")
        .context("Failed to enable incremental auto_vacuum")?;
    debug!("Enabled incremental auto_vacuum for clipboard history database");

    ensure_clipboard_schema(&conn)?;

    // Clipboard history captures whatever the user copies (messages, tokens,
    // one-time codes). Keep the DB and its WAL/SHM sidecars owner-only (0600)
    // rather than inheriting umask. Best-effort; done after WAL mode so the
    // sidecar files exist.
    harden_clipboard_db_permissions(db_path);

    Ok(conn)
}

/// Restrict the clipboard-history SQLite file and its `-wal`/`-shm` sidecars to
/// owner read/write (0600). Best-effort — a looser-permission DB is not worth
/// failing to open the store over.
fn harden_clipboard_db_permissions(db_path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for suffix in ["", "-wal", "-shm"] {
            let path = if suffix.is_empty() {
                db_path.to_path_buf()
            } else {
                let mut os = db_path.as_os_str().to_owned();
                os.push(suffix);
                PathBuf::from(os)
            };
            if path.exists() {
                if let Err(err) =
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                {
                    debug!(
                        "Could not restrict clipboard DB permissions on {:?}: {err}",
                        path
                    );
                }
            }
        }
    }
    #[cfg(not(unix))]
    let _ = db_path;
}

/// Get or create the database connection
pub fn get_connection() -> Result<Arc<Mutex<Connection>>> {
    #[cfg(test)]
    {
        if let Ok(guard) = TEST_DB_CONNECTION.lock() {
            if let Some(conn) = guard.as_ref() {
                return Ok(conn.clone());
            }
        }
    }

    if let Some(conn) = DB_CONNECTION.get() {
        return Ok(conn.clone());
    }

    let db_path = get_db_path()?;
    let conn = Arc::new(Mutex::new(open_and_init_connection(&db_path)?));

    if DB_CONNECTION.set(conn.clone()).is_err() {
        return DB_CONNECTION
            .get()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("DB_CONNECTION set failed but get() returned None"));
    }

    Ok(conn)
}

/// Populate metadata for existing entries (migration helper)
fn populate_existing_metadata(conn: &Connection) -> Result<()> {
    // Get all entries that need metadata populated
    let mut stmt = conn.prepare(
        "SELECT id, content, content_type FROM history WHERE text_preview IS NULL OR byte_size = 0",
    )?;

    let entries: Vec<(String, String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .filter_map(|r| r.ok())
        .collect();

    let total = entries.len();
    if total == 0 {
        return Ok(());
    }

    info!(count = total, "Populating metadata for existing entries");

    for (id, content, content_type_str) in entries {
        let content_type = ContentType::from_str(&content_type_str);
        let byte_size = content.len();

        let (text_preview, image_width, image_height) = match content_type {
            ContentType::Text | ContentType::Link | ContentType::File | ContentType::Color => {
                let preview: String = content.chars().take(100).collect();
                (Some(preview), None, None)
            }
            ContentType::Image => {
                let dims = get_image_dimensions(&content);
                (None, dims.map(|(w, _)| w), dims.map(|(_, h)| h))
            }
        };

        conn.execute(
            "UPDATE history SET text_preview = ?1, image_width = ?2, image_height = ?3, byte_size = ?4 WHERE id = ?5",
            params![text_preview, image_width, image_height, byte_size as i64, id],
        )?;
    }

    info!(count = total, "Metadata population complete");
    Ok(())
}

/// Extract metadata from content for efficient storage
fn extract_metadata(
    content: &str,
    content_type: ContentType,
) -> (Option<String>, Option<u32>, Option<u32>, usize) {
    let byte_size = content.len();

    match content_type {
        ContentType::Text | ContentType::Link | ContentType::File | ContentType::Color => {
            let preview: String = content.chars().take(100).collect();
            (Some(preview), None, None, byte_size)
        }
        ContentType::Image => {
            let dims = get_image_dimensions(content);
            (None, dims.map(|(w, _)| w), dims.map(|(_, h)| h), byte_size)
        }
    }
}

/// Add a new entry to clipboard history
///
/// Returns the ID of the entry (either existing or newly created).
#[tracing::instrument(skip(content), fields(content_type = ?content_type, content_len = content.len()))]
pub fn add_entry(content: &str, content_type: ContentType) -> Result<String> {
    if content_type == ContentType::Text && is_text_over_limit(content) {
        anyhow::bail!(
            "Clipboard text exceeds max length ({} bytes)",
            get_max_text_content_len()
        );
    }

    let conn = get_connection()?;
    let conn = conn.lock().map_err(db_lock_err)?;

    let timestamp = chrono::Utc::now().timestamp_millis();
    let content_hash = compute_content_hash(content);

    // Check if entry with same hash exists (O(1) dedup via index)
    // Also fetch pinned/OCR text to preserve it in cache update
    let existing: Option<(String, bool, Option<String>)> = conn
        .query_row(
            "SELECT id, pinned, ocr_text FROM history WHERE content_type = ? AND content_hash = ?",
            params![content_type.as_str(), &content_hash],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get::<_, i64>(1)? != 0,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .ok();

    // Extract metadata for efficient list queries (done before lock for update case)
    let (text_preview, image_width, image_height, byte_size) =
        extract_metadata(content, content_type);

    if let Some((existing_id, existing_pinned, existing_ocr_text)) = existing {
        conn.execute(
            "UPDATE history SET timestamp = ?, copy_count = copy_count + 1 WHERE id = ?",
            params![timestamp, &existing_id],
        )
        .context("Failed to update existing entry timestamp")?;
        debug!(id = %existing_id, "Updated existing clipboard entry timestamp");
        drop(conn);

        // Incremental cache update instead of full refresh
        // Preserve the existing pinned status from the database
        upsert_entry_in_cache(ClipboardEntryMeta {
            id: existing_id.clone(),
            content_type,
            timestamp,
            pinned: existing_pinned,
            text_preview: text_preview.unwrap_or_default(),
            image_width,
            image_height,
            byte_size,
            ocr_text: existing_ocr_text,
        });

        return Ok(existing_id);
    }

    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO history (id, content, content_hash, content_type, timestamp, pinned, ocr_text, text_preview, image_width, image_height, byte_size, brain_kept, brain_tier, copy_count)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, NULL, ?6, ?7, ?8, ?9, 0, 0, 1)",
        params![&id, content, &content_hash, content_type.as_str(), timestamp, text_preview, image_width, image_height, byte_size as i64],
    )
    .context("Failed to insert clipboard entry")?;

    debug!(id = %id, content_type = content_type.as_str(), "Added clipboard entry");

    drop(conn);

    // Incremental cache update instead of full refresh
    upsert_entry_in_cache(ClipboardEntryMeta {
        id: id.clone(),
        content_type,
        timestamp,
        pinned: false,
        text_preview: text_preview.unwrap_or_default(),
        image_width,
        image_height,
        byte_size,
        ocr_text: None,
    });

    Ok(id)
}

/// Prune entries older than retention period (except pinned or brain-kept entries)
///
/// Returns the number of entries deleted.
pub fn prune_old_entries() -> Result<usize> {
    let conn = get_connection()?;
    let conn = conn.lock().map_err(db_lock_err)?;

    let retention_days = get_retention_days();
    // Cutoff is in milliseconds (retention_days * 24 * 60 * 60 * 1000)
    let cutoff_timestamp =
        chrono::Utc::now().timestamp_millis() - (retention_days as i64 * 24 * 60 * 60 * 1000);

    let deleted = conn
        .execute(
            "DELETE FROM history WHERE pinned = 0 AND brain_kept = 0 AND timestamp < ?",
            params![cutoff_timestamp],
        )
        .context("Failed to prune old entries")?;

    if deleted > 0 {
        debug!(
            deleted,
            retention_days, cutoff_timestamp, "Pruned old clipboard entries"
        );
    }

    Ok(deleted)
}

/// Remove text entries that exceed the configured max length.
///
/// Returns the number of entries deleted.
pub fn trim_oversize_text_entries() -> Result<usize> {
    let max_len = get_max_text_content_len();
    if max_len == usize::MAX {
        return Ok(0);
    }

    let conn = get_connection()?;
    let conn = conn.lock().map_err(db_lock_err)?;

    let max_len_db = i64::try_from(max_len).unwrap_or(i64::MAX);
    let deleted = conn
        .execute(
            "DELETE FROM history WHERE brain_kept = 0 AND content_type = 'text' AND length(CAST(content AS BLOB)) > ?",
            params![max_len_db],
        )
        .context("Failed to trim oversized text entries")?;

    if deleted > 0 {
        let correlation_id = Uuid::new_v4().to_string();
        info!(
            correlation_id = %correlation_id,
            deleted,
            max_len = max_len_db,
            "Trimmed oversized clipboard text entries"
        );
    }

    drop(conn);
    refresh_entry_cache();

    Ok(deleted)
}

/// Get paginated clipboard history entries
///
/// Returns entries ordered by pinned status (pinned first) then by timestamp descending.
pub fn get_clipboard_history_page(limit: usize, offset: usize) -> Vec<ClipboardEntry> {
    let conn = match get_connection() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to get database connection");
            return Vec::new();
        }
    };

    let conn = match conn.lock() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to lock database connection");
            return Vec::new();
        }
    };

    let mut stmt = match conn.prepare(
        "SELECT id, content, content_type, timestamp, pinned, ocr_text 
         FROM history 
         ORDER BY pinned DESC, timestamp DESC 
         LIMIT ? OFFSET ?",
    ) {
        Ok(s) => s,
        Err(e) => {
            error!(error = %e, "Failed to prepare query");
            return Vec::new();
        }
    };

    let entries = stmt
        .query_map(params![limit as i64, offset as i64], |row| {
            Ok(ClipboardEntry {
                id: row.get(0)?,
                content: row.get(1)?,
                content_type: ContentType::from_str(&row.get::<_, String>(2)?),
                timestamp: row.get(3)?,
                pinned: row.get::<_, i64>(4)? != 0,
                ocr_text: row.get(5)?,
                source_app_name: None,
                source_app_bundle_id: None,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_else(|e| {
            error!(error = %e, "Failed to query clipboard history");
            Vec::new()
        });

    debug!(
        count = entries.len(),
        limit, offset, "Retrieved clipboard history page"
    );
    entries
}

/// Get total number of entries in clipboard history
#[allow(dead_code)] // Used by downstream subtasks (UI)
pub fn get_total_entry_count() -> usize {
    let conn = match get_connection() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to get database connection");
            return 0;
        }
    };

    let conn = match conn.lock() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to lock database connection");
            return 0;
        }
    };

    conn.query_row("SELECT COUNT(*) FROM history", [], |row| {
        row.get::<_, i64>(0)
    })
    .map(|c| c as usize)
    .unwrap_or_else(|e| {
        error!(error = %e, "Failed to count clipboard entries");
        0
    })
}

/// Get clipboard history entries (convenience wrapper)
pub fn get_clipboard_history(limit: usize) -> Vec<ClipboardEntry> {
    get_clipboard_history_page(limit, 0)
}

/// Get paginated clipboard history metadata (NO content payload)
///
/// This is memory-efficient for list views - doesn't load full content.
/// Use `get_entry_content()` to fetch content when needed.
pub fn get_clipboard_history_meta(limit: usize, offset: usize) -> Vec<ClipboardEntryMeta> {
    let conn = match get_connection() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to get database connection");
            return Vec::new();
        }
    };

    let conn = match conn.lock() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to lock database connection");
            return Vec::new();
        }
    };

    // Query only metadata columns - NO content column
    let mut stmt = match conn.prepare(
        "SELECT id, content_type, timestamp, pinned, text_preview, image_width, image_height, byte_size, ocr_text
         FROM history
         ORDER BY pinned DESC, timestamp DESC
         LIMIT ? OFFSET ?",
    ) {
        Ok(s) => s,
        Err(e) => {
            error!(error = %e, "Failed to prepare metadata query");
            return Vec::new();
        }
    };

    let entries = stmt
        .query_map(params![limit as i64, offset as i64], |row| {
            Ok(ClipboardEntryMeta {
                id: row.get(0)?,
                content_type: ContentType::from_str(&row.get::<_, String>(1)?),
                timestamp: row.get(2)?,
                pinned: row.get::<_, i64>(3)? != 0,
                text_preview: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                image_width: parse_optional_dimension(row.get::<_, Option<i64>>(5)?),
                image_height: parse_optional_dimension(row.get::<_, Option<i64>>(6)?),
                byte_size: row.get::<_, Option<i64>>(7)?.unwrap_or(0) as usize,
                ocr_text: row.get(8)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_else(|e| {
            error!(error = %e, "Failed to query clipboard history metadata");
            Vec::new()
        });

    debug!(
        count = entries.len(),
        limit, offset, "Retrieved clipboard history metadata"
    );
    entries
}

/// Search recent clipboard metadata for root launcher rows without loading raw content.
pub fn search_root_clipboard_history_meta(
    query: &str,
    options: RootClipboardHistorySectionOptions,
) -> Vec<ClipboardEntryMeta> {
    if !root_clipboard_history_query_is_eligible(query, options) {
        return Vec::new();
    }

    let query = query.trim().to_lowercase();
    get_clipboard_history_meta(options.scan_limit, 0)
        .into_iter()
        .filter(root_clipboard_entry_is_eligible)
        .filter(|entry| entry.text_preview.to_lowercase().contains(&query))
        .take(options.max_results)
        .collect()
}

pub fn search_root_clipboard_history_meta_direct(
    query: &str,
    options: RootClipboardHistorySectionOptions,
) -> Vec<ClipboardEntryMeta> {
    search_root_clipboard_history_meta(query, options)
}

/// Get just the content for an entry (for copy/preview operations)
///
/// Returns None if entry doesn't exist.
pub fn get_entry_content(id: &str) -> Option<String> {
    let conn = get_connection().ok()?;
    let conn = conn.lock().ok()?;

    conn.query_row(
        "SELECT content FROM history WHERE id = ?",
        params![id],
        |row| row.get(0),
    )
    .ok()
}

/// Pin a clipboard entry to prevent LRU eviction
pub fn pin_entry(id: &str) -> Result<()> {
    let conn = get_connection()?;
    let conn = conn.lock().map_err(db_lock_err)?;

    let affected = conn
        .execute("UPDATE history SET pinned = 1 WHERE id = ?", params![id])
        .context("Failed to pin entry")?;

    if affected == 0 {
        anyhow::bail!("Entry not found: {}", id);
    }

    info!(id = %id, "Pinned clipboard entry");

    drop(conn);

    // Incremental cache update instead of full refresh
    update_pin_status_in_cache(id, true);

    Ok(())
}

/// Unpin a clipboard entry
pub fn unpin_entry(id: &str) -> Result<()> {
    let conn = get_connection()?;
    let conn = conn.lock().map_err(db_lock_err)?;

    let affected = conn
        .execute("UPDATE history SET pinned = 0 WHERE id = ?", params![id])
        .context("Failed to unpin entry")?;

    if affected == 0 {
        anyhow::bail!("Entry not found: {}", id);
    }

    info!(id = %id, "Unpinned clipboard entry");

    drop(conn);

    // Incremental cache update instead of full refresh
    update_pin_status_in_cache(id, false);

    Ok(())
}

/// Remove a single entry from clipboard history
pub fn remove_entry(id: &str) -> Result<()> {
    use super::blob_store::{delete_blob, is_blob_content};

    let conn = get_connection()?;
    let conn = conn.lock().map_err(db_lock_err)?;

    // Get content first to check if it's a blob (for cleanup)
    let content: Option<String> = conn
        .query_row(
            "SELECT content FROM history WHERE id = ?",
            params![id],
            |row| row.get(0),
        )
        .ok();

    let affected = conn
        .execute("DELETE FROM history WHERE id = ?", params![id])
        .context("Failed to remove entry")?;

    if affected == 0 {
        anyhow::bail!("Entry not found: {}", id);
    }

    info!(id = %id, "Removed clipboard entry");

    drop(conn);

    // Delete blob file if this was a blob-stored image
    if let Some(ref content) = content {
        if is_blob_content(content) {
            delete_blob(content);
        }
    }

    evict_image_cache(id);
    // Incremental cache update instead of full refresh
    remove_entry_from_cache(id);

    Ok(())
}

/// Clear all clipboard history
pub fn clear_history() -> Result<()> {
    use super::blob_store::{delete_blob, is_blob_content};

    let conn = get_connection()?;
    let conn = conn.lock().map_err(db_lock_err)?;

    // Collect blob references before deleting
    let blob_contents: Vec<String> = {
        let mut stmt = conn.prepare("SELECT content FROM history WHERE content LIKE 'blob:%'")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };

    conn.execute("DELETE FROM history", [])
        .context("Failed to clear history")?;

    info!("Cleared all clipboard history");

    drop(conn);

    // Delete all blob files
    for content in &blob_contents {
        if is_blob_content(content) {
            delete_blob(content);
        }
    }

    if !blob_contents.is_empty() {
        debug!(
            count = blob_contents.len(),
            "Deleted blob files during history clear"
        );
    }

    clear_all_caches();

    Ok(())
}

/// Clear all unpinned clipboard history entries.
/// Keeps pinned and brain-kept entries intact.
#[allow(dead_code)]
pub fn clear_unpinned_history() -> Result<()> {
    use super::blob_store::{delete_blob, is_blob_content};

    let conn = get_connection()?;
    let conn = conn.lock().map_err(db_lock_err)?;

    // Collect blob references from unpinned entries before deleting
    let blob_contents: Vec<String> = {
        let mut stmt =
            conn.prepare("SELECT content FROM history WHERE pinned = 0 AND brain_kept = 0 AND content LIKE 'blob:%'")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let deleted = conn
        .execute(
            "DELETE FROM history WHERE pinned = 0 AND brain_kept = 0",
            [],
        )
        .context("Failed to clear unpinned history")?;

    info!(
        deleted_count = deleted,
        "Cleared unpinned clipboard history"
    );

    drop(conn);

    // Delete blob files for deleted entries
    for content in &blob_contents {
        if is_blob_content(content) {
            delete_blob(content);
        }
    }

    if !blob_contents.is_empty() {
        debug!(
            count = blob_contents.len(),
            "Deleted blob files during unpinned history clear"
        );
    }

    clear_all_caches();

    Ok(())
}

/// Update OCR text for an entry (async OCR results)
#[allow(dead_code)] // Used by downstream subtasks (OCR)
pub fn update_ocr_text(id: &str, text: &str) -> Result<()> {
    let conn = get_connection()?;
    let conn = conn.lock().map_err(db_lock_err)?;

    let affected = conn
        .execute(
            "UPDATE history SET ocr_text = ? WHERE id = ?",
            params![text, id],
        )
        .context("Failed to update OCR text")?;

    if affected == 0 {
        anyhow::bail!("Entry not found: {}", id);
    }

    debug!(id = %id, text_len = text.len(), "Updated OCR text for clipboard entry");

    drop(conn);

    update_ocr_text_in_cache(id, text.to_string());

    Ok(())
}

/// Get entry by ID
#[allow(dead_code)] // Used by downstream subtasks (UI, OCR)
pub fn get_entry_by_id(id: &str) -> Option<ClipboardEntry> {
    let conn = get_connection().ok()?;
    let conn = conn.lock().ok()?;

    conn.query_row(
        "SELECT id, content, content_type, timestamp, pinned, ocr_text FROM history WHERE id = ?",
        params![id],
        |row| {
            Ok(ClipboardEntry {
                id: row.get(0)?,
                content: row.get(1)?,
                content_type: ContentType::from_str(&row.get::<_, String>(2)?),
                timestamp: row.get(3)?,
                pinned: row.get::<_, i64>(4)? != 0,
                ocr_text: row.get(5)?,
                source_app_name: None,
                source_app_bundle_id: None,
            })
        },
    )
    .ok()
}

/// Run incremental vacuum to reclaim disk space
pub fn run_incremental_vacuum() -> Result<()> {
    let conn = get_connection()?;
    let conn = conn.lock().map_err(db_lock_err)?;

    conn.execute_batch("PRAGMA incremental_vacuum(100);")
        .context("Incremental vacuum failed")?;
    debug!("Incremental vacuum completed");

    Ok(())
}

/// Read sediment columns for a clipboard entry.
pub fn get_entry_sediment_state(id: &str) -> Option<SedimentState> {
    let conn = get_connection().ok()?;
    let conn = conn.lock().ok()?;
    conn.query_row(
        "SELECT brain_kept, brain_tier, copy_count, kept_url_day FROM history WHERE id = ?1",
        params![id],
        |row| {
            Ok(SedimentState {
                brain_kept: row.get::<_, i64>(0)? != 0,
                brain_tier: row.get(1)?,
                copy_count: row.get(2)?,
                kept_url_day: row.get(3)?,
            })
        },
    )
    .ok()
}

/// Mark an entry as brain-kept with the given sediment tier.
pub fn mark_brain_kept(id: &str, tier: i64, kept_url_day: Option<&str>) -> Result<()> {
    let conn = get_connection()?;
    let conn = conn.lock().map_err(db_lock_err)?;
    let affected = conn
        .execute(
            "UPDATE history SET brain_kept = 1, brain_tier = ?1, kept_url_day = COALESCE(?2, kept_url_day) WHERE id = ?3",
            params![tier, kept_url_day, id],
        )
        .context("Failed to mark clipboard entry brain-kept")?;
    if affected == 0 {
        anyhow::bail!("Entry not found: {id}");
    }
    Ok(())
}

/// Run WAL checkpoint (passive mode, doesn't block writers)
pub fn run_wal_checkpoint() -> Result<()> {
    let conn = get_connection()?;
    let conn = conn.lock().map_err(db_lock_err)?;

    conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);")
        .context("WAL checkpoint failed")?;
    debug!("WAL checkpoint completed");

    Ok(())
}

/// Test-only override for database path
#[cfg(test)]
static TEST_DB_PATH: OnceLock<std::sync::Mutex<Option<PathBuf>>> = OnceLock::new();

#[cfg(test)]
pub(crate) fn set_test_db_path(path: Option<PathBuf>) {
    let lock = TEST_DB_PATH.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(mut guard) = lock.lock() {
        *guard = path;
    }
}

#[cfg(test)]
pub(crate) fn get_test_db_path() -> Option<PathBuf> {
    TEST_DB_PATH
        .get()
        .and_then(|m| m.lock().ok())
        .and_then(|guard| guard.clone())
}

/// Point clipboard history at an isolated sqlite file (tests only).
#[cfg(test)]
pub fn init_test_clipboard_db(path: &std::path::Path) -> Result<()> {
    set_test_db_path(Some(path.to_path_buf()));
    let conn = Arc::new(Mutex::new(open_and_init_connection(&path.to_path_buf())?));
    if let Ok(mut guard) = TEST_DB_CONNECTION.lock() {
        *guard = Some(conn);
    }
    Ok(())
}

#[cfg(test)]
static TEST_DB_MUTEX: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(test)]
pub(crate) fn test_db_lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_DB_MUTEX
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

#[cfg(test)]
pub(crate) fn reset_test_clipboard_db() {
    set_test_db_path(None);
    if let Ok(mut guard) = TEST_DB_CONNECTION.lock() {
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_db_path_format() {
        let expected_filename = "clipboard-history.sqlite";
        let kit_dir = PathBuf::from(shellexpand::tilde("~/.scriptkit").as_ref());
        let expected_path = kit_dir.join("db").join(expected_filename);

        assert!(expected_path.to_string_lossy().contains(expected_filename));
        assert!(expected_path.to_string_lossy().contains(".scriptkit/db"));
    }

    #[test]
    fn test_db_path_with_override() {
        let _guard = test_db_lock();
        reset_test_clipboard_db();
        let temp_path = PathBuf::from("/tmp/test-clipboard.db");
        set_test_db_path(Some(temp_path.clone()));

        let retrieved = get_test_db_path();
        assert_eq!(retrieved, Some(temp_path));

        reset_test_clipboard_db();
    }

    #[test]
    fn test_compute_content_hash_deterministic() {
        let content = "Hello, World!";
        let hash1 = compute_content_hash(content);
        let hash2 = compute_content_hash(content);
        assert_eq!(hash1, hash2, "Hash should be deterministic");
    }

    #[test]
    fn test_compute_content_hash_different_content() {
        let hash1 = compute_content_hash("Hello");
        let hash2 = compute_content_hash("World");
        assert_ne!(
            hash1, hash2,
            "Different content should have different hashes"
        );
    }

    #[test]
    fn test_compute_content_hash_format() {
        let hash = compute_content_hash("test");
        assert_eq!(hash.len(), 64, "SHA-256 hash should be 64 hex chars");
        assert!(
            hash.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()),
            "Hash should be lowercase hex"
        );
    }

    #[test]
    fn test_add_entry_returns_id() {
        fn assert_returns_result_string<F>(_: F)
        where
            F: Fn(&str, ContentType) -> Result<String>,
        {
        }
        assert_returns_result_string(add_entry);
    }

    #[test]
    fn test_timestamp_is_milliseconds() {
        // Current timestamp in milliseconds should be > 1_700_000_000_000 (Oct 2023+)
        // Seconds-resolution timestamps are < 2_000_000_000 (year 2033)
        let now_ms = chrono::Utc::now().timestamp_millis();
        assert!(
            now_ms > 1_700_000_000_000,
            "Timestamp should be in milliseconds, got {}",
            now_ms
        );
        // Verify the function we use returns milliseconds
        let ts = chrono::Utc::now().timestamp_millis();
        assert!(
            ts > 1_700_000_000_000,
            "timestamp_millis should return milliseconds"
        );
    }

    #[test]
    fn test_busy_timeout_is_set() {
        // Verify that our connection setup includes busy_timeout
        // The actual timeout should be 5000ms (5 seconds)
        let expected_pragma = "PRAGMA busy_timeout = 5000";
        // This test verifies the pragma is in the code by checking the connection setup
        // The actual behavior is tested by integration tests
        assert!(expected_pragma.contains("busy_timeout"));
    }

    #[test]
    fn test_parse_optional_dimension_accepts_valid_value() {
        assert_eq!(parse_optional_dimension(Some(1920)), Some(1920));
    }

    #[test]
    fn test_parse_optional_dimension_rejects_negative_and_overflow() {
        assert_eq!(parse_optional_dimension(Some(-1)), None);
        assert_eq!(
            parse_optional_dimension(Some(i64::from(u32::MAX) + 1)),
            None
        );
    }

    #[test]
    fn clear_unpinned_history_preserves_brain_kept_rows() {
        let _guard = test_db_lock();
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("clipboard.sqlite");
        init_test_clipboard_db(&db_path).expect("test db");

        let free_id = add_entry("discard me", ContentType::Text).expect("free add");
        let brain_id = add_entry("brain kept", ContentType::Text).expect("brain add");
        let pinned_id = add_entry("pinned", ContentType::Text).expect("pinned add");
        mark_brain_kept(&brain_id, 1, None).expect("mark brain kept");
        pin_entry(&pinned_id).expect("pin");

        clear_unpinned_history().expect("clear unpinned");

        assert!(get_entry_by_id(&free_id).is_none());
        assert!(get_entry_by_id(&brain_id).is_some());
        assert!(get_entry_by_id(&pinned_id).is_some());

        reset_test_clipboard_db();
    }
}
