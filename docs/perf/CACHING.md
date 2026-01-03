# Script Kit GPUI: Caching Systems Audit

## Executive Summary

The application employs **8 distinct caching systems** spanning filter results, syntax previews, clipboard images, application icons, frecency scoring, and window references. Overall, the caching strategy is well-designed with proper invalidation patterns. Key findings:

- **High-impact caches**: Filter cache and preview cache provide significant performance gains
- **Memory concern**: Clipboard image cache has no size limits and could grow unbounded
- **Missing opportunity**: Script metadata is re-read from disk on every `read_scripts()` call
- **Static caches**: App cache and hotkey channel use `OnceLock` (never refresh without restart)

---

## Cache Inventory

| Cache | Location | Type | Size Limit | Invalidation Strategy | Hit Rate Logging |
|-------|----------|------|------------|----------------------|------------------|
| **Filter Cache** | `main.rs:777-778` | Single-entry | 1 item | Key mismatch | ✅ HIT/MISS logged |
| **Preview Cache** | `main.rs:781-783` | Single-entry | 1 file | Path change | ✅ HIT/MISS logged |
| **Clipboard Image Cache** | `main.rs:791` + `clipboard_history.rs:82` | HashMap | ⚠️ Unbounded | Never | ✅ Cached count logged |
| **Clipboard Entry Cache** | `clipboard_history.rs:87` | Vec | MAX_HISTORY (1000) | On add/update | ✅ Logged |
| **App Icons Cache** | `~/.sk/kit/cache/app-icons/` | Disk (PNG) | ⚠️ Unbounded | mtime comparison | ✅ Stats logged |
| **App List Cache** | `app_launcher.rs:72` | OnceLock Vec | Static | Never (restart required) | ❌ No HIT/MISS |
| **Frecency Store** | `~/.sk/kit/frecency.json` | HashMap→JSON | ⚠️ Unbounded | Dirty flag | ✅ Load/Save logged |
| **Window Cache** | `window_control.rs:421` | OnceLock HashMap | Cleared on scan | `clear_window_cache()` | ❌ No HIT/MISS |

---

## Detailed Analysis

### 1. Filter Cache (`main.rs`)

**Purpose**: Cache fuzzy search results to avoid recomputing on every render.

**Implementation**:
```rust
// Fields (line 777-778)
cached_filtered_results: Vec<scripts::SearchResult>,
filter_cache_key: String,
```

**Invalidation Triggers**:
- `filter_text` changes (key mismatch)
- Scripts/scriptlets reload (`invalidate_filter_cache()`)
- Apps loaded in background (`\0_APPS_LOADED_\0` sentinel)

**Strengths**:
- ✅ Proper sentinel values (`\0_UNINITIALIZED_\0`, `\0_INVALIDATED_\0`, `\0_APPS_LOADED_\0`)
- ✅ Logs HIT/MISS events with context
- ✅ Performance measurement on cache miss

**Issues**:
- ⚠️ Two different search functions exist: `filtered_results()` (non-caching clone) and `get_filtered_results_cached()` (caching reference) - potential for inconsistent use
- ⚠️ Cache key is the entire filter string - no normalization (case, whitespace trimming)

**Recommendations**:
1. Consolidate to single caching function
2. Consider LRU cache for recent filter strings (keep last 10-20 queries)

---

### 2. Preview Cache (`main.rs`)

**Purpose**: Avoid re-reading and re-highlighting script files on every render.

**Implementation**:
```rust
// Fields (line 781-783)
preview_cache_path: Option<String>,
preview_cache_lines: Vec<syntax::HighlightedLine>,
```

**Behavior**:
- Caches syntax-highlighted lines for ONE script at a time
- Reads first 15 lines of file
- Re-reads on path change

**Strengths**:
- ✅ Avoids expensive I/O + syntax highlighting during render
- ✅ Logs HIT/MISS with path context

**Issues**:
- ⚠️ No file modification detection - stale cache if script edited externally
- ⚠️ Single-entry cache - scrolling through list causes many misses

**Recommendations**:
1. Add file mtime check to detect external modifications
2. Consider LRU cache of 5-10 recent previews for faster list navigation

---

### 3. Clipboard Image Cache (`clipboard_history.rs`)

**Purpose**: Pre-decode base64 images to RenderImage for fast display.

**Implementation**:
```rust
// Global cache (line 82-83)
static IMAGE_CACHE: OnceLock<Mutex<HashMap<String, Arc<RenderImage>>>> = OnceLock::new();

// Local cache per-render (main.rs:791)
clipboard_image_cache: HashMap<String, Arc<RenderImage>>,
```

**Behavior**:
- Background thread pre-warms cache on init (`prewarm_image_cache()`)
- New images decoded immediately in monitor loop
- Two-tier: global static + per-app-instance

**Strengths**:
- ✅ Pre-warming avoids decode during render
- ✅ Arc sharing avoids memory duplication
- ✅ Logs cache size on insert

