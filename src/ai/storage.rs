//! AI Chat Storage Layer
//!
//! SQLite-backed persistence for AI chats with CRUD operations and FTS5 search.
//! Follows the same patterns as src/notes/storage.rs for consistency.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tracing::{debug, info};

use super::model::{Chat, ChatId, ChatSource, ImageAttachment, Message, MessageRole};

/// Global database connection for AI chats
static AI_DB: OnceLock<Arc<Mutex<Connection>>> = OnceLock::new();

fn db_lock_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("DB lock error: {e}")
}

/// Storage-level alias used by bulk chat creation APIs.
pub type ChatMessage = Message;

/// Get the path to the AI chats database (~/.scriptkit/db/ai-chats.sqlite)
fn get_ai_db_path() -> PathBuf {
    let kit_dir = crate::setup::get_kit_path();

    kit_dir.join("db").join("ai-chats.sqlite")
}

/// Initialize the AI chats database with a specific path.
///
/// This is the inner implementation shared by both production and test init.
fn init_ai_db_at(db_path: PathBuf) -> Result<()> {
    // Check if already initialized - return early (idempotent behavior)
    if AI_DB.get().is_some() {
        debug!("AI database already initialized, skipping");
        return Ok(());
    }

    // Ensure directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create AI db directory")?;
    }

    let conn = crate::utils::db_permissions::open_private_sqlite(&db_path)
        .context("Failed to open private AI chats database")?;

    // Enable WAL mode for better write performance and concurrency
    // This matches the pattern used in notes/storage.rs
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .context("Failed to enable WAL mode for AI database")?;

    // Set synchronous=NORMAL for good performance with WAL safety
    conn.execute_batch("PRAGMA synchronous=NORMAL;")
        .context("Failed to set synchronous mode for AI database")?;

    // Enable foreign keys enforcement - required for CASCADE to work
    // Without this, FOREIGN KEY constraints are parsed but not enforced!
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .context("Failed to enable foreign_keys for AI database")?;

    // Set busy timeout to avoid "database is locked" errors under concurrent access
    // 1000ms is a reasonable default for a desktop app
    conn.busy_timeout(std::time::Duration::from_millis(1000))
        .context("Failed to set busy_timeout for AI database")?;

    let safe_db_path = crate::logging::log_private_user_value(&db_path.display().to_string());
    debug!(
        db_path_bytes = safe_db_path.raw_bytes,
        db_path_sha256 = %safe_db_path.sha256,
        "AI database PRAGMAs configured: WAL, synchronous=NORMAL, foreign_keys=ON, busy_timeout=1000ms"
    );

    // Create tables
    conn.execute_batch(
        r#"
        -- Chats table
        CREATE TABLE IF NOT EXISTS chats (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL DEFAULT 'New Chat',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT,
            model_id TEXT NOT NULL,
            provider TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'ai_window'
        );

        CREATE INDEX IF NOT EXISTS idx_chats_updated_at ON chats(updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_chats_deleted_at ON chats(deleted_at);
        CREATE INDEX IF NOT EXISTS idx_chats_provider ON chats(provider);
        -- Note: idx_chats_source is created in the migration below to handle existing DBs

        -- Messages table
        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            chat_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            tokens_used INTEGER,
            FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_messages_chat_id ON messages(chat_id);
        CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages(created_at);

        -- Image attachments for multimodal messages
        CREATE TABLE IF NOT EXISTS message_images (
            message_id TEXT NOT NULL,
            idx INTEGER NOT NULL,
            data TEXT NOT NULL,
            media_type TEXT NOT NULL,
            PRIMARY KEY (message_id, idx),
            FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_message_images_message_id ON message_images(message_id);

        -- Machine-readable preflight audits for developer and agent inspection
        CREATE TABLE IF NOT EXISTS message_preparation_audits (
            correlation_id TEXT PRIMARY KEY,
            chat_id TEXT NOT NULL,
            message_id TEXT,
            decision TEXT NOT NULL,
            attempted INTEGER NOT NULL,
            resolved INTEGER NOT NULL,
            failures_count INTEGER NOT NULL,
            raw_content TEXT NOT NULL,
            authored_content TEXT NOT NULL,
            audit_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE,
            FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE SET NULL
        );

        CREATE INDEX IF NOT EXISTS idx_message_preparation_audits_chat_id_created_at
            ON message_preparation_audits(chat_id, created_at DESC);

        CREATE INDEX IF NOT EXISTS idx_message_preparation_audits_message_id
            ON message_preparation_audits(message_id);

        -- Full-text search support for chats (searches titles and message content)
        CREATE VIRTUAL TABLE IF NOT EXISTS chats_fts USING fts5(
            title,
            content='chats',
            content_rowid='rowid'
        );

        -- Full-text search for messages
        CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
            content,
            content='messages',
            content_rowid='rowid'
        );

        -- Recreate triggers atomically to avoid race conditions in concurrent test processes
        -- Using IMMEDIATE transaction to prevent concurrent DROP+CREATE conflicts
        BEGIN IMMEDIATE;

        -- Drop old triggers first (IF NOT EXISTS won't update existing triggers)
        -- This is needed to migrate from AFTER UPDATE to AFTER UPDATE OF column
        DROP TRIGGER IF EXISTS chats_ai;
        DROP TRIGGER IF EXISTS chats_ad;
        DROP TRIGGER IF EXISTS chats_au;
        DROP TRIGGER IF EXISTS messages_ai;
        DROP TRIGGER IF EXISTS messages_ad;
        DROP TRIGGER IF EXISTS messages_au;

        -- Triggers to keep chat FTS in sync
        CREATE TRIGGER chats_ai AFTER INSERT ON chats BEGIN
            INSERT INTO chats_fts(rowid, title)
            VALUES (NEW.rowid, NEW.title);
        END;

        CREATE TRIGGER chats_ad AFTER DELETE ON chats BEGIN
            INSERT INTO chats_fts(chats_fts, rowid, title)
            VALUES('delete', OLD.rowid, OLD.title);
        END;

        -- CRITICAL: Only trigger on title changes (not updated_at changes)
        -- save_message_internal updates chats.updated_at on every message,
        -- which would cause unnecessary DELETE+INSERT into chats_fts without this fix
        CREATE TRIGGER chats_au AFTER UPDATE OF title ON chats BEGIN
            INSERT INTO chats_fts(chats_fts, rowid, title)
            VALUES('delete', OLD.rowid, OLD.title);
            INSERT INTO chats_fts(rowid, title)
            VALUES (NEW.rowid, NEW.title);
        END;

        -- Triggers to keep message FTS in sync
        CREATE TRIGGER messages_ai AFTER INSERT ON messages BEGIN
            INSERT INTO messages_fts(rowid, content)
            VALUES (NEW.rowid, NEW.content);
        END;

        CREATE TRIGGER messages_ad AFTER DELETE ON messages BEGIN
            INSERT INTO messages_fts(messages_fts, rowid, content)
            VALUES('delete', OLD.rowid, OLD.content);
        END;

        -- CRITICAL: Only trigger on content changes (not tokens_used changes etc.)
        CREATE TRIGGER messages_au AFTER UPDATE OF content ON messages BEGIN
            INSERT INTO messages_fts(messages_fts, rowid, content)
            VALUES('delete', OLD.rowid, OLD.content);
            INSERT INTO messages_fts(rowid, content)
            VALUES (NEW.rowid, NEW.content);
        END;

        COMMIT;
        "#,
    )
    .context("Failed to create AI tables")?;

    // Migration: Add source column if it doesn't exist (for existing databases)
    // SQLite's ADD COLUMN is simple and doesn't require complex migrations
    let has_source: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('chats') WHERE name = 'source'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !has_source {
        conn.execute(
            "ALTER TABLE chats ADD COLUMN source TEXT NOT NULL DEFAULT 'ai_window'",
            [],
        )
        .context("Failed to add source column to chats table")?;
        info!("Migrated AI database: added source column to chats table");
    }

    // Always ensure the source index exists (handles both new and migrated DBs)
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_chats_source ON chats(source)",
        [],
    )
    .context("Failed to create source index")?;

    // SAFE-001 migration: v1/v2 audit rows persisted raw authored/model-bound
    // content. Preserve the redacted JSON compatibility record while removing
    // those legacy content-bearing columns.
    conn.execute(
        "UPDATE message_preparation_audits \
         SET raw_content = '', authored_content = '' \
         WHERE CASE \
             WHEN json_valid(audit_json) \
             THEN COALESCE(json_extract(audit_json, '$.schemaVersion'), 0) < 3 \
             ELSE 1 \
         END",
        [],
    )
    .context("Failed to redact legacy message preparation audits")?;

    info!(
        db_path_bytes = safe_db_path.raw_bytes,
        db_path_sha256 = %safe_db_path.sha256,
        "AI chats database initialized"
    );

    // ai-chats.sqlite stores full conversation history. Keep it and its
    // WAL/SHM sidecars owner-only rather than inheriting umask.
    crate::utils::db_permissions::harden_sqlite_permissions(&db_path)
        .context("Failed to protect private AI chat SQLite sidecars")?;

    // Try to set the global connection. If another thread beat us to it
    // (race condition), that's fine - just return success (idempotent).
    if AI_DB.set(Arc::new(Mutex::new(conn))).is_err() {
        debug!("AI database was initialized by another thread, using existing connection");
    }

    Ok(())
}

/// Initialize the AI chats database
///
/// This function is idempotent - calling it multiple times is safe and will
/// succeed if the database is already initialized.
pub fn init_ai_db() -> Result<()> {
    init_ai_db_at(get_ai_db_path())
}

/// Get a reference to the AI database connection
fn get_db() -> Result<Arc<Mutex<Connection>>> {
    AI_DB
        .get()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("AI database not initialized"))
}

fn insert_chat_record(conn: &Connection, chat: &Chat) -> Result<()> {
    conn.execute(
        r#"
        INSERT INTO chats (id, title, created_at, updated_at, deleted_at, model_id, provider, source)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
        params![
            chat.id.as_str(),
            chat.title,
            chat.created_at.to_rfc3339(),
            chat.updated_at.to_rfc3339(),
            chat.deleted_at.map(|dt| dt.to_rfc3339()),
            chat.model_id,
            chat.provider,
            chat.source.as_str(),
        ],
    )
    .context("Failed to create chat")?;

    Ok(())
}

fn replace_message_images(conn: &Connection, message: &Message) -> Result<()> {
    conn.execute(
        "DELETE FROM message_images WHERE message_id = ?1",
        params![message.id.as_str()],
    )
    .with_context(|| format!("Failed to clear existing images for message {}", message.id))?;

    if message.images.is_empty() {
        return Ok(());
    }

    let mut image_stmt = conn
        .prepare(
            r#"
            INSERT INTO message_images (message_id, idx, data, media_type)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .context("Failed to prepare image insert statement")?;

    for (idx, image) in message.images.iter().enumerate() {
        image_stmt
            .execute(params![
                message.id.as_str(),
                idx as i64,
                image.data.as_str(),
                image.media_type.as_str(),
            ])
            .with_context(|| {
                format!(
                    "Failed to save image idx={} for message {}",
                    idx, message.id
                )
            })?;
    }

    Ok(())
}

fn save_message_record(
    conn: &Connection,
    message: &Message,
    update_chat_timestamp: bool,
) -> Result<()> {
    conn.execute(
        r#"
        INSERT INTO messages (id, chat_id, role, content, created_at, tokens_used)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(id) DO UPDATE SET
            content = excluded.content,
            tokens_used = excluded.tokens_used
        "#,
        params![
            message.id,
            message.chat_id.as_str(),
            message.role.as_str(),
            message.content,
            message.created_at.to_rfc3339(),
            message.tokens_used,
        ],
    )
    .context("Failed to save message")?;

    replace_message_images(conn, message)?;

    if update_chat_timestamp {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE chats SET updated_at = ?2 WHERE id = ?1",
            params![message.chat_id.as_str(), now],
        )
        .context("Failed to update chat timestamp")?;
    }

    Ok(())
}

