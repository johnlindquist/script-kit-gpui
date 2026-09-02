//! Brain store: the unified memory substrate at `~/.scriptkit/db/brain.sqlite`.
//!
//! Documents from every source (notes, chat turns, future: clipboard
//! promotions, browser captures) are normalized into `brain_docs` with their
//! own FTS5 index, and embedded per-chunk into `brain_chunk_embeddings` by
//! the background indexer (qmd-style chunking lives in `super::chunker`).
//! `brain_signals` is the append-only attention log: what John
//! searches, asks, and selects — used to boost ranking toward what currently
//! matters.
//!
//! Design notes:
//! - Separate sqlite file (NOT notes.sqlite): the brain indexes many sources
//!   and must never contend with or complicate the notes schema.
//! - Vectors are BLOB f32 arrays, brute-force cosine in memory. At the scale
//!   of a personal knowledge base (thousands of docs) this is single-digit
//!   milliseconds and avoids shipping a vector-extension dependency.

use anyhow::{anyhow, Context as _, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

static BRAIN_DB: OnceLock<Arc<Mutex<Connection>>> = OnceLock::new();

/// A SEPARATE read-only connection for recall queries. All writes share the one
/// `BRAIN_DB` `Mutex<Connection>`, so a long indexer write (embedding batch
/// upserts) would otherwise queue submit-path recall SELECTs behind it — WAL
/// permits concurrent readers ONLY through a distinct connection. `Some(None)`
/// records that the read connection could not be opened (fall back to the write
/// connection); the outer `OnceLock` avoids retrying the open on every call.
static BRAIN_DB_READ: OnceLock<Option<Arc<Mutex<Connection>>>> = OnceLock::new();

/// A document source. Stable string keys — stored in sqlite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocSource {
    Note,
    ChatTurn,
    Clipboard,
    Activity,
    Capture,
    DayPage,
    Fragment,
}

impl DocSource {
    pub fn as_str(self) -> &'static str {
        match self {
            DocSource::Note => "note",
            DocSource::ChatTurn => "chat_turn",
            DocSource::Clipboard => "clipboard",
            DocSource::Activity => "activity",
            DocSource::Capture => "capture",
            DocSource::DayPage => "day_page",
            DocSource::Fragment => "fragment",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "note" => Some(DocSource::Note),
            "chat_turn" => Some(DocSource::ChatTurn),
            "clipboard" => Some(DocSource::Clipboard),
            "activity" => Some(DocSource::Activity),
            "capture" => Some(DocSource::Capture),
            "day_page" => Some(DocSource::DayPage),
            "fragment" => Some(DocSource::Fragment),
            _ => None,
        }
    }

    /// Human label used when rendering retrieved context for the agent.
    pub fn label(self) -> &'static str {
        match self {
            DocSource::Note => "Note",
            DocSource::ChatTurn => "Past conversation",
            DocSource::Clipboard => "Clipboard",
            DocSource::Activity => "Activity journal",
            DocSource::Capture => "Capture",
            DocSource::DayPage => "Day Page",
            DocSource::Fragment => "Fragment",
        }
    }
}

/// Max lines retained in a daily activity journal (newest first).
const ACTIVITY_JOURNAL_MAX_LINES: usize = 400;

/// Low attention weight for auto-kept clipboard sediment. Deliberate captures
/// and chat turns use weight 2 — kept copies must not outrank them at equal
/// relevance.
pub const CLIPBOARD_SEDIMENT_SIGNAL_WEIGHT: i64 = 1;

/// Memory tier for brain-kept clipboard entries (stored on the clipboard row;
/// T7 indexer reads this when syncing sediment docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardSedimentTier {
    /// Auto-kept URLs and re-copy promotions.
    Sediment = 1,
}

impl ClipboardSedimentTier {
    pub fn as_i64(self) -> i64 {
        self as i64
    }

    pub fn signal_weight(self) -> i64 {
        CLIPBOARD_SEDIMENT_SIGNAL_WEIGHT
    }
}

/// Record low-weight attention signals for clipboard sediment content.
/// Fire-and-forget; never blocks the monitor path.
pub fn record_sediment_signals(content: &str) {
    let text = content.trim().to_string();
    if text.is_empty() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("script-kit-clipboard-sediment-signal".to_string())
        .spawn(move || {
            let _ = init_brain_db();
            for topic in sediment_signal_topics(&text) {
                let _ = record_signal(
                    &topic,
                    ClipboardSedimentTier::Sediment.signal_weight(),
                    "clipboard_sediment",
                );
            }
        });
}

fn sediment_signal_topics(content: &str) -> Vec<String> {
    let trimmed = content.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        if let Some(host) = trimmed
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .filter(|host| !host.is_empty())
        {
            return vec![host.trim_start_matches("www.").to_lowercase()];
        }
    }
    trimmed
        .split_whitespace()
        .filter(|word| word.len() > 3)
        .take(3)
        .map(|word| word.to_lowercase())
        .collect()
}

/// Signal weight for deliberate `;` captures, chat turns, and pin acts.
pub const CAPTURE_SIGNAL_WEIGHT: i64 = 2;