**Issues**:
- ⚠️ **No size limit** - could grow unbounded if user copies many images
- ⚠️ No eviction policy - stale images never removed
- ⚠️ Memory can't be reclaimed without restart

**Recommendations**:
1. **HIGH PRIORITY**: Add LRU eviction (max 50-100 images)
2. Estimate memory: ~2MB per 1080p RGBA image
3. Consider evicting on entry deletion from SQLite

---

### 4. Clipboard Entry Cache (`clipboard_history.rs`)

**Purpose**: Avoid SQLite queries when opening clipboard history view.

**Implementation**:
```rust
// Global cache (line 87)
static ENTRY_CACHE: OnceLock<Mutex<Vec<ClipboardEntry>>> = OnceLock::new();
```

**Behavior**:
- Pre-warmed on `init_clipboard_history()`
- Refreshed after every `add_entry()` call
- Caches up to MAX_HISTORY_ENTRIES (1000)

**Strengths**:
- ✅ Proper refresh on mutations
- ✅ Bounded by MAX_HISTORY_ENTRIES
- ✅ Falls back to SQLite if empty

**Issues**:
- ⚠️ Full refresh on every add - could use incremental prepend
- ⚠️ Cache timestamp (`CACHE_UPDATED`) tracked but never used for staleness check

**Recommendations**:
1. Use incremental update: prepend new entry, truncate if > limit
2. Remove unused CACHE_UPDATED or implement TTL-based staleness

---

### 5. App Icons Disk Cache (`app_launcher.rs`)

**Purpose**: Persist extracted app icons to avoid expensive macOS API calls.

**Implementation**:
```rust
// Cache location: ~/.sk/kit/cache/app-icons/{hash}.png
fn get_or_extract_icon(app_path: &Path) -> Option<Vec<u8>> {
    // Uses mtime comparison for invalidation
}
```

**Behavior**:
- Cache key: hash of app bundle path
- Invalidation: cache file mtime < app bundle mtime
- Format: PNG files on disk

**Strengths**:
- ✅ Persists across app restarts
- ✅ Smart mtime-based invalidation
- ✅ Logs cache stats (count, size_kb)
- ✅ Sets cache file mtime to match app for easy comparison

**Issues**:
- ⚠️ **No size limit** - uninstalled apps leave orphan cache files
- ⚠️ No cleanup mechanism for stale entries
- ⚠️ Hash collisions theoretically possible (DefaultHasher)

**Recommendations**:
1. Add periodic cleanup: remove cache files for non-existent apps
2. Consider max cache size (e.g., 100MB) with LRU eviction
3. Log orphan count in `get_icon_cache_stats()`

---

### 6. App List Cache (`app_launcher.rs`)

**Purpose**: Avoid rescanning application directories.

**Implementation**:
```rust
static APP_CACHE: OnceLock<Vec<AppInfo>> = OnceLock::new();

pub fn scan_applications() -> &'static Vec<AppInfo> {
    APP_CACHE.get_or_init(|| { ... })
}
```

**Behavior**:
- One-time initialization on first call
- Never refreshes without app restart
- `scan_applications_fresh()` exists but doesn't update static cache

**Strengths**:
- ✅ Fast after initial scan (~100ms saved per access)
- ✅ Thread-safe via OnceLock

**Issues**:
- ⚠️ **Static cache** - new app installations not detected
- ⚠️ `scan_applications_fresh()` marked `#[allow(dead_code)]` - unused

**Recommendations**:
1. Add file watcher on /Applications directories (like scripts watcher)
2. Or: add "Refresh Apps" button/command
3. Consider background refresh every 5-10 minutes

---

### 7. Frecency Store (`frecency.rs`)

**Purpose**: Track script usage for relevance scoring.

**Implementation**:
```rust
pub struct FrecencyStore {
    entries: HashMap<String, FrecencyEntry>,
    file_path: PathBuf,  // ~/.sk/kit/frecency.json
    dirty: bool,
}
```

**Behavior**:
- Loaded on startup, scores recalculated (exponential decay)
- Dirty flag prevents unnecessary writes
- 7-day half-life for score decay

**Strengths**:
- ✅ Dirty flag optimization
- ✅ Score recalculation on load handles time passage
- ✅ Well-documented constants (HALF_LIFE_DAYS, SECONDS_PER_DAY)

**Issues**:
- ⚠️ **No size limit** - tracks deleted scripts forever
- ⚠️ No pruning of low-score entries
- ⚠️ Stale entries (score < 0.01) never removed

**Recommendations**:
1. Add pruning: remove entries with score < 0.01 on save
2. Consider max entries (e.g., 500) with LRU eviction
3. Remove entries for non-existent script paths on load

---

### 8. Window Cache (`window_control.rs`)

**Purpose**: Cache window AXUIElement references for faster access.

**Implementation**:
```rust
static WINDOW_CACHE: OnceLock<Mutex<HashMap<u32, usize>>> = OnceLock::new();
```

**Behavior**:
- Caches window ID → AXUIElementRef mapping
- Cleared on each `list_windows()` scan

