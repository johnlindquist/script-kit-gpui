//! Clipboard History Module
//!
//! Provides SQLite-backed clipboard history with background monitoring.
//!
//! ## Features
//! - Stores text and base64-encoded images
//! - Background polling every 500ms
//! - LRU eviction at 1000 entries
//! - Pin/unpin entries to prevent eviction
//!
//! ## Usage
//! ```ignore
//! use crate::clipboard_history::{init_clipboard_history, get_clipboard_history};
//!
//! // Initialize on app startup
//! init_clipboard_history()?;
//!
//! // Get recent entries
//! let entries = get_clipboard_history(50);
//! ```

use anyhow::{Context, Result};
use arboard::Clipboard;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use gpui::RenderImage;
use lru::LruCache;
use rusqlite::{params, Connection};
use smallvec::SmallVec;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Maximum number of entries to keep in history (LRU eviction)
const MAX_HISTORY_ENTRIES: usize = 1000;

/// Maximum number of decoded images to keep in memory (LRU eviction)
/// Each image can be 1-4MB, so 100 images = ~100-400MB max memory
const MAX_IMAGE_CACHE_ENTRIES: usize = 100;

/// Polling interval for clipboard changes
const POLL_INTERVAL_MS: u64 = 500;

/// Content types for clipboard entries
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentType {
    Text,
    Image,
}

impl ContentType {
    fn as_str(&self) -> &'static str {
        match self {
            ContentType::Text => "text",
            ContentType::Image => "image",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "image" => ContentType::Image,
            _ => ContentType::Text,
        }
    }
}

/// A single clipboard history entry
#[derive(Debug, Clone)]
pub struct ClipboardEntry {
    pub id: String,
    pub content: String,
    pub content_type: ContentType,
    pub timestamp: i64,
    pub pinned: bool,
}

/// Global database connection (thread-safe)
static DB_CONNECTION: OnceLock<Arc<Mutex<Connection>>> = OnceLock::new();

/// Flag to stop the monitoring thread
static STOP_MONITORING: OnceLock<Arc<Mutex<bool>>> = OnceLock::new();

/// Global image cache for decoded RenderImages (thread-safe)
/// Key: entry ID, Value: decoded RenderImage
/// Uses LRU eviction to cap memory usage at ~100-400MB (100 images max)
static IMAGE_CACHE: OnceLock<Mutex<LruCache<String, Arc<RenderImage>>>> = OnceLock::new();

/// Cached clipboard entries to avoid re-fetching from SQLite on each view open
/// Updated whenever a new entry is added
static ENTRY_CACHE: OnceLock<Mutex<Vec<ClipboardEntry>>> = OnceLock::new();

/// Timestamp of last cache update
static CACHE_UPDATED: OnceLock<Mutex<i64>> = OnceLock::new();

/// Get the global image cache, initializing if needed
fn get_image_cache() -> &'static Mutex<LruCache<String, Arc<RenderImage>>> {
    IMAGE_CACHE.get_or_init(|| {
        let cap = NonZeroUsize::new(MAX_IMAGE_CACHE_ENTRIES).expect("cache size must be non-zero");
        Mutex::new(LruCache::new(cap))
    })
}

/// Get the global entry cache, initializing if needed  
fn get_entry_cache() -> &'static Mutex<Vec<ClipboardEntry>> {
    ENTRY_CACHE.get_or_init(|| Mutex::new(Vec::new()))
}

/// Get cached image by entry ID (updates LRU order)
pub fn get_cached_image(id: &str) -> Option<Arc<RenderImage>> {
    get_image_cache().lock().ok()?.get(id).cloned()
}

/// Cache a decoded image (with LRU eviction at MAX_IMAGE_CACHE_ENTRIES limit)
pub fn cache_image(id: &str, image: Arc<RenderImage>) {
    if let Ok(mut cache) = get_image_cache().lock() {
        // LruCache automatically evicts oldest entry when capacity is exceeded
        let evicted = cache.len() >= cache.cap().get();
        cache.put(id.to_string(), image);
        if evicted {
            debug!(
                id = %id,
                cache_size = cache.len(),
                max_size = MAX_IMAGE_CACHE_ENTRIES,
                "Cached decoded image (evicted oldest)"
            );
        } else {
            debug!(id = %id, cache_size = cache.len(), "Cached decoded image");
        }
    }
}