// ============================================================================
// Chat Operations
// ============================================================================

/// Create a new chat
pub fn create_chat(chat: &Chat) -> Result<()> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    insert_chat_record(&conn, chat)?;

    let safe_title = crate::logging::log_private_user_value(&chat.title);
    debug!(
        chat_id = %chat.id,
        title_bytes = safe_title.raw_bytes,
        title_sha256 = %safe_title.sha256,
        "Chat created"
    );
    Ok(())
}

/// Create a chat and all of its messages in a single SQLite transaction.
pub fn create_chat_with_messages_bulk(chat: &Chat, messages: &[ChatMessage]) -> Result<()> {
    let db = get_db()?;
    let mut conn = db.lock().map_err(db_lock_err)?;

    let tx = conn
        .transaction()
        .context("Failed to start bulk chat transaction")?;

    insert_chat_record(&tx, chat).context("Failed to create chat in bulk insert")?;

    for message in messages {
        save_message_record(&tx, message, true).with_context(|| {
            format!(
                "Failed to save message {} while bulk-creating chat {}",
                message.id, chat.id
            )
        })?;
    }

    tx.commit()
        .context("Failed to commit bulk chat transaction")?;

    let safe_title = crate::logging::log_private_user_value(&chat.title);
    debug!(
        chat_id = %chat.id,
        title_bytes = safe_title.raw_bytes,
        title_sha256 = %safe_title.sha256,
        message_count = messages.len(),
        "Chat and messages created in bulk transaction"
    );
    Ok(())
}

/// Update an existing chat
pub fn update_chat(chat: &Chat) -> Result<()> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    conn.execute(
        r#"
        UPDATE chats
        SET title = ?2, updated_at = ?3, deleted_at = ?4, model_id = ?5, provider = ?6, source = ?7
        WHERE id = ?1
        "#,
        params![
            chat.id.as_str(),
            chat.title,
            chat.updated_at.to_rfc3339(),
            chat.deleted_at.map(|dt| dt.to_rfc3339()),
            chat.model_id,
            chat.provider,
            chat.source.as_str(),
        ],
    )
    .context("Failed to update chat")?;

    debug!(chat_id = %chat.id, "Chat updated");
    Ok(())
}

/// Update chat title
pub fn update_chat_title(chat_id: &ChatId, title: &str) -> Result<()> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let now = Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE chats SET title = ?2, updated_at = ?3 WHERE id = ?1",
        params![chat_id.as_str(), title, now],
    )
    .context("Failed to update chat title")?;

    let safe_title = crate::logging::log_private_user_value(title);
    debug!(
        chat_id = %chat_id,
        title_bytes = safe_title.raw_bytes,
        title_sha256 = %safe_title.sha256,
        "Chat title updated"
    );
    Ok(())
}

/// Get a chat by ID
#[must_use = "query result should be checked"]
pub fn get_chat(id: &ChatId) -> Result<Option<Chat>> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, title, created_at, updated_at, deleted_at, model_id, provider, source
            FROM chats
            WHERE id = ?1
            "#,
        )
        .context("Failed to prepare get_chat query")?;

    let result = stmt
        .query_row(params![id.as_str()], row_to_chat)
        .optional()
        .context("Failed to get chat")?;

    Ok(result)
}

/// Get all active chats (not deleted), sorted by updated_at desc
#[must_use = "query result should be checked"]
pub fn get_all_chats() -> Result<Vec<Chat>> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, title, created_at, updated_at, deleted_at, model_id, provider, source
            FROM chats
            WHERE deleted_at IS NULL
            ORDER BY updated_at DESC
            "#,
        )
        .context("Failed to prepare get_all_chats query")?;

    let chats = stmt
        .query_map([], row_to_chat)
        .context("Failed to query chats")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect chats")?;

    debug!(count = chats.len(), "Retrieved all chats");
    Ok(chats)
}

/// Get chats in trash (soft-deleted)
#[must_use = "query result should be checked"]
pub fn get_deleted_chats() -> Result<Vec<Chat>> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, title, created_at, updated_at, deleted_at, model_id, provider, source
            FROM chats
            WHERE deleted_at IS NOT NULL
            ORDER BY deleted_at DESC
            "#,
        )
        .context("Failed to prepare get_deleted_chats query")?;

    let chats = stmt
        .query_map([], row_to_chat)
        .context("Failed to query deleted chats")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect deleted chats")?;

    debug!(count = chats.len(), "Retrieved deleted chats");
    Ok(chats)
}

/// Soft delete a chat
pub fn delete_chat(chat_id: &ChatId) -> Result<()> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let now = Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE chats SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![chat_id.as_str(), now],
    )
    .context("Failed to soft delete chat")?;

    info!(chat_id = %chat_id, "Chat soft deleted");
    Ok(())
}

/// Permanently delete a chat and all its messages
pub fn delete_chat_permanently(chat_id: &ChatId) -> Result<()> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    // Delete messages first (foreign key constraint)
    conn.execute(
        "DELETE FROM messages WHERE chat_id = ?1",
        params![chat_id.as_str()],
    )
    .context("Failed to delete chat messages")?;

    conn.execute("DELETE FROM chats WHERE id = ?1", params![chat_id.as_str()])
        .context("Failed to delete chat")?;

    info!(chat_id = %chat_id, "Chat permanently deleted");
    Ok(())
}

/// Sanitize a query string for FTS5 MATCH.
/// FTS5 has special characters that can cause parse errors.
/// Supports prefix matching: each word gets a `*` suffix so "hel wor" matches "hello world".
fn sanitize_fts_query(query: &str) -> String {
    // Strip FTS5 special characters that can cause parse errors
    let cleaned: String = query
        .chars()
        .map(|c| match c {
            '"' | '\'' | '(' | ')' | ':' | '^' | '*' | '-' => ' ',
            _ => c,
        })
        .collect();

    // Split into words, quote each word, and add prefix wildcard
    let terms: Vec<String> = cleaned
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .map(|word| format!("\"{}\"*", word))
        .collect();

    if terms.is_empty() {
        // Fallback: wrap original as phrase (shouldn't happen with non-empty input)
        let escaped = query.replace('"', "\"\"");
        return format!("\"{}\"", escaped);
    }

    // Join with implicit AND (FTS5 default)
    terms.join(" ")
}

/// Search chats by title or message content
///
/// Searches both chat titles and message content using FTS5 when possible,
/// with a fallback to LIKE queries for robustness.
#[must_use = "query result should be checked"]
pub fn search_chats(query: &str) -> Result<Vec<Chat>> {
    if query.trim().is_empty() {
        return get_all_chats();
    }

    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    // Try FTS search first, fall back to LIKE on error
    let sanitized_query = sanitize_fts_query(query);

    // Attempt FTS search with corrected aliases
    let fts_result: rusqlite::Result<Vec<Chat>> = (|| {
        let mut stmt = conn.prepare(
            r#"
            SELECT DISTINCT c.id, c.title, c.created_at, c.updated_at,
                   c.deleted_at, c.model_id, c.provider, c.source
            FROM chats c
            LEFT JOIN chats_fts fts ON c.rowid = fts.rowid
            LEFT JOIN messages m ON c.id = m.chat_id
            LEFT JOIN messages_fts mfts ON m.rowid = mfts.rowid
            WHERE c.deleted_at IS NULL
              AND (fts MATCH ?1 OR mfts MATCH ?1)
            ORDER BY c.updated_at DESC
            "#,
        )?;

        let chats = stmt
            .query_map(params![sanitized_query], row_to_chat)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(chats)
    })();

    let safe_query = crate::logging::log_private_user_value(query);
    match fts_result {
        Ok(chats) => {
            debug!(
                query_bytes = safe_query.raw_bytes,
                query_sha256 = %safe_query.sha256,
                count = chats.len(),
                method = "fts",
                "Chat search completed"
            );
            Ok(chats)
        }
        Err(e) => {
            // FTS failed (possibly due to special characters or other issues)
            // Fall back to simple LIKE search on title
            let safe_error = crate::logging::log_private_user_value(&e.to_string());
            debug!(
                error_bytes = safe_error.raw_bytes,
                error_sha256 = %safe_error.sha256,
                query_bytes = safe_query.raw_bytes,
                query_sha256 = %safe_query.sha256,
                "FTS search failed, falling back to LIKE"
            );

            let like_pattern = format!("%{}%", query);
            let mut stmt = conn
                .prepare(
                    r#"
                    SELECT DISTINCT c.id, c.title, c.created_at, c.updated_at,
                           c.deleted_at, c.model_id, c.provider, c.source
                    FROM chats c
                    LEFT JOIN messages m ON c.id = m.chat_id
                    WHERE c.deleted_at IS NULL
                      AND (c.title LIKE ?1 OR m.content LIKE ?1)
                    ORDER BY c.updated_at DESC
                    "#,
                )
                .context("Failed to prepare LIKE search query")?;

            let chats = stmt
                .query_map(params![like_pattern], row_to_chat)
                .context("Failed to execute LIKE search")?
                .collect::<Result<Vec<_>, _>>()
                .context("Failed to collect LIKE search results")?;

            debug!(
                query_bytes = safe_query.raw_bytes,
                query_sha256 = %safe_query.sha256,
                count = chats.len(),
                method = "like",
                "Chat search completed (fallback)"
            );
            Ok(chats)
        }
    }
}

/// Result of a full-text search including match context snippets.
#[derive(Debug, Clone)]
pub struct ChatSearchResult {
    pub chat: Chat,
    /// If the match was in message content (not just title), this holds a snippet
    /// of the matching message text for display in the sidebar.
    pub match_snippet: Option<String>,
    /// Whether the match was found in the title (vs message content only).
    pub matched_title: bool,
}

