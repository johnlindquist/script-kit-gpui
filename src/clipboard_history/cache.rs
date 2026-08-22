//! Clipboard history caching
//!
//! LRU caching for decoded images and entry metadata.

use gpui::RenderImage;
use lru::LruCache;
use parking_lot::Mutex;
use std::num::NonZeroUsize;
use std::sync::{Arc, OnceLock};
use tracing::debug;

use super::database::get_clipboard_history_meta;
use super::types::{
    root_clipboard_entry_is_eligible, root_clipboard_history_query_is_eligible, ClipboardEntryMeta,
    RootClipboardHistorySectionOptions,
};

/// Maximum number of decoded images to keep in memory (LRU eviction)
/// Each image can be 1-4MB, so 100 images = ~100-400MB max memory
pub const MAX_IMAGE_CACHE_ENTRIES: usize = 100;

/// Maximum entries to cache in memory for fast access
pub const MAX_CACHED_ENTRIES: usize = 500;
const ROOT_CLIPBOARD_HISTORY_REFRESH_LABEL: &str = "root-clipboard-history-cache";

/// Global image cache for decoded RenderImages (thread-safe)
/// Key: entry ID, Value: decoded RenderImage
/// Uses LRU eviction to cap memory usage at ~100-400MB (100 images max)
static IMAGE_CACHE: OnceLock<Mutex<LruCache<String, Arc<RenderImage>>>> = OnceLock::new();

/// Cached clipboard entry metadata (NO content payload) for list views
/// Updated whenever a new entry is added. This is memory-efficient because
/// it doesn't include the full content field (which can be megabytes for images).
/// Wrapped in Arc to allow cheap clones when passing to UI.
static ENTRY_CACHE: OnceLock<Mutex<Arc<Vec<ClipboardEntryMeta>>>> = OnceLock::new();

/// Timestamp of last cache update
static CACHE_UPDATED: OnceLock<Mutex<i64>> = OnceLock::new();
static ENTRY_CACHE_READY: OnceLock<Mutex<bool>> = OnceLock::new();
static ENTRY_CACHE_REFRESH_LIFECYCLE: OnceLock<
    Mutex<crate::scripts::root_search_contract::RootOwnedProviderRefreshLifecycle>,
> = OnceLock::new();

pub(crate) type RootClipboardHistoryRefresh =
    crate::scripts::root_search_contract::RootOwnedProviderRefresh;

pub(crate) struct RootClipboardHistorySnapshot {
    entries: Vec<ClipboardEntryMeta>,
}

/// Get the global image cache, initializing if needed
pub fn get_image_cache() -> &'static Mutex<LruCache<String, Arc<RenderImage>>> {
    IMAGE_CACHE.get_or_init(|| {
        let cap = NonZeroUsize::new(MAX_IMAGE_CACHE_ENTRIES).unwrap_or(NonZeroUsize::MIN);
        Mutex::new(LruCache::new(cap))
    })
}

/// Get the global entry cache, initializing if needed
pub fn get_entry_cache() -> &'static Mutex<Arc<Vec<ClipboardEntryMeta>>> {
    ENTRY_CACHE.get_or_init(|| Mutex::new(Arc::new(Vec::new())))
}

fn entry_cache_ready() -> &'static Mutex<bool> {
    ENTRY_CACHE_READY.get_or_init(|| Mutex::new(false))
}

fn entry_cache_refresh_lifecycle(
) -> &'static Mutex<crate::scripts::root_search_contract::RootOwnedProviderRefreshLifecycle> {
    ENTRY_CACHE_REFRESH_LIFECYCLE.get_or_init(|| {
        Mutex::new(
            crate::scripts::root_search_contract::RootOwnedProviderRefreshLifecycle::default(),
        )
    })
}

/// Initialize the cache timestamp tracker
pub fn init_cache_timestamp() {
    let _ = CACHE_UPDATED.set(Mutex::new(0));
}

/// Get cached image by entry ID (updates LRU order)
pub fn get_cached_image(id: &str) -> Option<Arc<RenderImage>> {
    get_image_cache().lock().get(id).cloned()
}

