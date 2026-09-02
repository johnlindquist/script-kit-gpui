/// Get the apps database path (~/.scriptkit/db/apps.sqlite)
fn get_apps_db_path() -> PathBuf {
    let kit = PathBuf::from(shellexpand::tilde("~/.scriptkit").as_ref());
    kit.join("db").join("apps.sqlite")
}

/// Initialize the apps database schema
fn init_apps_db(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS apps (
            bundle_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            path TEXT NOT NULL UNIQUE,
            icon_blob BLOB,
            mtime INTEGER NOT NULL,
            last_seen INTEGER NOT NULL
        )",
        [],
    )
    .context("Failed to create apps table")?;

    // Index for path lookups (used during directory scan)
    conn.execute("CREATE INDEX IF NOT EXISTS idx_apps_path ON apps(path)", [])
        .context("Failed to create path index")?;

    Ok(())
}

/// Get or initialize the apps database connection
fn get_apps_db() -> Result<Arc<Mutex<Connection>>> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)?;
    if let Some(db) = APPS_DB.get() {
        return Ok(Arc::clone(db));
    }

    let db_path = get_apps_db_path();

    // Ensure directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create db directory")?;
    }

    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open apps database: {}", db_path.display()))?;

    init_apps_db(&conn)?;

    let db = Arc::new(Mutex::new(conn));

    // Try to store it, but another thread might beat us
    match APPS_DB.set(Arc::clone(&db)) {
        Ok(()) => Ok(db),
        Err(_) => {
            // Another thread initialized it first, use theirs
            APPS_DB
                .get()
                .map(Arc::clone)
                .ok_or_else(|| anyhow::anyhow!("APPS_DB unexpectedly uninitialized"))
        }
    }
}

/// Set the current loading state
fn set_loading_state(state: AppLoadingState) {
    if let Ok(mut guard) = APP_LOADING_STATE.lock() {
        *guard = state;
    }
}

/// Get the current loading state
#[allow(dead_code)]
pub fn get_app_loading_state() -> AppLoadingState {
    APP_LOADING_STATE
        .lock()
        .ok()
        .map(|g| *g)
        .unwrap_or(AppLoadingState::Failed)
}

/// Get a human-readable message for the current loading state
#[allow(dead_code)]
pub fn get_app_loading_message() -> &'static str {
    get_app_loading_state().message()
}

/// Check if apps are still loading
#[allow(dead_code)]
pub fn is_apps_loading() -> bool {
    matches!(
        get_app_loading_state(),
        AppLoadingState::LoadingFromCache | AppLoadingState::ScanningDirectories
    )
}

/// Look up a pre-decoded app icon from the in-memory cache by bundle ID.
pub fn cached_app_icon_for_bundle(bundle_id: &str) -> Option<DecodedIcon> {
    let bundle_id = bundle_id.trim();
    if bundle_id.is_empty() {
        return None;
    }

    let cache = APP_CACHE.lock().ok()?;
    cache
        .apps
        .as_ref()?
        .iter()
        .find(|app| app.bundle_id.as_deref() == Some(bundle_id))
        .and_then(|app| app.icon.clone())
}

/// Get modification time for a path as Unix timestamp
fn get_mtime(path: &Path) -> Result<i64> {
    let modified = path
        .metadata()
        .with_context(|| format!("Reading application metadata: {}", path.display()))?
        .modified()
        .with_context(|| format!("Reading application modification time: {}", path.display()))?;
    match modified.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_secs())
            .context("Application modification time exceeds SQLite range"),
        Err(before_epoch) => Ok(-i64::try_from(before_epoch.duration().as_secs())
            .context("Application modification time exceeds SQLite range")?),
    }
}

// ============================================================================
// SQLite Cache Operations
// ============================================================================

/// Load all apps from the SQLite cache with icons decoded synchronously.
///
/// Returns apps with their icons already decoded as RenderImages.
/// This is the fast path for startup - no filesystem scanning needed.
fn load_apps_from_db() -> Result<Vec<AppInfo>> {
    let db = get_apps_db()?;
    let conn = db
        .lock()
        .map_err(|_| anyhow::anyhow!("Application database lock poisoned"))?;
    load_apps_from_connection(&conn)
}

