//! Notes Storage Layer
//!
//! Markdown files under `brain/notes/` are canonical; `notes.sqlite` is a
//! derived, rebuildable index (FTS, tags, aliases, backlinks). See ADR 0003.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use notify::{recommended_watcher, RecursiveMode, Watcher};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::brain::substrate::{BrainFrontmatter, BrainSlugDir, BrainSubstrate};

use super::metadata;
use super::model::{Note, NoteId};

/// SQLite index schema generation — bump when index shape changes.
const NOTES_INDEX_SCHEMA_VERSION: i32 = 2;

/// Global database connection for notes
static NOTES_DB: OnceLock<Arc<Mutex<Connection>>> = OnceLock::new();
static NOTES_SUBSTRATE: OnceLock<Arc<BrainSubstrate>> = OnceLock::new();
static NOTE_CONTENT_HASHES: OnceLock<Mutex<HashMap<NoteId, String>>> = OnceLock::new();
static ROOT_NOTES_SEARCH_CACHE: OnceLock<Mutex<RootNotesSearchCache>> = OnceLock::new();
static ROOT_NOTES_SEARCH_CACHE_GENERATION: AtomicU64 = AtomicU64::new(0);
static NOTES_STORAGE_GENERATION: AtomicU64 = AtomicU64::new(0);
static NOTES_DIR_WATCHER_STARTED: AtomicBool = AtomicBool::new(false);

fn db_lock_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("DB lock error: {e}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RootNotesSectionOptions {
    pub enabled: bool,
    pub max_results: usize,
    pub min_query_chars: usize,
    pub search_content: bool,
}

impl Default for RootNotesSectionOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            max_results: 3,
            min_query_chars: 3,
            search_content: true,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RootNoteSearchHit {
    pub id: NoteId,
    pub title: String,
    pub updated_at: DateTime<Utc>,
    pub is_pinned: bool,
    pub char_count: usize,
    pub score: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoteBacklinkSummary {
    pub id: NoteId,
    pub title: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct RootNotesSearchCacheKey {
    query: String,
    enabled: bool,
    max_results: usize,
    min_query_chars: usize,
    search_content: bool,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct RootNotesSearchFlightKey {
    generation: u64,
    search: RootNotesSearchCacheKey,
}

#[derive(Default)]
struct RootNotesSearchCache {
    hits_by_query: HashMap<RootNotesSearchCacheKey, Vec<RootNoteSearchHit>>,
    in_flight: HashSet<RootNotesSearchFlightKey>,
}

fn root_notes_search_cache() -> &'static Mutex<RootNotesSearchCache> {
    ROOT_NOTES_SEARCH_CACHE.get_or_init(|| Mutex::new(RootNotesSearchCache::default()))
}

fn root_notes_search_cache_key(
    query: &str,
    options: RootNotesSectionOptions,
) -> RootNotesSearchCacheKey {
    RootNotesSearchCacheKey {
        query: query.trim().to_string(),
        enabled: options.enabled,
        max_results: options.max_results,
        min_query_chars: options.min_query_chars,
        search_content: options.search_content,
    }
}

fn invalidate_root_notes_search_cache() {
    ROOT_NOTES_SEARCH_CACHE_GENERATION.fetch_add(1, Ordering::Relaxed);
    NOTES_STORAGE_GENERATION.fetch_add(1, Ordering::Relaxed);
    if let Some(cache) = ROOT_NOTES_SEARCH_CACHE.get() {
        if let Ok(mut guard) = cache.lock() {
            guard.hits_by_query.clear();
        }
    }
}

fn fingerprint_path(path: &std::path::Path) -> (String, usize) {
    let path_text = path.to_string_lossy();
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in path_text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    (format!("fnv1a64:{hash:016x}"), path_text.chars().count())
}

/// Redacted storage identity for automation receipts. Never returns raw
/// paths.
///
/// The database and the canonical Brain markdown substrate are sandboxed by
/// SEPARATE environment variables (`SCRIPT_KIT_TEST_NOTES_DB_PATH` and
/// `SCRIPT_KIT_TEST_NOTES_BRAIN_PATH`); a DB-only sandbox can still point
/// canonical note writes at the real Brain directory. `fullySandboxed` is
/// therefore the conjunction, and mutation probes must gate on it — not on
/// the legacy `testSandbox` (kept for older readers: DB-only signal).
pub(crate) fn automation_storage_identity() -> serde_json::Value {
    let (db_fingerprint, db_len) = fingerprint_path(&get_notes_db_path());
    let (brain_fingerprint, brain_len) = fingerprint_path(&get_notes_brain_base_path());
    let db_sandbox = std::env::var_os("SCRIPT_KIT_TEST_NOTES_DB_PATH").is_some() || cfg!(test);
    let brain_sandbox =
        std::env::var_os("SCRIPT_KIT_TEST_NOTES_BRAIN_PATH").is_some() || cfg!(test);

    serde_json::json!({
        "schemaVersion": 2,
        "redacted": true,
        "generation": NOTES_STORAGE_GENERATION.load(Ordering::Relaxed),
        "rootSearchCacheGeneration": ROOT_NOTES_SEARCH_CACHE_GENERATION.load(Ordering::Relaxed),
        "dbPathFingerprint": db_fingerprint,
        "dbPathLength": db_len,
        "brainPathFingerprint": brain_fingerprint,
        "brainPathLength": brain_len,
        "dbSandbox": db_sandbox,
        "brainSandbox": brain_sandbox,
        "fullySandboxed": db_sandbox && brain_sandbox,
        "testSandbox": db_sandbox,
    })
}

pub(crate) fn root_notes_query_is_eligible(query: &str, options: RootNotesSectionOptions) -> bool {
    let query = query.trim();
    options.enabled
        && !query.contains('\n')
        && crate::scripts::search::query_meets_min_query_chars(query, options.min_query_chars)
}

/// Get the path to the notes database
fn get_notes_db_path() -> PathBuf {
    if let Ok(path) = std::env::var("SCRIPT_KIT_TEST_NOTES_DB_PATH") {
        return PathBuf::from(path);
    }

    if cfg!(test) {
        return std::env::temp_dir()
            .join("script-kit-gpui-tests")
            .join(std::process::id().to_string())
            .join("db")
            .join("notes.sqlite");
    }

    let kit_dir = dirs::home_dir()
        .map(|h| h.join(".scriptkit"))
        .unwrap_or_else(|| PathBuf::from(".scriptkit"));

    kit_dir.join("db").join("notes.sqlite")
}

fn get_notes_brain_base_path() -> PathBuf {
    if let Ok(path) = std::env::var("SCRIPT_KIT_TEST_NOTES_BRAIN_PATH") {
        return PathBuf::from(path);
    }

    if cfg!(test) {
        return std::env::temp_dir()
            .join("script-kit-gpui-tests")
            .join(std::process::id().to_string())
            .join("brain");
    }

    crate::setup::get_kit_path().join("brain")
}

/// Days directory under the notes brain root. The Notes Cmd+P switcher
/// lists day pages read-through from here; day files are never copied into
/// the notes database.
pub(crate) fn notes_brain_days_dir() -> PathBuf {
    get_notes_brain_base_path().join("days")
}

pub(crate) fn note_file_path(id: NoteId) -> Result<Option<PathBuf>> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;
    let slug = lookup_note_slug(&conn, id)?;
    slug.map(|slug| notes_substrate().map(|substrate| substrate.paths().note_file(&slug)))
        .transpose()
}

fn notes_substrate() -> Result<Arc<BrainSubstrate>> {
    Ok(NOTES_SUBSTRATE
        .get_or_init(|| Arc::new(BrainSubstrate::new(get_notes_brain_base_path())))
        .clone())
}

fn note_content_hashes() -> &'static Mutex<HashMap<NoteId, String>> {
    NOTE_CONTENT_HASHES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn content_hash(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    format!("{:x}", digest)
}

fn slug_from_path(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_string)
        .filter(|slug| !slug.is_empty())
}

fn is_conflict_copy_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.contains(".conflict-"))
}

fn note_body_for_file(content: &str) -> String {
    metadata::strip_frontmatter(content).to_string()
}

fn user_facing_content(frontmatter: &BrainFrontmatter, body: &str) -> String {
    frontmatter.merge_into_body(body)
}

fn brain_frontmatter_from_note(note: &Note, preserved_source: Option<String>) -> BrainFrontmatter {
    let parsed = metadata::parse_note_metadata(&note.title, &note.content);
    let source = preserved_source.or_else(|| source_from_note_content(&note.content));
    BrainFrontmatter {
        id: note.id,
        created: note.created_at,
        updated: note.updated_at,
        tags: parsed.tags.into_iter().map(|tag| tag.display).collect(),
        aliases: parsed
            .aliases
            .into_iter()
            .filter(|alias| alias.source != "title")
            .map(|alias| alias.alias)
            .collect(),
        pinned: note.is_pinned,
        source,
        why: None,
    }
}

fn source_from_note_content(content: &str) -> Option<String> {
    if let Some(source) = metadata::parse_frontmatter_source(content) {
        return Some(source);
    }
    if let Ok(substrate) = notes_substrate() {
        if let Ok((frontmatter, _)) = substrate.parse_document(content) {
            return frontmatter.source;
        }
    }
    None
}

fn note_from_brain_document(
    frontmatter: BrainFrontmatter,
    body: &str,
    deleted_at: Option<DateTime<Utc>>,
    sort_order: i32,
) -> Note {
    let content = user_facing_content(&frontmatter, body);
    let title = Note::with_content(&content).title;
    Note {
        id: frontmatter.id,
        title,
        content,
        created_at: frontmatter.created,
        updated_at: frontmatter.updated,
        deleted_at,
        is_pinned: frontmatter.pinned,
        sort_order,
    }
}

fn load_note_from_file(
    substrate: &BrainSubstrate,
    path: &Path,
    deleted_at: Option<DateTime<Utc>>,
    sort_order: i32,
) -> Result<(Note, String, String)> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("reading note file {}", path.display()))?;
    let hash = content_hash(&raw);
    let (frontmatter, body) = substrate
        .parse_document(&raw)
        .with_context(|| format!("parsing note file {}", path.display()))?;
    let slug = slug_from_path(path).context("note file missing slug stem")?;
    let note = note_from_brain_document(frontmatter, &body, deleted_at, sort_order);
    Ok((note, slug, hash))
}

fn lookup_note_slug(conn: &Connection, note_id: NoteId) -> Result<Option<String>> {
    conn.query_row(
        "SELECT file_slug FROM notes WHERE id = ?1",
        params![note_id.as_str()],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()
    .context("Failed to look up note slug")
    .map(|row| row.flatten())
}

fn resolve_note_slug(conn: &Connection, note: &Note) -> Result<String> {
    if let Some(slug) = lookup_note_slug(conn, note.id)? {
        return Ok(slug);
    }

    let substrate = notes_substrate()?;
    Ok(substrate.allocate_slug(&note.title, BrainSlugDir::Notes))
}

fn write_conflict_copy(path: &Path, contents: &str) -> Result<()> {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return Ok(());
    };
    let parent = path.parent().context("conflict copy path missing parent")?;
    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    let conflict_path = parent.join(format!("{stem}.conflict-{timestamp}.md"));
    fs::write(&conflict_path, contents)
        .with_context(|| format!("writing conflict copy {}", conflict_path.display()))?;
    // A conflict copy holds a full note body; keep it owner-only rather than the
    // umask default that a raw fs::write would otherwise leave (0644).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&conflict_path, fs::Permissions::from_mode(0o600));
    }
    warn!(
        original = %path.display(),
        conflict = %conflict_path.display(),
        "External note edit conflict preserved as conflict copy"
    );
    Ok(())
}

