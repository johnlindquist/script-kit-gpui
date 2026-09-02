// SQLite schema, private connection initialization, and corruption recovery.
/// Ensure the notes tables and virtual search table exist.
fn ensure_notes_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT,
            is_pinned INTEGER NOT NULL DEFAULT 0,
            sort_order INTEGER NOT NULL DEFAULT 0,
            file_slug TEXT,
            content_hash TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_notes_updated_at ON notes(updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_notes_deleted_at ON notes(deleted_at);
        CREATE INDEX IF NOT EXISTS idx_notes_is_pinned ON notes(is_pinned);

        CREATE TABLE IF NOT EXISTS note_cart_items (
            id TEXT PRIMARY KEY,
            note_id TEXT NOT NULL,
            label TEXT NOT NULL DEFAULT '',
            payload_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY(note_id) REFERENCES notes(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_note_cart_items_note_id_sort
            ON note_cart_items(note_id, sort_order, updated_at DESC);

        CREATE TABLE IF NOT EXISTS note_tags (
            note_id TEXT NOT NULL,
            tag TEXT NOT NULL,
            normalized_tag TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'markdown',
            updated_at TEXT NOT NULL,
            PRIMARY KEY(note_id, normalized_tag),
            FOREIGN KEY(note_id) REFERENCES notes(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_note_tags_normalized
            ON note_tags(normalized_tag, note_id);

        CREATE TABLE IF NOT EXISTS note_aliases (
            note_id TEXT NOT NULL,
            alias TEXT NOT NULL,
            slug TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'title',
            updated_at TEXT NOT NULL,
            PRIMARY KEY(note_id, slug),
            FOREIGN KEY(note_id) REFERENCES notes(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_note_aliases_slug
            ON note_aliases(slug, note_id);

        CREATE TABLE IF NOT EXISTS note_links (
            source_note_id TEXT NOT NULL,
            target_note_id TEXT,
            target_ref TEXT NOT NULL,
            target_slug TEXT NOT NULL,
            label TEXT,
            kind TEXT NOT NULL DEFAULT 'wiki',
            byte_start INTEGER NOT NULL DEFAULT 0,
            byte_end INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL,
            PRIMARY KEY(source_note_id, target_slug, byte_start, byte_end, kind),
            FOREIGN KEY(source_note_id) REFERENCES notes(id) ON DELETE CASCADE,
            FOREIGN KEY(target_note_id) REFERENCES notes(id) ON DELETE SET NULL
        );

        CREATE INDEX IF NOT EXISTS idx_note_links_target
            ON note_links(target_note_id, source_note_id);
        CREATE INDEX IF NOT EXISTS idx_note_links_target_slug
            ON note_links(target_slug, source_note_id);

        CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
            title,
            content,
            content='notes',
            content_rowid='rowid'
        );
        "#,
    )
    .context("Failed to create notes tables")?;

    migrate_notes_schema(conn)?;
    ensure_notes_fts_triggers(conn)?;
    Ok(())
}

fn migrate_notes_schema(conn: &Connection) -> Result<()> {
    let columns = [("file_slug", "TEXT"), ("content_hash", "TEXT")];
    for (name, column_type) in columns {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('notes') WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if exists == 0 {
            conn.execute(
                &format!("ALTER TABLE notes ADD COLUMN {name} {column_type}"),
                [],
            )
            .with_context(|| format!("Failed to add notes.{name} column"))?;
        }
    }
    Ok(())
}

/// Recreate the FTS triggers so migrations are applied even on an existing DB connection.
fn ensure_notes_fts_triggers(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        DROP TRIGGER IF EXISTS notes_ai;
        DROP TRIGGER IF EXISTS notes_ad;
        DROP TRIGGER IF EXISTS notes_au;

        CREATE TRIGGER notes_ai AFTER INSERT ON notes BEGIN
            INSERT INTO notes_fts(rowid, title, content)
            VALUES (NEW.rowid, NEW.title, NEW.content);
        END;

        CREATE TRIGGER notes_ad AFTER DELETE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, title, content)
            VALUES('delete', OLD.rowid, OLD.title, OLD.content);
        END;

        CREATE TRIGGER notes_au AFTER UPDATE ON notes
        WHEN OLD.title <> NEW.title OR OLD.content <> NEW.content
        BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, title, content)
            VALUES('delete', OLD.rowid, OLD.title, OLD.content);
            INSERT INTO notes_fts(rowid, title, content)
            VALUES (NEW.rowid, NEW.title, NEW.content);
        END;
        "#,
    )
    .context("Failed to create FTS triggers")?;

    Ok(())
}

/// Serializes first-time notes DB initialization across threads.
///
/// Without this, concurrent callers can each pass the `NOTES_DB.get()` miss,
/// open separate connections to the same sqlite file, and race the
/// DROP/CREATE TRIGGER batch in `ensure_notes_schema` ("Failed to create FTS
/// triggers"). Poison-tolerant: a panicking initializer must not wedge every
/// later caller.
static NOTES_DB_INIT_LOCK: Mutex<()> = Mutex::new(());

/// Open a connection at `path`, apply the standard pragmas, verify integrity
/// with `quick_check`, and ensure the schema. Any failure returns Err with the
/// connection already dropped, so the caller can safely rename files aside.
fn open_and_check_notes_db(path: &Path) -> Result<Connection> {
    if let Some(policy) = crate::runtime_policy::owned_evaluation() {
        policy.require_owned_path(path)?;
    }
    let conn = crate::utils::db_permissions::open_private_sqlite(path)
        .context("Failed to open private notes database")?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .context("Failed to enable WAL mode")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .context("Failed to enable notes foreign keys")?;
    let integrity: String = conn
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .context("notes quick_check")?;
    if integrity != "ok" {
        anyhow::bail!("notes quick_check reported: {integrity}");
    }
    ensure_notes_schema(&conn)?;
    // notes.sqlite stores full note titles + bodies (+ FTS shadow). Keep it and
    // its WAL/SHM sidecars owner-only rather than inheriting umask.
    crate::utils::db_permissions::harden_sqlite_permissions(path)
        .context("Failed to protect private Notes SQLite sidecars")?;
    Ok(conn)
}

fn notes_path_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.push_str(suffix);
    path.with_file_name(name)
}