/// Get cached clipboard entries (faster than querying SQLite)
/// Falls back to SQLite if cache is empty
pub fn get_cached_entries(limit: usize) -> Vec<ClipboardEntry> {
    if let Ok(cache) = get_entry_cache().lock() {
        if !cache.is_empty() {
            let result: Vec<_> = cache.iter().take(limit).cloned().collect();
            debug!(count = result.len(), cached = true, "Retrieved clipboard entries from cache");
            return result;
        }
    }
    // Fall back to database
    get_clipboard_history(limit)
}

/// Invalidate the entry cache (called when entries change)
#[allow(dead_code)]
fn invalidate_entry_cache() {
    if let Ok(mut cache) = get_entry_cache().lock() {
        cache.clear();
    }
}

/// Refresh the entry cache from database
fn refresh_entry_cache() {
    let entries = get_clipboard_history(MAX_HISTORY_ENTRIES);
    if let Ok(mut cache) = get_entry_cache().lock() {
        *cache = entries;
        debug!(count = cache.len(), "Refreshed entry cache");
    }
    if let Some(updated) = CACHE_UPDATED.get() {
        if let Ok(mut ts) = updated.lock() {
            *ts = chrono::Utc::now().timestamp();
        }
    }
}

/// Get the database path (~/.kenv/clipboard-history.db)
fn get_db_path() -> Result<PathBuf> {
    let kenv_dir = PathBuf::from(shellexpand::tilde("~/.kenv").as_ref());

    // Create ~/.kenv if it doesn't exist
    if !kenv_dir.exists() {
        std::fs::create_dir_all(&kenv_dir).context("Failed to create ~/.kenv directory")?;
    }

    Ok(kenv_dir.join("clipboard-history.db"))
}

/// Get or create the database connection
fn get_connection() -> Result<Arc<Mutex<Connection>>> {
    if let Some(conn) = DB_CONNECTION.get() {
        return Ok(conn.clone());
    }

    let db_path = get_db_path()?;
    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open database at {:?}", db_path))?;

    // Create the table if it doesn't exist
    conn.execute(
        "CREATE TABLE IF NOT EXISTS history (
            id TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            content_type TEXT NOT NULL DEFAULT 'text',
            timestamp INTEGER NOT NULL,
            pinned INTEGER DEFAULT 0
        )",
        [],
    )
    .context("Failed to create history table")?;

    // Create index for faster queries
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_timestamp ON history(timestamp DESC)",
        [],
    )
    .context("Failed to create timestamp index")?;

    let conn = Arc::new(Mutex::new(conn));

    // Try to set it globally (ignore if already set by another thread)
    let _ = DB_CONNECTION.set(conn.clone());

    Ok(conn)
}

/// Initialize clipboard history: create DB and start monitoring
///
/// This should be called once at application startup. It will:
/// 1. Create the SQLite database if it doesn't exist
/// 2. Pre-warm the entry cache
/// 3. Pre-decode images in background
/// 4. Start a background thread that polls the clipboard every 500ms
///
/// # Errors
/// Returns error if database creation fails.
pub fn init_clipboard_history() -> Result<()> {
    info!("Initializing clipboard history");

    // Initialize the database connection
    let _conn = get_connection().context("Failed to initialize database")?;

    // Initialize the cache timestamp
    let _ = CACHE_UPDATED.set(Mutex::new(0));

    // Pre-warm the entry cache from database
    refresh_entry_cache();

    // Pre-decode images in a background thread
    thread::spawn(|| {
        prewarm_image_cache();
    });

    // Initialize the stop flag
    let stop_flag = Arc::new(Mutex::new(false));
    let _ = STOP_MONITORING.set(stop_flag.clone());

    // Start the monitoring thread
    let stop_flag_clone = stop_flag.clone();
    thread::spawn(move || {
        if let Err(e) = clipboard_monitor_loop(stop_flag_clone) {
            error!(error = %e, "Clipboard monitor thread failed");
        }
    });

    info!("Clipboard history initialized");
    Ok(())
}