fn guard_external_edit_before_write(path: &Path, note_id: NoteId) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let disk = fs::read_to_string(path)
        .with_context(|| format!("reading note file before write {}", path.display()))?;
    let disk_hash = content_hash(&disk);
    let known_hash = note_content_hashes()
        .lock()
        .map_err(db_lock_err)?
        .get(&note_id)
        .cloned();
    if let Some(known_hash) = known_hash {
        if known_hash != disk_hash {
            write_conflict_copy(path, &disk)?;
        }
    }
    Ok(())
}

fn remember_note_hash(note_id: NoteId, hash: String) {
    if let Ok(mut guard) = note_content_hashes().lock() {
        guard.insert(note_id, hash);
    }
}

fn forget_note_hash(note_id: NoteId) {
    if let Ok(mut guard) = note_content_hashes().lock() {
        guard.remove(&note_id);
    }
}

fn deleted_at_from_trash_path(path: &Path) -> DateTime<Utc> {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(|_| Utc::now())
}

fn preserve_note_cart_items_for_rebuild(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        DROP TABLE IF EXISTS temp.note_cart_items_rebuild_snapshot;
        CREATE TEMP TABLE note_cart_items_rebuild_snapshot AS
        SELECT id, note_id, label, payload_json, created_at, updated_at, sort_order
        FROM note_cart_items;
        "#,
    )
    .context("Failed to preserve note cart items before notes index rebuild")?;
    Ok(())
}

fn clear_index_tables(conn: &Connection) -> Result<()> {
    preserve_note_cart_items_for_rebuild(conn)?;
    conn.execute_batch(
        r#"
        DELETE FROM note_links;
        DELETE FROM note_aliases;
        DELETE FROM note_tags;
        DELETE FROM notes;
        "#,
    )
    .context("Failed to clear notes index tables")?;
    Ok(())
}

fn restore_note_cart_items_after_rebuild(conn: &Connection) -> Result<()> {
    let restored = conn
        .execute(
            r#"
            INSERT OR REPLACE INTO note_cart_items
                (id, note_id, label, payload_json, created_at, updated_at, sort_order)
            SELECT
                snapshot.id,
                snapshot.note_id,
                snapshot.label,
                snapshot.payload_json,
                snapshot.created_at,
                snapshot.updated_at,
                snapshot.sort_order
            FROM temp.note_cart_items_rebuild_snapshot snapshot
            WHERE EXISTS (
                SELECT 1 FROM notes WHERE notes.id = snapshot.note_id
            )
            "#,
            [],
        )
        .context("Failed to restore note cart items after notes index rebuild")?;
    let pruned = conn
        .execute(
            r#"
            DELETE FROM note_cart_items
            WHERE NOT EXISTS (
                SELECT 1 FROM notes WHERE notes.id = note_cart_items.note_id
            )
            "#,
            [],
        )
        .context("Failed to prune orphaned note cart items after notes index rebuild")?;
    conn.execute_batch("DROP TABLE IF EXISTS temp.note_cart_items_rebuild_snapshot;")
        .context("Failed to drop note cart item rebuild snapshot")?;
    debug!(
        restored,
        pruned, "Restored note cart items after notes index rebuild"
    );
    Ok(())
}

fn schema_needs_rebuild(conn: &Connection) -> Result<bool> {
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap_or(0);
    if version != NOTES_INDEX_SCHEMA_VERSION {
        return Ok(true);
    }

    let has_file_slug: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('notes') WHERE name = 'file_slug'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    Ok(has_file_slug == 0)
}

/// Rebuild the sqlite index from canonical markdown files.
///
/// Contract: delete the DB, rebuild from files, nothing user-visible is lost.
pub fn rebuild_index_from_files() -> Result<()> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;
    rebuild_index_from_files_with_conn(&conn)
}

fn rebuild_index_from_files_with_conn(conn: &Connection) -> Result<()> {
    conn.execute_batch("BEGIN IMMEDIATE;")
        .context("Failed to begin notes index rebuild transaction")?;
    let result = rebuild_index_from_files_with_conn_inner(conn);
    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT;")
                .context("Failed to commit notes index rebuild transaction")?;
            invalidate_root_notes_search_cache();
            info!("Rebuilt notes sqlite index from brain files");
            Ok(())
        }
        Err(error) => {
            if let Err(rollback_error) = conn.execute_batch("ROLLBACK;") {
                warn!(
                    %rollback_error,
                    "Failed to roll back notes index rebuild transaction"
                );
            }
            Err(error)
        }
    }
}

fn rebuild_index_from_files_with_conn_inner(conn: &Connection) -> Result<()> {
    clear_index_tables(conn)?;

    let substrate = notes_substrate()?;
    let notes_dir = substrate.paths().notes_dir();
    if notes_dir.exists() {
        for entry in fs::read_dir(&notes_dir)
            .with_context(|| format!("reading notes dir {}", notes_dir.display()))?
        {
            let entry = entry.context("reading notes dir entry")?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            if is_conflict_copy_path(&path) {
                continue;
            }
            // Skip-and-log a single malformed/newer note file rather than
            // aborting the whole rebuild — one hand-edited or version-skewed
            // file must not be able to wedge the entire notes index (which
            // rebuilds on init after a corrupt-DB recovery).
            let (note, slug, hash) = match load_note_from_file(&substrate, &path, None, 0) {
                Ok(loaded) => loaded,
                Err(error) => {
                    warn!(
                        path = %path.display(),
                        %error,
                        "Skipping unreadable note during index rebuild"
                    );
                    continue;
                }
            };
            upsert_note_index_with_conn(conn, &note, &slug, &hash)?;
            remember_note_hash(note.id, hash);
        }
    }

    let trash_dir = substrate.paths().trash_dir();
    if trash_dir.exists() {
        for entry in fs::read_dir(&trash_dir)
            .with_context(|| format!("reading trash dir {}", trash_dir.display()))?
        {
            let entry = entry.context("reading trash dir entry")?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            if is_conflict_copy_path(&path) {
                continue;
            }
            let deleted_at = Some(deleted_at_from_trash_path(&path));
            let (note, slug, hash) = match load_note_from_file(&substrate, &path, deleted_at, 0) {
                Ok(loaded) => loaded,
                Err(error) => {
                    warn!(
                        path = %path.display(),
                        %error,
                        "Skipping unreadable trashed note during index rebuild"
                    );
                    continue;
                }
            };
            upsert_note_index_with_conn(conn, &note, &slug, &hash)?;
            remember_note_hash(note.id, hash);
        }
    }

    restore_note_cart_items_after_rebuild(conn)?;
    recompute_all_note_link_targets_with_conn(conn)?;
    rebuild_notes_search_index_with_conn(conn)?;
    conn.execute(
        &format!("PRAGMA user_version = {NOTES_INDEX_SCHEMA_VERSION}"),
        [],
    )
    .context("Failed to set notes index schema version")?;
    Ok(())
}

fn upsert_note_index_with_conn(
    conn: &Connection,
    note: &Note,
    slug: &str,
    hash: &str,
) -> Result<()> {
    conn.execute(
        r#"
        INSERT INTO notes (
            id, title, content, created_at, updated_at, deleted_at,
            is_pinned, sort_order, file_slug, content_hash
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            content = excluded.content,
            updated_at = excluded.updated_at,
            deleted_at = excluded.deleted_at,
            is_pinned = excluded.is_pinned,
            sort_order = excluded.sort_order,
            file_slug = excluded.file_slug,
            content_hash = excluded.content_hash
        "#,
        params![
            note.id.as_str(),
            note.title,
            note.content,
            note.created_at.to_rfc3339(),
            note.updated_at.to_rfc3339(),
            note.deleted_at.map(|dt| dt.to_rfc3339()),
            note.is_pinned as i32,
            note.sort_order,
            slug,
            hash,
        ],
    )
    .context("Failed to upsert note index row")?;
    replace_note_metadata_with_conn(conn, note)?;
    Ok(())
}

fn write_canonical_note_file(
    substrate: &BrainSubstrate,
    note: &Note,
    slug: &str,
) -> Result<String> {
    let path = substrate.paths().note_file(slug);
    let preserved_source = if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|raw| substrate.parse_document(&raw).ok())
            .and_then(|(frontmatter, _)| frontmatter.source)
    } else {
        None
    };

    guard_external_edit_before_write(&path, note.id)?;

    let frontmatter = brain_frontmatter_from_note(note, preserved_source);
    let body = note_body_for_file(&note.content);
    substrate.write_document(&path, &frontmatter, &body)?;

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("reading note file after write {}", path.display()))?;
    Ok(content_hash(&raw))
}

fn trash_canonical_note_file(substrate: &BrainSubstrate, slug: &str) -> Result<()> {
    let path = substrate.paths().note_file(slug);
    if path.exists() {
        substrate.trash(&path)?;
    }
    Ok(())
}

fn restore_canonical_note_file(substrate: &BrainSubstrate, slug: &str) -> Result<()> {
    let destination = substrate.paths().note_file(slug);
    let trash_dir = substrate.paths().trash_dir();
    if !trash_dir.exists() {
        return Ok(());
    }

    let suffix = format!("{slug}.md");
    for entry in fs::read_dir(&trash_dir)
        .with_context(|| format!("reading trash dir {}", trash_dir.display()))?
    {
        let entry = entry.context("reading trash entry")?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.ends_with(".md") {
            continue;
        }
        if name == suffix || name.starts_with(&format!("{slug}-")) {
            substrate.restore(&path, &destination)?;
            return Ok(());
        }
    }
    Ok(())
}

fn delete_trashed_note_file(substrate: &BrainSubstrate, slug: &str) -> Result<()> {
    let trash_dir = substrate.paths().trash_dir();
    if !trash_dir.exists() {
        return Ok(());
    }
    let suffix = format!("{slug}.md");
    for entry in fs::read_dir(&trash_dir)
        .with_context(|| format!("reading trash dir {}", trash_dir.display()))?
    {
        let entry = entry.context("reading trash entry")?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name == suffix || name.starts_with(&format!("{slug}-")) {
            fs::remove_file(&path)
                .with_context(|| format!("removing trashed note {}", path.display()))?;
            return Ok(());
        }
    }
    Ok(())
}