/// Append a line to today's activity journal — the brain's record of what
/// the user actually DID (searches run, files opened, items chosen). One doc
/// per day, newest line first, so recall excerpts always lead with the most
/// recent actions. The whole read-modify-write happens under one lock.
pub fn append_activity(line: &str) -> Result<()> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    let now = chrono::Local::now();
    let day = now.format("%Y-%m-%d").to_string();
    let source_id = format!("activity:{day}");
    let title = format!("Activity journal {day}");
    let stamped = format!("{} — {}", now.format("%H:%M"), line);

    let existing: Option<String> = conn
        .query_row(
            "SELECT content FROM brain_docs WHERE source = 'activity' AND source_id = ?1",
            params![source_id],
            |row| row.get(0),
        )
        .optional()?;
    let mut lines: Vec<String> = vec![stamped];
    if let Some(existing) = existing {
        lines.extend(existing.lines().map(str::to_string));
        lines.truncate(ACTIVITY_JOURNAL_MAX_LINES);
    }
    let content = lines.join("\n");
    let hash = content_hash(&title, &content);
    conn.execute(
        "INSERT INTO brain_docs (source, source_id, title, content, content_hash, updated_at)
         VALUES ('activity', ?1, ?2, ?3, ?4, unixepoch())
         ON CONFLICT(source, source_id) DO UPDATE SET
            title = excluded.title,
            content = excluded.content,
            content_hash = excluded.content_hash,
            updated_at = excluded.updated_at",
        params![source_id, title, content, hash],
    )
    .context("append brain activity")?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct BrainDoc {
    pub id: i64,
    pub source: DocSource,
    pub source_id: String,
    pub title: String,
    pub content: String,
    pub canonical_path: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct BrainSignal {
    pub topic: String,
    pub weight: i64,
    pub source: String,
    pub created_at: i64,
}

fn brain_db_path() -> PathBuf {
    if let Ok(path) = std::env::var("SCRIPT_KIT_TEST_BRAIN_DB_PATH") {
        return PathBuf::from(path);
    }
    // Unit tests must never bind the process-global connection to the real
    // ~/.scriptkit brain db. `BRAIN_DB` is a `OnceLock`, so whichever test
    // thread touches the brain first wins the path for the whole process —
    // and brain-adjacent paths (input-history selection signals, launcher
    // grouping, MCP resources) fire before `brain::tests::init_test_db` can
    // point the connection at a temp file. Resolving every cfg(test) caller
    // to one fresh-per-process temp path keeps all suite writers on the same
    // isolated database and keeps test junk out of the developer's live db.
    #[cfg(test)]
    {
        static TEST_BRAIN_DB_PATH: OnceLock<PathBuf> = OnceLock::new();
        return TEST_BRAIN_DB_PATH
            .get_or_init(|| {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or_default();
                std::env::temp_dir().join(format!(
                    "script-kit-brain-test-{}-{nanos}.sqlite",
                    std::process::id()
                ))
            })
            .clone();
    }
    #[cfg(not(test))]
    {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".scriptkit").join("db").join("brain.sqlite")
    }
}

fn ensure_brain_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS brain_docs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source TEXT NOT NULL,
            source_id TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            canonical_path TEXT,
            content_hash TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
            UNIQUE(source, source_id)
        );
        CREATE INDEX IF NOT EXISTS idx_brain_docs_source ON brain_docs(source);
        CREATE INDEX IF NOT EXISTS idx_brain_docs_updated ON brain_docs(updated_at DESC);

        CREATE VIRTUAL TABLE IF NOT EXISTS brain_docs_fts USING fts5(
            title, content, content='brain_docs', content_rowid='id',
            tokenize='porter unicode61'
        );

        CREATE TRIGGER IF NOT EXISTS brain_docs_ai AFTER INSERT ON brain_docs BEGIN
            INSERT INTO brain_docs_fts(rowid, title, content)
            VALUES (new.id, new.title, new.content);
        END;
        CREATE TRIGGER IF NOT EXISTS brain_docs_ad AFTER DELETE ON brain_docs BEGIN
            INSERT INTO brain_docs_fts(brain_docs_fts, rowid, title, content)
            VALUES ('delete', old.id, old.title, old.content);
        END;
        CREATE TRIGGER IF NOT EXISTS brain_docs_au AFTER UPDATE ON brain_docs BEGIN
            INSERT INTO brain_docs_fts(brain_docs_fts, rowid, title, content)
            VALUES ('delete', old.id, old.title, old.content);
            INSERT INTO brain_docs_fts(rowid, title, content)
            VALUES (new.id, new.title, new.content);
        END;

        CREATE TABLE IF NOT EXISTS brain_chunk_embeddings (
            doc_id INTEGER NOT NULL REFERENCES brain_docs(id) ON DELETE CASCADE,
            model_id TEXT NOT NULL,
            chunk_index INTEGER NOT NULL,
            content_hash TEXT NOT NULL,
            chunk_start INTEGER NOT NULL DEFAULT 0,
            dim INTEGER NOT NULL,
            vec BLOB NOT NULL,
            embedded_at INTEGER NOT NULL DEFAULT (unixepoch()),
            PRIMARY KEY (doc_id, model_id, chunk_index)
        );
        CREATE INDEX IF NOT EXISTS idx_brain_chunk_embeddings_model
            ON brain_chunk_embeddings(model_id);

        CREATE TABLE IF NOT EXISTS brain_signals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            topic TEXT NOT NULL,
            weight INTEGER NOT NULL DEFAULT 1,
            source TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT (unixepoch())
        );
        CREATE INDEX IF NOT EXISTS idx_brain_signals_created ON brain_signals(created_at DESC);

        CREATE TABLE IF NOT EXISTS brain_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS brain_inbox (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            detail TEXT NOT NULL DEFAULT '',
            source TEXT NOT NULL DEFAULT '',
            source_id TEXT NOT NULL DEFAULT '',
            dedupe_hash TEXT NOT NULL UNIQUE,
            created_at INTEGER NOT NULL DEFAULT (unixepoch()),
            resolved_at INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_brain_inbox_open
            ON brain_inbox(resolved_at, created_at DESC);",
    )
    .context("ensure brain schema")?;
    migrate_brain_docs_canonical_path(conn)?;
    migrate_chunk_embeddings_pk(conn)?;
    migrate_whole_doc_embeddings(conn)?;
    migrate_fts_tokenizer(conn)
}

fn migrate_brain_docs_canonical_path(conn: &Connection) -> Result<()> {
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('brain_docs') WHERE name = 'canonical_path'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if exists == 0 {
        conn.execute("ALTER TABLE brain_docs ADD COLUMN canonical_path TEXT", [])
            .context("add brain_docs.canonical_path")?;
    }
    Ok(())
}

/// FTS schema version. v2 = porter stemming ("search" matches "searched").
/// Bump + extend the match arm below when the tokenizer changes again.
const FTS_VERSION: &str = "2";