/// Search chats with match context snippets.
///
/// Returns `ChatSearchResult` with snippet excerpts showing where the match occurred.
/// Used by the sidebar to display contextual search results.
#[must_use = "query result should be checked"]
pub fn search_chats_with_snippets(query: &str) -> Result<Vec<ChatSearchResult>> {
    if query.trim().is_empty() {
        let chats = get_all_chats()?;
        return Ok(chats
            .into_iter()
            .map(|chat| ChatSearchResult {
                chat,
                match_snippet: None,
                matched_title: false,
            })
            .collect());
    }

    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let sanitized_query = sanitize_fts_query(query);
    let query_lower = query.trim().to_lowercase();

    // Try FTS search first
    let fts_result: rusqlite::Result<Vec<ChatSearchResult>> = (|| {
        let mut stmt = conn.prepare(
            r#"
            SELECT DISTINCT c.id, c.title, c.created_at, c.updated_at,
                   c.deleted_at, c.model_id, c.provider, c.source,
                   m.content AS match_content
            FROM chats c
            LEFT JOIN chats_fts fts ON c.rowid = fts.rowid
            LEFT JOIN messages m ON c.id = m.chat_id
            LEFT JOIN messages_fts mfts ON m.rowid = mfts.rowid
            WHERE c.deleted_at IS NULL
              AND (fts MATCH ?1 OR mfts MATCH ?1)
            ORDER BY c.updated_at DESC
            "#,
        )?;

        let results = stmt
            .query_map(params![sanitized_query], |row| {
                let chat = row_to_chat(row)?;
                let match_content: Option<String> = row.get(8)?;
                Ok((chat, match_content))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(deduplicate_search_results(results, &query_lower))
    })();

    let safe_query = crate::logging::log_private_user_value(query);
    match fts_result {
        Ok(results) => {
            info!(
                query_bytes = safe_query.raw_bytes,
                query_sha256 = %safe_query.sha256,
                count = results.len(),
                method = "fts_snippets",
                "Chat search with snippets completed"
            );
            Ok(results)
        }
        Err(e) => {
            let safe_error = crate::logging::log_private_user_value(&e.to_string());
            debug!(
                error_bytes = safe_error.raw_bytes,
                error_sha256 = %safe_error.sha256,
                query_bytes = safe_query.raw_bytes,
                query_sha256 = %safe_query.sha256,
                "FTS snippet search failed, falling back to LIKE"
            );
            let like_pattern = format!("%{}%", query);
            let mut stmt = conn
                .prepare(
                    r#"
                    SELECT DISTINCT c.id, c.title, c.created_at, c.updated_at,
                           c.deleted_at, c.model_id, c.provider, c.source,
                           m.content AS match_content
                    FROM chats c
                    LEFT JOIN messages m ON c.id = m.chat_id
                    WHERE c.deleted_at IS NULL
                      AND (c.title LIKE ?1 OR m.content LIKE ?1)
                    ORDER BY c.updated_at DESC
                    "#,
                )
                .context("Failed to prepare LIKE snippet search")?;

            let results = stmt
                .query_map(params![like_pattern], |row| {
                    let chat = row_to_chat(row)?;
                    let match_content: Option<String> = row.get(8)?;
                    Ok((chat, match_content))
                })
                .context("Failed to execute LIKE snippet search")?
                .collect::<Result<Vec<_>, _>>()
                .context("Failed to collect LIKE snippet results")?;

            let deduplicated = deduplicate_search_results(results, &query_lower);
            info!(
                query_bytes = safe_query.raw_bytes,
                query_sha256 = %safe_query.sha256,
                count = deduplicated.len(),
                method = "like_snippets",
                "Chat search with snippets completed (fallback)"
            );
            Ok(deduplicated)
        }
    }
}

/// Deduplicate search results (a chat may match multiple messages) and extract snippets.
fn deduplicate_search_results(
    results: Vec<(Chat, Option<String>)>,
    query_lower: &str,
) -> Vec<ChatSearchResult> {
    let mut seen = std::collections::HashMap::<ChatId, ChatSearchResult>::new();

    for (chat, match_content) in results {
        let chat_id = chat.id;
        let matched_title = chat.title.to_lowercase().contains(query_lower);

        let snippet = match_content
            .as_deref()
            .filter(|content| content.to_lowercase().contains(query_lower))
            .map(|content| extract_match_snippet(content, query_lower));

        seen.entry(chat_id)
            .and_modify(|existing| {
                // Prefer a snippet if we don't have one yet
                if existing.match_snippet.is_none() && snippet.is_some() {
                    existing.match_snippet.clone_from(&snippet);
                }
                if matched_title {
                    existing.matched_title = true;
                }
            })
            .or_insert(ChatSearchResult {
                chat,
                match_snippet: snippet,
                matched_title,
            });
    }

    // Preserve updated_at DESC order
    let mut results: Vec<ChatSearchResult> = seen.into_values().collect();
    results.sort_by(|a, b| b.chat.updated_at.cmp(&a.chat.updated_at));
    results
}

/// Extract a short snippet around the first occurrence of the query in content.
/// Returns up to ~80 chars centered on the match.
fn extract_match_snippet(content: &str, query_lower: &str) -> String {
    let content_lower = content.to_lowercase();
    let Some(match_pos) = content_lower.find(query_lower) else {
        // Shouldn't happen, but fallback to first 80 chars
        return content.chars().take(80).collect();
    };

    let snippet_radius = 40;
    let start = content[..match_pos]
        .char_indices()
        .rev()
        .nth(snippet_radius)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let end = content[match_pos..]
        .char_indices()
        .nth(query_lower.len() + snippet_radius)
        .map(|(i, _)| match_pos + i)
        .unwrap_or(content.len());

    let mut snippet = String::new();
    if start > 0 {
        snippet.push_str("...");
    }

    // Clean up the snippet: collapse whitespace and newlines
    let raw = &content[start..end];
    let cleaned: String = raw
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    snippet.push_str(cleaned.trim());

    if end < content.len() {
        snippet.push_str("...");
    }

    snippet
}

// ============================================================================
// Message Operations
// ============================================================================

/// Save a message
pub fn save_message(message: &Message) -> Result<()> {
    save_message_internal(message, true)
}

/// Save a message without updating the chat's updated_at timestamp.
/// Used for mock data insertion where we want to preserve historical dates.
fn save_message_without_update(message: &Message) -> Result<()> {
    save_message_internal(message, false)
}

/// Internal message save with optional chat timestamp update
///
/// Uses a transaction to ensure atomicity - either both the message insert
/// and chat timestamp update succeed, or both are rolled back.
/// This also reduces fsync overhead by committing once instead of twice.
fn save_message_internal(message: &Message, update_chat_timestamp: bool) -> Result<()> {
    let db = get_db()?;
    let mut conn = db.lock().map_err(db_lock_err)?;

    // Wrap both operations in a single transaction for:
    // 1. Atomicity: both succeed or both fail
    // 2. Performance: one fsync instead of two autocommit fsyncs
    let tx = conn.transaction().context("Failed to start transaction")?;

    save_message_record(&tx, message, update_chat_timestamp)?;

    tx.commit()
        .context("Failed to commit message transaction")?;

    debug!(
        message_id = %message.id,
        chat_id = %message.chat_id,
        role = %message.role,
        image_count = message.images.len(),
        "Message saved"
    );
    Ok(())
}

/// Delete a single message by ID
pub fn delete_message(message_id: &str) -> Result<()> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    conn.execute("DELETE FROM messages WHERE id = ?1", params![message_id])
        .context("Failed to delete message")?;

    debug!(message_id = %message_id, "Message deleted");
    Ok(())
}

/// Delete multiple messages atomically.
///
/// If any message ID is missing or cannot be deleted, the transaction is
/// rolled back and no messages are deleted.
pub fn delete_messages_batch(message_ids: &[String]) -> Result<()> {
    if message_ids.is_empty() {
        return Ok(());
    }

    let db = get_db()?;
    let mut conn = db.lock().map_err(db_lock_err)?;

    let tx = conn
        .transaction()
        .context("Failed to start batch delete transaction")?;

    let mut delete_stmt = tx
        .prepare("DELETE FROM messages WHERE id = ?1")
        .context("Failed to prepare batch message delete statement")?;

    for message_id in message_ids {
        let rows_deleted = delete_stmt
            .execute(params![message_id])
            .with_context(|| format!("Failed to delete message {} in batch", message_id))?;

        if rows_deleted != 1 {
            drop(delete_stmt);
            tx.rollback()
                .context("Failed to rollback batch delete after mismatch")?;
            return Err(anyhow::anyhow!(
                "Batch delete mismatch for message {}: expected 1 row deleted, got {}",
                message_id,
                rows_deleted
            ));
        }
    }

    drop(delete_stmt);
    tx.commit()
        .context("Failed to commit batch message delete transaction")?;

    debug!(count = message_ids.len(), "Batch messages deleted");
    Ok(())
}

/// Get all messages for a chat, ordered by creation time
#[must_use = "query result should be checked"]
pub fn get_chat_messages(chat_id: &ChatId) -> Result<Vec<Message>> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, chat_id, role, content, created_at, tokens_used
            FROM messages
            WHERE chat_id = ?1
            ORDER BY created_at ASC
            "#,
        )
        .context("Failed to prepare get_chat_messages query")?;

    let mut messages = stmt
        .query_map(params![chat_id.as_str()], row_to_message)
        .context("Failed to query messages")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect messages")?;

    populate_message_images(&conn, &mut messages)
        .with_context(|| format!("Failed to populate images for chat {}", chat_id))?;

    debug!(chat_id = %chat_id, count = messages.len(), "Retrieved chat messages");
    Ok(messages)
}

/// Get the last N messages for a chat
#[must_use = "query result should be checked"]
pub fn get_recent_messages(chat_id: &ChatId, limit: usize) -> Result<Vec<Message>> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, chat_id, role, content, created_at, tokens_used
            FROM messages
            WHERE chat_id = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )
        .context("Failed to prepare get_recent_messages query")?;

    let mut messages: Vec<Message> = stmt
        .query_map(params![chat_id.as_str(), limit as i64], row_to_message)
        .context("Failed to query recent messages")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect recent messages")?;

    // Reverse to get chronological order
    messages.reverse();

    populate_message_images(&conn, &mut messages).with_context(|| {
        format!(
            "Failed to populate images for recent messages in chat {}",
            chat_id
        )
    })?;

    Ok(messages)
}

// ============================================================================
// Row Converters
// ============================================================================

/// Populate message image attachments in-place for a batch of messages.
fn populate_message_images(conn: &Connection, messages: &mut [Message]) -> Result<()> {
    if messages.is_empty() {
        return Ok(());
    }

    // Keep below SQLite parameter limits with headroom for future query changes.
    const MESSAGE_IMAGE_QUERY_CHUNK_SIZE: usize = 900;
    let message_ids: Vec<&str> = messages.iter().map(|m| m.id.as_str()).collect();
    let mut images_by_message_id: HashMap<String, Vec<ImageAttachment>> = HashMap::new();

    for message_id_chunk in message_ids.chunks(MESSAGE_IMAGE_QUERY_CHUNK_SIZE) {
        let placeholders = vec!["?"; message_id_chunk.len()].join(", ");
        let sql = format!(
            "SELECT message_id, data, media_type
             FROM message_images
             WHERE message_id IN ({})
             ORDER BY message_id ASC, idx ASC",
            placeholders
        );

        let mut stmt = conn
            .prepare(&sql)
            .context("Failed to prepare message image query")?;

        let image_rows = stmt
            .query_map(params_from_iter(message_id_chunk.iter().copied()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ImageAttachment {
                        data: row.get(1)?,
                        media_type: row.get(2)?,
                    },
                ))
            })
            .context("Failed to query message images")?;

        for image_row in image_rows {
            let (message_id, image) = image_row.context("Failed to read message image row")?;
            images_by_message_id
                .entry(message_id)
                .or_default()
                .push(image);
        }
    }

    for message in messages {
        message.images = images_by_message_id.remove(&message.id).unwrap_or_default();
    }

    Ok(())
}