fn reindex_external_note_file(path: &Path) -> Result<()> {
    if is_conflict_copy_path(path) {
        return Ok(());
    }
    if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return Ok(());
    }

    let substrate = notes_substrate()?;
    let notes_dir = substrate.paths().notes_dir();
    if !path.starts_with(&notes_dir) {
        return Ok(());
    }

    if !path.exists() {
        if let Some(slug) = slug_from_path(path) {
            let db = get_db()?;
            let conn = db.lock().map_err(db_lock_err)?;
            if let Some(note_id) = conn
                .query_row(
                    "SELECT id FROM notes WHERE file_slug = ?1",
                    params![slug],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .context("Failed to look up note id for deleted file")?
            {
                if let Some(id) = NoteId::parse(&note_id) {
                    conn.execute("DELETE FROM notes WHERE id = ?1", params![id.as_str()])
                        .context("Failed to remove deleted note from index")?;
                    forget_note_hash(id);
                    invalidate_root_notes_search_cache();
                }
            }
        }
        return Ok(());
    }

    let raw = fs::read_to_string(path)
        .with_context(|| format!("reading external note edit {}", path.display()))?;
    let hash = content_hash(&raw);
    let (note, slug, _) = load_note_from_file(&substrate, path, None, 0)?;

    let known_hash = note_content_hashes()
        .lock()
        .map_err(db_lock_err)?
        .get(&note.id)
        .cloned();
    if known_hash.as_deref() == Some(hash.as_str()) {
        return Ok(());
    }

    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;
    let sort_order = conn
        .query_row(
            "SELECT sort_order FROM notes WHERE id = ?1",
            params![note.id.as_str()],
            |row| row.get::<_, i32>(0),
        )
        .unwrap_or(0);
    let mut indexed = note;
    indexed.sort_order = sort_order;
    upsert_note_index_with_conn(&conn, &indexed, &slug, &hash)?;
    remember_note_hash(indexed.id, hash);
    invalidate_root_notes_search_cache();
    debug!(note_id = %indexed.id, file = %path.display(), "Reindexed externally edited note");
    Ok(())
}

fn start_notes_dir_watcher() {
    if NOTES_DIR_WATCHER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    let Ok(substrate) = notes_substrate() else {
        return;
    };
    let notes_dir = substrate.paths().notes_dir();
    let _ = fs::create_dir_all(&notes_dir);

    let spawn_result = std::thread::Builder::new()
        .name("notes-brain-watcher".to_string())
        .spawn(move || notes_dir_watcher_loop(notes_dir));

    if let Err(error) = spawn_result {
        warn!(%error, "Failed to start notes brain directory watcher");
        NOTES_DIR_WATCHER_STARTED.store(false, Ordering::SeqCst);
    }
}

fn notes_dir_watcher_loop(notes_dir: PathBuf) {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = match recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(watcher) => watcher,
        Err(error) => {
            warn!(%error, "Failed to create notes brain watcher");
            NOTES_DIR_WATCHER_STARTED.store(false, Ordering::SeqCst);
            return;
        }
    };

    if let Err(error) = watcher.watch(&notes_dir, RecursiveMode::NonRecursive) {
        warn!(
            %error,
            dir = %notes_dir.display(),
            "Failed to watch notes brain directory"
        );
        NOTES_DIR_WATCHER_STARTED.store(false, Ordering::SeqCst);
        return;
    }

    let debounce = Duration::from_millis(crate::config::defaults::DEFAULT_WATCHER_DEBOUNCE_MS);
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(event)) => {
                for path in event.paths {
                    if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                        pending.insert(path, Instant::now() + debounce);
                    }
                }
            }
            Ok(Err(error)) => {
                warn!(%error, "Notes brain watcher notify error");
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }

        let now = Instant::now();
        let ready: Vec<PathBuf> = pending
            .iter()
            .filter_map(|(path, deadline)| (*deadline <= now).then_some(path.clone()))
            .collect();
        for path in ready {
            pending.remove(&path);
            if let Err(error) = reindex_external_note_file(&path) {
                warn!(%error, file = %path.display(), "Failed to reindex external note edit");
            }
        }
    }
}

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
    let conn = Connection::open(path).context("Failed to open notes database")?;
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
    crate::utils::db_permissions::harden_sqlite_permissions(path);
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

/// Initialize the notes database
///
/// This function is idempotent - it's safe to call multiple times.
/// If the database is already initialized, it verifies schema and triggers
/// are up-to-date on the existing connection.
pub fn init_notes_db() -> Result<()> {
    let _init_guard = NOTES_DB_INIT_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let substrate = notes_substrate()?;
    let _ = fs::create_dir_all(substrate.paths().notes_dir());
    let _ = fs::create_dir_all(substrate.paths().trash_dir());

    if let Some(db) = NOTES_DB.get() {
        let conn = db.lock().map_err(db_lock_err)?;

        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .context("Failed to enable notes foreign keys")?;
        ensure_notes_schema(&conn)?;
        if schema_needs_rebuild(&conn)? {
            rebuild_index_from_files_with_conn(&conn)
                .context("Failed to rebuild notes index from brain files")?;
        } else {
            backfill_note_metadata_with_conn(&conn)
                .context("Failed to backfill notes metadata schema")?;
        }
        start_notes_dir_watcher();
        debug!("Notes database already initialized, schema verified");
        return Ok(());
    }

    let db_path = get_notes_db_path();

    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).context("Failed to create notes db directory")?;
    }

    let db_exists = db_path.exists();
    let (conn, recovered) = open_or_recover_notes_db(&db_path)?;

    if !db_exists || recovered || schema_needs_rebuild(&conn)? {
        rebuild_index_from_files_with_conn(&conn)
            .context("Failed to rebuild notes index from brain files")?;
    } else {
        backfill_note_metadata_with_conn(&conn)
            .context("Failed to backfill notes metadata schema")?;
        rebuild_notes_search_index_with_conn(&conn)
            .context("Failed to backfill notes FTS index")?;
        conn.execute(
            &format!("PRAGMA user_version = {NOTES_INDEX_SCHEMA_VERSION}"),
            [],
        )
        .context("Failed to set notes index schema version")?;
    }

    info!(db_path = %db_path.display(), "Notes database initialized");

    let _ = NOTES_DB.get_or_init(|| Arc::new(Mutex::new(conn)));

    start_notes_dir_watcher();
    Ok(())
}

/// Rebuild the FTS index so that pre-existing notes rows become searchable.
///
/// Uses the FTS5 `'rebuild'` command which drops and repopulates the index
/// from the content table. Safe to call repeatedly (idempotent).
fn rebuild_notes_search_index_with_conn(conn: &Connection) -> Result<()> {
    conn.execute("INSERT INTO notes_fts(notes_fts) VALUES('rebuild')", [])
        .context("Failed to rebuild notes FTS index")?;
    info!("Rebuilt notes FTS index");
    Ok(())
}

/// Rebuild the full-text search index for notes.
///
/// Public wrapper that acquires the DB lock. Call this when you suspect the
/// FTS index is out of sync with the notes table (e.g. after a migration).
pub fn rebuild_notes_search_index() -> Result<()> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;
    rebuild_notes_search_index_with_conn(&conn)
}

/// Get a reference to the notes database connection
fn get_db() -> Result<Arc<Mutex<Connection>>> {
    NOTES_DB
        .get()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Notes database not initialized"))
}

/// Save a note (insert or update)
pub fn save_note(note: &Note) -> Result<()> {
    let substrate = notes_substrate()?;
    let db = get_db()?;
    let mut conn = db.lock().map_err(db_lock_err)?;

    let slug = resolve_note_slug(&conn, note)?;
    let hash = if note.deleted_at.is_some() {
        trash_canonical_note_file(&substrate, &slug)?;
        String::new()
    } else {
        restore_canonical_note_file(&substrate, &slug)?;
        write_canonical_note_file(&substrate, note, &slug)?
    };

    let tx = conn
        .transaction()
        .context("Failed to start note save transaction")?;

    upsert_note_index_with_conn(&tx, note, &slug, &hash)?;
    tx.commit()
        .context("Failed to commit note save transaction")?;

    if !hash.is_empty() {
        remember_note_hash(note.id, hash);
    } else if note.deleted_at.is_some() {
        forget_note_hash(note.id);
    }

    debug!(note_id = %note.id, title = %note.title, slug = %slug, "Note saved to brain file");
    crate::dev_marker::log_marker_note_explanation_if_ready(&note.id, &note.content);
    invalidate_root_notes_search_cache();
    Ok(())
}

fn replace_note_metadata_with_conn(conn: &Connection, note: &Note) -> Result<()> {
    let parsed = metadata::parse_note_metadata(&note.title, &note.content);
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "DELETE FROM note_tags WHERE note_id = ?1",
        params![note.id.as_str()],
    )
    .context("Failed to clear note tags")?;
    conn.execute(
        "DELETE FROM note_aliases WHERE note_id = ?1",
        params![note.id.as_str()],
    )
    .context("Failed to clear note aliases")?;
    conn.execute(
        "DELETE FROM note_links WHERE source_note_id = ?1",
        params![note.id.as_str()],
    )
    .context("Failed to clear note links")?;

    for tag in parsed.tags {
        conn.execute(
            r#"
            INSERT OR REPLACE INTO note_tags (note_id, tag, normalized_tag, source, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                note.id.as_str(),
                tag.display,
                tag.normalized,
                tag.source,
                now,
            ],
        )
        .context("Failed to insert note tag")?;
    }

    for alias in parsed.aliases {
        conn.execute(
            r#"
            INSERT OR REPLACE INTO note_aliases (note_id, alias, slug, source, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![note.id.as_str(), alias.alias, alias.slug, alias.source, now,],
        )
        .context("Failed to insert note alias")?;
    }

    for link in parsed.links {
        let target_note_id = resolve_note_link_target(conn, &link.target_slug)?;
        conn.execute(
            r#"
            INSERT OR REPLACE INTO note_links
                (source_note_id, target_note_id, target_ref, target_slug, label, kind, byte_start, byte_end, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                note.id.as_str(),
                target_note_id.map(|id| id.as_str()),
                link.target_ref,
                link.target_slug,
                link.label,
                link.kind,
                link.byte_start as i64,
                link.byte_end as i64,
                now,
            ],
        )
        .context("Failed to insert note link")?;
    }

    recompute_all_note_link_targets_with_conn(conn)?;
    Ok(())
}

fn resolve_note_link_target(conn: &Connection, target_slug: &str) -> Result<Option<NoteId>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT note_id
            FROM note_aliases
            WHERE slug = ?1
            ORDER BY source = 'title' DESC, updated_at DESC
            LIMIT 2
            "#,
        )
        .context("Failed to prepare note link resolution query")?;
    let matches = stmt
        .query_map(params![target_slug], |row| row.get::<_, String>(0))
        .context("Failed to query note aliases for link resolution")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect note alias matches")?;

    if matches.len() == 1 {
        Ok(NoteId::parse(&matches[0]))
    } else {
        Ok(None)
    }
}

fn resolve_unresolved_links_with_conn(conn: &Connection) -> Result<()> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT DISTINCT target_slug
            FROM note_links
            WHERE target_note_id IS NULL
            "#,
        )
        .context("Failed to prepare unresolved note links query")?;
    let slugs = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .context("Failed to query unresolved note links")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect unresolved note links")?;
    drop(stmt);

    for slug in slugs {
        if let Some(target_id) = resolve_note_link_target(conn, &slug)? {
            conn.execute(
                "UPDATE note_links SET target_note_id = ?1 WHERE target_slug = ?2 AND target_note_id IS NULL",
                params![target_id.as_str(), slug],
            )
            .context("Failed to resolve note links")?;
        }
    }

    Ok(())
}

fn recompute_all_note_link_targets_with_conn(conn: &Connection) -> Result<()> {
    conn.execute("UPDATE note_links SET target_note_id = NULL", [])
        .context("Failed to clear note link targets")?;
    resolve_unresolved_links_with_conn(conn)
}

fn backfill_note_metadata_with_conn(conn: &Connection) -> Result<()> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, title, content, created_at, updated_at, deleted_at, is_pinned, sort_order
            FROM notes
            "#,
        )
        .context("Failed to prepare notes metadata backfill query")?;
    let notes = stmt
        .query_map([], row_to_note)
        .context("Failed to query notes for metadata backfill")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect notes for metadata backfill")?;
    drop(stmt);

    for note in notes {
        replace_note_metadata_with_conn(conn, &note)?;
    }

    recompute_all_note_link_targets_with_conn(conn)?;
    Ok(())
}