fn migrate_fts_tokenizer(conn: &Connection) -> Result<()> {
    let current: Option<String> = conn
        .query_row(
            "SELECT value FROM brain_meta WHERE key = 'fts_version'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if current.as_deref() == Some(FTS_VERSION) {
        return Ok(());
    }
    conn.execute_batch(
        "DROP TRIGGER IF EXISTS brain_docs_ai;
         DROP TRIGGER IF EXISTS brain_docs_ad;
         DROP TRIGGER IF EXISTS brain_docs_au;
         DROP TABLE IF EXISTS brain_docs_fts;
         CREATE VIRTUAL TABLE brain_docs_fts USING fts5(
            title, content, content='brain_docs', content_rowid='id',
            tokenize='porter unicode61'
         );
         INSERT INTO brain_docs_fts(rowid, title, content)
            SELECT id, title, content FROM brain_docs;
         CREATE TRIGGER brain_docs_ai AFTER INSERT ON brain_docs BEGIN
            INSERT INTO brain_docs_fts(rowid, title, content)
            VALUES (new.id, new.title, new.content);
         END;
         CREATE TRIGGER brain_docs_ad AFTER DELETE ON brain_docs BEGIN
            INSERT INTO brain_docs_fts(brain_docs_fts, rowid, title, content)
            VALUES ('delete', old.id, old.title, old.content);
         END;
         CREATE TRIGGER brain_docs_au AFTER UPDATE ON brain_docs BEGIN
            INSERT INTO brain_docs_fts(brain_docs_fts, rowid, title, content)
            VALUES ('delete', old.id, old.title, old.content);
            INSERT INTO brain_docs_fts(rowid, title, content)
            VALUES (new.id, new.title, new.content);
         END;",
    )
    .context("migrate brain fts tokenizer")?;
    conn.execute(
        "INSERT INTO brain_meta (key, value) VALUES ('fts_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![FTS_VERSION],
    )
    .context("record brain fts version")?;
    Ok(())
}

/// Append `suffix` to the full path string (SQLite forms its sidecar and
/// backup names by suffixing the whole db path, e.g. `brain.sqlite-wal`).
fn brain_path_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// Open a connection at `path`, apply the standard pragmas, verify integrity
/// with `quick_check`, and ensure the schema. Any failure returns Err with the
/// connection already dropped, so the caller can safely rename files.
fn open_and_check(path: &Path) -> Result<Connection> {
    let conn = crate::utils::db_permissions::open_private_sqlite(path)
        .context("open private brain.sqlite")?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .context("brain WAL")?;
    conn.pragma_update(None, "busy_timeout", 5000)
        .context("brain busy_timeout")?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .context("brain foreign_keys")?;
    let integrity: String = conn
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .context("brain quick_check")?;
    if integrity != "ok" {
        return Err(anyhow!("brain quick_check reported: {integrity}"));
    }
    ensure_brain_schema(&conn)?;
    // brain.sqlite holds the full text of notes, day pages, fragments, and chat
    // turns. Keep it (and its WAL/SHM sidecars) owner-only rather than umask.
    crate::utils::db_permissions::harden_sqlite_permissions(path)
        .context("protect private Brain SQLite sidecars")?;
    Ok(conn)
}

/// Move the damaged db and its WAL/SHM sidecars aside to `*.corrupt-<secs>`
/// siblings. Returns the destination of the primary db file for logging.
fn move_corrupt_brain_db_aside(path: &Path) -> Result<PathBuf> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let corrupt = format!(".corrupt-{secs}");
    let mut primary_dest = brain_path_with_suffix(path, &corrupt);
    for suffix in ["", "-wal", "-shm"] {
        let from = brain_path_with_suffix(path, suffix);
        if !from.exists() {
            continue;
        }
        let dest = brain_path_with_suffix(&from, &corrupt);
        std::fs::rename(&from, &dest)
            .with_context(|| format!("move corrupt brain file {} aside", from.display()))?;
        if suffix.is_empty() {
            primary_dest = dest;
        }
    }
    Ok(primary_dest)
}

/// Open the brain DB at `path`, recovering from corruption by moving the
/// damaged database aside and starting fresh. The SQLite index is derived
/// from canonical markdown, so a fresh DB heals on the next indexer cycle.
/// The returned bool is `true` when recovery ran (the caller should wake the
/// indexer so the empty index repopulates promptly).
fn open_or_recover_brain_db(path: &Path) -> Result<(Connection, bool)> {
    crate::utils::db_permissions::prepare_private_sqlite(path)
        .context("refuse unsafe Brain SQLite ownership before corruption recovery")?;
    match open_and_check(path) {
        Ok(conn) => Ok((conn, false)),
        Err(err) => {
            // `open_and_check` dropped its connection on the error path, so the
            // files are unlocked and safe to rename before retrying.
            let moved = move_corrupt_brain_db_aside(path)?;
            tracing::warn!(
                target: "script_kit::brain",
                error = %err,
                moved_to = %moved.display(),
                "brain.sqlite failed integrity check; moved aside and rebuilding index from markdown"
            );
            Ok((open_and_check(path)?, true))
        }
    }
}

/// Initialize (or return) the global brain DB connection. Idempotent.
pub fn init_brain_db() -> Result<()> {
    if BRAIN_DB.get().is_some() {
        return Ok(());
    }
    let path = brain_db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create brain db dir")?;
    }
    let (conn, recovered) = open_or_recover_brain_db(&path)?;
    let _ = BRAIN_DB.set(Arc::new(Mutex::new(conn)));
    if recovered {
        // The fresh index is empty; nudge the indexer to repopulate it from
        // canonical markdown now instead of waiting for the next cycle. This is
        // a no-op before the indexer thread starts (its startup cycle populates
        // the fresh DB anyway); the wake only matters for corruption detected
        // at runtime after the indexer is already running.
        crate::brain::indexer::wake_indexer();
    }
    Ok(())
}

fn get_db() -> Result<Arc<Mutex<Connection>>> {
    init_brain_db()?;
    BRAIN_DB
        .get()
        .cloned()
        .ok_or_else(|| anyhow!("brain db not initialized"))
}

/// Run a closure against the shared brain connection (one lock acquisition).
/// Sibling modules that own their own table (e.g. [`super::inbox`]) use this
/// instead of reimplementing connection management.
pub(crate) fn with_conn<T>(f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    f(&conn)
}

/// Open the dedicated read-only connection. Requires the primary db (file +
/// schema) to already exist — `init_brain_db()` guarantees that. Applies the
/// same `busy_timeout` as the write connection; no schema/WAL pragmas because a
/// read-only connection follows the WAL created by the write connection.
fn open_read_conn() -> Result<Connection> {
    let path = crate::utils::db_permissions::private_sqlite_open_path(&brain_db_path())?;
    let conn = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .context("open brain read-only connection")?;
    conn.pragma_update(None, "busy_timeout", 5000)
        .context("brain read busy_timeout")?;
    Ok(conn)
}