fn load_apps_from_connection(conn: &Connection) -> Result<Vec<AppInfo>> {
    let _span = info_span!("load_apps_from_db").entered();
    let start = Instant::now();
    let mut stmt = conn
        .prepare("SELECT bundle_id, name, path, icon_blob FROM apps ORDER BY name COLLATE NOCASE")
        .context("Preparing cached application query")?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<Vec<u8>>>(3)?,
            ))
        })
        .context("Reading cached application rows")?;
    let mut apps = Vec::new();
    let mut icons_decoded = 0;
    for row in rows {
        let (bundle_id, name, path, icon_blob) = row.context("Decoding cached application row")?;
        let path = PathBuf::from(path);
        match path.metadata() {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => continue, // An individual cached path is no longer an app bundle.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Reading cached application path: {}", path.display())
                });
            }
        }
        // A malformed optional image is not an invalid application catalogue.
        let icon = icon_blob.and_then(|bytes| {
            crate::list_item::decode_png_to_render_image_with_bgra_conversion(&bytes)
                .ok()
                .map(DecodedIcon::new)
        });
        if icon.is_some() {
            icons_decoded += 1;
        }
        apps.push(AppInfo {
            name,
            path,
            bundle_id,
            icon,
        });
    }
    info!(
        app_count = apps.len(),
        icons_decoded,
        duration_ms = start.elapsed().as_millis(),
        "Loaded apps from DB with icons"
    );
    Ok(apps)
}

/// Commit one complete scan atomically, preserving existing optional icon bytes.
fn save_apps_to_db(conn: &mut Connection, apps: &[ScannedApp]) -> Result<()> {
    let transaction = conn
        .transaction()
        .context("Starting application cache update")?;
    let now = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .context("Reading application cache timestamp")?
            .as_secs(),
    )
    .context("Application cache timestamp exceeds SQLite range")?;
    let mut expected = std::collections::HashSet::with_capacity(apps.len());
    for entry in apps {
        let app = &entry.app;
        let path = app.path.to_string_lossy();
        let bundle_id = app
            .bundle_id
            .as_deref()
            .map(std::borrow::Cow::Borrowed)
            .unwrap_or_else(|| app.path.to_string_lossy());
        transaction
            .execute(
                "INSERT INTO apps (bundle_id, name, path, icon_blob, mtime, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(bundle_id) DO UPDATE SET
                 name = excluded.name,
                 path = excluded.path,
                 icon_blob = COALESCE(excluded.icon_blob, apps.icon_blob),
                 mtime = excluded.mtime,
                 last_seen = excluded.last_seen",
                params![
                    bundle_id.as_ref(),
                    app.name,
                    path.as_ref(),
                    entry.icon_bytes.as_deref(),
                    entry.mtime,
                    now
                ],
            )
            .with_context(|| format!("Saving application cache entry: {}", app.path.display()))?;
        expected.insert(bundle_id);
    }
    let stale = {
        let mut statement = transaction.prepare("SELECT bundle_id FROM apps")?;
        let mut rows = statement.query([])?;
        let mut stale = Vec::new();
        while let Some(row) = rows.next()? {
            let bundle_id = row.get_ref(0)?.as_str()?;
            if !expected.contains(bundle_id) {
                stale.push(bundle_id.to_owned());
            }
        }
        stale
    };
    for bundle_id in stale {
        transaction.execute("DELETE FROM apps WHERE bundle_id = ?1", [bundle_id])?;
    }
    transaction
        .commit()
        .context("Committing application cache update")
}

/// Get database statistics for logging
pub fn get_apps_db_stats() -> Result<(usize, u64)> {
    let db = get_apps_db()?;
    let conn = db
        .lock()
        .map_err(|_| anyhow::anyhow!("Application database lock poisoned"))?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM apps", [], |row| row.get(0))?;
    let total_icon_size: i64 = conn.query_row(
        "SELECT COALESCE(SUM(LENGTH(icon_blob)), 0) FROM apps WHERE icon_blob IS NOT NULL",
        [],
        |row| row.get(0),
    )?;
    Ok((usize::try_from(count)?, u64::try_from(total_icon_size)?))
}