/// Get a note by ID
/// Count active (non-deleted) notes carrying the given normalized tag.
///
/// Used to decide whether instruction notes should be staged on new Agent
/// Chat threads without paying for a full list read.
pub(crate) fn count_active_notes_with_tag(tag: &str) -> Result<u64> {
    let Some(normalized) = metadata::normalize_tag(tag) else {
        return Ok(0);
    };

    init_notes_db()?;
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let count: i64 = conn
        .query_row(
            r#"
            SELECT COUNT(DISTINCT t.note_id)
            FROM note_tags t
            JOIN notes n ON n.id = t.note_id
            WHERE t.normalized_tag = ?1 AND n.deleted_at IS NULL
            "#,
            params![normalized],
            |row| row.get(0),
        )
        .context("Failed to count notes with tag")?;

    Ok(count.max(0) as u64)
}

/// Result of resolving a wiki-link target reference against note aliases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoteRefResolution {
    Unique(NoteId),
    Ambiguous,
    NotFound,
}

/// Resolve a `[[wiki link]]` target (title or alias text) to a note.
pub(crate) fn resolve_note_ref(target: &str) -> Result<NoteRefResolution> {
    let slug = metadata::slugify_note_ref(target);
    if slug.is_empty() {
        return Ok(NoteRefResolution::NotFound);
    }

    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT DISTINCT a.note_id
            FROM note_aliases a
            JOIN notes n ON n.id = a.note_id
            WHERE a.slug = ?1 AND n.deleted_at IS NULL
            "#,
        )
        .context("Failed to prepare note ref resolution query")?;
    let matches = stmt
        .query_map(params![slug], |row| row.get::<_, String>(0))
        .context("Failed to query note aliases for ref resolution")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect note ref matches")?;

    match matches.len() {
        0 => Ok(NoteRefResolution::NotFound),
        1 => Ok(NoteId::parse(&matches[0])
            .map(NoteRefResolution::Unique)
            .unwrap_or(NoteRefResolution::NotFound)),
        _ => Ok(NoteRefResolution::Ambiguous),
    }
}

pub fn get_note(id: NoteId) -> Result<Option<Note>> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, title, content, created_at, updated_at, deleted_at, is_pinned, sort_order
            FROM notes
            WHERE id = ?1
            "#,
        )
        .context("Failed to prepare get_note query")?;

    let result = stmt
        .query_row(params![id.as_str()], row_to_note)
        .optional()
        .context("Failed to get note")?;

    Ok(result)
}

/// Get all active notes (not deleted), sorted by pinned first then updated_at desc
pub fn get_all_notes() -> Result<Vec<Note>> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, title, content, created_at, updated_at, deleted_at, is_pinned, sort_order
            FROM notes
            WHERE deleted_at IS NULL
            ORDER BY is_pinned DESC, updated_at DESC
            "#,
        )
        .context("Failed to prepare get_all_notes query")?;

    let notes = stmt
        .query_map([], row_to_note)
        .context("Failed to query notes")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect notes")?;

    debug!(count = notes.len(), "Retrieved all notes");
    Ok(notes)
}

/// Get notes in trash (soft-deleted)
pub fn get_deleted_notes() -> Result<Vec<Note>> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, title, content, created_at, updated_at, deleted_at, is_pinned, sort_order
            FROM notes
            WHERE deleted_at IS NOT NULL
            ORDER BY deleted_at DESC
            "#,
        )
        .context("Failed to prepare get_deleted_notes query")?;

    let notes = stmt
        .query_map([], row_to_note)
        .context("Failed to query deleted notes")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect deleted notes")?;

    debug!(count = notes.len(), "Retrieved deleted notes");
    Ok(notes)
}

/// Sanitize a query string for FTS5 MATCH
///
/// FTS5 special characters that need escaping: * " ' ( ) : - ^
/// We wrap the query in double quotes for phrase matching and escape internal quotes.
fn sanitize_fts_query(query: &str) -> String {
    let escaped = query.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

/// Search notes using full-text search
///
/// Uses FTS5 search when possible with a fallback to LIKE queries for robustness
/// against special characters that break FTS5 MATCH syntax.
pub fn search_notes(query: &str) -> Result<Vec<Note>> {
    if query.trim().is_empty() {
        return get_all_notes();
    }

    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    if let Some(metadata_notes) = search_notes_metadata_only(&conn, query)? {
        debug!(query = %query, count = metadata_notes.len(), method = "metadata_only", "Note search completed");
        return Ok(metadata_notes);
    }

    // Try FTS search first with sanitized query
    let sanitized_query = sanitize_fts_query(query);

    // FTS5 search with BM25 ranking
    let fts_result: rusqlite::Result<Vec<Note>> = (|| {
        let mut stmt = conn.prepare(
            r#"
            SELECT n.id, n.title, n.content, n.created_at, n.updated_at,
                   n.deleted_at, n.is_pinned, n.sort_order
            FROM notes n
            INNER JOIN notes_fts fts ON n.rowid = fts.rowid
            WHERE notes_fts MATCH ?1 AND n.deleted_at IS NULL
            ORDER BY bm25(notes_fts)
            LIMIT 200
            "#,
        )?;

        let notes = stmt
            .query_map(params![sanitized_query], row_to_note)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(notes)
    })();

    match fts_result {
        Ok(notes) => {
            debug!(query = %query, count = notes.len(), method = "fts", "Note search completed");
            Ok(notes)
        }
        Err(e) => {
            // FTS failed (possibly due to special characters), fall back to LIKE search
            debug!(
                query = %query,
                error = %e,
                method = "like_fallback",
                "FTS search failed, using LIKE fallback"
            );

            let like_pattern = format!("%{}%", query);
            let mut stmt = conn
                .prepare(
                    r#"
                    SELECT id, title, content, created_at, updated_at,
                           deleted_at, is_pinned, sort_order
                    FROM notes
                    WHERE deleted_at IS NULL
                      AND (title LIKE ?1 OR content LIKE ?1)
                    ORDER BY updated_at DESC
                    "#,
                )
                .context("Failed to prepare LIKE fallback query")?;

            let notes = stmt
                .query_map(params![like_pattern], row_to_note)
                .context("Failed to execute LIKE fallback search")?
                .collect::<Result<Vec<_>, _>>()
                .context("Failed to collect LIKE fallback results")?;

            debug!(query = %query, count = notes.len(), method = "like_fallback", "Note search completed");
            Ok(notes)
        }
    }
}

fn search_notes_metadata_only(conn: &Connection, query: &str) -> Result<Option<Vec<Note>>> {
    let trimmed = query.trim();
    if let Some(tag) = trimmed
        .strip_prefix("tag:")
        .or_else(|| trimmed.strip_prefix('#'))
    {
        return search_notes_by_metadata(conn, "tag", tag).map(Some);
    }
    if let Some(alias) = trimmed.strip_prefix("alias:") {
        return search_notes_by_metadata(conn, "alias", alias).map(Some);
    }
    if let Some(link) = trimmed.strip_prefix("link:") {
        return search_notes_by_metadata(conn, "link", link).map(Some);
    }
    Ok(None)
}

fn search_notes_by_metadata(conn: &Connection, mode: &str, query: &str) -> Result<Vec<Note>> {
    let normalized = match mode {
        "tag" => metadata::normalize_tag(query),
        "alias" | "link" => {
            let slug = metadata::slugify_note_ref(query);
            (!slug.is_empty()).then_some(slug)
        }
        _ => metadata::normalize_tag(query).or_else(|| {
            let slug = metadata::slugify_note_ref(query);
            (!slug.is_empty()).then_some(slug)
        }),
    };
    let Some(normalized) = normalized else {
        return Ok(Vec::new());
    };
    let pattern = format!("{}%", normalized);

    let condition = match mode {
        "tag" => "t.normalized_tag LIKE ?1",
        "alias" => "a.slug LIKE ?1",
        "link" => "l.target_slug LIKE ?1",
        _ => "t.normalized_tag LIKE ?1 OR a.slug LIKE ?1 OR l.target_slug LIKE ?1",
    };
    let sql = format!(
        r#"
        SELECT DISTINCT n.id, n.title, n.content, n.created_at, n.updated_at,
               n.deleted_at, n.is_pinned, n.sort_order
        FROM notes n
        LEFT JOIN note_tags t ON t.note_id = n.id
        LEFT JOIN note_aliases a ON a.note_id = n.id
        LEFT JOIN note_links l ON l.source_note_id = n.id
        WHERE n.deleted_at IS NULL AND ({condition})
        ORDER BY n.is_pinned DESC, n.updated_at DESC
        LIMIT 200
        "#
    );

    let mut stmt = conn
        .prepare(&sql)
        .context("Failed to prepare notes metadata search query")?;
    let notes = stmt
        .query_map(params![pattern], row_to_note)
        .context("Failed to execute notes metadata search")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect notes metadata search results")?;
    Ok(notes)
}

pub(crate) fn get_note_tags(note_id: NoteId) -> Result<Vec<String>> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT tag
            FROM note_tags
            WHERE note_id = ?1
            ORDER BY normalized_tag ASC
            "#,
        )
        .context("Failed to prepare note tags query")?;
    let tags = stmt
        .query_map(params![note_id.as_str()], |row| row.get::<_, String>(0))
        .context("Failed to query note tags")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect note tags")?;
    Ok(tags)
}

pub(crate) fn get_note_aliases(note_id: NoteId) -> Result<Vec<String>> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT alias
            FROM note_aliases
            WHERE note_id = ?1
            ORDER BY slug ASC
            "#,
        )
        .context("Failed to prepare note aliases query")?;
    let aliases = stmt
        .query_map(params![note_id.as_str()], |row| row.get::<_, String>(0))
        .context("Failed to query note aliases")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect note aliases")?;
    Ok(aliases)
}

pub(crate) fn get_note_outbound_link_count(note_id: NoteId) -> Result<usize> {
    count_note_links(
        "SELECT COUNT(*) FROM note_links WHERE source_note_id = ?1",
        note_id,
    )
}

pub(crate) fn get_note_backlink_count(note_id: NoteId) -> Result<usize> {
    count_note_links(
        r#"
        SELECT COUNT(DISTINCT l.source_note_id)
        FROM note_links l
        JOIN notes n ON n.id = l.source_note_id
        WHERE l.target_note_id = ?1
          AND n.deleted_at IS NULL
        "#,
        note_id,
    )
}

pub(crate) fn get_note_backlinks(note_id: NoteId) -> Result<Vec<NoteBacklinkSummary>> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT DISTINCT n.id, n.title, n.updated_at
            FROM note_links l
            JOIN notes n ON n.id = l.source_note_id
            WHERE l.target_note_id = ?1 AND n.deleted_at IS NULL
            ORDER BY n.updated_at DESC
            "#,
        )
        .context("Failed to prepare note backlinks query")?;
    let backlinks = stmt
        .query_map(params![note_id.as_str()], |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let updated_at_str: String = row.get(2)?;
            let id = NoteId::parse(&id).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    format!("Invalid backlink source note UUID: {id}").into(),
                )
            })?;
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            Ok(NoteBacklinkSummary {
                id,
                title,
                updated_at,
            })
        })
        .context("Failed to query note backlinks")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect note backlinks")?;
    Ok(backlinks)
}