/// Lazily initialize (or return) the read-only connection. `None` when it could
/// not be opened (e.g. first-launch ordering) — callers fall back to the write
/// connection so recall never fails just because the read path is unavailable.
fn read_db() -> Option<Arc<Mutex<Connection>>> {
    // Ensure the file and schema exist before opening a READ_ONLY handle.
    if init_brain_db().is_err() {
        return None;
    }
    BRAIN_DB_READ
        .get_or_init(|| open_read_conn().ok().map(|conn| Arc::new(Mutex::new(conn))))
        .clone()
}

/// Run a pure-SELECT closure against the read-only connection so it never queues
/// behind an in-flight indexer write on the shared write `Mutex`. Falls back to
/// [`with_conn`] when the read connection is unavailable. MUST NOT be used for
/// anything that writes (upserts, `meta_set`, signal inserts) — the connection
/// is READ_ONLY and will error on writes.
pub(crate) fn with_read_conn<T>(f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
    match read_db() {
        Some(db) => {
            let conn = db
                .lock()
                .map_err(|_| anyhow!("brain read db lock poisoned"))?;
            f(&conn)
        }
        None => with_conn(f),
    }
}

/// Reset mutable brain rows between unit tests while preserving the process
/// global connection and schema. Brain tests share `BRAIN_DB`, so per-test
/// isolation has to clear derived rows instead of trying to rebind the
/// `OnceLock` to a different sqlite file.
#[cfg(test)]
pub(crate) fn reset_for_test() -> Result<()> {
    with_conn(|conn| {
        conn.execute_batch(
            "DELETE FROM brain_chunk_embeddings;
             DELETE FROM brain_inbox;
             DELETE FROM brain_signals;
             DELETE FROM brain_docs;
             DELETE FROM brain_meta;
             DELETE FROM sqlite_sequence
                WHERE name IN ('brain_docs', 'brain_signals', 'brain_inbox');",
        )
        .context("reset brain test rows")?;
        migrate_fts_tokenizer(conn)?;
        Ok(())
    })
}

/// Short-lived shape fix: the first chunked schema keyed rows on
/// (doc_id, chunk_index) only, so two models' rows collided. Recreate with
/// model_id in the key; vectors re-embed lazily over the next cycles.
fn migrate_chunk_embeddings_pk(conn: &Connection) -> Result<()> {
    let create_sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='brain_chunk_embeddings'",
            [],
            |row| row.get(0),
        )
        .optional()
        .context("inspect chunk embeddings shape")?;
    let Some(create_sql) = create_sql else {
        return Ok(());
    };
    if create_sql.contains("PRIMARY KEY (doc_id, model_id, chunk_index)") {
        return Ok(());
    }
    conn.execute_batch(
        "DROP TABLE brain_chunk_embeddings;
        CREATE TABLE brain_chunk_embeddings (
            doc_id INTEGER NOT NULL REFERENCES brain_docs(id) ON DELETE CASCADE,
            model_id TEXT NOT NULL,
            chunk_index INTEGER NOT NULL,
            content_hash TEXT NOT NULL,
            chunk_start INTEGER NOT NULL DEFAULT 0,
            dim INTEGER NOT NULL,
            vec BLOB NOT NULL,
            embedded_at INTEGER NOT NULL DEFAULT (unixepoch()),
            PRIMARY KEY (doc_id, model_id, chunk_index)
        );
        CREATE INDEX IF NOT EXISTS idx_brain_chunk_embeddings_model
            ON brain_chunk_embeddings(model_id);",
    )
    .context("rekey chunk embeddings table")?;
    tracing::info!(
        target: "script_kit::brain",
        "rekeyed brain_chunk_embeddings to include model_id"
    );
    Ok(())
}

/// One-time migration from whole-doc embeddings (`brain_embeddings`, one
/// vector per doc, embed text truncated at 6 KB) to chunked embeddings.
/// Vectors for docs that fit in a single chunk are carried over unchanged —
/// their embed text is identical under both schemes. Longer docs are dropped
/// so the indexer re-embeds them in full as chunks (their old vectors only
/// ever saw the truncated prefix).
fn migrate_whole_doc_embeddings(conn: &Connection) -> Result<()> {
    let legacy_exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='brain_embeddings')",
            [],
            |row| row.get(0),
        )
        .context("check legacy embeddings table")?;
    if !legacy_exists {
        return Ok(());
    }
    let carried = conn
        .execute(
            "INSERT OR IGNORE INTO brain_chunk_embeddings
                (doc_id, chunk_index, model_id, content_hash, chunk_start, dim, vec, embedded_at)
             SELECT e.doc_id, 0, e.model_id, e.content_hash, 0, e.dim, e.vec, e.embedded_at
             FROM brain_embeddings e
             JOIN brain_docs d ON d.id = e.doc_id
             WHERE length(d.title) + 1 + length(d.content) <= ?1",
            params![super::chunker::CHUNK_TARGET_BYTES as i64],
        )
        .context("carry single-chunk embeddings forward")?;
    conn.execute("DROP TABLE brain_embeddings", [])
        .context("drop legacy embeddings table")?;
    tracing::info!(
        target: "script_kit::brain",
        carried,
        "migrated whole-doc embeddings to chunked schema"
    );
    Ok(())
}