/// Convert a database row to a Chat
fn row_to_chat(row: &rusqlite::Row) -> rusqlite::Result<Chat> {
    let id_str: String = row.get(0)?;
    let title: String = row.get(1)?;
    let created_at_str: String = row.get(2)?;
    let updated_at_str: String = row.get(3)?;
    let deleted_at_str: Option<String> = row.get(4)?;
    let model_id: String = row.get(5)?;
    let provider: String = row.get(6)?;
    let source_str: String = row.get(7)?;

    let id = ChatId::parse(&id_str).unwrap_or_default();

    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let deleted_at = deleted_at_str.and_then(|s| {
        DateTime::parse_from_rfc3339(&s)
            .map(|dt| dt.with_timezone(&Utc))
            .ok()
    });

    let source = ChatSource::parse(&source_str);

    Ok(Chat {
        id,
        title,
        created_at,
        updated_at,
        deleted_at,
        model_id,
        provider,
        source,
    })
}

/// Convert a database row to a Message
fn row_to_message(row: &rusqlite::Row) -> rusqlite::Result<Message> {
    let id: String = row.get(0)?;
    let chat_id_str: String = row.get(1)?;
    let role_str: String = row.get(2)?;
    let content: String = row.get(3)?;
    let created_at_str: String = row.get(4)?;
    let tokens_used: Option<u32> = row.get(5)?;

    let chat_id = ChatId::parse(&chat_id_str).unwrap_or_default();

    let role = MessageRole::parse(&role_str).unwrap_or(MessageRole::User);

    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    Ok(Message {
        id,
        chat_id,
        role,
        content,
        created_at,
        tokens_used,
        images: Vec::new(),
    })
}

// ============================================================================
// Mock Data for Testing
// ============================================================================