/// Move a damaged notes db and its WAL/SHM sidecars aside to
/// `*.corrupt-<secs>` siblings. Returns the destination of the primary db
/// file for logging.
fn move_corrupt_notes_db_aside(path: &Path) -> Result<PathBuf> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let corrupt = format!(".corrupt-{secs}");
    let mut primary_dest = notes_path_with_suffix(path, &corrupt);
    for suffix in ["", "-wal", "-shm"] {
        let from = notes_path_with_suffix(path, suffix);
        if !from.exists() {
            continue;
        }
        let dest = notes_path_with_suffix(&from, &corrupt);
        fs::rename(&from, &dest)
            .with_context(|| format!("move corrupt notes file {} aside", from.display()))?;
        if suffix.is_empty() {
            primary_dest = dest;
        }
    }
    Ok(primary_dest)
}

/// Open the notes DB at `path`, recovering from corruption by moving the
/// damaged database aside and starting fresh. Markdown under `brain/notes/`
/// is canonical (ADR 0003), so the caller rebuilds the index from files when
/// the returned bool is `true` — a corrupt index must never present as an
/// empty Notes list while the notes still exist on disk.
fn open_or_recover_notes_db(path: &Path) -> Result<(Connection, bool)> {
    crate::utils::db_permissions::prepare_private_sqlite(path)
        .context("Refuse unsafe Notes SQLite ownership before corruption recovery")?;
    match open_and_check_notes_db(path) {
        Ok(conn) => Ok((conn, false)),
        Err(err) => {
            // open_and_check_notes_db dropped its connection on the error
            // path, so the files are unlocked and safe to rename.
            let moved = move_corrupt_notes_db_aside(path)?;
            warn!(
                error = %err,
                moved_to = %moved.display(),
                "notes.sqlite failed integrity check; moved aside and rebuilding index from markdown"
            );
            Ok((open_and_check_notes_db(path)?, true))
        }
    }
}
