//! Menu Cache Layer
//!
//! SQLite-backed persistence for caching application menu bar data.
//! Caches menu hierarchies by bundle_id to avoid expensive rescanning.
//! Follows the same patterns as notes/storage.rs for consistency.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

/// A menu bar item with its hierarchy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MenuBarItem {
    pub title: String,
    pub enabled: bool,
    pub shortcut: Option<String>,
    pub children: Vec<MenuBarItem>,
    pub menu_path: Vec<String>, // e.g., ["File", "New Window"]
}

/// Global database connection for menu cache
static MENU_CACHE_DB: OnceLock<Arc<Mutex<Connection>>> = OnceLock::new();

/// Get the path to the menu cache database
fn get_menu_cache_db_path() -> PathBuf {
    let kit_dir = dirs::home_dir()
        .map(|h| h.join(".sk/kit"))
        .unwrap_or_else(|| PathBuf::from(".sk/kit"));

    kit_dir.join("db").join("menu-cache.sqlite")
}

/// Initialize the menu cache database
///
/// This function is idempotent - it's safe to call multiple times.
/// If the database is already initialized, it returns Ok(()) immediately.
pub fn init_menu_cache_db() -> Result<()> {
    // Check if already initialized - this is the common case after first init
    if MENU_CACHE_DB.get().is_some() {
        debug!("Menu cache database already initialized, skipping");
        return Ok(());
    }

    let db_path = get_menu_cache_db_path();

    // Ensure directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create menu cache db directory")?;
    }

    let conn = Connection::open(&db_path).context("Failed to open menu cache database")?;

    // Create tables
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS menu_cache (
            bundle_id TEXT PRIMARY KEY,
            menu_json TEXT NOT NULL,
            last_scanned INTEGER NOT NULL,
            app_version TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_menu_cache_last_scanned ON menu_cache(last_scanned);
        "#,
    )
    .context("Failed to create menu cache table")?;

    info!(db_path = %db_path.display(), "Menu cache database initialized");

    // Use get_or_init pattern to handle race condition where another thread
    // might have initialized the DB between our check and set
    let _ = MENU_CACHE_DB.get_or_init(|| Arc::new(Mutex::new(conn)));

    Ok(())
}

/// Get a reference to the menu cache database connection
fn get_db() -> Result<Arc<Mutex<Connection>>> {
    MENU_CACHE_DB
        .get()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Menu cache database not initialized"))
}

/// Get the current timestamp as Unix epoch seconds
fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Get cached menu items for an application by bundle_id
pub fn get_cached_menu(bundle_id: &str) -> Result<Option<Vec<MenuBarItem>>> {
    let db = get_db()?;
    let conn = db
        .lock()
        .map_err(|e| anyhow::anyhow!("DB lock error: {}", e))?;

    let result: Option<String> = conn
        .query_row(
            "SELECT menu_json FROM menu_cache WHERE bundle_id = ?1",
            params![bundle_id],
            |row| row.get(0),
        )
        .optional()
        .context("Failed to query menu cache")?;

    match result {
        Some(json) => {
            let items: Vec<MenuBarItem> =
                serde_json::from_str(&json).context("Failed to deserialize menu items")?;
            debug!(bundle_id = %bundle_id, item_count = items.len(), "Retrieved cached menu");
            Ok(Some(items))
        }
        None => {
            debug!(bundle_id = %bundle_id, "No cached menu found");
            Ok(None)
        }
    }
}

/// Set (insert or update) cached menu items for an application
pub fn set_cached_menu(
    bundle_id: &str,
    items: &[MenuBarItem],
    app_version: Option<&str>,
) -> Result<()> {
    let db = get_db()?;
    let conn = db
        .lock()
        .map_err(|e| anyhow::anyhow!("DB lock error: {}", e))?;

    let menu_json = serde_json::to_string(items).context("Failed to serialize menu items")?;
    let timestamp = current_timestamp();

    conn.execute(
        r#"
        INSERT INTO menu_cache (bundle_id, menu_json, last_scanned, app_version)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(bundle_id) DO UPDATE SET
            menu_json = excluded.menu_json,
            last_scanned = excluded.last_scanned,
            app_version = excluded.app_version
        "#,
        params![bundle_id, menu_json, timestamp, app_version],
    )
    .context("Failed to save menu cache")?;

    debug!(
        bundle_id = %bundle_id,
        item_count = items.len(),
        app_version = app_version.unwrap_or("none"),
        "Menu cache updated"
    );
    Ok(())
}