/// Pre-warm the image cache by decoding all cached image entries
fn prewarm_image_cache() {
    let entries = get_cached_entries(100);
    let mut decoded_count = 0;

    for entry in entries {
        if entry.content_type == ContentType::Image {
            // Skip if already cached
            if get_cached_image(&entry.id).is_some() {
                continue;
            }

            // Decode and cache
            if let Some(render_image) = decode_to_render_image(&entry.content) {
                cache_image(&entry.id, render_image);
                decoded_count += 1;
            }
        }
    }

    info!(decoded_count, "Pre-warmed image cache");
}

/// Stop the clipboard monitoring thread
#[allow(dead_code)]
pub fn stop_clipboard_monitoring() {
    if let Some(stop_flag) = STOP_MONITORING.get() {
        if let Ok(mut flag) = stop_flag.lock() {
            *flag = true;
            info!("Clipboard monitoring stopped");
        }
    }
}

/// Background loop that monitors clipboard changes
fn clipboard_monitor_loop(stop_flag: Arc<Mutex<bool>>) -> Result<()> {
    let mut clipboard = Clipboard::new().context("Failed to create clipboard instance")?;
    let mut last_text: Option<String> = None;
    let mut last_image_hash: Option<u64> = None;
    let poll_interval = Duration::from_millis(POLL_INTERVAL_MS);

    info!(poll_interval_ms = POLL_INTERVAL_MS, "Clipboard monitor started");

    loop {
        // Check if we should stop
        if let Ok(stop) = stop_flag.lock() {
            if *stop {
                info!("Clipboard monitor stopping");
                break;
            }
        }

        let start = Instant::now();

        // Check for text changes
        if let Ok(text) = clipboard.get_text() {
            if !text.is_empty() {
                let is_new = match &last_text {
                    Some(last) => last != &text,
                    None => true,
                };

                if is_new {
                    debug!(text_len = text.len(), "New text detected in clipboard");
                    if let Err(e) = add_entry(&text, ContentType::Text) {
                        warn!(error = %e, "Failed to add text entry to history");
                    }
                    last_text = Some(text);
                }
            }
        }

        // Check for image changes
        if let Ok(image_data) = clipboard.get_image() {
            // Simple hash of image dimensions + first few bytes
            let hash = compute_image_hash(&image_data);

            let is_new = match last_image_hash {
                Some(last) => last != hash,
                None => true,
            };

            if is_new {
                debug!(
                    width = image_data.width,
                    height = image_data.height,
                    "New image detected in clipboard"
                );

                // Encode image as base64 PNG
                if let Ok(base64_content) = encode_image_as_base64(&image_data) {
                    // Add entry first to get the ID
                    if let Err(e) = add_entry(&base64_content, ContentType::Image) {
                        warn!(error = %e, "Failed to add image entry to history");
                    } else {
                        // Pre-decode the image immediately so it's ready for display
                        // This runs in the background monitor thread, not during render
                        if let Some(render_image) = decode_to_render_image(&base64_content) {
                            // Get the entry ID from the cache (it was just added)
                            if let Ok(cache) = get_entry_cache().lock() {
                                if let Some(entry) = cache.first() {
                                    if entry.content_type == ContentType::Image {
                                        cache_image(&entry.id, render_image);
                                        debug!(id = %entry.id, "Pre-cached new image during monitoring");
                                    }
                                }
                            }
                        }
                    }
                }
                last_image_hash = Some(hash);
            }
        }

        // Sleep for remaining time in poll interval
        let elapsed = start.elapsed();
        if elapsed < poll_interval {
            thread::sleep(poll_interval - elapsed);
        }
    }

    Ok(())
}