/// Insert comprehensive mock chat data for visual testing.
/// Creates many chats across different time periods with long conversations
/// to verify search functionality and scrolling behavior.
#[allow(clippy::useless_vec)] // vec! is more readable for mock data definitions
pub fn insert_mock_data() -> Result<()> {
    use chrono::Duration;

    // First, ensure the database is initialized
    if AI_DB.get().is_none() {
        init_ai_db()?;
    }

    let now = Utc::now();

    // Helper to create a chat with messages
    let mut total_chats = 0;
    let mut total_messages = 0;

    // ========================================================================
    // TODAY - 5 chats
    // ========================================================================

    // Chat 1: Long conversation about Python (TODAY - scrollable)
    let chat1_id = ChatId::new();
    create_chat(&Chat {
        id: chat1_id,
        title: "Python Best Practices Discussion".to_string(),
        created_at: now - Duration::hours(2),
        updated_at: now - Duration::minutes(10),
        deleted_at: None,
        model_id: "claude-3-5-sonnet-20241022".to_string(),
        provider: "anthropic".to_string(),
        source: ChatSource::default(),
    })?;
    total_chats += 1;

    let python_messages = vec![
        (
            MessageRole::User,
            "What are some Python best practices I should follow?",
        ),
        (
            MessageRole::Assistant,
            "Great question! Here are the key Python best practices:\n\n## 1. Follow PEP 8 Style Guide\n\n```python\n# Good\ndef calculate_total(items: list[Item]) -> float:\n    return sum(item.price for item in items)\n\n# Bad\ndef calculateTotal(Items):\n    total=0\n    for i in Items:total+=i.price\n    return total\n```\n\n## 2. Use Type Hints\n\nType hints improve code readability and enable better IDE support:\n\n```python\nfrom typing import Optional, List\n\ndef find_user(user_id: int) -> Optional[User]:\n    return users.get(user_id)\n```\n\n## 3. Use Context Managers\n\n```python\nwith open('file.txt', 'r') as f:\n    content = f.read()\n```\n\nWould you like me to elaborate on any of these?",
        ),
        (MessageRole::User, "Yes, tell me more about type hints"),
        (
            MessageRole::Assistant,
            "## Deep Dive into Type Hints\n\nType hints were introduced in Python 3.5 and have become increasingly important.\n\n### Basic Types\n\n```python\nname: str = \"Alice\"\nage: int = 30\nheight: float = 5.9\nis_active: bool = True\n```\n\n### Collection Types\n\n```python\nfrom typing import List, Dict, Set, Tuple\n\nnames: List[str] = [\"Alice\", \"Bob\"]\nages: Dict[str, int] = {\"Alice\": 30}\nunique_ids: Set[int] = {1, 2, 3}\ncoordinates: Tuple[float, float] = (10.5, 20.3)\n```\n\n### Modern Syntax (Python 3.9+)\n\n```python\n# You can use built-in types directly\nnames: list[str] = [\"Alice\", \"Bob\"]\nages: dict[str, int] = {\"Alice\": 30}\n```\n\n### Optional and Union\n\n```python\nfrom typing import Optional, Union\n\ndef find_user(id: int) -> Optional[User]:\n    return None  # or User instance\n\n# Union for multiple types\ndef process(data: Union[str, bytes]) -> str:\n    if isinstance(data, bytes):\n        return data.decode()\n    return data\n```\n\n### Generic Types\n\n```python\nfrom typing import TypeVar, Generic\n\nT = TypeVar('T')\n\nclass Stack(Generic[T]):\n    def __init__(self) -> None:\n        self._items: list[T] = []\n    \n    def push(self, item: T) -> None:\n        self._items.append(item)\n    \n    def pop(self) -> T:\n        return self._items.pop()\n```",
        ),
        (MessageRole::User, "What about dataclasses?"),
        (
            MessageRole::Assistant,
            "## Python Dataclasses\n\nDataclasses reduce boilerplate for classes that primarily store data.\n\n### Basic Usage\n\n```python\nfrom dataclasses import dataclass\n\n@dataclass\nclass User:\n    name: str\n    age: int\n    email: str = \"\"\n```\n\nThis automatically generates:\n- `__init__`\n- `__repr__`\n- `__eq__`\n\n### Frozen (Immutable) Dataclasses\n\n```python\n@dataclass(frozen=True)\nclass Point:\n    x: float\n    y: float\n```\n\n### With Default Factory\n\n```python\nfrom dataclasses import dataclass, field\nfrom typing import List\n\n@dataclass\nclass Team:\n    name: str\n    members: List[str] = field(default_factory=list)\n```\n\n### Post-Init Processing\n\n```python\n@dataclass\nclass Rectangle:\n    width: float\n    height: float\n    area: float = field(init=False)\n    \n    def __post_init__(self):\n        self.area = self.width * self.height\n```",
        ),
        (
            MessageRole::User,
            "Thanks! One more question - what about async/await?",
        ),
        (
            MessageRole::Assistant,
            "## Async/Await in Python\n\nAsynchronous programming allows concurrent execution without threads.\n\n### Basic Async Function\n\n```python\nimport asyncio\n\nasync def fetch_data(url: str) -> dict:\n    # Simulate network delay\n    await asyncio.sleep(1)\n    return {\"url\": url, \"data\": \"...\"}\n```\n\n### Running Async Code\n\n```python\nasync def main():\n    result = await fetch_data(\"https://api.example.com\")\n    print(result)\n\nasyncio.run(main())\n```\n\n### Concurrent Execution\n\n```python\nasync def fetch_all(urls: list[str]) -> list[dict]:\n    tasks = [fetch_data(url) for url in urls]\n    results = await asyncio.gather(*tasks)\n    return results\n```\n\n### Async Context Managers\n\n```python\nclass AsyncDatabase:\n    async def __aenter__(self):\n        await self.connect()\n        return self\n    \n    async def __aexit__(self, *args):\n        await self.disconnect()\n\nasync def main():\n    async with AsyncDatabase() as db:\n        await db.query(\"SELECT * FROM users\")\n```\n\n### Async Iterators\n\n```python\nasync def read_lines(filename: str):\n    async with aiofiles.open(filename) as f:\n        async for line in f:\n            yield line.strip()\n\nasync def main():\n    async for line in read_lines(\"data.txt\"):\n        print(line)\n```",
        ),
    ];

    for (i, (role, content)) in python_messages.iter().enumerate() {
        save_message_without_update(&Message {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id: chat1_id,
            role: *role,
            content: content.to_string(),
            created_at: now - Duration::minutes(120 - i as i64 * 10),
            tokens_used: Some(content.len() as u32 / 4),
            images: Vec::new(),
        })?;
        total_messages += 1;
    }

    // Chat 2: Quick question (TODAY)
    let chat2_id = ChatId::new();
    create_chat(&Chat {
        id: chat2_id,
        title: "Git Rebase vs Merge".to_string(),
        created_at: now - Duration::hours(1),
        updated_at: now - Duration::minutes(30),
        deleted_at: None,
        model_id: "gpt-4o".to_string(),
        provider: "openai".to_string(),
        source: ChatSource::default(),
    })?;
    total_chats += 1;

    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat2_id,
        role: MessageRole::User,
        content: "What's the difference between git rebase and merge?".to_string(),
        created_at: now - Duration::minutes(35),
        tokens_used: Some(12),
        images: Vec::new(),
    })?;
    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat2_id,
        role: MessageRole::Assistant,
        content: "## Git Merge vs Rebase\n\n**Merge** creates a new commit that combines two branches:\n```\n  A---B---C feature\n /         \\\nD---E---F---G main (merge commit)\n```\n\n**Rebase** replays your commits on top of another branch:\n```\n          A'--B'--C' feature\n         /\nD---E---F main\n```\n\n### When to use each:\n- **Merge**: Preserves history, good for shared branches\n- **Rebase**: Cleaner history, good for local/feature branches".to_string(),
        created_at: now - Duration::minutes(34),
        tokens_used: Some(85),
            images: Vec::new(),
    })?;
    total_messages += 2;

    // Chat 3: Another today chat (searchable keyword: "typescript")
    let chat3_id = ChatId::new();
    create_chat(&Chat {
        id: chat3_id,
        title: "TypeScript Generics Tutorial".to_string(),
        created_at: now - Duration::hours(3),
        updated_at: now - Duration::hours(2),
        deleted_at: None,
        model_id: "claude-3-5-sonnet-20241022".to_string(),
        provider: "anthropic".to_string(),
        source: ChatSource::default(),
    })?;
    total_chats += 1;

    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat3_id,
        role: MessageRole::User,
        content: "Explain TypeScript generics with examples".to_string(),
        created_at: now - Duration::hours(3),
        tokens_used: Some(8),
        images: Vec::new(),
    })?;
    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat3_id,
        role: MessageRole::Assistant,
        content: "## TypeScript Generics\n\nGenerics allow you to write reusable, type-safe code.\n\n```typescript\nfunction identity<T>(arg: T): T {\n    return arg;\n}\n\nconst num = identity<number>(42);\nconst str = identity<string>(\"hello\");\n```\n\n### Generic Interfaces\n\n```typescript\ninterface Container<T> {\n    value: T;\n    getValue(): T;\n}\n```\n\n### Generic Constraints\n\n```typescript\ninterface Lengthwise {\n    length: number;\n}\n\nfunction logLength<T extends Lengthwise>(arg: T): void {\n    console.log(arg.length);\n}\n```".to_string(),
        created_at: now - Duration::hours(3) + Duration::minutes(1),
        tokens_used: Some(95),
            images: Vec::new(),
    })?;
    total_messages += 2;

    // Chat 4: Docker question (TODAY)
    let chat4_id = ChatId::new();
    create_chat(&Chat {
        id: chat4_id,
        title: "Docker Compose Setup".to_string(),
        created_at: now - Duration::hours(4),
        updated_at: now - Duration::hours(3),
        deleted_at: None,
        model_id: "gpt-4o".to_string(),
        provider: "openai".to_string(),
        source: ChatSource::default(),
    })?;
    total_chats += 1;

    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat4_id,
        role: MessageRole::User,
        content: "How do I set up Docker Compose for a Node.js app with PostgreSQL?".to_string(),
        created_at: now - Duration::hours(4),
        tokens_used: Some(15),
        images: Vec::new(),
    })?;
    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat4_id,
        role: MessageRole::Assistant,
        content: "Here's a complete Docker Compose setup:\n\n```yaml\nversion: '3.8'\n\nservices:\n  app:\n    build: .\n    ports:\n      - \"3000:3000\"\n    environment:\n      - DATABASE_URL=postgresql://user:pass@db:5432/mydb\n    depends_on:\n      - db\n\n  db:\n    image: postgres:15\n    environment:\n      - POSTGRES_USER=user\n      - POSTGRES_PASSWORD=pass\n      - POSTGRES_DB=mydb\n    volumes:\n      - postgres_data:/var/lib/postgresql/data\n\nvolumes:\n  postgres_data:\n```\n\nAnd your Dockerfile:\n\n```dockerfile\nFROM node:20-alpine\nWORKDIR /app\nCOPY package*.json ./\nRUN npm install\nCOPY . .\nEXPOSE 3000\nCMD [\"npm\", \"start\"]\n```".to_string(),
        created_at: now - Duration::hours(4) + Duration::minutes(1),
        tokens_used: Some(120),
            images: Vec::new(),
    })?;
    total_messages += 2;

    // Chat 5: React hooks (TODAY)
    let chat5_id = ChatId::new();
    create_chat(&Chat {
        id: chat5_id,
        title: "React useEffect Cleanup".to_string(),
        created_at: now - Duration::hours(5),
        updated_at: now - Duration::hours(4),
        deleted_at: None,
        model_id: "claude-3-5-sonnet-20241022".to_string(),
        provider: "anthropic".to_string(),
        source: ChatSource::default(),
    })?;
    total_chats += 1;

    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat5_id,
        role: MessageRole::User,
        content: "When do I need cleanup in useEffect?".to_string(),
        created_at: now - Duration::hours(5),
        tokens_used: Some(10),
        images: Vec::new(),
    })?;
    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat5_id,
        role: MessageRole::Assistant,
        content: "## useEffect Cleanup\n\nYou need cleanup when your effect:\n\n1. **Subscribes to events**\n```jsx\nuseEffect(() => {\n  window.addEventListener('resize', handler);\n  return () => window.removeEventListener('resize', handler);\n}, []);\n```\n\n2. **Sets up timers**\n```jsx\nuseEffect(() => {\n  const id = setInterval(tick, 1000);\n  return () => clearInterval(id);\n}, []);\n```\n\n3. **Opens connections**\n```jsx\nuseEffect(() => {\n  const ws = new WebSocket(url);\n  return () => ws.close();\n}, [url]);\n```".to_string(),
        created_at: now - Duration::hours(5) + Duration::minutes(1),
        tokens_used: Some(80),
            images: Vec::new(),
    })?;
    total_messages += 2;

    // ========================================================================
    // YESTERDAY - 4 chats
    // ========================================================================
    let yesterday = now - Duration::days(1);

    // Chat 6: API Design (YESTERDAY) - Long conversation
    let chat6_id = ChatId::new();
    create_chat(&Chat {
        id: chat6_id,
        title: "RESTful API Design Principles".to_string(),
        created_at: yesterday - Duration::hours(5),
        updated_at: yesterday,
        deleted_at: None,
        model_id: "gpt-4o".to_string(),
        provider: "openai".to_string(),
        source: ChatSource::default(),
    })?;
    total_chats += 1;

    let api_messages = vec![
        (
            MessageRole::User,
            "What are the best practices for designing a REST API?",
        ),
        (
            MessageRole::Assistant,
            "## REST API Design Best Practices\n\n### 1. Use Nouns for Resources\n```\nGET /users          # List users\nGET /users/123      # Get user 123\nPOST /users         # Create user\nPUT /users/123      # Update user 123\nDELETE /users/123   # Delete user 123\n```\n\n### 2. Use HTTP Methods Correctly\n- **GET**: Read (idempotent)\n- **POST**: Create\n- **PUT**: Full update (idempotent)\n- **PATCH**: Partial update\n- **DELETE**: Remove (idempotent)\n\n### 3. Use Proper Status Codes\n- 200 OK\n- 201 Created\n- 204 No Content\n- 400 Bad Request\n- 401 Unauthorized\n- 403 Forbidden\n- 404 Not Found\n- 500 Internal Server Error",
        ),
        (MessageRole::User, "What about pagination?"),
        (
            MessageRole::Assistant,
            "## API Pagination\n\n### Offset-Based (Simple)\n```\nGET /users?page=2&limit=20\n```\n\nResponse:\n```json\n{\n  \"data\": [...],\n  \"pagination\": {\n    \"page\": 2,\n    \"limit\": 20,\n    \"total\": 150,\n    \"pages\": 8\n  }\n}\n```\n\n### Cursor-Based (Scalable)\n```\nGET /users?cursor=abc123&limit=20\n```\n\nResponse:\n```json\n{\n  \"data\": [...],\n  \"next_cursor\": \"xyz789\",\n  \"has_more\": true\n}\n```\n\n### Link Headers (RFC 5988)\n```\nLink: </users?page=3>; rel=\"next\",\n      </users?page=1>; rel=\"prev\"",
        ),
        (MessageRole::User, "How do I handle errors?"),
        (
            MessageRole::Assistant,
            "## API Error Handling\n\n### Standard Error Response Format\n\n```json\n{\n  \"error\": {\n    \"code\": \"VALIDATION_ERROR\",\n    \"message\": \"Invalid input data\",\n    \"details\": [\n      {\n        \"field\": \"email\",\n        \"message\": \"Invalid email format\"\n      }\n    ],\n    \"request_id\": \"req_abc123\"\n  }\n}\n```\n\n### Error Codes by Category\n\n**Client Errors (4xx)**\n- 400: Validation failed\n- 401: Missing/invalid auth\n- 403: Insufficient permissions\n- 404: Resource not found\n- 409: Conflict (duplicate)\n- 422: Unprocessable entity\n- 429: Rate limited\n\n**Server Errors (5xx)**\n- 500: Internal error\n- 502: Bad gateway\n- 503: Service unavailable\n- 504: Gateway timeout",
        ),
    ];

    for (i, (role, content)) in api_messages.iter().enumerate() {
        save_message_without_update(&Message {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id: chat6_id,
            role: *role,
            content: content.to_string(),
            created_at: yesterday - Duration::hours(5) + Duration::minutes(i as i64 * 5),
            tokens_used: Some(content.len() as u32 / 4),
            images: Vec::new(),
        })?;
        total_messages += 1;
    }

    // Chat 7: SQL Query (YESTERDAY)
    let chat7_id = ChatId::new();
    create_chat(&Chat {
        id: chat7_id,
        title: "Complex SQL JOIN Query".to_string(),
        created_at: yesterday - Duration::hours(8),
        updated_at: yesterday - Duration::hours(7),
        deleted_at: None,
        model_id: "claude-3-5-sonnet-20241022".to_string(),
        provider: "anthropic".to_string(),
        source: ChatSource::default(),
    })?;
    total_chats += 1;

    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat7_id,
        role: MessageRole::User,
        content: "Help me write a SQL query to get users with their orders".to_string(),
        created_at: yesterday - Duration::hours(8),
        tokens_used: Some(15),
        images: Vec::new(),
    })?;
    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat7_id,
        role: MessageRole::Assistant,
        content: "```sql\nSELECT \n    u.id,\n    u.name,\n    u.email,\n    COUNT(o.id) as order_count,\n    COALESCE(SUM(o.total), 0) as total_spent\nFROM users u\nLEFT JOIN orders o ON u.id = o.user_id\nWHERE u.active = true\nGROUP BY u.id, u.name, u.email\nHAVING COUNT(o.id) > 0\nORDER BY total_spent DESC\nLIMIT 100;\n```".to_string(),
        created_at: yesterday - Duration::hours(8) + Duration::minutes(1),
        tokens_used: Some(65),
            images: Vec::new(),
    })?;
    total_messages += 2;

    // Chat 8: Kubernetes (YESTERDAY)
    let chat8_id = ChatId::new();
    create_chat(&Chat {
        id: chat8_id,
        title: "Kubernetes Deployment YAML".to_string(),
        created_at: yesterday - Duration::hours(10),
        updated_at: yesterday - Duration::hours(9),
        deleted_at: None,
        model_id: "gpt-4o".to_string(),
        provider: "openai".to_string(),
        source: ChatSource::default(),
    })?;
    total_chats += 1;

    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat8_id,
        role: MessageRole::User,
        content: "Create a Kubernetes deployment for a web app".to_string(),
        created_at: yesterday - Duration::hours(10),
        tokens_used: Some(12),
        images: Vec::new(),
    })?;
    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat8_id,
        role: MessageRole::Assistant,
        content: "```yaml\napiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: web-app\n  labels:\n    app: web-app\nspec:\n  replicas: 3\n  selector:\n    matchLabels:\n      app: web-app\n  template:\n    metadata:\n      labels:\n        app: web-app\n    spec:\n      containers:\n      - name: web-app\n        image: myapp:latest\n        ports:\n        - containerPort: 8080\n        resources:\n          limits:\n            memory: \"256Mi\"\n            cpu: \"500m\"\n        livenessProbe:\n          httpGet:\n            path: /health\n            port: 8080\n          initialDelaySeconds: 30\n---\napiVersion: v1\nkind: Service\nmetadata:\n  name: web-app-service\nspec:\n  selector:\n    app: web-app\n  ports:\n  - port: 80\n    targetPort: 8080\n  type: LoadBalancer\n```".to_string(),
        created_at: yesterday - Duration::hours(10) + Duration::minutes(1),
        tokens_used: Some(120),
            images: Vec::new(),
    })?;
    total_messages += 2;

    // Chat 9: CSS Grid (YESTERDAY)
    let chat9_id = ChatId::new();
    create_chat(&Chat {
        id: chat9_id,
        title: "CSS Grid Layout Examples".to_string(),
        created_at: yesterday - Duration::hours(12),
        updated_at: yesterday - Duration::hours(11),
        deleted_at: None,
        model_id: "claude-3-5-sonnet-20241022".to_string(),
        provider: "anthropic".to_string(),
        source: ChatSource::default(),
    })?;
    total_chats += 1;

    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat9_id,
        role: MessageRole::User,
        content: "Show me CSS Grid examples".to_string(),
        created_at: yesterday - Duration::hours(12),
        tokens_used: Some(8),
        images: Vec::new(),
    })?;
    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat9_id,
        role: MessageRole::Assistant,
        content: "## CSS Grid Examples\n\n### Basic Grid\n```css\n.container {\n  display: grid;\n  grid-template-columns: repeat(3, 1fr);\n  gap: 20px;\n}\n```\n\n### Named Areas\n```css\n.layout {\n  display: grid;\n  grid-template-areas:\n    \"header header header\"\n    \"sidebar main main\"\n    \"footer footer footer\";\n  grid-template-columns: 200px 1fr 1fr;\n}\n\n.header { grid-area: header; }\n.sidebar { grid-area: sidebar; }\n.main { grid-area: main; }\n```".to_string(),
        created_at: yesterday - Duration::hours(12) + Duration::minutes(1),
        tokens_used: Some(75),
            images: Vec::new(),
    })?;
    total_messages += 2;

    // ========================================================================
    // THIS WEEK - 5 chats (2-6 days ago)
    // ========================================================================

    // Chat 10: Rust Ownership (3 days ago) - Very long conversation for scrolling
    let three_days_ago = now - Duration::days(3);
    let chat10_id = ChatId::new();
    create_chat(&Chat {
        id: chat10_id,
        title: "Understanding Rust Ownership".to_string(),
        created_at: three_days_ago - Duration::hours(2),
        updated_at: three_days_ago,
        deleted_at: None,
        model_id: "claude-3-5-sonnet-20241022".to_string(),
        provider: "anthropic".to_string(),
        source: ChatSource::default(),
    })?;
    total_chats += 1;

    let rust_messages = vec![
        (MessageRole::User, "Explain Rust ownership to me"),
        (
            MessageRole::Assistant,
            "## Rust Ownership\n\nOwnership is Rust's most unique feature and enables memory safety without garbage collection.\n\n### The Three Rules\n\n1. Each value has an owner\n2. Only one owner at a time\n3. Value is dropped when owner goes out of scope\n\n```rust\nfn main() {\n    let s1 = String::from(\"hello\"); // s1 owns the string\n    let s2 = s1;                     // ownership moves to s2\n    // println!(\"{}\", s1);          // ERROR! s1 no longer valid\n    println!(\"{}\", s2);              // OK\n}\n```",
        ),
        (MessageRole::User, "What about borrowing?"),
        (
            MessageRole::Assistant,
            "## Borrowing in Rust\n\nBorrowing lets you reference data without taking ownership.\n\n### Immutable Borrows\n\n```rust\nfn calculate_length(s: &String) -> usize {\n    s.len()\n}\n\nfn main() {\n    let s = String::from(\"hello\");\n    let len = calculate_length(&s); // borrow s\n    println!(\"Length of '{}' is {}\", s, len); // s still valid\n}\n```\n\n### Mutable Borrows\n\n```rust\nfn change(s: &mut String) {\n    s.push_str(\", world\");\n}\n\nfn main() {\n    let mut s = String::from(\"hello\");\n    change(&mut s);\n    println!(\"{}\", s); // \"hello, world\"\n}\n```\n\n### Borrowing Rules\n\n1. You can have either ONE mutable reference OR any number of immutable references\n2. References must always be valid",
        ),
        (MessageRole::User, "Can you explain lifetimes?"),
        (
            MessageRole::Assistant,
            "## Rust Lifetimes\n\nLifetimes ensure references are valid for as long as they're used.\n\n### Why Lifetimes?\n\n```rust\n// This won't compile - dangling reference\nfn dangling() -> &String {\n    let s = String::from(\"hello\");\n    &s  // s goes out of scope, reference would be invalid\n}\n```\n\n### Lifetime Annotations\n\n```rust\n// 'a is a lifetime parameter\nfn longest<'a>(x: &'a str, y: &'a str) -> &'a str {\n    if x.len() > y.len() { x } else { y }\n}\n```\n\nThis says: \"The returned reference will live at least as long as the shorter of x and y.\"\n\n### Lifetime Elision\n\nRust can often infer lifetimes:\n\n```rust\n// These are equivalent\nfn first_word(s: &str) -> &str { ... }\nfn first_word<'a>(s: &'a str) -> &'a str { ... }\n```\n\n### Static Lifetime\n\n```rust\nlet s: &'static str = \"I live forever!\";\n```",
        ),
        (MessageRole::User, "What are smart pointers?"),
        (
            MessageRole::Assistant,
            "## Smart Pointers in Rust\n\n### Box<T> - Heap Allocation\n\n```rust\nlet b = Box::new(5);\nprintln!(\"b = {}\", b);\n\n// Useful for recursive types\nenum List {\n    Cons(i32, Box<List>),\n    Nil,\n}\n```\n\n### Rc<T> - Reference Counting\n\n```rust\nuse std::rc::Rc;\n\nlet a = Rc::new(5);\nlet b = Rc::clone(&a); // Increases ref count\nlet c = Rc::clone(&a);\n\nprintln!(\"count = {}\", Rc::strong_count(&a)); // 3\n```\n\n### Arc<T> - Thread-Safe Rc\n\n```rust\nuse std::sync::Arc;\nuse std::thread;\n\nlet data = Arc::new(vec![1, 2, 3]);\n\nfor _ in 0..3 {\n    let data = Arc::clone(&data);\n    thread::spawn(move || {\n        println!(\"{:?}\", data);\n    });\n}\n```\n\n### RefCell<T> - Interior Mutability\n\n```rust\nuse std::cell::RefCell;\n\nlet data = RefCell::new(5);\n\n*data.borrow_mut() += 1;\nprintln!(\"{}\", data.borrow()); // 6\n```",
        ),
        (MessageRole::User, "How do I handle errors in Rust?"),
        (
            MessageRole::Assistant,
            "## Error Handling in Rust\n\n### Result<T, E>\n\n```rust\nuse std::fs::File;\nuse std::io::{self, Read};\n\nfn read_file(path: &str) -> Result<String, io::Error> {\n    let mut file = File::open(path)?;\n    let mut contents = String::new();\n    file.read_to_string(&mut contents)?;\n    Ok(contents)\n}\n```\n\n### The ? Operator\n\nPropagates errors automatically:\n\n```rust\nfn get_user(id: u32) -> Result<User, Error> {\n    let conn = database::connect()?;\n    let user = conn.query_user(id)?;\n    Ok(user)\n}\n```\n\n### Custom Error Types\n\n```rust\n#[derive(Debug)]\nenum AppError {\n    IoError(io::Error),\n    ParseError(String),\n    NotFound,\n}\n\nimpl From<io::Error> for AppError {\n    fn from(err: io::Error) -> Self {\n        AppError::IoError(err)\n    }\n}\n```\n\n### anyhow for Applications\n\n```rust\nuse anyhow::{Context, Result};\n\nfn main() -> Result<()> {\n    let config = read_config()\n        .context(\"Failed to read config\")?;\n    Ok(())\n}\n```\n\n### thiserror for Libraries\n\n```rust\nuse thiserror::Error;\n\n#[derive(Error, Debug)]\npub enum DataError {\n    #[error(\"database error: {0}\")]\n    Database(#[from] sqlx::Error),\n    #[error(\"not found: {0}\")]\n    NotFound(String),\n}\n```",
        ),
    ];

    for (i, (role, content)) in rust_messages.iter().enumerate() {
        save_message_without_update(&Message {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id: chat10_id,
            role: *role,
            content: content.to_string(),
            created_at: three_days_ago - Duration::hours(2) + Duration::minutes(i as i64 * 8),
            tokens_used: Some(content.len() as u32 / 4),
            images: Vec::new(),
        })?;
        total_messages += 1;
    }

    // Chat 11: Authentication (4 days ago)
    let four_days_ago = now - Duration::days(4);
    let chat11_id = ChatId::new();
    create_chat(&Chat {
        id: chat11_id,
        title: "JWT Authentication Setup".to_string(),
        created_at: four_days_ago,
        updated_at: four_days_ago,
        deleted_at: None,
        model_id: "gpt-4o".to_string(),
        provider: "openai".to_string(),
        source: ChatSource::default(),
    })?;
    total_chats += 1;

    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat11_id,
        role: MessageRole::User,
        content: "How do I implement JWT authentication?".to_string(),
        created_at: four_days_ago,
        tokens_used: Some(10),
        images: Vec::new(),
    })?;
    save_message_without_update(&Message {
        id: uuid::Uuid::new_v4().to_string(),
        chat_id: chat11_id,
        role: MessageRole::Assistant,
        content: "## JWT Authentication\n\n```javascript\nconst jwt = require('jsonwebtoken');\n\n// Generate token\nconst token = jwt.sign(\n  { userId: user.id },\n  process.env.JWT_SECRET,\n  { expiresIn: '24h' }\n);\n\n// Verify token middleware\nfunction authenticate(req, res, next) {\n  const token = req.headers.authorization?.split(' ')[1];\n  try {\n    const decoded = jwt.verify(token, process.env.JWT_SECRET);\n    req.user = decoded;\n    next();\n  } catch (err) {\n    res.status(401).json({ error: 'Invalid token' });\n  }\n}\n```".to_string(),
        created_at: four_days_ago + Duration::minutes(1),
        tokens_used: Some(85),
            images: Vec::new(),
    })?;
    total_messages += 2;

    // Chat 12-14: More chats for variety
    for (i, (title, topic)) in [
        ("GraphQL Schema Design", "graphql"),
        ("WebSocket Implementation", "websocket"),
        ("CI/CD Pipeline Setup", "pipeline"),
    ]
    .iter()
    .enumerate()
    {
        let days_ago = now - Duration::days(5 + i as i64);
        let chat_id = ChatId::new();
        create_chat(&Chat {
            id: chat_id,
            title: title.to_string(),
            created_at: days_ago,
            updated_at: days_ago,
            deleted_at: None,
            model_id: if i % 2 == 0 {
                "claude-3-5-sonnet-20241022"
            } else {
                "gpt-4o"
            }
            .to_string(),
            provider: if i % 2 == 0 { "anthropic" } else { "openai" }.to_string(),
            source: ChatSource::default(),
        })?;
        total_chats += 1;

        save_message_without_update(&Message {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id,
            role: MessageRole::User,
            content: format!("Tell me about {}", topic),
            created_at: days_ago,
            tokens_used: Some(6),
            images: Vec::new(),
        })?;
        save_message_without_update(&Message {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id,
            role: MessageRole::Assistant,
            content: format!(
                "Here's an overview of {}...\n\n(This is mock content for testing)",
                topic
            ),
            created_at: days_ago + Duration::minutes(1),
            tokens_used: Some(20),
            images: Vec::new(),
        })?;
        total_messages += 2;
    }

    // ========================================================================
    // OLDER - 5+ chats (8+ days ago)
    // ========================================================================

    for (i, title) in [
        "Machine Learning Basics",
        "Database Optimization",
        "Security Best Practices",
        "Microservices Architecture",
        "Testing Strategies",
        "Performance Tuning",
        "Code Review Guidelines",
    ]
    .iter()
    .enumerate()
    {
        let days_ago = now - Duration::days(10 + i as i64 * 3);
        let chat_id = ChatId::new();
        create_chat(&Chat {
            id: chat_id,
            title: title.to_string(),
            created_at: days_ago,
            updated_at: days_ago,
            deleted_at: None,
            model_id: "claude-3-5-sonnet-20241022".to_string(),
            provider: "anthropic".to_string(),
            source: ChatSource::default(),
        })?;
        total_chats += 1;

        save_message_without_update(&Message {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id,
            role: MessageRole::User,
            content: format!("Explain {} in detail", title.to_lowercase()),
            created_at: days_ago,
            tokens_used: Some(8),
            images: Vec::new(),
        })?;
        save_message_without_update(&Message {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id,
            role: MessageRole::Assistant,
            content: format!(
                "## {}\n\nThis is a comprehensive topic...\n\n(Mock content for testing the older section)",
                title
            ),
            created_at: days_ago + Duration::minutes(1),
            tokens_used: Some(25),
            images: Vec::new(),
        })?;
        total_messages += 2;
    }

    info!(
        chat_count = total_chats,
        message_count = total_messages,
        "Comprehensive mock data inserted for AI visual testing"
    );

    Ok(())
}