pub(crate) fn content_hash(title: &str, content: &str) -> String {
    // FNV-1a 64: fast, stable, no new dependency; collisions only cost a
    // redundant re-embed.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in title
        .as_bytes()
        .iter()
        .chain([0u8].iter())
        .chain(content.as_bytes())
    {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    format!("{hash:016x}")
}

const BRAIN_DOC_SELECT_COLUMNS: &str =
    "id, source, source_id, title, content, canonical_path, updated_at";

fn brain_doc_from_row(row: &Row<'_>) -> rusqlite::Result<Option<BrainDoc>> {
    let source: String = row.get(1)?;
    let Some(source) = DocSource::parse(&source) else {
        return Ok(None);
    };
    Ok(Some(BrainDoc {
        id: row.get(0)?,
        source,
        source_id: row.get(2)?,
        title: row.get(3)?,
        content: row.get(4)?,
        canonical_path: row.get(5)?,
        updated_at: row.get(6)?,
    }))
}

/// Insert or update a document. Returns the doc id. The embedding is
/// invalidated automatically when content changes (hash mismatch leaves the
/// stale row for the indexer to refresh).
pub fn upsert_doc(
    source: DocSource,
    source_id: &str,
    title: &str,
    content: &str,
    updated_at: i64,
) -> Result<i64> {
    upsert_doc_with_canonical_path(source, source_id, title, content, updated_at, None)
}

pub fn upsert_doc_with_canonical_path(
    source: DocSource,
    source_id: &str,
    title: &str,
    content: &str,
    updated_at: i64,
    canonical_path: Option<&str>,
) -> Result<i64> {
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    let hash = content_hash(title, content);
    conn.execute(
        "INSERT INTO brain_docs (
            source, source_id, title, content, canonical_path, content_hash, updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(source, source_id) DO UPDATE SET
            title = excluded.title,
            content = excluded.content,
            canonical_path = excluded.canonical_path,
            content_hash = excluded.content_hash,
            updated_at = excluded.updated_at
         WHERE brain_docs.content_hash != excluded.content_hash
            OR brain_docs.updated_at != excluded.updated_at
            OR COALESCE(brain_docs.canonical_path, '') != COALESCE(excluded.canonical_path, '')",
        params![
            source.as_str(),
            source_id,
            title,
            content,
            canonical_path,
            hash,
            updated_at
        ],
    )
    .context("upsert brain doc")?;
    let id: i64 = conn
        .query_row(
            "SELECT id FROM brain_docs WHERE source = ?1 AND source_id = ?2",
            params![source.as_str(), source_id],
            |row| row.get(0),
        )
        .context("read brain doc id")?;
    Ok(id)
}

/// Remove all docs of a source whose source_id is NOT in `keep` — the
/// deletion-sync primitive (a note deleted from notes.sqlite must also be
/// forgotten by the brain). Returns the number removed.
pub fn retain_docs(source: DocSource, keep: &[String]) -> Result<usize> {
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    let existing: Vec<String> = {
        let mut stmt = conn.prepare("SELECT source_id FROM brain_docs WHERE source = ?1")?;
        let rows = stmt
            .query_map(params![source.as_str()], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows
    };
    let keep: std::collections::HashSet<&str> = keep.iter().map(String::as_str).collect();
    let mut removed = 0usize;
    for source_id in existing {
        if !keep.contains(source_id.as_str()) {
            conn.execute(
                "DELETE FROM brain_docs WHERE source = ?1 AND source_id = ?2",
                params![source.as_str(), source_id],
            )?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Delete specific docs of one source by source_id — the targeted-forget
/// primitive for the file syncs' previous-set tracking (unlike
/// [`retain_docs`], this never touches docs the caller didn't enumerate).
/// Returns the number removed. Embeddings cascade via the FK.
pub fn delete_docs_by_source_ids(source: DocSource, source_ids: &[String]) -> Result<usize> {
    if source_ids.is_empty() {
        return Ok(0);
    }
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    let mut removed = 0usize;
    for source_id in source_ids {
        removed += conn
            .execute(
                "DELETE FROM brain_docs WHERE source = ?1 AND source_id = ?2",
                params![source.as_str(), source_id],
            )
            .context("delete brain doc by source id")?;
    }
    Ok(removed)
}

/// Fetch one document by its source identity.
pub fn get_doc(source: DocSource, source_id: &str) -> Result<Option<BrainDoc>> {
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    conn.query_row(
        &format!(
            "SELECT {BRAIN_DOC_SELECT_COLUMNS}
             FROM brain_docs WHERE source = ?1 AND source_id = ?2"
        ),
        params![source.as_str(), source_id],
        brain_doc_from_row,
    )
    .optional()
    .context("get brain doc")
    .map(|row| row.flatten())
}

/// Remove a document (e.g. when its source note is deleted).
pub fn remove_doc(source: DocSource, source_id: &str) -> Result<()> {
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    conn.execute(
        "DELETE FROM brain_docs WHERE source = ?1 AND source_id = ?2",
        params![source.as_str(), source_id],
    )
    .context("remove brain doc")?;
    Ok(())
}

/// Minimum trimmed content length (chars) for a doc to participate in the
/// SEMANTIC leg of search. Measured against the real embeddinggemma index:
/// tiny docs ("clipstays too", "Link: GitHub") embed to near-generic vectors
/// that score 0.35–0.49 cosine against every query — including nonsense —
/// while genuine matches on substantive docs score ≥0.43. Short docs stay
/// fully searchable lexically via FTS on title and content.
pub(crate) const MIN_SEMANTIC_CONTENT_CHARS: usize = 40;

/// Docs whose embedding is missing or stale for the given model. Ordered by
/// recency so fresh material becomes searchable first. A doc is current only
/// when chunk rows exist for this model AND they were produced from the
/// doc's current content (chunk rows all carry the doc-level hash). Docs
/// below [`MIN_SEMANTIC_CONTENT_CHARS`] are never embedded (similarity
/// magnets — see the const doc).
pub fn docs_needing_embedding(model_id: &str, limit: usize) -> Result<Vec<BrainDoc>> {
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    let columns = BRAIN_DOC_SELECT_COLUMNS
        .split(", ")
        .map(|column| format!("d.{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut stmt = conn.prepare(&format!(
        "SELECT {columns}
         FROM brain_docs d
         WHERE length(trim(d.content)) >= {MIN_SEMANTIC_CONTENT_CHARS}
           AND NOT EXISTS (
            SELECT 1 FROM brain_chunk_embeddings e
            WHERE e.doc_id = d.id
              AND e.model_id = ?1
              AND e.content_hash = d.content_hash
         )
         ORDER BY d.updated_at DESC
         LIMIT ?2",
    ))?;
    let rows = stmt
        .query_map(params![model_id, limit as i64], brain_doc_from_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().flatten().collect())
}

/// Single-vector convenience wrapper over [`store_chunk_embeddings`] — the
/// whole doc as chunk 0. Kept for short docs and existing call sites/tests.
pub fn store_embedding(
    doc_id: i64,
    model_id: &str,
    title: &str,
    content: &str,
    vec: &[f32],
) -> Result<()> {
    store_chunk_embeddings(doc_id, model_id, title, content, &[(0, vec.to_vec())])
}

/// Replace all chunk vectors for `doc_id` atomically. Every chunk row carries
/// the doc-level content hash, so staleness stays a single comparison in
/// [`docs_needing_embedding`]. `chunks` is (chunk_start_byte, vector).
pub fn store_chunk_embeddings(
    doc_id: i64,
    model_id: &str,
    title: &str,
    content: &str,
    chunks: &[(usize, Vec<f32>)],
) -> Result<()> {
    let db = get_db()?;
    let mut conn_guard = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    let hash = content_hash(title, content);
    let tx = conn_guard.transaction().context("chunk embed tx")?;
    tx.execute(
        "DELETE FROM brain_chunk_embeddings WHERE doc_id = ?1 AND model_id = ?2",
        params![doc_id, model_id],
    )
    .context("clear stale chunk embeddings")?;
    for (index, (chunk_start, vec)) in chunks.iter().enumerate() {
        if vec.is_empty() {
            continue;
        }
        let bytes: Vec<u8> = vec.iter().flat_map(|f| f.to_le_bytes()).collect();
        tx.execute(
            "INSERT INTO brain_chunk_embeddings
                (doc_id, chunk_index, model_id, content_hash, chunk_start, dim, vec)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                doc_id,
                index as i64,
                model_id,
                hash,
                *chunk_start as i64,
                vec.len() as i64,
                bytes
            ],
        )
        .context("store brain chunk embedding")?;
    }
    tx.commit().context("commit chunk embeddings")?;
    Ok(())
}

/// All chunk embeddings for the given model: (doc_id, vector). A doc appears
/// once per chunk; cosine ranking dedupes to best-chunk-per-doc. Loaded into
/// memory for brute-force cosine — see module docs for why this is fine.
pub fn load_embeddings(model_id: &str) -> Result<Vec<(i64, Vec<f32>)>> {
    with_read_conn(|conn| {
        let mut stmt = conn.prepare(&format!(
            "SELECT e.doc_id, e.vec
             FROM brain_chunk_embeddings e
             JOIN brain_docs d ON d.id = e.doc_id
             WHERE e.model_id = ?1 AND length(trim(d.content)) >= {MIN_SEMANTIC_CONTENT_CHARS}
             ORDER BY e.doc_id, e.chunk_index",
        ))?;
        let rows = stmt
            .query_map(params![model_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows
            .into_iter()
            .map(|(id, bytes)| {
                let vec = bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                (id, vec)
            })
            .collect())
    })
}

/// FTS5 BM25 search over all brain docs. Returns (doc_id, rank) best-first.
pub fn fts_search(query: &str, limit: usize) -> Result<Vec<i64>> {
    let fts_query = sanitize_fts_query(query);
    fts_search_sanitized(&fts_query, limit)
}

/// FTS5 search requiring multi-word launcher queries to occur within ten
/// tokens. Single-term behavior is identical to [`fts_search`].
pub fn fts_search_proximity(query: &str, limit: usize) -> Result<Vec<i64>> {
    let fts_query = sanitize_fts_query_proximity(query);
    fts_search_sanitized(&fts_query, limit)
}

fn fts_search_sanitized(fts_query: &str, limit: usize) -> Result<Vec<i64>> {
    if fts_query.is_empty() {
        return Ok(Vec::new());
    }
    with_read_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT rowid FROM brain_docs_fts WHERE brain_docs_fts MATCH ?1
             ORDER BY bm25(brain_docs_fts, 2.0, 1.0) LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![fts_query, limit as i64], |row| row.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

/// Quote each term (so punctuation can't break FTS5 syntax) and join with OR:
/// recall queries are natural-language questions ("what branch does bluefin
/// deploy from"), and FTS5's implicit AND would require every filler word to
/// appear in a document. OR + BM25 ranking keeps precision: documents
/// matching more terms rank higher.
///
/// Short terms are kept (only 1-byte noise is dropped): "git", "vim", "npm",
/// "k8s", and "ai" are exactly the kinds of topics users recall, and the old
/// `len() > 3` filter silently produced an empty query for them. BM25 already
/// down-ranks high-frequency filler, so dropping it here is unnecessary.
/// Substring fallback for queries the FTS tokenizer cannot index: unicode61
/// drops emoji and symbol-only tokens, so `🚀` MATCHes nothing even when the
/// text sits verbatim in a doc title. Scans title+content with LIKE (newest
/// first) so those queries still recall. Callers should only reach for this
/// when [`fts_search`] returns empty — it is a table scan, not an index hit.
pub fn substring_search(query: &str, limit: usize) -> Result<Vec<i64>> {
    substring_search_with_join(query, limit, " OR ")
}

/// Substring fallback for proximity search. Every query term must occur in
/// the title or content, preventing an emoji/symbol fallback from restoring
/// the broad OR behavior that proximity mode intentionally avoids.
pub fn substring_search_proximity(query: &str, limit: usize) -> Result<Vec<i64>> {
    substring_search_with_join(query, limit, " AND ")
}

fn substring_search_with_join(query: &str, limit: usize, joiner: &str) -> Result<Vec<i64>> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|term| {
            term.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        })
        .collect();
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    with_read_conn(|conn| {
        let clause = (1..=terms.len())
            .map(|n| format!("(title LIKE ?{n} ESCAPE '\\' OR content LIKE ?{n} ESCAPE '\\')"))
            .collect::<Vec<_>>()
            .join(joiner);
        let sql = format!(
            "SELECT id FROM brain_docs WHERE {clause}
             ORDER BY updated_at DESC LIMIT {limit}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let patterns = terms.iter().map(|term| format!("%{term}%"));
        let rows = stmt
            .query_map(rusqlite::params_from_iter(patterns), |row| {
                row.get::<_, i64>(0)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

fn sanitize_fts_query(query: &str) -> String {
    sanitized_fts_terms(query).join(" OR ")
}

fn sanitize_fts_query_proximity(query: &str) -> String {
    let terms = sanitized_fts_terms(query);
    match terms.len() {
        0 => String::new(),
        1 => terms.into_iter().next().unwrap_or_default(),
        _ => format!("NEAR({}, 10)", terms.join(" ")),
    }
}

fn sanitized_fts_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|term| term.replace('"', ""))
        .filter(|term| term.len() > 1)
        .map(|term| format!("\"{term}\""))
        .collect()
}

pub fn get_docs_by_ids(ids: &[i64]) -> Result<Vec<BrainDoc>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    with_read_conn(|conn| {
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT {BRAIN_DOC_SELECT_COLUMNS}
             FROM brain_docs WHERE id IN ({placeholders})"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(ids.iter()), brain_doc_from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows.into_iter().flatten().collect())
    })
}

/// Most recently updated docs across all sources, newest first. Backs the
/// armed-but-empty `brain:` launcher filter ("show me what my brain holds")
/// so the explicit source filter never renders a blank dead end.
pub fn recent_docs(limit: usize) -> Result<Vec<BrainDoc>> {
    with_read_conn(|conn| {
        let mut stmt = conn.prepare(&format!(
            "SELECT {BRAIN_DOC_SELECT_COLUMNS}
             FROM brain_docs ORDER BY updated_at DESC LIMIT ?1",
        ))?;
        let rows = stmt
            .query_map(params![limit as i64], brain_doc_from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows.into_iter().flatten().collect())
    })
}

/// Docs of one source updated at/after `since`, newest first. Evidence
/// gathering for the curator (e.g. recent chat turns for inbox extraction).
pub fn recent_docs_for_source(
    source: DocSource,
    since: i64,
    limit: usize,
) -> Result<Vec<BrainDoc>> {
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {BRAIN_DOC_SELECT_COLUMNS}
         FROM brain_docs WHERE source = ?1 AND updated_at >= ?2
         ORDER BY updated_at DESC LIMIT ?3",
    ))?;
    let rows = stmt
        .query_map(
            params![source.as_str(), since, limit as i64],
            brain_doc_from_row,
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().flatten().collect())
}

/// Append an attention signal. Topics are free-form lowercase strings.
pub fn record_signal(topic: &str, weight: i64, source: &str) -> Result<()> {
    let topic = topic.trim().to_lowercase();
    if topic.is_empty() {
        return Ok(());
    }
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    conn.execute(
        "INSERT INTO brain_signals (topic, weight, source) VALUES (?1, ?2, ?3)",
        params![topic, weight, source],
    )
    .context("record brain signal")?;
    Ok(())
}

/// Recent signals (newest first), for ranking boosts and the focus view.
/// Test-only: record a signal with an explicit `created_at`, so tests can
/// deterministically simulate signals that arrive strictly later (the
/// second-granularity default timestamp makes same-second ordering under
/// `recent_signals`' LIMIT window implementation-defined).
#[cfg(test)]
pub(crate) fn record_signal_at(
    topic: &str,
    weight: i64,
    source: &str,
    created_at: i64,
) -> Result<()> {
    let topic = topic.trim().to_lowercase();
    if topic.is_empty() {
        return Ok(());
    }
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    conn.execute(
        "INSERT INTO brain_signals (topic, weight, source, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![topic, weight, source, created_at],
    )
    .context("record brain signal at explicit time")?;
    Ok(())
}

pub fn recent_signals(limit: usize) -> Result<Vec<BrainSignal>> {
    with_read_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT topic, weight, source, created_at FROM brain_signals
             ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(BrainSignal {
                    topic: row.get(0)?,
                    weight: row.get(1)?,
                    source: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

/// Retention windows for the brain's own ambient records. Content the user
/// explicitly created (notes, pinned clipboard) is never pruned — only what
/// the brain wrote for itself ages out.
const ACTIVITY_RETENTION_DAYS: i64 = 90;
const SIGNAL_RETENTION_DAYS: i64 = 30;
/// Resolved inbox items linger this long (recently-dismissed context), then
/// age out. Open items are never pruned.
const INBOX_RESOLVED_RETENTION_DAYS: i64 = 30;
/// Backstop row cap so a runaway signal source can't bloat the db inside the
/// retention window.
const SIGNAL_MAX_ROWS: i64 = 20_000;

/// Prune aged ambient data: daily activity journals older than
/// [`ACTIVITY_RETENTION_DAYS`], attention signals older than
/// [`SIGNAL_RETENTION_DAYS`] (plus the row-cap backstop), and resolved inbox
/// items older than [`INBOX_RESOLVED_RETENTION_DAYS`]. The focus-review doc
/// is exempt (its source_id doesn't match the `activity:` day prefix).
/// Returns (journals_removed, signals_removed, inbox_removed).
pub fn prune_ambient_data() -> Result<(usize, usize, usize)> {
    prune_ambient_data_at(chrono::Utc::now().timestamp())
}

/// Testable core of [`prune_ambient_data`] — `now` is injectable.
pub(crate) fn prune_ambient_data_at(now: i64) -> Result<(usize, usize, usize)> {
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    let journal_cutoff = now - ACTIVITY_RETENTION_DAYS * 86_400;
    let journals = conn
        .execute(
            "DELETE FROM brain_docs
             WHERE source = 'activity'
               AND source_id LIKE 'activity:%'
               AND updated_at < ?1",
            params![journal_cutoff],
        )
        .context("prune brain activity journals")?;
    let signal_cutoff = now - SIGNAL_RETENTION_DAYS * 86_400;
    let mut signals = conn
        .execute(
            "DELETE FROM brain_signals WHERE created_at < ?1",
            params![signal_cutoff],
        )
        .context("prune brain signals by age")?;
    signals += conn
        .execute(
            "DELETE FROM brain_signals WHERE id NOT IN (
                SELECT id FROM brain_signals ORDER BY id DESC LIMIT ?1)",
            params![SIGNAL_MAX_ROWS],
        )
        .context("prune brain signals by cap")?;
    let inbox_cutoff = now - INBOX_RESOLVED_RETENTION_DAYS * 86_400;
    let inbox = conn
        .execute(
            "DELETE FROM brain_inbox
             WHERE resolved_at IS NOT NULL AND resolved_at < ?1",
            params![inbox_cutoff],
        )
        .context("prune resolved brain inbox items")?;
    Ok((journals, signals, inbox))
}

/// Doc counts per source, for the health surface.
pub fn source_counts() -> Result<Vec<(String, i64)>> {
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    let mut stmt =
        conn.prepare("SELECT source, COUNT(*) FROM brain_docs GROUP BY source ORDER BY source")?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// On-disk size of the brain database file, for the health surface.
/// 0 when unreadable.
pub fn db_size_bytes() -> u64 {
    std::fs::metadata(brain_db_path())
        .map(|meta| meta.len())
        .unwrap_or(0)
}

pub fn meta_get(key: &str) -> Result<Option<String>> {
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    conn.query_row(
        "SELECT value FROM brain_meta WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .context("brain meta get")
}

pub fn meta_set(key: &str, value: &str) -> Result<()> {
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    conn.execute(
        "INSERT INTO brain_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .context("brain meta set")?;
    Ok(())
}

/// Stats for the brain status surface and doctor checks.
pub fn doc_stats() -> Result<(i64, i64, i64)> {
    let db = get_db()?;
    let conn = db.lock().map_err(|_| anyhow!("brain db lock poisoned"))?;
    let docs: i64 = conn.query_row("SELECT COUNT(*) FROM brain_docs", [], |r| r.get(0))?;
    let embedded: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT doc_id) FROM brain_chunk_embeddings",
        [],
        |r| r.get(0),
    )?;
    let signals: i64 = conn.query_row("SELECT COUNT(*) FROM brain_signals", [], |r| r.get(0))?;
    Ok((docs, embedded, signals))
}

#[cfg(test)]
mod store_migration_tests {
    use super::*;

    /// Upgrading from the whole-doc embedding era: short docs carry their
    /// vector forward as chunk 0 (identical embed text under both schemes);
    /// long docs are dropped so the indexer re-embeds them chunked — their
    /// old vector only ever saw the truncated 6 KB prefix.
    #[test]
    fn whole_doc_embeddings_migrate_to_chunks() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE brain_docs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source TEXT NOT NULL,
                source_id TEXT NOT NULL,
                title TEXT NOT NULL DEFAULT '',
                content TEXT NOT NULL DEFAULT '',
                content_hash TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL DEFAULT (unixepoch()),
                updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
                UNIQUE(source, source_id)
            );
            CREATE TABLE brain_embeddings (
                doc_id INTEGER PRIMARY KEY REFERENCES brain_docs(id) ON DELETE CASCADE,
                model_id TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                dim INTEGER NOT NULL,
                vec BLOB NOT NULL,
                embedded_at INTEGER NOT NULL DEFAULT (unixepoch())
            );",
        )
        .expect("create legacy schema");
        let long_content = "x".repeat(super::super::chunker::CHUNK_TARGET_BYTES + 100);
        conn.execute(
            "INSERT INTO brain_docs (id, source, source_id, title, content, content_hash)
             VALUES (1, 'note', 'short', 'T', 'small body', 'h1'),
                    (2, 'note', 'long', 'T', ?1, 'h2')",
            params![long_content],
        )
        .expect("seed docs");
        conn.execute(
            "INSERT INTO brain_embeddings (doc_id, model_id, content_hash, dim, vec)
             VALUES (1, 'm', 'h1', 1, x'0000803f'), (2, 'm', 'h2', 1, x'0000803f')",
            [],
        )
        .expect("seed legacy embeddings");

        ensure_brain_schema(&conn).expect("schema upgrade");

        let carried: i64 = conn
            .query_row("SELECT COUNT(*) FROM brain_chunk_embeddings", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(carried, 1, "only the single-chunk doc carries forward");
        let carried_doc: i64 = conn
            .query_row("SELECT doc_id FROM brain_chunk_embeddings", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(carried_doc, 1);
        let legacy_gone: bool = conn
            .query_row(
                "SELECT NOT EXISTS(SELECT 1 FROM sqlite_master WHERE name='brain_embeddings')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(legacy_gone, "legacy table dropped");
        // Idempotent: running again must not error.
        ensure_brain_schema(&conn).expect("second upgrade is a no-op");
    }
}

#[cfg(test)]
mod store_recovery_tests {
    use super::*;

    /// The SQLite index is derived-only (canonical data is markdown), so a
    /// corrupt `brain.sqlite` must never brick recall: it is moved aside and a
    /// fresh, healable database takes its place. These tests exercise
    /// `open_or_recover_brain_db` on explicit temp paths so they never touch
    /// the process-global `BRAIN_DB` connection.
    fn brain_docs_present(conn: &Connection) -> bool {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master \
             WHERE type='table' AND name='brain_docs')",
            [],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn corrupt_sibling_exists(path: &Path) -> bool {
        let dir = path.parent().expect("temp path has a parent");
        std::fs::read_dir(dir)
            .expect("read temp dir")
            .filter_map(|entry| entry.ok())
            .any(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"))
    }

    #[test]
    fn fresh_path_opens_with_schema() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("brain.sqlite");
        let (conn, recovered) = open_or_recover_brain_db(&path).expect("open fresh brain db");
        assert!(!recovered, "a fresh path must not report recovery");
        assert!(brain_docs_present(&conn), "fresh db has brain_docs schema");
        assert!(
            !corrupt_sibling_exists(&path),
            "a fresh path must not trigger recovery"
        );
    }

    #[test]
    fn corrupt_db_is_moved_aside_and_healed() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("brain.sqlite");
        std::fs::write(&path, b"not a sqlite db").expect("prime corrupt db");
        let (conn, recovered) = open_or_recover_brain_db(&path).expect("recover corrupt brain db");
        assert!(recovered, "a corrupt db must report recovery");
        assert!(
            corrupt_sibling_exists(&path),
            "corrupt db is moved to a *.corrupt-* sibling"
        );
        assert!(brain_docs_present(&conn), "recovered db has the schema");
    }

    #[test]
    fn valid_db_reopens_without_recovery() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("brain.sqlite");
        {
            let (conn, _) = open_or_recover_brain_db(&path).expect("create brain db");
            conn.execute(
                "INSERT INTO brain_docs (source, source_id, title, content) \
                 VALUES ('note', 'seed', 'T', 'body')",
                [],
            )
            .expect("seed a doc row");
        }
        let (conn, recovered) = open_or_recover_brain_db(&path).expect("reopen valid brain db");
        assert!(!recovered, "a valid db must not report recovery");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM brain_docs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "rows survive a clean reopen");
        assert!(
            !corrupt_sibling_exists(&path),
            "a valid db must not trigger recovery"
        );
    }

    #[cfg(unix)]
    #[test]
    fn brain_recovery_refuses_hostile_database_symlinks_without_moving_foreign_owner() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("isolated brain recovery fixture");
        let path = directory.path().join("brain.sqlite");
        let foreign = directory.path().join("foreign.sqlite");
        std::fs::write(&foreign, b"foreign private brain documents")
            .expect("seed foreign brain owner");
        symlink(&foreign, &path).expect("plant brain recovery symlink");

        assert!(open_or_recover_brain_db(&path).is_err());
        assert!(!corrupt_sibling_exists(&path));
        assert_eq!(
            std::fs::read(&foreign).expect("foreign brain owner remains untouched"),
            b"foreign private brain documents"
        );
        assert!(std::fs::symlink_metadata(&path)
            .expect("hostile brain link remains visible for repair")
            .file_type()
            .is_symlink());
    }
}