/// Compute a simple hash of image data for change detection
fn compute_image_hash(image: &arboard::ImageData) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    image.width.hash(&mut hasher);
    image.height.hash(&mut hasher);

    // Hash first 1KB of pixels for quick comparison
    let sample_size = 1024.min(image.bytes.len());
    image.bytes[..sample_size].hash(&mut hasher);

    hasher.finish()
}

/// Encode image data as base64 PNG string
fn encode_image_as_base64(image: &arboard::ImageData) -> Result<String> {
    // For now, just encode the raw RGBA bytes with metadata prefix
    // Format: "rgba:{width}:{height}:{base64_data}"
    let base64_data = BASE64.encode(&image.bytes);
    Ok(format!("rgba:{}:{}:{}", image.width, image.height, base64_data))
}

/// Add a new entry to clipboard history
fn add_entry(content: &str, content_type: ContentType) -> Result<()> {
    let conn = get_connection()?;
    let conn = conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

    let id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().timestamp();

    // Check if this exact content already exists (dedup)
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM history WHERE content = ? AND content_type = ?",
            params![content, content_type.as_str()],
            |row| row.get(0),
        )
        .ok();

    if let Some(existing_id) = existing {
        // Update timestamp of existing entry instead of creating duplicate
        conn.execute(
            "UPDATE history SET timestamp = ? WHERE id = ?",
            params![timestamp, existing_id],
        )
        .context("Failed to update existing entry timestamp")?;
        debug!(id = %existing_id, "Updated existing clipboard entry timestamp");
        // Refresh cache to reflect updated ordering
        drop(conn);
        refresh_entry_cache();
        return Ok(());
    }

    // Insert new entry
    conn.execute(
        "INSERT INTO history (id, content, content_type, timestamp, pinned) VALUES (?1, ?2, ?3, ?4, 0)",
        params![id, content, content_type.as_str(), timestamp],
    )
    .context("Failed to insert clipboard entry")?;

    debug!(id = %id, content_type = content_type.as_str(), "Added clipboard entry");

    // Enforce LRU eviction
    enforce_max_entries(&conn)?;

    // Drop lock before refreshing cache
    drop(conn);

    // Refresh the entry cache so it includes the new entry
    refresh_entry_cache();

    Ok(())
}

/// Enforce maximum entry count by removing oldest unpinned entries
fn enforce_max_entries(conn: &Connection) -> Result<()> {
    // Count total entries
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))?;

    if count as usize <= MAX_HISTORY_ENTRIES {
        return Ok(());
    }

    let to_remove = count as usize - MAX_HISTORY_ENTRIES;

    // Delete oldest unpinned entries
    conn.execute(
        "DELETE FROM history WHERE id IN (
            SELECT id FROM history 
            WHERE pinned = 0 
            ORDER BY timestamp ASC 
            LIMIT ?
        )",
        params![to_remove],
    )
    .context("Failed to evict old entries")?;

    debug!(removed = to_remove, "Evicted old clipboard entries (LRU)");

    Ok(())
}