/// Check if the cache for a bundle_id is still valid (not expired)
pub fn is_cache_valid(bundle_id: &str, max_age_secs: u64) -> Result<bool> {
    let db = get_db()?;
    let conn = db
        .lock()
        .map_err(|e| anyhow::anyhow!("DB lock error: {}", e))?;

    let result: Option<i64> = conn
        .query_row(
            "SELECT last_scanned FROM menu_cache WHERE bundle_id = ?1",
            params![bundle_id],
            |row| row.get(0),
        )
        .optional()
        .context("Failed to query cache validity")?;

    match result {
        Some(last_scanned) => {
            let now = current_timestamp();
            let age = (now - last_scanned) as u64;
            let valid = age <= max_age_secs;
            debug!(
                bundle_id = %bundle_id,
                last_scanned,
                age_secs = age,
                max_age_secs,
                valid,
                "Cache validity check"
            );
            Ok(valid)
        }
        None => {
            debug!(bundle_id = %bundle_id, "No cache entry found, treating as invalid");
            Ok(false)
        }
    }
}

/// Delete cached menu for an application (useful when app is uninstalled)
pub fn delete_cached_menu(bundle_id: &str) -> Result<()> {
    let db = get_db()?;
    let conn = db
        .lock()
        .map_err(|e| anyhow::anyhow!("DB lock error: {}", e))?;

    conn.execute(
        "DELETE FROM menu_cache WHERE bundle_id = ?1",
        params![bundle_id],
    )
    .context("Failed to delete menu cache")?;

    info!(bundle_id = %bundle_id, "Menu cache entry deleted");
    Ok(())
}

/// Prune cache entries older than the specified age in seconds
pub fn prune_old_cache_entries(max_age_secs: u64) -> Result<usize> {
    let db = get_db()?;
    let conn = db
        .lock()
        .map_err(|e| anyhow::anyhow!("DB lock error: {}", e))?;

    let cutoff = current_timestamp() - max_age_secs as i64;

    let count = conn
        .execute(
            "DELETE FROM menu_cache WHERE last_scanned < ?1",
            params![cutoff],
        )
        .context("Failed to prune old cache entries")?;

    if count > 0 {
        info!(count, max_age_secs, "Pruned old menu cache entries");
    }

    Ok(count)
}

/// Get the total number of cached menus
pub fn get_cache_count() -> Result<usize> {
    let db = get_db()?;
    let conn = db
        .lock()
        .map_err(|e| anyhow::anyhow!("DB lock error: {}", e))?;

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM menu_cache", [], |row| row.get(0))
        .context("Failed to count cache entries")?;

    Ok(count as usize)
}