fn count_note_links(sql: &str, note_id: NoteId) -> Result<usize> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;
    let count: i64 = conn
        .query_row(sql, params![note_id.as_str()], |row| row.get(0))
        .context("Failed to count note links")?;
    Ok(count.max(0) as usize)
}

/// Search notes for root launcher rows without returning note body content.
pub(crate) fn search_root_notes_meta(
    query: &str,
    options: RootNotesSectionOptions,
) -> Vec<RootNoteSearchHit> {
    if !root_notes_query_is_eligible(query, options) {
        return Vec::new();
    }

    match search_root_notes_meta_result(query.trim(), options) {
        Ok(hits) => hits,
        Err(error) => {
            tracing::warn!(
                query = %query,
                error = %error,
                "root_notes_search_failed"
            );
            Vec::new()
        }
    }
}

pub(crate) fn search_root_notes_meta_direct(
    query: &str,
    options: RootNotesSectionOptions,
) -> Vec<RootNoteSearchHit> {
    search_root_notes_meta(query, options)
}

/// Cache-only root notes lookup for the launcher foreground search path.
///
/// A cold query starts a background SQLite search and returns no hits for the
/// active frame. The worker only warms a future frame cache; it does not notify
/// or invalidate the current launcher rows.
pub(crate) fn search_root_notes_meta_cached(
    query: &str,
    options: RootNotesSectionOptions,
) -> Vec<RootNoteSearchHit> {
    if !root_notes_query_is_eligible(query, options) {
        return Vec::new();
    }

    let key = root_notes_search_cache_key(query, options);
    let generation = ROOT_NOTES_SEARCH_CACHE_GENERATION.load(Ordering::Relaxed);
    let flight = RootNotesSearchFlightKey {
        generation,
        search: key.clone(),
    };

    if let Ok(mut guard) = root_notes_search_cache().lock() {
        if let Some(hits) = guard.hits_by_query.get(&key) {
            return hits.clone();
        }
        if !guard.in_flight.insert(flight.clone()) {
            return Vec::new();
        }
    } else {
        return Vec::new();
    }

    let query = key.query.clone();
    let key_for_worker = key.clone();
    let flight_for_worker = flight.clone();
    let spawn_result = std::thread::Builder::new()
        .name("root-notes-search-cache".to_string())
        .spawn(move || {
            let hits = search_root_notes_meta(&query, options);
            if let Ok(mut guard) = root_notes_search_cache().lock() {
                guard.in_flight.remove(&flight_for_worker);
                if ROOT_NOTES_SEARCH_CACHE_GENERATION.load(Ordering::Relaxed) == generation {
                    guard.hits_by_query.insert(key_for_worker, hits);
                }
            }
        });

    if spawn_result.is_err() {
        if let Ok(mut guard) = root_notes_search_cache().lock() {
            guard.in_flight.remove(&flight);
        }
    }

    Vec::new()
}

fn search_root_notes_meta_result(
    query: &str,
    options: RootNotesSectionOptions,
) -> Result<Vec<RootNoteSearchHit>> {
    init_notes_db()?;
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let limit = options.max_results.clamp(1, 5) as i64;
    let hits = if query.trim().is_empty() {
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, title, updated_at, is_pinned, length(content)
                FROM notes
                WHERE deleted_at IS NULL
                ORDER BY is_pinned DESC, updated_at DESC
                LIMIT ?1
                "#,
            )
            .context("Failed to prepare root notes recent query")?;

        let rows = stmt
            .query_map(params![limit], row_to_root_note_hit)
            .context("Failed to execute root notes recent query")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to collect root notes recent results")?;
        rows
    } else if options.search_content {
        let sanitized_query = sanitize_fts_query(query);
        let mut stmt = conn
            .prepare(
                r#"
                SELECT n.id, n.title, n.updated_at, n.is_pinned, length(n.content)
                FROM notes n
                INNER JOIN notes_fts fts ON n.rowid = fts.rowid
                WHERE notes_fts MATCH ?1 AND n.deleted_at IS NULL
                ORDER BY bm25(notes_fts, 8.0, 1.0), n.is_pinned DESC, n.updated_at DESC
                LIMIT ?2
                "#,
            )
            .context("Failed to prepare root notes FTS query")?;

        let hits = stmt
            .query_map(params![sanitized_query, limit], row_to_root_note_hit)
            .context("Failed to execute root notes FTS query")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to collect root notes FTS results")?;
        if hits.is_empty() {
            search_root_notes_meta_like(&conn, query, true, limit)?
        } else {
            hits
        }
    } else {
        search_root_notes_meta_like(&conn, query, false, limit)?
    };

    Ok(hits
        .into_iter()
        .enumerate()
        .map(|(rank, mut hit)| {
            hit.score = i32::MAX.saturating_sub(rank as i32);
            hit
        })
        .collect())
}

fn search_root_notes_meta_like(
    conn: &Connection,
    query: &str,
    search_content: bool,
    limit: i64,
) -> Result<Vec<RootNoteSearchHit>> {
    let like_pattern = format!("%{}%", query);
    let exact = query.to_lowercase();
    let prefix = format!("{}%", exact);
    let mut stmt = if search_content {
        conn.prepare(
            r#"
            SELECT id, title, updated_at, is_pinned, length(content)
            FROM notes
            WHERE deleted_at IS NULL AND (title LIKE ?1 OR content LIKE ?1)
            ORDER BY
                CASE
                    WHEN lower(title) = ?2 THEN 0
                    WHEN lower(title) LIKE ?3 THEN 1
                    WHEN lower(title) LIKE ?1 THEN 2
                    ELSE 3
                END,
                is_pinned DESC,
                updated_at DESC
            LIMIT ?4
            "#,
        )
        .context("Failed to prepare root notes content LIKE query")?
    } else {
        conn.prepare(
            r#"
            SELECT id, title, updated_at, is_pinned, length(content)
            FROM notes
            WHERE deleted_at IS NULL AND title LIKE ?1
            ORDER BY
                CASE
                    WHEN lower(title) = ?2 THEN 0
                    WHEN lower(title) LIKE ?3 THEN 1
                    ELSE 2
                END,
                is_pinned DESC,
                updated_at DESC
            LIMIT ?4
            "#,
        )
        .context("Failed to prepare root notes title LIKE query")?
    };

    let hits = stmt
        .query_map(
            params![like_pattern, exact, prefix, limit],
            row_to_root_note_hit,
        )
        .context("Failed to execute root notes LIKE query")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect root notes LIKE results")?;
    Ok(hits)
}

/// Permanently delete a note
pub fn delete_note_permanently(id: NoteId) -> Result<()> {
    let substrate = notes_substrate()?;
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let slug = lookup_note_slug(&conn, id)?;
    conn.execute("DELETE FROM notes WHERE id = ?1", params![id.as_str()])
        .context("Failed to delete note")?;

    if let Some(slug) = slug {
        let active_path = substrate.paths().note_file(&slug);
        if active_path.exists() {
            fs::remove_file(&active_path)
                .with_context(|| format!("removing active note file {}", active_path.display()))?;
        }
        delete_trashed_note_file(&substrate, &slug)?;
    }

    forget_note_hash(id);
    info!(note_id = %id, "Note permanently deleted");
    invalidate_root_notes_search_cache();
    Ok(())
}

/// Permanently delete all soft-deleted notes in a single batch operation.
pub fn delete_all_deleted_notes() -> Result<()> {
    let substrate = notes_substrate()?;
    let db = get_db()?;
    let mut conn = db.lock().map_err(db_lock_err)?;

    let slugs: Vec<String> = conn
        .prepare(
            "SELECT file_slug FROM notes WHERE deleted_at IS NOT NULL AND file_slug IS NOT NULL",
        )?
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    let tx = conn
        .transaction()
        .context("Failed to start delete_all_deleted_notes transaction")?;

    let count = tx
        .execute("DELETE FROM notes WHERE deleted_at IS NOT NULL", [])
        .context("Failed to delete all soft-deleted notes")?;

    tx.commit()
        .context("Failed to commit delete_all_deleted_notes transaction")?;

    for slug in slugs {
        delete_trashed_note_file(&substrate, &slug)?;
    }

    info!(deleted_count = count, "Deleted all soft-deleted notes");
    if count > 0 {
        invalidate_root_notes_search_cache();
    }
    Ok(())
}

/// Prune notes deleted more than `days` ago
pub fn prune_old_deleted_notes(days: u32) -> Result<usize> {
    let substrate = notes_substrate()?;
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let cutoff = Utc::now() - chrono::Duration::days(days as i64);

    let slugs: Vec<String> = conn
        .prepare(
            "SELECT file_slug FROM notes WHERE deleted_at IS NOT NULL AND deleted_at < ?1 AND file_slug IS NOT NULL",
        )?
        .query_map(params![cutoff.to_rfc3339()], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    let count = conn
        .execute(
            "DELETE FROM notes WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
            params![cutoff.to_rfc3339()],
        )
        .context("Failed to prune old deleted notes")?;

    for slug in slugs {
        delete_trashed_note_file(&substrate, &slug)?;
    }

    if count > 0 {
        info!(count, days, "Pruned old deleted notes");
        invalidate_root_notes_search_cache();
    }

    Ok(count)
}

// ── Cart item persistence ───────────────────────────────────────────

/// Save a cart item (insert or update).
pub fn save_note_cart_item(item: &super::model::NoteCartItem) -> Result<()> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let payload_json =
        serde_json::to_string(&item.payload).context("Failed to serialize cart item payload")?;

    conn.execute(
        r#"
        INSERT INTO note_cart_items (id, note_id, label, payload_json, created_at, updated_at, sort_order)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(id) DO UPDATE SET
            label = excluded.label,
            payload_json = excluded.payload_json,
            updated_at = excluded.updated_at,
            sort_order = excluded.sort_order
        "#,
        params![
            item.id,
            item.note_id.as_str(),
            item.label,
            payload_json,
            item.created_at.to_rfc3339(),
            item.updated_at.to_rfc3339(),
            item.sort_order,
        ],
    )
    .context("Failed to save cart item")?;

    debug!(cart_item_id = %item.id, note_id = %item.note_id, "Cart item saved");
    Ok(())
}

/// List all cart items for a note, ordered by sort_order ascending.
pub fn list_note_cart_items(note_id: NoteId) -> Result<Vec<super::model::NoteCartItem>> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, note_id, label, payload_json, created_at, updated_at, sort_order
            FROM note_cart_items
            WHERE note_id = ?1
            ORDER BY sort_order ASC, updated_at DESC
            "#,
        )
        .context("Failed to prepare list_note_cart_items query")?;

    let items = stmt
        .query_map(params![note_id.as_str()], row_to_cart_item)
        .context("Failed to query cart items")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect cart items")?;

    debug!(note_id = %note_id, count = items.len(), "Retrieved cart items");
    Ok(items)
}

/// List cart items for a note, dropping duplicate payloads while preserving order.
pub fn list_note_cart_items_deduped(note_id: NoteId) -> Result<Vec<super::model::NoteCartItem>> {
    let mut items = list_note_cart_items(note_id)?;
    let mut seen = std::collections::HashSet::new();
    items.retain(|item| seen.insert(item.dedup_key()));
    Ok(items)
}

/// Delete a cart item by ID.
pub fn delete_note_cart_item(item_id: &str) -> Result<()> {
    let db = get_db()?;
    let conn = db.lock().map_err(db_lock_err)?;

    conn.execute(
        "DELETE FROM note_cart_items WHERE id = ?1",
        params![item_id],
    )
    .context("Failed to delete cart item")?;

    info!(cart_item_id = %item_id, "Cart item deleted");
    Ok(())
}