/// Get clipboard history entries
///
/// Returns the most recent entries, ordered by timestamp descending.
/// Pinned entries are included in the results.
///
/// # Arguments
/// * `limit` - Maximum number of entries to return
///
/// # Returns
/// Vector of clipboard entries, newest first.
pub fn get_clipboard_history(limit: usize) -> Vec<ClipboardEntry> {
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
        "SELECT id, content, content_type, timestamp, pinned 
         FROM history 
         ORDER BY pinned DESC, timestamp DESC 
         LIMIT ?",
    ) {
        Ok(s) => s,
        Err(e) => {
            error!(error = %e, "Failed to prepare query");
            return Vec::new();
        }
    };

    let entries = stmt
        .query_map(params![limit], |row| {
            Ok(ClipboardEntry {
                id: row.get(0)?,
                content: row.get(1)?,
                content_type: ContentType::from_str(&row.get::<_, String>(2)?),
                timestamp: row.get(3)?,
                pinned: row.get::<_, i64>(4)? != 0,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_else(|e| {
            error!(error = %e, "Failed to query clipboard history");
            Vec::new()
        });

    debug!(count = entries.len(), limit = limit, "Retrieved clipboard history");
    entries
}

/// Pin a clipboard entry to prevent LRU eviction
///
/// # Arguments
/// * `id` - The entry ID to pin
///
/// # Errors
/// Returns error if the entry doesn't exist or database operation fails.
pub fn pin_entry(id: &str) -> Result<()> {
    let conn = get_connection()?;
    let conn = conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

    let affected = conn
        .execute("UPDATE history SET pinned = 1 WHERE id = ?", params![id])
        .context("Failed to pin entry")?;

    if affected == 0 {
        anyhow::bail!("Entry not found: {}", id);
    }

    info!(id = %id, "Pinned clipboard entry");
    Ok(())
}

/// Unpin a clipboard entry
///
/// # Arguments
/// * `id` - The entry ID to unpin
///
/// # Errors
/// Returns error if the entry doesn't exist or database operation fails.
pub fn unpin_entry(id: &str) -> Result<()> {
    let conn = get_connection()?;
    let conn = conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

    let affected = conn
        .execute("UPDATE history SET pinned = 0 WHERE id = ?", params![id])
        .context("Failed to unpin entry")?;

    if affected == 0 {
        anyhow::bail!("Entry not found: {}", id);
    }

    info!(id = %id, "Unpinned clipboard entry");
    Ok(())
}

/// Remove a single entry from clipboard history
///
/// # Arguments
/// * `id` - The entry ID to remove
///
/// # Errors
/// Returns error if the entry doesn't exist or database operation fails.
pub fn remove_entry(id: &str) -> Result<()> {
    let conn = get_connection()?;
    let conn = conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

    let affected = conn
        .execute("DELETE FROM history WHERE id = ?", params![id])
        .context("Failed to remove entry")?;

    if affected == 0 {
        anyhow::bail!("Entry not found: {}", id);
    }

    info!(id = %id, "Removed clipboard entry");
    Ok(())
}

/// Clear all clipboard history
///
/// This removes ALL entries, including pinned ones.
///
/// # Errors
/// Returns error if database operation fails.
pub fn clear_history() -> Result<()> {
    let conn = get_connection()?;
    let conn = conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

    conn.execute("DELETE FROM history", [])
        .context("Failed to clear history")?;

    info!("Cleared all clipboard history");
    Ok(())
}

/// Copy an entry back to the clipboard
///
/// # Arguments
/// * `id` - The entry ID to copy
///
/// # Errors
/// Returns error if the entry doesn't exist or clipboard operation fails.
#[allow(dead_code)]
pub fn copy_entry_to_clipboard(id: &str) -> Result<()> {
    let conn = get_connection()?;
    let conn = conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

    let (content, content_type): (String, String) = conn
        .query_row(
            "SELECT content, content_type FROM history WHERE id = ?",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .context("Entry not found")?;

    drop(conn); // Release lock before clipboard operation

    let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;

    match ContentType::from_str(&content_type) {
        ContentType::Text => {
            clipboard
                .set_text(&content)
                .context("Failed to set clipboard text")?;
        }
        ContentType::Image => {
            // Decode the base64 image
            if let Some(image_data) = decode_base64_image(&content) {
                clipboard
                    .set_image(image_data)
                    .context("Failed to set clipboard image")?;
            } else {
                anyhow::bail!("Failed to decode image data");
            }
        }
    }

    // Update timestamp to move entry to top
    let conn = get_connection()?;
    let conn = conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
    let timestamp = chrono::Utc::now().timestamp();
    conn.execute(
        "UPDATE history SET timestamp = ? WHERE id = ?",
        params![timestamp, id],
    )?;

    info!(id = %id, "Copied entry to clipboard");
    Ok(())
}

/// Decode a base64 image string back to ImageData
#[allow(dead_code)]
fn decode_base64_image(content: &str) -> Option<arboard::ImageData<'static>> {
    // Format: "rgba:{width}:{height}:{base64_data}"
    let parts: Vec<&str> = content.splitn(4, ':').collect();
    if parts.len() != 4 || parts[0] != "rgba" {
        return None;
    }

    let width: usize = parts[1].parse().ok()?;
    let height: usize = parts[2].parse().ok()?;
    let bytes = BASE64.decode(parts[3]).ok()?;

    Some(arboard::ImageData {
        width,
        height,
        bytes: bytes.into(),
    })
}

/// Decode a clipboard image content string to GPUI RenderImage
///
/// Parses the RGBA format "rgba:{width}:{height}:{base64_data}" and creates
/// a RenderImage suitable for display in GPUI. Returns an Arc<RenderImage>
/// for efficient caching.
///
/// **IMPORTANT**: Call this ONCE per entry and cache the result. Do NOT
/// decode during rendering as this is expensive.
pub fn decode_to_render_image(content: &str) -> Option<Arc<RenderImage>> {
    // Format: "rgba:{width}:{height}:{base64_data}"
    let parts: Vec<&str> = content.splitn(4, ':').collect();
    if parts.len() != 4 || parts[0] != "rgba" {
        warn!("Invalid clipboard image format, expected rgba:W:H:data");
        return None;
    }

    let width: u32 = parts[1].parse().ok()?;
    let height: u32 = parts[2].parse().ok()?;
    let rgba_bytes = BASE64.decode(parts[3]).ok()?;

    // Verify byte count matches dimensions (RGBA = 4 bytes per pixel)
    let expected_bytes = (width as usize) * (height as usize) * 4;
    if rgba_bytes.len() != expected_bytes {
        warn!(
            "Clipboard image byte count mismatch: expected {}, got {}",
            expected_bytes,
            rgba_bytes.len()
        );
        return None;
    }

    // Create image::RgbaImage from raw bytes
    let rgba_image = image::RgbaImage::from_raw(width, height, rgba_bytes)?;

    // Create Frame from RGBA buffer
    let frame = image::Frame::new(rgba_image);

    // Create RenderImage with a single frame
    let render_image = RenderImage::new(SmallVec::from_elem(frame, 1));

    debug!(width, height, "Decoded clipboard image to RenderImage");
    Some(Arc::new(render_image))
}

/// Get image dimensions from content string without decoding
///
/// Returns (width, height) if the content is a valid image format.
pub fn get_image_dimensions(content: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = content.splitn(4, ':').collect();
    if parts.len() >= 3 && parts[0] == "rgba" {
        let width: u32 = parts[1].parse().ok()?;
        let height: u32 = parts[2].parse().ok()?;
        Some((width, height))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type_conversion() {
        assert_eq!(ContentType::Text.as_str(), "text");
        assert_eq!(ContentType::Image.as_str(), "image");
        assert_eq!(ContentType::from_str("text"), ContentType::Text);
        assert_eq!(ContentType::from_str("image"), ContentType::Image);
        assert_eq!(ContentType::from_str("unknown"), ContentType::Text);
    }

    #[test]
    fn test_db_path() {
        let path = get_db_path().expect("Should get DB path");
        assert!(path.to_string_lossy().contains("clipboard-history.db"));
    }

    #[test]
    fn test_image_hash_deterministic() {
        let image = arboard::ImageData {
            width: 100,
            height: 100,
            bytes: vec![0u8; 40000].into(),
        };

        let hash1 = compute_image_hash(&image);
        let hash2 = compute_image_hash(&image);
        assert_eq!(hash1, hash2, "Hash should be deterministic");
    }

    #[test]
    fn test_base64_image_roundtrip() {
        let original = arboard::ImageData {
            width: 2,
            height: 2,
            bytes: vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255].into(),
        };

        let encoded = encode_image_as_base64(&original).expect("Should encode");
        let decoded = decode_base64_image(&encoded).expect("Should decode");

        assert_eq!(original.width, decoded.width);
        assert_eq!(original.height, decoded.height);
        assert_eq!(original.bytes.as_ref(), decoded.bytes.as_ref());
    }
}