/// Cache a decoded image (with LRU eviction at MAX_IMAGE_CACHE_ENTRIES limit)
pub fn cache_image(id: &str, image: Arc<RenderImage>) {
    let mut cache = get_image_cache().lock();
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

/// Get cached clipboard entry metadata (faster than querying SQLite)
/// Falls back to SQLite if cache is empty.
/// Returns Arc to avoid cloning the entire cache - caller can clone individual entries if needed.
pub fn get_cached_entries(limit: usize) -> Vec<ClipboardEntryMeta> {
    let cache = get_entry_cache().lock();
    if !cache.is_empty() {
        // Only clone the entries we need, not the entire cache
        let result: Vec<_> = cache.iter().take(limit).cloned().collect();
        debug!(
            count = result.len(),
            cached = true,
            "Retrieved clipboard entry metadata from cache"
        );
        return result;
    }
    drop(cache);
    // Fall back to database (metadata-only query)
    get_clipboard_history_meta(limit, 0)
}

/// Invalidate the entry cache (called when entries change)
pub fn invalidate_entry_cache() {
    let mut cache = get_entry_cache().lock();
    *cache = Arc::new(Vec::new());
    drop(cache);
    *entry_cache_ready().lock() = false;
}

/// Refresh the entry cache from database (metadata only, no content payload)
pub fn refresh_entry_cache() {
    // Use metadata-only query to avoid loading full content
    let entries = get_clipboard_history_meta(MAX_CACHED_ENTRIES, 0);
    let mut cache = get_entry_cache().lock();
    // Replace the Arc with a new one containing the refreshed entries
    *cache = Arc::new(entries);
    debug!(count = cache.len(), "Refreshed entry metadata cache");
    drop(cache);
    *entry_cache_ready().lock() = true;
    if let Some(updated) = CACHE_UPDATED.get() {
        let mut ts = updated.lock();
        *ts = chrono::Utc::now().timestamp_millis();
    }
}

pub(crate) fn root_clipboard_history_cache_is_fresh() -> bool {
    !get_entry_cache().lock().is_empty() || *entry_cache_ready().lock()
}

pub(crate) fn try_begin_root_clipboard_history_refresh() -> Option<RootClipboardHistoryRefresh> {
    let cache_is_fresh = root_clipboard_history_cache_is_fresh();
    entry_cache_refresh_lifecycle().lock().begin(
        sk_protocol::command_contract::CommandSource::Clipboard,
        cache_is_fresh,
    )
}

pub(crate) fn read_root_clipboard_history_snapshot() -> RootClipboardHistorySnapshot {
    tracing::debug!(
        target: "script_kit::search",
        worker = ROOT_CLIPBOARD_HISTORY_REFRESH_LABEL,
        "Reading owned private clipboard metadata snapshot"
    );
    RootClipboardHistorySnapshot {
        entries: get_clipboard_history_meta(MAX_CACHED_ENTRIES, 0),
    }
}

pub(crate) fn finish_root_clipboard_history_refresh(
    refresh: RootClipboardHistoryRefresh,
    snapshot: RootClipboardHistorySnapshot,
) -> bool {
    if !entry_cache_refresh_lifecycle().lock().finish(refresh) {
        return false;
    }
    let mut cache = get_entry_cache().lock();
    *cache = Arc::new(snapshot.entries);
    let count = cache.len();
    drop(cache);
    *entry_cache_ready().lock() = true;
    update_cache_timestamp();
    debug!(count, "Published owned clipboard metadata snapshot");
    true
}

pub(crate) fn discard_root_clipboard_history_refresh(refresh: RootClipboardHistoryRefresh) -> bool {
    entry_cache_refresh_lifecycle().lock().finish(refresh)
}

/// Cache-only root clipboard search used by foreground launcher grouping.
///
/// A cold metadata cache returns no hits and never starts SQLite work. The
/// launcher input owner coordinates one generation-fenced background refresh.
pub fn search_root_clipboard_history_meta_cached(
    query: &str,
    options: RootClipboardHistorySectionOptions,
) -> Vec<ClipboardEntryMeta> {
    if !root_clipboard_history_query_is_eligible(query, options) {
        return Vec::new();
    }

    let entries = {
        let cache = get_entry_cache().lock();
        if cache.is_empty() {
            None
        } else {
            Some(
                cache
                    .iter()
                    .take(options.scan_limit)
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        }
    };

    let Some(entries) = entries else {
        return Vec::new();
    };

    let query = query.trim().to_lowercase();
    entries
        .into_iter()
        .filter(root_clipboard_entry_is_eligible)
        .filter(|entry| entry.text_preview.to_lowercase().contains(&query))
        .take(options.max_results)
        .collect()
}

/// Evict a single entry from the image cache
pub fn evict_image_cache(id: &str) {
    if let Some(cache) = IMAGE_CACHE.get() {
        let mut cache = cache.lock();
        cache.pop(id);
        debug!(id = %id, "Evicted image from cache");
    }
}

/// Incrementally upsert an entry in the cache.
///
/// This is much faster than refresh_entry_cache() because it:
/// - Doesn't query SQLite
/// - Only updates/inserts a single entry
/// - Re-sorts only when necessary
///
/// Use this after add_entry() instead of refresh_entry_cache().
pub fn upsert_entry_in_cache(entry: ClipboardEntryMeta) {
    let mut cache_arc = get_entry_cache().lock();
    // Clone the Vec to modify it (Arc::make_mut would clone if refcount > 1)
    let mut cache = (**cache_arc).clone();

    // Remove existing entry with same ID (if any)
    cache.retain(|e| e.id != entry.id);

    // Insert at the correct position (pinned first, then by timestamp desc)
    // For efficiency, we insert at position 0 or after pinned entries
    // since new/touched entries have the latest timestamp
    let insert_pos = if entry.pinned {
        0 // Pinned entries go to front
    } else {
        // Find first non-pinned entry
        cache.iter().position(|e| !e.pinned).unwrap_or(0)
    };

    cache.insert(insert_pos, entry);

    // Truncate to max size
    cache.truncate(MAX_CACHED_ENTRIES);

    debug!(cache_size = cache.len(), "Incremental cache upsert");

    // Replace the Arc with the modified Vec
    *cache_arc = Arc::new(cache);
    drop(cache_arc);

    // Update timestamp
    update_cache_timestamp();
}

/// Remove an entry from the cache by ID.
///
/// Use this after remove_entry() instead of refresh_entry_cache().
pub fn remove_entry_from_cache(id: &str) {
    let mut cache_arc = get_entry_cache().lock();
    let mut cache = (**cache_arc).clone();
    let before = cache.len();
    cache.retain(|e| e.id != id);
    if cache.len() < before {
        debug!(id = %id, "Removed entry from cache");
        // Only update Arc if we actually removed something
        *cache_arc = Arc::new(cache);
    }
    drop(cache_arc);
    update_cache_timestamp();
}

/// Update the pinned status of an entry in the cache.
///
/// Re-sorts the cache to maintain pinned-first ordering.
/// Use this after pin_entry/unpin_entry() instead of refresh_entry_cache().
pub fn update_pin_status_in_cache(id: &str, pinned: bool) {
    let mut cache_arc = get_entry_cache().lock();
    let mut cache = (**cache_arc).clone();
    if let Some(entry) = cache.iter_mut().find(|e| e.id == id) {
        entry.pinned = pinned;
    }
    // Re-sort: pinned first, then by timestamp descending
    cache.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| b.timestamp.cmp(&a.timestamp))
    });
    debug!(id = %id, pinned = pinned, "Updated pin status in cache");
    *cache_arc = Arc::new(cache);
    drop(cache_arc);
    update_cache_timestamp();
}