/// Delete multiple cart items for a note in one note-scoped transaction.
pub fn delete_note_cart_items(note_id: NoteId, item_ids: &[String]) -> Result<usize> {
    if item_ids.is_empty() {
        return Ok(0);
    }
    if std::env::var("SCRIPT_KIT_TEST_STATUS").ok().as_deref() == Some("1")
        && std::env::var("SCRIPT_KIT_TEST_NOTES_CART_DELETE_FAIL")
            .ok()
            .as_deref()
            == Some("1")
    {
        anyhow::bail!("test fixture refused note cart consumption");
    }

    let db = get_db()?;
    let mut conn = db.lock().map_err(db_lock_err)?;

    let tx = conn
        .transaction()
        .context("Failed to start note cart item delete transaction")?;

    let mut deleted = 0usize;
    for item_id in item_ids {
        deleted += tx
            .execute(
                "DELETE FROM note_cart_items WHERE note_id = ?1 AND id = ?2",
                params![note_id.as_str(), item_id],
            )
            .context("Failed to delete note-scoped cart item")?;
    }

    tx.commit()
        .context("Failed to commit note cart item delete transaction")?;

    info!(
        note_id = %note_id,
        requested = item_ids.len(),
        deleted,
        "Note cart items deleted"
    );
    Ok(deleted)
}

/// Convert a database row to a NoteCartItem.
fn row_to_cart_item(row: &rusqlite::Row) -> rusqlite::Result<super::model::NoteCartItem> {
    let id: String = row.get(0)?;
    let note_id_str: String = row.get(1)?;
    let label: String = row.get(2)?;
    let payload_json: String = row.get(3)?;
    let created_at_str: String = row.get(4)?;
    let updated_at_str: String = row.get(5)?;
    let sort_order: i32 = row.get(6)?;

    let note_id = NoteId::parse(&note_id_str).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Text,
            format!("Invalid note_id UUID in note_cart_items: {note_id_str}").into(),
        )
    })?;

    let payload: super::model::NoteCartItemPayload =
        serde_json::from_str(&payload_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e))
        })?;

    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    Ok(super::model::NoteCartItem {
        id,
        note_id,
        label,
        payload,
        created_at,
        updated_at,
        sort_order,
    })
}

/// Convert a database row to a Note
fn row_to_note(row: &rusqlite::Row) -> rusqlite::Result<Note> {
    let id_str: String = row.get(0)?;
    let title: String = row.get(1)?;
    let content: String = row.get(2)?;
    let created_at_str: String = row.get(3)?;
    let updated_at_str: String = row.get(4)?;
    let deleted_at_str: Option<String> = row.get(5)?;
    let is_pinned: i32 = row.get(6)?;
    let sort_order: i32 = row.get(7)?;

    let id = NoteId::parse(&id_str).unwrap_or_default();

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

    Ok(Note {
        id,
        title,
        content,
        created_at,
        updated_at,
        deleted_at,
        is_pinned: is_pinned != 0,
        sort_order,
    })
}

fn row_to_root_note_hit(row: &rusqlite::Row) -> rusqlite::Result<RootNoteSearchHit> {
    let id_str: String = row.get(0)?;
    let title: String = row.get(1)?;
    let updated_at_str: String = row.get(2)?;
    let is_pinned: i32 = row.get(3)?;
    let char_count: i64 = row.get(4)?;

    let id = NoteId::parse(&id_str).unwrap_or_default();
    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    Ok(RootNoteSearchHit {
        id,
        title,
        updated_at,
        is_pinned: is_pinned != 0,
        char_count: char_count.max(0) as usize,
        score: 0,
    })
}