// ============================================================================
// Message Preparation Audit Operations
// ============================================================================

/// Persist a privacy-safe message-preparation receipt as JSON.
/// Legacy content columns receive decimal character counts only; raw/authored
/// message bytes never cross this audit boundary.
/// Safe to call multiple times for the same correlation_id; later calls upsert.
pub fn save_message_preparation_audit(audit: &crate::ai::AiPreflightAudit) -> Result<()> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let audit_json = serde_json::to_string(audit).context("Failed to serialize preflight audit")?;

    conn.execute(
        r#"
        INSERT INTO message_preparation_audits (
            correlation_id,
            chat_id,
            message_id,
            decision,
            attempted,
            resolved,
            failures_count,
            raw_content,
            authored_content,
            audit_json,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ON CONFLICT(correlation_id) DO UPDATE SET
            message_id = excluded.message_id,
            decision = excluded.decision,
            attempted = excluded.attempted,
            resolved = excluded.resolved,
            failures_count = excluded.failures_count,
            raw_content = excluded.raw_content,
            authored_content = excluded.authored_content,
            audit_json = excluded.audit_json,
            created_at = excluded.created_at
        "#,
        params![
            audit.correlation_id.as_str(),
            audit.chat_id.as_str(),
            audit.message_id.as_deref(),
            format!("{:?}", audit.decision),
            audit.receipt.context.attempted as i64,
            audit.receipt.context.resolved as i64,
            audit.receipt.context.failed as i64,
            audit.raw_content_chars.to_string(),
            audit.authored_content_chars.to_string(),
            audit_json,
            audit.created_at.as_str(),
        ],
    )
    .context("Failed to save message preparation audit")?;

    tracing::info!(
        target: "script_kit::ai_preflight",
        correlation_id = %audit.correlation_id,
        chat_id = %audit.chat_id,
        message_id = ?audit.message_id,
        decision = ?audit.decision,
        "message_preparation_audit_saved"
    );

    Ok(())
}