/// Update OCR text for a single cached entry.
pub(crate) fn update_ocr_text_in_cache(id: &str, text: String) {
    let mut cache_arc = get_entry_cache().lock();
    let mut cache = (**cache_arc).clone();
    let mut updated = false;

    if let Some(entry) = cache.iter_mut().find(|e| e.id == id) {
        entry.ocr_text = Some(text);
        updated = true;
    }

    if updated {
        debug!(id = %id, "Updated OCR text in cache");
        *cache_arc = Arc::new(cache);
        drop(cache_arc);
        update_cache_timestamp();
    }
}

/// Update the cache timestamp (internal helper)
fn update_cache_timestamp() {
    *entry_cache_ready().lock() = true;
    if let Some(updated) = CACHE_UPDATED.get() {
        let mut ts = updated.lock();
        *ts = chrono::Utc::now().timestamp_millis();
    }
}

/// Clear all caches (entry + image)
pub fn clear_all_caches() {
    invalidate_entry_cache();
    if let Some(cache) = IMAGE_CACHE.get() {
        let mut cache = cache.lock();
        cache.clear();
        debug!("Cleared image cache");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard_history::types::ContentType;
    use parking_lot::Mutex as ParkingMutex;

    /// All cache tests must hold this lock to prevent interference
    /// from parallel test execution on the shared global ENTRY_CACHE.
    static TEST_LOCK: ParkingMutex<()> = ParkingMutex::new(());

    fn make_meta(id: &str, timestamp: i64, pinned: bool) -> ClipboardEntryMeta {
        ClipboardEntryMeta {
            id: id.to_string(),
            content_type: ContentType::Text,
            timestamp,
            pinned,
            text_preview: format!("preview-{}", id),
            image_width: None,
            image_height: None,
            byte_size: 10,
            ocr_text: None,
        }
    }

    #[test]
    fn root_clipboard_refresh_accepts_an_empty_owned_snapshot_without_restarting_forever() {
        let _lock = TEST_LOCK.lock();
        invalidate_entry_cache();
        assert!(!root_clipboard_history_cache_is_fresh());

        let refresh = try_begin_root_clipboard_history_refresh()
            .expect("one owned worker for a cold clipboard cache");
        assert!(try_begin_root_clipboard_history_refresh().is_none());
        assert!(finish_root_clipboard_history_refresh(
            refresh,
            RootClipboardHistorySnapshot {
                entries: Vec::new()
            },
        ));
        assert!(root_clipboard_history_cache_is_fresh());
        assert!(try_begin_root_clipboard_history_refresh().is_none());
        invalidate_entry_cache();
    }

    #[test]
    fn root_clipboard_refresh_refuses_another_providers_snapshot_before_publication() {
        let _lock = TEST_LOCK.lock();
        invalidate_entry_cache();
        let refresh =
            try_begin_root_clipboard_history_refresh().expect("clipboard owns its refresh worker");
        let forged = crate::scripts::root_search_contract::RootOwnedProviderRefresh {
            source: sk_protocol::command_contract::CommandSource::Dictation,
            generation: refresh.generation,
        };

        assert!(!finish_root_clipboard_history_refresh(
            forged,
            RootClipboardHistorySnapshot {
                entries: vec![make_meta("foreign-owner", 1, false)],
            },
        ));
        assert!(get_entry_cache().lock().is_empty());
        assert!(!root_clipboard_history_cache_is_fresh());
        assert!(discard_root_clipboard_history_refresh(refresh));
        invalidate_entry_cache();
    }

    #[test]
    fn test_upsert_new_entry() {
        let _guard = TEST_LOCK.lock();
        invalidate_entry_cache();

        let entry = make_meta("test1", 1000, false);
        upsert_entry_in_cache(entry.clone());

        let cached = get_cached_entries(10);
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].id, "test1");
    }

    #[test]
    fn test_upsert_updates_existing() {
        let _guard = TEST_LOCK.lock();
        invalidate_entry_cache();

        // Add initial entry
        upsert_entry_in_cache(make_meta("test2", 1000, false));

        // Update with new timestamp
        upsert_entry_in_cache(make_meta("test2", 2000, false));

        let cached = get_cached_entries(10);
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].timestamp, 2000);
    }

    #[test]
    fn test_upsert_maintains_pinned_order() {
        let _guard = TEST_LOCK.lock();
        invalidate_entry_cache();

        // Add unpinned entry first
        upsert_entry_in_cache(make_meta("unpinned", 2000, false));
        // Add pinned entry
        upsert_entry_in_cache(make_meta("pinned", 1000, true));

        let cached = get_cached_entries(10);
        assert_eq!(cached.len(), 2);
        // Pinned should be first despite lower timestamp
        assert_eq!(cached[0].id, "pinned");
        assert_eq!(cached[1].id, "unpinned");
    }

    #[test]
    fn test_remove_entry_from_cache() {
        let _guard = TEST_LOCK.lock();
        invalidate_entry_cache();

        upsert_entry_in_cache(make_meta("to_remove", 1000, false));
        upsert_entry_in_cache(make_meta("to_keep", 2000, false));

        remove_entry_from_cache("to_remove");

        let cached = get_cached_entries(10);
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].id, "to_keep");
    }

    #[test]
    fn test_update_pin_status() {
        let _guard = TEST_LOCK.lock();
        invalidate_entry_cache();

        upsert_entry_in_cache(make_meta("entry1", 2000, false));
        upsert_entry_in_cache(make_meta("entry2", 1000, false));

        // Pin entry2 (lower timestamp)
        update_pin_status_in_cache("entry2", true);

        let cached = get_cached_entries(10);
        // entry2 should now be first (pinned)
        assert_eq!(cached[0].id, "entry2");
        assert!(cached[0].pinned);
    }

    #[test]
    fn test_update_ocr_text_in_cache_sets_text_when_entry_exists() {
        let _guard = TEST_LOCK.lock();
        init_cache_timestamp();
        invalidate_entry_cache();

        upsert_entry_in_cache(make_meta("ocr-target", 3000, false));
        update_ocr_text_in_cache("ocr-target", "recognized text".to_string());

        let cached = get_cached_entries(10);
        assert_eq!(cached[0].ocr_text.as_deref(), Some("recognized text"));
    }
}