/// Serialize tests that mutate the shared per-process notes DB.
///
/// Shared with `notes::menu_syntax_capture` tests, which hit the same DB.
/// Poison-tolerant so one failing test reports its own assertion instead of
/// cascading `PoisonError` panics into unrelated tests.
#[cfg(test)]
pub(crate) fn notes_db_test_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// A corrupt notes.sqlite must be moved aside and replaced with a fresh,
    /// working database — never surfaced as an error (which breaks deeplink
    /// open, search, and MCP note tools) or as a silently empty Notes list.
    /// Markdown is canonical, so the caller rebuilds the index when the
    /// recovery flag is true.
    #[test]
    fn open_or_recover_notes_db_moves_corrupt_db_aside_and_starts_fresh() {
        let dir = std::env::temp_dir().join(format!(
            "sk_notes_recovery_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        let db_path = dir.join("notes.sqlite");
        fs::write(&db_path, b"this is not a sqlite database").expect("write garbage db");

        let (conn, recovered) =
            open_or_recover_notes_db(&db_path).expect("recovery must succeed on corrupt db");
        assert!(recovered, "corrupt db must be reported as recovered");

        // The damaged file was preserved aside, not deleted.
        let corrupt_siblings = fs::read_dir(&dir)
            .expect("read temp dir")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"))
            .count();
        assert_eq!(corrupt_siblings, 1, "damaged db must be moved aside");

        // The fresh connection is usable with the notes schema in place.
        let note_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
            .expect("fresh db must have the notes schema");
        assert_eq!(note_count, 0);

        // A healthy db must open without triggering recovery.
        drop(conn);
        let (_conn, recovered_again) =
            open_or_recover_notes_db(&db_path).expect("healthy reopen must succeed");
        assert!(
            !recovered_again,
            "healthy db must not be treated as corrupt"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    fn unique_test_token(prefix: &str) -> String {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        format!(
            "{prefix}_{millis}_{}",
            NoteId::new().as_str().replace('-', "")
        )
    }

    fn text_cart_item(
        note_id: NoteId,
        id: String,
        label: String,
        text: String,
        sort_order: i32,
    ) -> crate::notes::model::NoteCartItem {
        let now = Utc::now();
        crate::notes::model::NoteCartItem {
            id,
            note_id,
            label,
            payload: crate::notes::model::NoteCartItemPayload::Text {
                text,
                source: "agentic://notes-cart-rebuild-test".to_string(),
                mime_type: Some("text/plain".to_string()),
            },
            created_at: now,
            updated_at: now,
            sort_order,
        }
    }

    #[test]
    fn test_db_path() {
        let path = get_notes_db_path();
        assert!(path.to_string_lossy().contains("notes.sqlite"));
    }

    #[test]
    fn test_search_notes_handles_special_characters() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize for special-character search");

        // Search with special characters should not error (even if no results)
        // These are FTS5 special characters that can break MATCH queries
        let special_queries = [
            "test@example.com", // @ symbol
            "foo*bar",          // wildcard
            "hello\"world",     // quote
            "foo:bar",          // colon (FTS column prefix syntax)
            "(test)",           // parentheses
            "test^2",           // caret (boost syntax)
            "test-query",       // hyphen (can be operator)
            "'test'",           // single quotes
            "test AND OR NOT",  // operators
        ];

        for query in special_queries {
            let result = search_notes(query);
            assert!(
                result.is_ok(),
                "Search with '{}' should not error: {:?}",
                query,
                result.err()
            );
        }
    }

    #[test]
    fn test_notes_au_trigger_has_when_guard_for_real_content_changes() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before trigger inspection");

        let db = get_db().expect("notes db should be initialized");
        let conn = db.lock().expect("notes db lock should succeed");

        let trigger_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'notes_au'",
                [],
                |row| row.get(0),
            )
            .expect("notes_au trigger should exist");

        assert!(
            trigger_sql.contains("WHEN OLD.title <> NEW.title OR OLD.content <> NEW.content"),
            "notes_au trigger should only fire when title/content differ: {trigger_sql}"
        );
    }

    #[test]
    fn test_init_notes_db_recreates_triggers_for_existing_connection() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before trigger recreation");

        let db = get_db().expect("notes db should be initialized");
        let conn = db.lock().expect("notes db lock should succeed");

        // Install a legacy unguarded trigger to simulate stale schema
        conn.execute_batch(
            r#"
            DROP TRIGGER IF EXISTS notes_au;
            CREATE TRIGGER notes_au AFTER UPDATE ON notes BEGIN
                INSERT INTO notes_fts(notes_fts, rowid, title, content)
                VALUES('delete', OLD.rowid, OLD.title, OLD.content);
                INSERT INTO notes_fts(rowid, title, content)
                VALUES (NEW.rowid, NEW.title, NEW.content);
            END;
            "#,
        )
        .expect("should install legacy notes_au trigger");
        drop(conn);

        // Re-init should verify schema and recreate triggers
        init_notes_db().expect("re-init should verify schema and recreate triggers");

        let db = get_db().expect("notes db should still be initialized");
        let conn = db.lock().expect("notes db lock should still succeed");

        let trigger_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'notes_au'",
                [],
                |row| row.get(0),
            )
            .expect("notes_au trigger should exist after re-init");

        assert!(
            trigger_sql.contains("WHEN OLD.title <> NEW.title OR OLD.content <> NEW.content"),
            "re-init should restore the guarded notes_au trigger: {trigger_sql}"
        );
    }

    #[test]
    fn test_search_notes_limits_fts_results_to_200() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before search limit test");
        let token = unique_test_token("search_limit");
        let now = Utc::now();
        let mut note_ids = Vec::new();

        for index in 0..220 {
            let note = Note {
                id: NoteId::new(),
                title: format!("{token} title {index}"),
                content: format!("{token} content {index}"),
                created_at: now,
                updated_at: now,
                deleted_at: None,
                is_pinned: false,
                sort_order: index,
            };

            save_note(&note).expect("failed to save note for search limit test");
            note_ids.push(note.id);
        }

        let results = search_notes(&token).expect("search should succeed");

        for id in note_ids {
            delete_note_permanently(id).expect("cleanup failed for search limit test");
        }

        assert!(
            results.len() <= 200,
            "search should cap FTS results at 200, got {}",
            results.len()
        );
    }

    #[test]
    fn test_root_notes_query_eligibility_respects_config() {
        let options = RootNotesSectionOptions {
            enabled: true,
            min_query_chars: 3,
            ..Default::default()
        };

        assert!(root_notes_query_is_eligible("fix", options));
        assert!(!root_notes_query_is_eligible("fi", options));
        assert!(!root_notes_query_is_eligible("fix\nnote", options));
        assert!(!root_notes_query_is_eligible(
            "fix",
            RootNotesSectionOptions {
                enabled: false,
                ..options
            }
        ));
    }

    #[test]
    fn test_search_root_notes_meta_is_bounded_active_only_and_metadata_only() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before root notes search test");
        let token = unique_test_token("root_notes");
        let now = Utc::now();
        let active = Note {
            id: NoteId::new(),
            title: format!("{token} active"),
            content: format!("{token} body that must not be returned"),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            is_pinned: true,
            sort_order: 0,
        };
        let deleted = Note {
            id: NoteId::new(),
            title: format!("{token} deleted"),
            content: format!("{token} deleted body"),
            created_at: now,
            updated_at: now,
            deleted_at: Some(now),
            is_pinned: false,
            sort_order: 1,
        };

        save_note(&active).expect("failed to save active note");
        save_note(&deleted).expect("failed to save deleted note");

        let hits = search_root_notes_meta(
            &token,
            RootNotesSectionOptions {
                enabled: true,
                max_results: 1,
                min_query_chars: 3,
                search_content: true,
            },
        );

        delete_note_permanently(active.id).expect("cleanup failed for active note");
        delete_note_permanently(deleted.id).expect("cleanup failed for deleted note");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, active.id);
        assert_eq!(hits[0].title, active.title);
        assert!(hits[0].is_pinned);
        assert_eq!(hits[0].char_count, active.content.chars().count());
    }

    #[test]
    fn test_search_root_notes_meta_matches_title_substrings_when_fts_has_no_hit() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before root notes substring test");
        let now = Utc::now();
        let note = Note {
            id: NoteId::new(),
            title: "Welcome to Notes".to_string(),
            content: "Starter content for source-filter search.".to_string(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            is_pinned: true,
            sort_order: 0,
        };

        save_note(&note).expect("failed to save welcome note");

        let hits = search_root_notes_meta(
            "not",
            RootNotesSectionOptions {
                enabled: true,
                max_results: 5,
                min_query_chars: 0,
                search_content: true,
            },
        );

        delete_note_permanently(note.id).expect("cleanup failed for welcome note");

        assert!(
            hits.iter()
                .any(|candidate| candidate.id == note.id && candidate.title == "Welcome to Notes"),
            "root note search should treat `not` as a substring/prefix match for `Notes`"
        );
    }

    #[test]
    fn test_delete_all_deleted_notes_removes_soft_deleted_notes_in_batch() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before batch delete test");
        let token = unique_test_token("batch_delete");
        let now = Utc::now();

        let deleted_note = Note {
            id: NoteId::new(),
            title: format!("{token} deleted"),
            content: format!("{token} deleted content"),
            created_at: now,
            updated_at: now,
            deleted_at: Some(now),
            is_pinned: false,
            sort_order: 0,
        };
        save_note(&deleted_note).expect("failed to save soft-deleted note");

        let active_note = Note {
            id: NoteId::new(),
            title: format!("{token} active"),
            content: format!("{token} active content"),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            is_pinned: false,
            sort_order: 1,
        };
        save_note(&active_note).expect("failed to save active note");

        delete_all_deleted_notes().expect("batch delete should succeed");

        let deleted_result = get_note(deleted_note.id).expect("query deleted note should succeed");
        let active_result = get_note(active_note.id).expect("query active note should succeed");

        delete_note_permanently(active_note.id).expect("cleanup failed for active note");

        assert!(
            deleted_result.is_none(),
            "soft-deleted note should be permanently removed by batch delete"
        );
        assert!(
            active_result.is_some(),
            "active note should not be removed by batch delete"
        );
    }

    #[test]
    fn test_rebuild_notes_search_index_recovers_desynced_rows() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before FTS rebuild test");
        let token = unique_test_token("fts_rebuild");
        let now = Utc::now();

        let note = Note {
            id: NoteId::new(),
            title: format!("{token} title"),
            content: format!("{token} content"),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            is_pinned: false,
            sort_order: 0,
        };

        save_note(&note).expect("failed to save note for fts rebuild test");

        // Manually remove the FTS row to simulate a desynced index
        let db = get_db().expect("notes db should be initialized");
        let conn = db.lock().expect("notes db lock should succeed");

        conn.execute(
            r#"
            INSERT INTO notes_fts(notes_fts, rowid, title, content)
            VALUES(
                'delete',
                (SELECT rowid FROM notes WHERE id = ?1),
                ?2,
                ?3
            )
            "#,
            params![note.id.as_str(), note.title.clone(), note.content.clone()],
        )
        .expect("failed to desync notes_fts row");
        drop(conn);

        // The note should NOT be searchable while desynced
        let missing = search_notes(&token).expect("search before rebuild should succeed");
        assert!(
            missing.iter().all(|candidate| candidate.id != note.id),
            "desynced note should not be searchable before rebuild"
        );

        // Rebuild should restore the index
        rebuild_notes_search_index().expect("fts rebuild should succeed");

        let rebuilt = search_notes(&token).expect("search after rebuild should succeed");
        delete_note_permanently(note.id).expect("cleanup failed for fts rebuild test");

        assert!(
            rebuilt.iter().any(|candidate| candidate.id == note.id),
            "fts rebuild should restore existing rows into notes_fts"
        );
    }

    #[test]
    fn test_search_notes_returns_matching_note_for_special_character_content() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before special-character match test");
        let token = unique_test_token("search_special_match");
        let query = format!("{token}@example.com");
        let now = Utc::now();

        let note = Note {
            id: NoteId::new(),
            title: format!("Contact {query}"),
            content: format!("Reach me at {query}"),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            is_pinned: false,
            sort_order: 0,
        };

        save_note(&note).expect("failed to save note for special character search test");

        // FTS5 index updates may lag under concurrent writes (nextest parallelism).
        // Retry briefly so the test is not flaky.
        let mut results = Vec::new();
        for _ in 0..5 {
            results = search_notes(&query).expect("search should succeed");
            if results.iter().any(|c| c.id == note.id) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        delete_note_permanently(note.id).expect("cleanup failed for special character search test");

        assert!(
            results.iter().any(|candidate| candidate.id == note.id),
            "search should return the note that contains the special-character query"
        );
    }

    #[test]
    fn test_note_metadata_tables_roundtrip() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before metadata roundtrip test");
        let token = unique_test_token("metadata_roundtrip");
        let note = Note::with_content(format!(
            "---\ntags: [{token}, notes/metadata]\naliases: [{token} Alias]\n---\n# Metadata Roundtrip\nBody #{token} [[Missing Target]]"
        ));
        let id = note.id;

        save_note(&note).expect("failed to save note with metadata");
        let tags = get_note_tags(id).expect("metadata tags should be readable");
        let aliases = get_note_aliases(id).expect("metadata aliases should be readable");
        let outbound_count =
            get_note_outbound_link_count(id).expect("outbound links should be countable");

        delete_note_permanently(id).expect("cleanup failed for metadata note");

        assert!(
            tags.iter().any(|tag| tag == &token),
            "frontmatter/hash tag should be indexed"
        );
        assert!(
            aliases
                .iter()
                .any(|alias| alias == &format!("{token} Alias")),
            "frontmatter alias should be indexed"
        );
        assert_eq!(outbound_count, 1);
    }

    #[test]
    fn test_search_notes_matches_tags_and_aliases() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before metadata search test");
        let token = unique_test_token("metadata_search");
        let note = Note::with_content(format!(
            "---\ntags: [{token}]\naliases: [{token} Alias]\n---\n# Searchable Metadata\nBody"
        ));
        let id = note.id;

        save_note(&note).expect("failed to save searchable metadata note");
        let tag_results = search_notes(&format!("tag:{token}")).expect("tag search should succeed");
        let alias_results =
            search_notes(&format!("alias:{token}-alias")).expect("alias search should succeed");

        delete_note_permanently(id).expect("cleanup failed for metadata search note");

        assert!(tag_results.iter().any(|candidate| candidate.id == id));
        assert!(alias_results.iter().any(|candidate| candidate.id == id));
    }

    #[test]
    fn test_count_active_notes_with_tag_ignores_soft_deleted_notes() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before tag count test");
        let token = unique_test_token("instr_count");
        let mut note = Note::with_content(format!("---\ntags: [{token}]\n---\n# Instruction"));
        let id = note.id;

        save_note(&note).expect("failed to save instruction note");
        let active_count = count_active_notes_with_tag(&token).expect("tag count should succeed");

        note.soft_delete();
        save_note(&note).expect("failed to soft-delete instruction note");
        let deleted_count =
            count_active_notes_with_tag(&token).expect("tag count after delete should succeed");

        delete_note_permanently(id).expect("cleanup failed for tag count note");

        assert_eq!(active_count, 1);
        assert_eq!(deleted_count, 0);
    }

    #[test]
    fn test_backlinks_resolve_after_target_note_is_created() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before backlink test");
        let token = unique_test_token("backlink_target");
        let source = Note::with_content(format!("# Source\n[[{token} Target]]"));
        let source_id = source.id;

        save_note(&source).expect("failed to save unresolved source link");
        let target = Note::with_content(format!("# {token} Target\nBody"));
        let target_id = target.id;
        save_note(&target).expect("failed to save target note");

        let backlink_count =
            get_note_backlink_count(target_id).expect("backlinks should be countable");
        let backlinks = get_note_backlinks(target_id).expect("backlinks should be readable");

        delete_note_permanently(source_id).expect("cleanup failed for source note");
        delete_note_permanently(target_id).expect("cleanup failed for target note");

        assert_eq!(backlink_count, 1);
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].id, source_id);
        assert_eq!(backlinks[0].title, "Source");
    }

    #[test]
    fn test_backlink_count_matches_distinct_active_backlink_sources() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before backlink count test");
        let token = unique_test_token("backlink_distinct");
        let target = Note::with_content(format!("# {token} Target\nBody"));
        let target_id = target.id;
        save_note(&target).expect("failed to save target note");
        let source = Note::with_content(format!(
            "# Source\n[[{token} Target]] and again [[{token} Target]]"
        ));
        let source_id = source.id;
        save_note(&source).expect("failed to save source note");

        assert_eq!(
            get_note_backlink_count(target_id).expect("backlink count should work"),
            1
        );
        assert_eq!(
            get_note_backlinks(target_id)
                .expect("backlinks should work")
                .len(),
            1
        );

        let mut deleted_source = get_note(source_id)
            .expect("source note lookup should work")
            .expect("source note should exist");
        deleted_source.soft_delete();
        save_note(&deleted_source).expect("failed to soft-delete source note");

        assert_eq!(
            get_note_backlink_count(target_id)
                .expect("backlink count should ignore deleted sources"),
            0
        );
        assert_eq!(
            get_note_backlinks(target_id)
                .expect("backlinks should ignore deleted sources")
                .len(),
            0
        );

        delete_note_permanently(source_id).expect("cleanup failed for source note");
        delete_note_permanently(target_id).expect("cleanup failed for target note");
    }

    #[test]
    fn test_metadata_backfills_existing_notes_after_schema_creation() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before metadata backfill test");
        let token = unique_test_token("metadata_backfill");
        let note = Note::with_content(format!("# Backfill\nBody #{token}"));
        let id = note.id;

        save_note(&note).expect("failed to save note before simulated metadata loss");
        {
            let db = get_db().expect("db should be initialized");
            let conn = db.lock().expect("db lock");
            conn.execute(
                "DELETE FROM note_tags WHERE note_id = ?1",
                params![id.as_str()],
            )
            .expect("failed to clear note tags");
        }

        init_notes_db().expect("init should backfill missing metadata");
        let tags = get_note_tags(id).expect("tags should be backfilled");

        delete_note_permanently(id).expect("cleanup failed for metadata backfill note");

        assert!(tags.iter().any(|tag| tag == &token));
    }

    #[test]
    fn test_save_note_persists_canonical_brain_markdown_file() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before canonical file test");
        let token = unique_test_token("canonical_file");
        let note = Note::with_content(format!("# {token}\nBody with #{token}"));
        let id = note.id;

        save_note(&note).expect("failed to save note");

        let db = get_db().expect("db");
        let conn = db.lock().expect("lock");
        let slug = lookup_note_slug(&conn, id)
            .expect("slug lookup")
            .expect("slug should exist after save");
        drop(conn);

        let substrate = notes_substrate().expect("substrate");
        let path = substrate.paths().note_file(&slug);
        assert!(
            path.exists(),
            "save_note should write canonical markdown at {}",
            path.display()
        );

        let raw = fs::read_to_string(&path).expect("read canonical note file");
        assert!(
            raw.contains(&id.as_str()),
            "file frontmatter should preserve note id"
        );
        assert!(
            raw.contains(&token),
            "file body should preserve note content"
        );

        delete_note_permanently(id).expect("cleanup");
    }

    #[test]
    fn test_save_note_preserves_source_frontmatter_in_canonical_file() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before source frontmatter test");
        let token = unique_test_token("canonical_source");
        let source = format!("scriptkit://agent-chat/{token}");
        let note = Note::with_content(format!(
            "---\nsource: {source}\n---\n# Agent Chat Conversation\nBody with {token}"
        ));
        let id = note.id;

        save_note(&note).expect("failed to save note with source frontmatter");

        let db = get_db().expect("db");
        let conn = db.lock().expect("lock");
        let slug = lookup_note_slug(&conn, id)
            .expect("slug lookup")
            .expect("slug should exist after save");
        drop(conn);

        let substrate = notes_substrate().expect("substrate");
        let path = substrate.paths().note_file(&slug);
        let raw = fs::read_to_string(&path).expect("read canonical note file");

        delete_note_permanently(id).expect("cleanup");

        assert!(
            raw.contains(&format!("source: {source}")),
            "canonical file should preserve source provenance in frontmatter: {raw}"
        );
    }

    #[test]
    fn test_rebuild_index_from_files_preserves_cart_items_for_rebuilt_notes() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before cart rebuild preserve test");
        let token = unique_test_token("cart_rebuild_preserve");
        let note = Note::with_content(format!("# {token}\nBody"));
        let note_id = note.id;
        save_note(&note).expect("failed to save canonical note");
        let item = text_cart_item(
            note_id,
            format!("{token}-cart"),
            "Cart Preserve".to_string(),
            format!("payload {token}"),
            7,
        );
        save_note_cart_item(&item).expect("failed to save cart item before rebuild");

        rebuild_index_from_files().expect("notes rebuild should succeed");
        let rebuilt_items =
            list_note_cart_items(note_id).expect("cart items should be readable after rebuild");

        delete_note_permanently(note_id).expect("cleanup failed for preserved cart note");

        assert_eq!(rebuilt_items.len(), 1);
        assert_eq!(rebuilt_items[0].id, item.id);
        assert_eq!(rebuilt_items[0].label, item.label);
        assert_eq!(rebuilt_items[0].sort_order, item.sort_order);
        assert_eq!(rebuilt_items[0].payload, item.payload);
    }

    #[test]
    fn test_rebuild_index_from_files_prunes_cart_items_for_missing_canonical_notes() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before cart prune test");
        let token = unique_test_token("cart_rebuild_prune");
        let stale_note = Note::with_content(format!("# {token}\nDB only"));
        let stale_note_id = stale_note.id;
        {
            let db = get_db().expect("db should be initialized");
            let conn = db.lock().expect("db lock should succeed");
            upsert_note_index_with_conn(
                &conn,
                &stale_note,
                &format!("{token}-missing-file"),
                "synthetic-hash",
            )
            .expect("failed to insert stale db-only note row");
        }
        let item = text_cart_item(
            stale_note_id,
            format!("{token}-orphan-cart"),
            "Orphan Cart".to_string(),
            format!("orphan payload {token}"),
            0,
        );
        save_note_cart_item(&item).expect("failed to save cart item for stale note");
        assert_eq!(
            list_note_cart_items(stale_note_id)
                .expect("cart item should exist before rebuild")
                .len(),
            1
        );

        rebuild_index_from_files().expect("notes rebuild should succeed");

        assert!(
            get_note(stale_note_id)
                .expect("stale note lookup should succeed")
                .is_none(),
            "db-only note should disappear after markdown-source rebuild"
        );
        assert!(
            list_note_cart_items(stale_note_id)
                .expect("cart lookup after rebuild should succeed")
                .is_empty(),
            "cart rows for notes absent from canonical files should be pruned"
        );
    }

    #[test]
    fn test_rebuild_index_from_files_preserves_cart_items_for_trashed_notes() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before trashed cart rebuild test");
        let token = unique_test_token("cart_rebuild_trash");
        let mut note = Note::with_content(format!("# {token}\nBody"));
        let note_id = note.id;
        save_note(&note).expect("failed to save active note");
        let item = text_cart_item(
            note_id,
            format!("{token}-trash-cart"),
            "Trash Cart".to_string(),
            format!("trash payload {token}"),
            4,
        );
        save_note_cart_item(&item).expect("failed to save cart item before soft delete");

        note.soft_delete();
        save_note(&note).expect("failed to soft-delete note");
        rebuild_index_from_files().expect("notes rebuild should succeed");
        let rebuilt_items = list_note_cart_items(note_id)
            .expect("cart items should be readable after trash rebuild");

        delete_note_permanently(note_id).expect("cleanup failed for trashed note");

        assert_eq!(rebuilt_items.len(), 1);
        assert_eq!(rebuilt_items[0].id, item.id);
        assert_eq!(rebuilt_items[0].payload, item.payload);
    }

    #[test]
    fn test_rebuild_index_from_files_restores_search_tags_pins_and_backlinks() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before rebuild contract test");
        let token = unique_test_token("rebuild_contract");

        let target = Note::with_content(format!("# {token} Target\nBody"));
        let target_id = target.id;
        save_note(&target).expect("failed to save target note");

        let mut source = Note::with_content(format!(
            "---\ntags: [{token}, instructions]\naliases: [{token} Alias]\n---\n# Source\n[[{token} Target]]"
        ));
        source.is_pinned = true;
        let source_id = source.id;
        save_note(&source).expect("failed to save source note");

        let golden_search = search_notes(&token).expect("search should work");
        let golden_tags = get_note_tags(source_id).expect("tags should work");
        let golden_aliases = get_note_aliases(source_id).expect("aliases should work");
        let golden_backlinks = get_note_backlinks(target_id).expect("backlinks should work");
        let golden_backlink_count =
            get_note_backlink_count(target_id).expect("backlink count should work");
        let golden_pin = get_note(source_id)
            .expect("get note should work")
            .expect("source note should exist")
            .is_pinned;

        clear_index_tables(&get_db().expect("db").lock().expect("lock"))
            .expect("failed to clear index for rebuild test");
        rebuild_index_from_files().expect("rebuild should succeed");

        let rebuilt_search = search_notes(&token).expect("search after rebuild should work");
        let rebuilt_tags = get_note_tags(source_id).expect("tags after rebuild should work");
        let rebuilt_aliases =
            get_note_aliases(source_id).expect("aliases after rebuild should work");
        let rebuilt_backlinks =
            get_note_backlinks(target_id).expect("backlinks after rebuild should work");
        let rebuilt_backlink_count =
            get_note_backlink_count(target_id).expect("backlink count after rebuild should work");
        let rebuilt_pin = get_note(source_id)
            .expect("get note after rebuild should work")
            .expect("source note should exist after rebuild")
            .is_pinned;

        delete_note_permanently(source_id).expect("cleanup source");
        delete_note_permanently(target_id).expect("cleanup target");

        assert_eq!(
            golden_search.iter().map(|note| note.id).collect::<Vec<_>>(),
            rebuilt_search
                .iter()
                .map(|note| note.id)
                .collect::<Vec<_>>()
        );
        assert_eq!(golden_tags, rebuilt_tags);
        assert_eq!(golden_aliases, rebuilt_aliases);
        assert_eq!(golden_backlinks.len(), rebuilt_backlinks.len());
        assert_eq!(
            golden_backlinks.first().map(|hit| hit.id),
            rebuilt_backlinks.first().map(|hit| hit.id)
        );
        assert_eq!(golden_backlink_count, rebuilt_backlink_count);
        assert_eq!(golden_pin, rebuilt_pin);
        assert!(golden_pin, "fixture note should be pinned");
    }

    #[test]
    fn test_soft_delete_moves_file_to_trash_and_restore_returns_it() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before trash roundtrip test");
        let token = unique_test_token("trash_roundtrip");
        let mut note = Note::with_content(format!("# {token}\nBody"));
        let id = note.id;
        save_note(&note).expect("failed to save active note");

        let substrate = notes_substrate().expect("substrate");
        let slug = lookup_note_slug(&get_db().expect("db").lock().expect("lock"), id)
            .expect("slug lookup")
            .expect("slug should exist");
        let active_path = substrate.paths().note_file(&slug);
        assert!(active_path.exists(), "canonical note file should exist");

        note.soft_delete();
        save_note(&note).expect("failed to soft-delete note");
        assert!(!active_path.exists(), "active note file should be trashed");

        let deleted = get_deleted_notes().expect("deleted notes");
        assert!(deleted.iter().any(|candidate| candidate.id == id));

        note.restore();
        save_note(&note).expect("failed to restore note");
        assert!(active_path.exists(), "restored note file should exist");

        delete_note_permanently(id).expect("cleanup");
    }

    #[test]
    fn test_backlinks_recompute_when_target_alias_changes() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before stale backlink test");
        let token = unique_test_token("stale_backlink");
        let source = Note::with_content(format!("# Source\n[[{token} Target]]"));
        let source_id = source.id;
        let mut target = Note::with_content(format!("# {token} Target\nBody"));
        let target_id = target.id;

        save_note(&target).expect("failed to save target note");
        save_note(&source).expect("failed to save source note");
        assert_eq!(
            get_note_backlink_count(target_id).expect("backlinks should resolve"),
            1
        );

        target.title = format!("{token} Renamed");
        target.content = format!("# {token} Renamed\nBody");
        save_note(&target).expect("failed to save renamed target note");
        let backlink_count =
            get_note_backlink_count(target_id).expect("backlinks should recompute");

        delete_note_permanently(source_id).expect("cleanup failed for source note");
        delete_note_permanently(target_id).expect("cleanup failed for target note");

        assert_eq!(backlink_count, 0);
    }

    #[test]
    fn test_backlinks_do_not_resolve_ambiguous_aliases() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before ambiguous backlink test");
        let token = unique_test_token("ambiguous_backlink");
        let source = Note::with_content(format!("# Source\n[[{token} Target]]"));
        let source_id = source.id;
        let target_a = Note::with_content(format!("# {token} Target\nA"));
        let target_a_id = target_a.id;
        let target_b = Note::with_content(format!("# {token} Target\nB"));
        let target_b_id = target_b.id;

        save_note(&target_a).expect("failed to save first target note");
        save_note(&target_b).expect("failed to save second target note");
        save_note(&source).expect("failed to save source note");

        let backlinks_a = get_note_backlink_count(target_a_id).expect("backlinks should count");
        let backlinks_b = get_note_backlink_count(target_b_id).expect("backlinks should count");

        delete_note_permanently(source_id).expect("cleanup failed for source note");
        delete_note_permanently(target_a_id).expect("cleanup failed for first target note");
        delete_note_permanently(target_b_id).expect("cleanup failed for second target note");

        assert_eq!(backlinks_a + backlinks_b, 0);
    }

    #[test]
    fn test_search_notes_matches_link_metadata() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before link metadata search test");
        let token = unique_test_token("link_search");
        let source = Note::with_content(format!("# Source\n[[{token} Target]]"));
        let source_id = source.id;

        save_note(&source).expect("failed to save link source note");
        let results = search_notes(&format!("link:{token}-target"))
            .expect("link metadata search should succeed");

        delete_note_permanently(source_id).expect("cleanup failed for link source note");

        assert!(results.iter().any(|note| note.id == source_id));
    }
}