// ============================================================================
// TESTS - Written FIRST following TDD
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    /// Helper to create an isolated test database
    fn setup_test_db() -> Result<(TempDir, Arc<Mutex<Connection>>)> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test-menu-cache.sqlite");

        let conn = Connection::open(&db_path)?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS menu_cache (
                bundle_id TEXT PRIMARY KEY,
                menu_json TEXT NOT NULL,
                last_scanned INTEGER NOT NULL,
                app_version TEXT
            );
            "#,
        )?;

        Ok((temp_dir, Arc::new(Mutex::new(conn))))
    }

    /// Helper to create test menu items
    fn create_test_menu_items() -> Vec<MenuBarItem> {
        vec![
            MenuBarItem {
                title: "File".to_string(),
                enabled: true,
                shortcut: None,
                children: vec![
                    MenuBarItem {
                        title: "New Window".to_string(),
                        enabled: true,
                        shortcut: Some("Cmd+N".to_string()),
                        children: vec![],
                        menu_path: vec!["File".to_string(), "New Window".to_string()],
                    },
                    MenuBarItem {
                        title: "Close".to_string(),
                        enabled: true,
                        shortcut: Some("Cmd+W".to_string()),
                        children: vec![],
                        menu_path: vec!["File".to_string(), "Close".to_string()],
                    },
                ],
                menu_path: vec!["File".to_string()],
            },
            MenuBarItem {
                title: "Edit".to_string(),
                enabled: true,
                shortcut: None,
                children: vec![MenuBarItem {
                    title: "Copy".to_string(),
                    enabled: true,
                    shortcut: Some("Cmd+C".to_string()),
                    children: vec![],
                    menu_path: vec!["Edit".to_string(), "Copy".to_string()],
                }],
                menu_path: vec!["Edit".to_string()],
            },
        ]
    }

    /// Test inserting and retrieving cache entries
    #[test]
    fn test_cache_insert_and_retrieve() {
        let (_temp_dir, db) = setup_test_db().expect("Failed to setup test db");
        let bundle_id = "com.apple.Safari";
        let items = create_test_menu_items();

        // Insert
        {
            let conn = db.lock().unwrap();
            let menu_json = serde_json::to_string(&items).unwrap();
            let timestamp = current_timestamp();
            conn.execute(
                r#"
                INSERT INTO menu_cache (bundle_id, menu_json, last_scanned, app_version)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                params![bundle_id, menu_json, timestamp, "17.0"],
            )
            .expect("Insert should succeed");
        }

        // Retrieve
        {
            let conn = db.lock().unwrap();
            let result: Option<String> = conn
                .query_row(
                    "SELECT menu_json FROM menu_cache WHERE bundle_id = ?1",
                    params![bundle_id],
                    |row| row.get(0),
                )
                .optional()
                .expect("Query should succeed");

            assert!(result.is_some(), "Should have cached menu");
            let retrieved: Vec<MenuBarItem> =
                serde_json::from_str(&result.unwrap()).expect("Should deserialize");

            assert_eq!(retrieved.len(), 2, "Should have 2 top-level menu items");
            assert_eq!(retrieved[0].title, "File");
            assert_eq!(retrieved[1].title, "Edit");
            assert_eq!(retrieved[0].children.len(), 2);
            assert_eq!(retrieved[0].children[0].shortcut, Some("Cmd+N".to_string()));
        }
    }

    /// Test cache expiry checking
    #[test]
    fn test_cache_expiry_check() {
        let (_temp_dir, db) = setup_test_db().expect("Failed to setup test db");
        let bundle_id = "com.apple.Finder";
        let items = create_test_menu_items();

        // Insert with a timestamp in the past (5 seconds ago)
        let old_timestamp = current_timestamp() - 5;
        {
            let conn = db.lock().unwrap();
            let menu_json = serde_json::to_string(&items).unwrap();
            conn.execute(
                r#"
                INSERT INTO menu_cache (bundle_id, menu_json, last_scanned, app_version)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                params![bundle_id, menu_json, old_timestamp, None::<String>],
            )
            .expect("Insert should succeed");
        }

        // Check with 10 second max age - should be valid
        {
            let conn = db.lock().unwrap();
            let result: Option<i64> = conn
                .query_row(
                    "SELECT last_scanned FROM menu_cache WHERE bundle_id = ?1",
                    params![bundle_id],
                    |row| row.get(0),
                )
                .optional()
                .expect("Query should succeed");

            assert!(result.is_some());
            let last_scanned = result.unwrap();
            let now = current_timestamp();
            let age = (now - last_scanned) as u64;

            // Should be valid with 10 second max age
            assert!(age <= 10, "Cache should be valid within 10 seconds");
        }

        // Check with 2 second max age - should be expired
        {
            let conn = db.lock().unwrap();
            let result: Option<i64> = conn
                .query_row(
                    "SELECT last_scanned FROM menu_cache WHERE bundle_id = ?1",
                    params![bundle_id],
                    |row| row.get(0),
                )
                .optional()
                .expect("Query should succeed");

            let last_scanned = result.unwrap();
            let now = current_timestamp();
            let age = (now - last_scanned) as u64;

            // Should be invalid with 2 second max age (it's at least 5 seconds old)
            assert!(age > 2, "Cache should be expired after 2 seconds");
        }
    }

    /// Test cache update on rescan (upsert behavior)
    #[test]
    fn test_cache_update_on_rescan() {
        let (_temp_dir, db) = setup_test_db().expect("Failed to setup test db");
        let bundle_id = "com.google.Chrome";

        // Initial insert with version 1.0
        let initial_items = vec![MenuBarItem {
            title: "Chrome".to_string(),
            enabled: true,
            shortcut: None,
            children: vec![],
            menu_path: vec!["Chrome".to_string()],
        }];

        {
            let conn = db.lock().unwrap();
            let menu_json = serde_json::to_string(&initial_items).unwrap();
            let timestamp = current_timestamp();
            conn.execute(
                r#"
                INSERT INTO menu_cache (bundle_id, menu_json, last_scanned, app_version)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                params![bundle_id, menu_json, timestamp, "1.0"],
            )
            .expect("Initial insert should succeed");
        }

        // Verify initial state
        {
            let conn = db.lock().unwrap();
            let (version, count): (Option<String>, i64) = conn
                .query_row(
                    "SELECT app_version, (SELECT COUNT(*) FROM menu_cache) FROM menu_cache WHERE bundle_id = ?1",
                    params![bundle_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("Query should succeed");

            assert_eq!(version, Some("1.0".to_string()));
            assert_eq!(count, 1);
        }

        // Small delay to ensure timestamp changes
        thread::sleep(Duration::from_millis(10));

        // Update with version 2.0 and more items (simulating rescan)
        let updated_items = vec![
            MenuBarItem {
                title: "Chrome".to_string(),
                enabled: true,
                shortcut: None,
                children: vec![],
                menu_path: vec!["Chrome".to_string()],
            },
            MenuBarItem {
                title: "File".to_string(),
                enabled: true,
                shortcut: None,
                children: vec![],
                menu_path: vec!["File".to_string()],
            },
        ];

        {
            let conn = db.lock().unwrap();
            let menu_json = serde_json::to_string(&updated_items).unwrap();
            let timestamp = current_timestamp();
            conn.execute(
                r#"
                INSERT INTO menu_cache (bundle_id, menu_json, last_scanned, app_version)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(bundle_id) DO UPDATE SET
                    menu_json = excluded.menu_json,
                    last_scanned = excluded.last_scanned,
                    app_version = excluded.app_version
                "#,
                params![bundle_id, menu_json, timestamp, "2.0"],
            )
            .expect("Upsert should succeed");
        }

        // Verify update
        {
            let conn = db.lock().unwrap();
            let (version, json, count): (Option<String>, String, i64) = conn
                .query_row(
                    "SELECT app_version, menu_json, (SELECT COUNT(*) FROM menu_cache) FROM menu_cache WHERE bundle_id = ?1",
                    params![bundle_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("Query should succeed");

            // Version should be updated
            assert_eq!(version, Some("2.0".to_string()));

            // Should still be only 1 row (upsert, not insert)
            assert_eq!(count, 1, "Should have exactly 1 row after upsert");

            // Menu items should be updated
            let items: Vec<MenuBarItem> = serde_json::from_str(&json).unwrap();
            assert_eq!(items.len(), 2, "Should have 2 menu items after update");
        }
    }

    /// Test missing cache returns None
    #[test]
    fn test_cache_miss_returns_none() {
        let (_temp_dir, db) = setup_test_db().expect("Failed to setup test db");
        let bundle_id = "com.nonexistent.App";

        let conn = db.lock().unwrap();
        let result: Option<String> = conn
            .query_row(
                "SELECT menu_json FROM menu_cache WHERE bundle_id = ?1",
                params![bundle_id],
                |row| row.get(0),
            )
            .optional()
            .expect("Query should succeed");

        assert!(result.is_none(), "Should return None for missing cache");
    }

    /// Test db path construction
    #[test]
    fn test_db_path() {
        let path = get_menu_cache_db_path();
        assert!(path.to_string_lossy().contains("menu-cache.sqlite"));
        assert!(path.to_string_lossy().contains(".sk/kit/db"));
    }

    /// Test MenuBarItem serialization round-trip
    #[test]
    fn test_menu_item_serialization() {
        let items = create_test_menu_items();
        let json = serde_json::to_string(&items).expect("Should serialize");
        let deserialized: Vec<MenuBarItem> =
            serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(items.len(), deserialized.len());
        assert_eq!(items[0], deserialized[0]);
        assert_eq!(items[1], deserialized[1]);
    }
}