**Strengths**:
- ✅ Proper clear before each scan
- ✅ Fast lookup by window ID

**Issues**:
- ⚠️ No HIT/MISS logging
- ⚠️ Stores raw pointer as usize - could be invalid if window closed

**Recommendations**:
1. Add debug logging for cache operations
2. Consider validity check on `get_cached_window()`

---

## Missing Cache Opportunities

### 1. Script Metadata Caching (HIGH PRIORITY)

**Current**: `read_scripts()` reads and parses every script file on every call.

```rust
// scripts.rs:591 - Called on every refresh
let script_metadata = extract_metadata(&path);  // Reads file, parses first 20 lines
```

**Impact**: With 100 scripts, refresh reads 100 files from disk.

**Recommendation**:
```rust
struct ScriptMetadataCache {
    entries: HashMap<PathBuf, (SystemTime, ScriptMetadata)>,  // path -> (mtime, metadata)
}
```
- Cache metadata with file mtime
- Only re-read if mtime changed
- Expected speedup: 10-50x for script refresh

### 2. Syntax Set/Theme Caching

**Current**: `SyntaxSet::load_defaults_newlines()` and `ThemeSet::load_defaults()` called on every highlight.

```rust
// syntax.rs:83-84 - Called on every preview render
let ps = SyntaxSet::load_defaults_newlines();
let ts = ThemeSet::load_defaults();
```

**Recommendation**: Use `lazy_static!` or `OnceLock` to cache syntax/theme sets:
```rust
static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();
```

### 3. Config Caching

**Current**: `load_config()` reads file on startup and theme reload.

**Observation**: Config is already cached in `ScriptListApp.config` field. ✅ No issue.

---

## Memory Usage Estimates

| Cache | Estimated Size | Concern Level |
|-------|---------------|---------------|
| Filter Cache | ~1MB max (1000 SearchResults) | Low |
| Preview Cache | ~10KB (15 lines highlighted) | Low |
| Clipboard Image Cache | **~200MB+ potential** (100 images @ 2MB each) | ⚠️ HIGH |
| Entry Cache | ~100KB (1000 text entries) | Low |
| App Icons (memory) | ~50MB (200 apps @ 256KB PNG decoded) | Medium |
| App Icons (disk) | ~10MB (200 apps @ 50KB PNG) | Low |
| Frecency Store | ~50KB (500 entries) | Low |
| Window Cache | ~1KB (100 windows) | Low |

---

## Cache Invalidation Correctness

| Cache | Invalidation Correct? | Notes |
|-------|----------------------|-------|
| Filter Cache | ✅ Yes | Proper sentinel patterns |
| Preview Cache | ⚠️ Partial | Missing file modification detection |
| Clipboard Image | ⚠️ No | Never evicts |
| Entry Cache | ✅ Yes | Refreshed on mutations |
| App Icons | ✅ Yes | mtime comparison works |
| App List | ❌ No | Never refreshes |
| Frecency | ✅ Yes | Dirty flag works |
| Window Cache | ✅ Yes | Cleared each scan |

---

## Recommendations Summary

### High Priority
1. **Add LRU eviction to clipboard image cache** - Memory leak risk
2. **Cache script metadata with mtime** - Performance on refresh
3. **Add eviction to app icons disk cache** - Disk space leak

### Medium Priority
4. Cache syntax sets (OnceLock) - Minor CPU savings
5. LRU cache for preview (5-10 recent) - Faster list navigation
6. Prune frecency entries on load - Cleaner data

### Low Priority
7. Add file modification detection to preview cache
8. Implement app list refresh mechanism
9. Add HIT/MISS logging to window cache

---

## Monitoring Recommendations

### Current Logging (Good)
```
CACHE: Filter cache HIT for 'query'
CACHE: Filter cache MISS - recomputing for 'query'
CACHE: Preview cache HIT for '/path/script.ts'
```

### Suggested Additions
```rust
// Periodic cache stats (every 5 minutes or on idle)
logging::log("CACHE_STATS", &format!(
    "filter_hits={} filter_misses={} preview_hits={} preview_misses={} image_cache_size={} frecency_entries={}",
    filter_hits, filter_misses, preview_hits, preview_misses,
    clipboard_image_cache.len(), frecency_store.len()
));
```

---

## Appendix: Cache Code Locations

| Cache | File | Line Numbers |
|-------|------|-------------|
| Filter Cache | `src/main.rs` | 777-778, 1060-1124 |
| Preview Cache | `src/main.rs` | 781-783, 1126-1164 |
| Clipboard Image | `src/clipboard_history.rs` | 82-113, 244-265 |
| Entry Cache | `src/clipboard_history.rs` | 87-149 |
| App Icons | `src/app_launcher.rs` | 93-209 |
| App List | `src/app_launcher.rs` | 72, 224-243 |
| Frecency | `src/frecency.rs` | 91-280 |
| Window Cache | `src/window_control.rs` | 420-445 |

---

*Audit completed: 2025-12-29*
*Auditor: CacheAuditor (Swarm Agent)*