/// Fetch the most recent preflight audit for a chat.
pub fn get_last_message_preparation_audit(
    chat_id: &ChatId,
) -> Result<Option<crate::ai::AiPreflightAudit>> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let audit_json = conn
        .query_row(
            r#"
            SELECT audit_json
            FROM message_preparation_audits
            WHERE chat_id = ?1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            params![chat_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("Failed to load latest message preparation audit")?;

    audit_json
        .map(|json| {
            serde_json::from_str::<crate::ai::AiPreflightAudit>(&json)
                .context("Failed to deserialize message preparation audit")
        })
        .transpose()
}

/// Clear all mock data (for test cleanup)
pub fn clear_all_chats() -> Result<()> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    conn.execute("DELETE FROM messages", [])
        .context("Failed to delete all messages")?;
    conn.execute("DELETE FROM chats", [])
        .context("Failed to delete all chats")?;

    info!("All chats and messages cleared");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Per-process temp directory for test DB isolation.
    /// All tests in this binary share one temp DB (serialized by the global Mutex),
    /// but it is separate from the production DB and from other test binaries.
    static TEST_DB_INIT: std::sync::Once = std::sync::Once::new();
    static TEST_DB_DIR: OnceLock<PathBuf> = OnceLock::new();

    fn init_test_db() {
        TEST_DB_INIT.call_once(|| {
            let dir =
                std::env::temp_dir().join(format!("script-kit-ai-test-{}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("Should create test DB directory");
            let db_path = dir.join("ai-chats-test.sqlite");
            let _ = TEST_DB_DIR.set(dir);
            init_ai_db_at(db_path).expect("Should initialize test DB");
        });
        // For threads that lost the race, just wait for init to complete (Once handles this).
    }

    #[test]
    fn test_db_path() {
        let path = get_ai_db_path();
        let expected = crate::setup::get_kit_path()
            .join("db")
            .join("ai-chats.sqlite");

        assert_eq!(path, expected);
    }

    #[test]
    fn test_init_ai_db_is_idempotent() {
        // First call via test helper sets up the temp DB
        init_test_db();

        // Subsequent calls to init_ai_db should succeed (OnceLock already set)
        let result2 = init_ai_db();
        assert!(
            result2.is_ok(),
            "init_ai_db() should be idempotent, second call failed: {:?}",
            result2.err()
        );

        // Third call for good measure
        let result3 = init_ai_db();
        assert!(
            result3.is_ok(),
            "init_ai_db() should be idempotent, third call failed: {:?}",
            result3.err()
        );
    }

    #[test]
    fn test_search_chats_does_not_error() {
        // Ensure DB is initialized
        init_test_db();

        // Empty search should return all chats (not error)
        let result = search_chats("");
        assert!(
            result.is_ok(),
            "Empty search should not error: {:?}",
            result.err()
        );

        // Simple text search should not error (even if no results)
        let result = search_chats("test");
        assert!(
            result.is_ok(),
            "Simple text search should not error: {:?}",
            result.err()
        );

        // Search with special characters should not crash
        // (FTS MATCH is fragile with special characters - should fall back gracefully)
        let result = search_chats("test@example.com");
        assert!(
            result.is_ok(),
            "Search with @ should not error: {:?}",
            result.err()
        );

        let result = search_chats("foo*bar");
        assert!(
            result.is_ok(),
            "Search with * should not error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_fts_triggers_use_update_of_column() {
        // Ensure DB is initialized
        init_test_db();

        let db = get_db().expect("Should get db connection");
        let conn = db.lock().expect("Should lock connection");

        // Query the trigger SQL to verify it uses "UPDATE OF" syntax
        let chat_trigger_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='trigger' AND name='chats_au'",
                [],
                |row| row.get(0),
            )
            .expect("Should find chats_au trigger");

        // The trigger should only fire on UPDATE OF title, not on all updates
        assert!(
            chat_trigger_sql.to_lowercase().contains("update of title"),
            "chats_au trigger should use 'UPDATE OF title' to avoid FTS churn on updated_at changes. Got: {}",
            chat_trigger_sql
        );

        let message_trigger_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='trigger' AND name='messages_au'",
                [],
                |row| row.get(0),
            )
            .expect("Should find messages_au trigger");

        // The trigger should only fire on UPDATE OF content, not on all updates
        assert!(
            message_trigger_sql
                .to_lowercase()
                .contains("update of content"),
            "messages_au trigger should use 'UPDATE OF content' to avoid FTS churn. Got: {}",
            message_trigger_sql
        );
    }

    #[test]
    fn test_ai_db_has_required_pragmas() {
        // Ensure DB is initialized
        init_test_db();

        let db = get_db().expect("Should get db connection");
        let conn = db.lock().expect("Should lock connection");

        // Verify WAL mode is enabled
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("Should query journal_mode");
        assert_eq!(
            journal_mode.to_lowercase(),
            "wal",
            "AI DB should use WAL mode for better concurrency"
        );

        // Verify foreign keys are enabled
        let foreign_keys: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("Should query foreign_keys");
        assert_eq!(
            foreign_keys, 1,
            "AI DB should have foreign_keys=ON for CASCADE to work"
        );

        // Verify busy_timeout is set (should be > 0)
        let busy_timeout: i32 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .expect("Should query busy_timeout");
        assert!(
            busy_timeout >= 1000,
            "AI DB should have busy_timeout >= 1000ms, got {}",
            busy_timeout
        );
    }

    #[test]
    fn test_save_message_persists_images_and_getters_populate_them() {
        init_test_db();

        let chat = Chat::new("test-model-images", "test-provider-images");
        create_chat(&chat).expect("Should create chat");

        let mut message = Message::user(chat.id, "user message with image attachments");
        message.images = vec![
            ImageAttachment::png("base64-image-1".to_string()),
            ImageAttachment::jpeg("base64-image-2".to_string()),
        ];

        save_message(&message).expect("Should save message with images");

        let all_messages = get_chat_messages(&chat.id).expect("Should fetch chat messages");
        let stored_message = all_messages
            .iter()
            .find(|m| m.id == message.id)
            .expect("Saved message should exist in full chat query");

        assert_eq!(stored_message.images.len(), 2);
        assert_eq!(stored_message.images[0].data, "base64-image-1");
        assert_eq!(stored_message.images[0].media_type, "image/png");
        assert_eq!(stored_message.images[1].data, "base64-image-2");
        assert_eq!(stored_message.images[1].media_type, "image/jpeg");

        let recent_messages =
            get_recent_messages(&chat.id, 1).expect("Should fetch recent message");
        assert_eq!(recent_messages.len(), 1);
        assert_eq!(recent_messages[0].id, message.id);
        assert_eq!(recent_messages[0].images.len(), 2);

        delete_chat_permanently(&chat.id).expect("Should cleanup test chat");
    }

    #[test]
    fn test_save_message_replaces_existing_images_on_upsert() {
        init_test_db();

        let chat = Chat::new("test-model-image-upsert", "test-provider-image-upsert");
        create_chat(&chat).expect("Should create chat");

        let mut message = Message::user(chat.id, "first revision");
        message.images = vec![ImageAttachment::png("stale-base64".to_string())];
        save_message(&message).expect("Should save initial message image");

        message.content = "second revision".to_string();
        message.images = vec![
            ImageAttachment::jpeg("fresh-base64-1".to_string()),
            ImageAttachment::png("fresh-base64-2".to_string()),
        ];
        save_message(&message).expect("Should replace message image attachments");

        let stored_messages = get_chat_messages(&chat.id).expect("Should read back chat messages");
        let stored = stored_messages
            .iter()
            .find(|m| m.id == message.id)
            .expect("Updated message should exist");

        assert_eq!(stored.images.len(), 2);
        assert_eq!(stored.images[0].data, "fresh-base64-1");
        assert_eq!(stored.images[0].media_type, "image/jpeg");
        assert_eq!(stored.images[1].data, "fresh-base64-2");
        assert_eq!(stored.images[1].media_type, "image/png");

        delete_chat_permanently(&chat.id).expect("Should cleanup test chat");
    }

    #[test]
    fn test_delete_messages_batch_rolls_back_when_any_message_missing() {
        init_test_db();

        let chat = Chat::new("test-model-batch-delete", "test-provider-batch-delete");
        create_chat(&chat).expect("Should create chat");

        let mut first = Message::user(chat.id, "first");
        first.images = vec![ImageAttachment::png("rollback-image".to_string())];
        let second = Message::assistant(chat.id, "second");

        save_message(&first).expect("Should save first message");
        save_message(&second).expect("Should save second message");

        let missing_id = format!("missing-{}", ChatId::new());
        let failed_delete =
            delete_messages_batch(&[first.id.clone(), missing_id.clone(), second.id.clone()]);
        assert!(
            failed_delete.is_err(),
            "Batch delete should fail when any message id is missing"
        );

        let still_present = get_chat_messages(&chat.id).expect("Should read chat after rollback");
        assert!(
            still_present.iter().any(|m| m.id == first.id),
            "Rollback should preserve first message when batch delete mismatches"
        );
        assert!(
            still_present.iter().any(|m| m.id == second.id),
            "Rollback should preserve second message when batch delete mismatches"
        );
        let first_after_rollback = still_present
            .iter()
            .find(|m| m.id == first.id)
            .expect("First message should be present after rollback");
        assert_eq!(
            first_after_rollback.images.len(),
            1,
            "Rollback should also preserve image attachments"
        );

        delete_messages_batch(&[first.id.clone(), second.id.clone()])
            .expect("Batch delete should succeed when all ids exist");
        let after_success =
            get_chat_messages(&chat.id).expect("Should read chat after successful delete");
        assert!(
            after_success.is_empty(),
            "All messages should be removed after successful batch delete"
        );

        delete_chat_permanently(&chat.id).expect("Should cleanup test chat");
    }

    #[test]
    fn test_create_chat_with_messages_bulk_persists_chat_and_messages() {
        init_test_db();

        let mut chat = Chat::new("test-model-bulk-create", "test-provider-bulk-create");
        chat.title = "Bulk create test chat".to_string();

        let mut first = Message::user(chat.id, "bulk user message").with_tokens(11);
        first.images = vec![ImageAttachment::png("bulk-image-1".to_string())];
        let second = Message::assistant(chat.id, "bulk assistant message").with_tokens(17);

        create_chat_with_messages_bulk(&chat, &[first.clone(), second.clone()])
            .expect("Should create chat and messages in a single transaction");

        let stored_chat = get_chat(&chat.id)
            .expect("Should query stored chat")
            .expect("Bulk-created chat should exist");
        assert_eq!(stored_chat.id, chat.id);
        assert_eq!(stored_chat.title, chat.title);
        assert_eq!(stored_chat.model_id, chat.model_id);
        assert_eq!(stored_chat.provider, chat.provider);

        let stored_messages = get_chat_messages(&chat.id).expect("Should fetch stored messages");
        assert_eq!(
            stored_messages.len(),
            2,
            "Should persist both bulk messages"
        );

        let stored_first = stored_messages
            .iter()
            .find(|message| message.id == first.id)
            .expect("First bulk message should exist");
        assert_eq!(stored_first.content, first.content);
        assert_eq!(stored_first.tokens_used, first.tokens_used);
        assert_eq!(stored_first.images.len(), 1);
        assert_eq!(stored_first.images[0].data, "bulk-image-1");
        assert_eq!(stored_first.images[0].media_type, "image/png");

        let stored_second = stored_messages
            .iter()
            .find(|message| message.id == second.id)
            .expect("Second bulk message should exist");
        assert_eq!(stored_second.content, second.content);
        assert_eq!(stored_second.tokens_used, second.tokens_used);
        assert!(
            stored_second.images.is_empty(),
            "Second bulk message should not have images"
        );

        delete_chat_permanently(&chat.id).expect("Should cleanup test chat");
    }

    #[test]
    fn test_create_chat_with_messages_bulk_rolls_back_when_message_insert_fails() {
        init_test_db();

        let chat = Chat::new("test-model-bulk-rollback", "test-provider-bulk-rollback");
        let first = Message::user(chat.id, "first bulk message");
        let failing = Message::assistant(ChatId::new(), "message with missing parent chat");

        let result = create_chat_with_messages_bulk(&chat, &[first, failing]);
        assert!(
            result.is_err(),
            "Bulk create should fail when one message insert violates FK constraints"
        );

        let stored_chat = get_chat(&chat.id).expect("Should query bulk-created chat");
        assert!(
            stored_chat.is_none(),
            "Bulk transaction should rollback the chat insert when a message insert fails"
        );

        let stored_messages =
            get_chat_messages(&chat.id).expect("Should query messages after rollback");
        assert!(
            stored_messages.is_empty(),
            "Bulk transaction should rollback prior message inserts on failure"
        );
    }

    #[test]
    fn test_sanitize_fts_query_supports_prefix_matching() {
        // Single word gets quoted + wildcard
        let result = sanitize_fts_query("hel");
        assert_eq!(result, "\"hel\"*");

        // Multiple words each get prefix wildcards
        let result = sanitize_fts_query("hello wor");
        assert_eq!(result, "\"hello\"* \"wor\"*");

        // Special chars are stripped
        let result = sanitize_fts_query("test:query");
        assert!(result.contains("test"));
        assert!(result.contains("query"));
        assert!(!result.contains(':'));
    }

    #[test]
    fn test_extract_match_snippet_centers_on_match() {
        let content =
            "The quick brown fox jumps over the lazy dog and then keeps running across the meadow";
        let snippet = extract_match_snippet(content, "fox");
        assert!(
            snippet.contains("fox"),
            "Snippet should contain the match: {}",
            snippet
        );
        assert!(
            snippet.len() <= 90,
            "Snippet should be bounded: len={}",
            snippet.len()
        );
    }

    #[test]
    fn test_extract_match_snippet_adds_ellipsis_when_truncated() {
        let content = "A".repeat(20) + " MATCH " + &"B".repeat(200);
        let snippet = extract_match_snippet(&content, "match");
        assert!(snippet.contains("MATCH"), "Snippet should contain match");
        assert!(
            snippet.ends_with("..."),
            "Should have trailing ellipsis when truncated"
        );
    }

    #[test]
    fn test_search_chats_with_snippets_does_not_error() {
        init_test_db();

        // Empty search should return all chats
        let result = search_chats_with_snippets("");
        assert!(
            result.is_ok(),
            "Empty search should not error: {:?}",
            result.err()
        );

        // Simple text search should not error
        let result = search_chats_with_snippets("test");
        assert!(
            result.is_ok(),
            "Text search should not error: {:?}",
            result.err()
        );

        // Special characters should fall back gracefully
        let result = search_chats_with_snippets("test@example.com");
        assert!(
            result.is_ok(),
            "Special char search should not error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_search_chats_with_snippets_returns_match_context() {
        init_test_db();

        // Create a test chat with a message containing a unique keyword
        let chat = Chat::new("test-model-snippet", "test-provider-snippet");
        let chat_id = chat.id;
        create_chat(&chat).expect("Should create chat");

        let unique_keyword = "xyzzyplugh42";
        save_message(&Message {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id,
            role: MessageRole::User,
            content: format!("Tell me about {}", unique_keyword),
            created_at: chrono::Utc::now(),
            tokens_used: None,
            images: Vec::new(),
        })
        .expect("Should save message");

        let results = search_chats_with_snippets(unique_keyword).expect("Search should succeed");

        // Should find the chat
        assert!(!results.is_empty(), "Should find chat with unique keyword");
        let found = results.iter().find(|r| r.chat.id == chat_id);
        assert!(found.is_some(), "Should find our specific chat");

        let result = found.expect("already checked");
        // Should have a snippet containing the keyword
        assert!(
            result.match_snippet.is_some(),
            "Should have a match snippet for message content match"
        );
        let snippet = result.match_snippet.as_deref().unwrap_or("");
        assert!(
            snippet.to_lowercase().contains(unique_keyword),
            "Snippet should contain the keyword: got '{}'",
            snippet
        );

        // Cleanup
        delete_chat_permanently(&chat_id).expect("Should cleanup");
    }

    #[test]
    fn test_message_preparation_audit_round_trip() {
        use crate::ai::message_parts::{
            prepare_user_message, AiContextPart, ContextPreparationItem, PreparedMessageDecision,
        };
        use crate::ai::preflight_audit::AiPreflightAudit;

        init_test_db();

        // Create a parent chat so the FK is satisfied
        let chat_id = ChatId::new();
        let chat = Chat {
            id: chat_id,
            title: "Audit round-trip test".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
            model_id: "test-model".to_string(),
            provider: "test".to_string(),
            source: ChatSource::AiWindow,
        };
        create_chat(&chat).expect("Should create parent chat");

        let prepared = prepare_user_message(
            "Summarize this page",
            &[ContextPreparationItem::primary(AiContextPart::FilePath {
                path: "/missing/PATH_CANARY.txt".to_string(),
                label: "Missing attachment".to_string(),
            })],
            &[],
            &[],
        );
        assert_eq!(prepared.decision, PreparedMessageDecision::Blocked);
        let mut audit = AiPreflightAudit::new(
            &chat_id,
            "Summarize this page",
            "Summarize this page",
            false,
            true,
            prepared.receipt,
        );
        audit.correlation_id = format!("test-roundtrip-{}", std::process::id());
        audit.preflight_generation = 1;

        // Save
        save_message_preparation_audit(&audit).expect("Should save audit");
        {
            let db = get_db().expect("database");
            let conn = db.lock().expect("database lock");
            let persisted: (String, String) = conn
                .query_row(
                    "SELECT raw_content, authored_content FROM message_preparation_audits WHERE correlation_id = ?1",
                    params![audit.correlation_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("read redacted compatibility columns");
            assert_eq!(persisted, ("19".to_string(), "19".to_string()));
        }

        // Load back
        let loaded = get_last_message_preparation_audit(&chat_id)
            .expect("Should query audit")
            .expect("Should find saved audit");

        assert_eq!(loaded.schema_version, audit.schema_version);
        assert_eq!(loaded.correlation_id, audit.correlation_id);
        assert_eq!(loaded.chat_id, audit.chat_id);
        assert_eq!(loaded.decision, audit.decision);
        assert_eq!(loaded.receipt.context.attempted, 1);
        assert_eq!(loaded.receipt.context.resolved, 0);
        assert_eq!(loaded.receipt.context.failed, 1);
        assert_eq!(loaded.actionable_failures.len(), 1);
        assert_eq!(loaded.actionable_failures[0].code, "attachment_unavailable");
        let serialized = serde_json::to_string(&loaded).expect("serialize loaded receipt");
        assert!(!serialized.contains("PATH_CANARY"));
        assert!(!serialized.contains("Summarize this page"));
        assert_eq!(loaded.message_id, None);

        // Create a real message so the FK constraint is satisfied on upsert
        let msg = Message {
            id: "msg-audit-rt".to_string(),
            chat_id,
            role: MessageRole::User,
            content: "Summarize this page".to_string(),
            created_at: chrono::Utc::now(),
            tokens_used: None,
            images: Vec::new(),
        };
        save_message(&msg).expect("Should save message");

        // Upsert with message_id
        audit.attach_message_id(&msg.id);
        save_message_preparation_audit(&audit).expect("Should upsert audit");

        let reloaded = get_last_message_preparation_audit(&chat_id)
            .expect("Should re-query audit")
            .expect("Should find upserted audit");
        assert_eq!(reloaded.message_id, Some("msg-audit-rt".to_string()));

        // Cleanup
        delete_chat_permanently(&chat_id).expect("Should cleanup");
    }
}
