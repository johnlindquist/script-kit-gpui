# Memory Allocation Audit

**Generated**: 2025-12-29  
**Scope**: All `.rs` files in `src/`  
**Status**: READ-ONLY audit - recommendations only

---

## Executive Summary

This audit identifies memory allocation hotspots, unnecessary clones, and opportunities for optimization in the Script Kit GPUI codebase.

### Key Findings

| Category | Count | Severity | Potential Savings |
|----------|-------|----------|-------------------|
| Arc/Mutex patterns | 19 Arc::new, 15 Mutex::new | Low | Minimal - most are appropriate |
| Clone in hot paths | ~15 critical | **High** | Significant render perf |
| String allocations | 100+ format!/to_string | Medium | Moderate - many in cold paths |
| VecDeque sizing | 4 locations | Low | Fixed capacity already used |
| Struct sizes | 2 large structs | Medium | Consider Box for Options |

**Estimated Overall Memory Reduction Potential**: 10-20% in render loops

---

## 1. Arc/Mutex Usage Analysis

### Appropriate Uses (Keep As-Is)

| Location | Pattern | Justification |
|----------|---------|---------------|
| `src/main.rs:926` | `Arc::new(Mutex::new(None))` for script_session | Cross-thread script state sharing |
| `src/clipboard_history.rs:193` | `Arc::new(Mutex::new(conn))` | Database connection shared across threads |
| `src/terminal/alacritty.rs:454` | `Arc::new(Mutex::new(state))` | Terminal state for async reader thread |
| `src/list_item.rs:559` | `Arc::new(render_image)` | RenderImage sharing - GPUI requires Arc |
| `src/watcher.rs:94,234,372` | `Arc::new(Mutex::new(false))` for debounce | Simple boolean flags for debouncing |

### Global Statics (OnceLock Pattern)

These use `OnceLock<Mutex<T>>` - appropriate for lazy-initialized global state:

```
src/clipboard_history.rs:94  - IMAGE_CACHE (HashMap of decoded images)
src/clipboard_history.rs:99  - ENTRY_CACHE (Vec of entries)
src/logging.rs:317           - LOG_BUFFER (VecDeque of log lines)
src/window_control.rs:425    - WINDOW_CACHE (HashMap of windows)
src/window_manager.rs:204    - WINDOW_MANAGER (singleton)
src/perf.rs:368              - PERF_TRACKER (performance metrics)
```

**Recommendation**: These are all appropriate - OnceLock ensures single initialization, Mutex provides thread safety.

### Potential Improvements

| Location | Current | Suggestion |
|----------|---------|------------|
| `src/main.rs:1498,2829,2830,2877,2878` | `Arc::new(self.theme.clone())` | Pass `&Theme` if lifetime permits |
| `src/main.rs:2811,2852` | `Arc::new(move \|id, value\| {})` | Consider static callbacks |

---

## 2. Clone Hotspots in Render Paths

### CRITICAL: Clones in Render Loops

These clones happen during `uniform_list` rendering, which runs every frame:

#### `src/main.rs:3463` - ArgPrompt choices
```rust
choices.iter().enumerate().map(|(i, c)| (i, c.clone())).collect()
```
**Problem**: Clones all `Choice` structs every frame when filtering.
**Impact**: O(n) allocations per frame where n = number of choices.
**Fix**: Use indices only, access via `&choices[idx]` in render.

#### `src/main.rs:5846` - Clipboard history entries
```rust
filtered_entries.iter().map(|(i, e)| (*i, (*e).clone())).collect()
```
**Problem**: Clones `ClipboardEntry` (contains String content).
**Impact**: High - clipboard entries can be large (image base64).
**Fix**: Use `Arc<ClipboardEntry>` or indices only.

#### `src/main.rs:6409` - App launcher entries
```rust
filtered_apps.iter().map(|(i, a)| (*i, (*a).clone())).collect()
```
**Problem**: Clones `AppInfo` (contains `Arc<RenderImage>` + Strings).
**Impact**: Moderate - Arc clone is cheap but Strings are not.
**Fix**: Use indices only.

#### `src/main.rs:6723` - Window list entries
```rust
filtered_windows.iter().map(|(i, w)| (*i, (*w).clone())).collect()
```
**Problem**: Clones `WindowInfo` structs.
**Fix**: Use indices only.

#### `src/main.rs:7142` - Design gallery items
```rust
gallery_items.iter().enumerate().map(|(i, item)| (i, item.clone())).collect()
```
**Problem**: Clones gallery items every frame.
**Fix**: Use indices only.

### HIGH: View Clone in Render
```rust
// src/main.rs:3538
let current_view = self.current_view.clone();
```
**Problem**: `AppView` enum contains large variants with Strings, Choices, etc.
**Impact**: Every render clones the entire view state.
**Fix**: Use pattern matching with references: `match &self.current_view { ... }`

### MEDIUM: request_id Clones in Message Handlers
```rust
// src/main.rs:1962-2002 (multiple occurrences)
Message::clipboard_history_list_response(request_id.clone(), entry_data)
Message::clipboard_history_success(request_id.clone())
Message::clipboard_history_error(request_id.clone(), e.to_string())
```
**Problem**: `request_id` is cloned 20+ times in message handling.
**Fix**: Clone once at handler entry, use references internally.

---

## 3. String Allocation Hotspots

### format!() in Hot Paths

#### Render-time format! (CRITICAL)
```rust
// src/main.rs:5870-5896 - Clipboard history render
format!("{}x{} image", w, h)
format!("{}...", truncated)
format!("{}m ago", age_secs / 60)
format!("{}h ago", age_secs / 3600)
format!("{}d ago", age_secs / 86400)
```
**Problem**: String formatting every render frame.
**Fix**: Cache formatted strings in entry struct or use static strings.

#### Logging format! (ACCEPTABLE)
The 100+ `format!()` calls in `logging::log()` are in cold paths and acceptable.

### to_string() Patterns

#### Repeated Static Conversions
```rust
// src/main.rs - Multiple occurrences
"popover".to_string()  // theme.rs:58
"typescript".to_string()  // main.rs:2874
"just now".to_string()  // main.rs:5889, 6103
```
**Fix**: Use `String::from` for compile-time strings, or SharedString.

#### Path Conversions
```rust
// src/main.rs:1318-1319
sm.script.path.to_string_lossy().to_string()
am.app.path.to_string_lossy().to_string()
```
**Problem**: Double allocation (Cow -> String).
**Fix**: Store paths as `String` if lossy conversion is always needed.

### SharedString Usage - Good Examples

The codebase already uses `SharedString` in several places:
```rust
src/main.rs:735       - last_output: Option<SharedString>
src/list_item.rs:125  - name: SharedString
src/actions.rs:486    - SharedString::from("Search actions...")
```

**Recommendation**: Expand SharedString usage for:
- `toast.message` (currently `SharedString` - good)
- `Choice.name` / `Choice.value` (currently `String`)
- Static placeholder strings

---

## 4. PNG/RenderImage Caching

### Current Implementation (GOOD)

```rust
// src/clipboard_history.rs:82-83
static IMAGE_CACHE: OnceLock<Mutex<HashMap<String, Arc<RenderImage>>>>

// src/main.rs:790-791
clipboard_image_cache: HashMap<String, Arc<gpui::RenderImage>>
```

**Analysis**: 
- Images are decoded once and cached as `Arc<RenderImage>`
- App icons use same pattern via `DecodedIcon = Arc<RenderImage>`
- Disk cache exists at `~/.sk/kit/cache/app-icons/`

**Current Status**: Well-optimized. No changes needed.

### Potential Improvement
Consider lazy loading images on-demand instead of pre-loading all:
```rust
// Current: Load all app icons at startup (~500ms)
// Alternative: Load icons as apps become visible
```

---

## 5. Theme/ColorScheme Cloning

### Current Pattern
```rust
// src/main.rs:1498, 2829, 2877
std::sync::Arc::new(self.theme.clone())
```

### Theme Structure Analysis
```rust
pub struct Theme {
    pub colors: ColorScheme,              // ~48 bytes (4 sub-structs)
    pub focus_aware: Option<...>,         // ~8 + 96 bytes (optional)
    pub opacity: Option<BackgroundOpacity>, // ~8 + 16 bytes
    pub drop_shadow: Option<DropShadow>,  // ~8 + 32 bytes
    pub vibrancy: Option<VibrancySettings>, // ~8 + 32 bytes (contains String)
    pub padding: Option<Padding>,         // ~8 + 48 bytes
}
```

**Estimated Size**: ~200-300 bytes (mostly primitives)

**Recommendation**: Theme cloning is acceptable given the size. However:
1. `VibrancySettings.material` is a `String` - could be an enum
2. Could use `Arc<Theme>` app-wide instead of cloning

### ListItemColors Pattern (GOOD)
```rust
#[derive(Clone, Copy)]
pub struct ListItemColors {
    // All u32 primitives - cheap to copy
}
```
This is the correct pattern - extract Copy-able primitives for closures.

---

## 6. VecDeque Sizing in Performance Tracking

### Current Implementation
```rust
// src/perf.rs
const MAX_SAMPLES: usize = 100;

// All VecDeques pre-allocated:
event_times: VecDeque::with_capacity(MAX_SAMPLES),        // 100 Instants
processing_durations: VecDeque::with_capacity(MAX_SAMPLES), // 100 Durations
durations: VecDeque::with_capacity(MAX_SAMPLES),          // 100 Durations
frame_times: VecDeque::with_capacity(MAX_SAMPLES),        // 100 Durations

// src/logging.rs:317
const MAX_LOG_LINES: usize = 50;
VecDeque::with_capacity(MAX_LOG_LINES)  // 50 Strings
```

**Analysis**: 
- Pre-allocated with `with_capacity()` - good
- Ring buffer behavior with `pop_front()` + `push_back()` - good
- No growth/reallocation after initial capacity

**Memory per VecDeque**:
- Instants: 100 * 16 bytes = 1.6 KB
- Durations: 100 * 16 bytes = 1.6 KB  
- Strings (logs): 50 * ~32 bytes (pointer) = 1.6 KB (+ string content)

**Status**: Well-optimized. No changes needed.

---

## 7. Large Struct Size Analysis

### ScriptListApp (~2000+ bytes estimated)
```rust
struct ScriptListApp {
    scripts: Vec<Script>,                    // 24 bytes (vec header)
    scriptlets: Vec<Scriptlet>,              // 24 bytes
    builtin_entries: Vec<BuiltInEntry>,      // 24 bytes
    apps: Vec<AppInfo>,                      // 24 bytes
    filter_text: String,                     // 24 bytes
    arg_input_text: String,                  // 24 bytes
    theme: Theme,                            // ~300 bytes
    config: Config,                          // ~200 bytes
    cached_filtered_results: Vec<SearchResult>, // 24 bytes
    clipboard_image_cache: HashMap<...>,     // 48 bytes
    // ... 30+ more fields
}
```

**Recommendation**: This struct is stack-allocated. Consider:
1. Boxing large optional fields
2. Splitting into sub-structs (ViewState, CacheState, etc.)

### AppView Enum Variants
```rust
enum AppView {
    ArgPrompt { choices: Vec<Choice>, ... },  // Can be large
    DivPrompt { html: String, ... },          // HTML can be large
    EditorPrompt { content: String, ... },    // Content can be large
}
```

**Recommendation**: Box large String/Vec fields:
```rust
ArgPrompt { choices: Box<Vec<Choice>>, ... }
```

### Struct Size Test Recommendation
Add compile-time size assertions:
```rust
const _: () = assert!(std::mem::size_of::<Script>() <= 128);
const _: () = assert!(std::mem::size_of::<Choice>() <= 128);
```

---

## 8. Specific File Recommendations

### src/main.rs

| Line | Issue | Recommendation |
|------|-------|----------------|
| 3463, 3469 | Choice clone in filter | Use indices, access by reference |
| 3538 | `current_view.clone()` | Match on `&self.current_view` |
| 5846 | ClipboardEntry clone | Use `Arc<ClipboardEntry>` |
| 6409 | AppInfo clone | Use indices only |
| 1962-2002 | request_id.clone() x20 | Clone once at handler start |

### src/scripts.rs

| Line | Issue | Recommendation |
|------|-------|----------------|
| 649, 711, 777, 847 | Iterator map creating new structs | Return references where possible |

### src/theme.rs

| Line | Issue | Recommendation |
|------|-------|----------------|
| 58 | `"popover".to_string()` | Use enum instead of String |
| 339-343 | ColorScheme clone in `to_color_scheme()` | Return reference if possible |

### src/clipboard_history.rs

| Line | Issue | Recommendation |
|------|-------|----------------|
| 120 | `cache.iter().take(limit).cloned().collect()` | Return reference if possible |

---

## 9. Quick Wins (Low Effort, High Impact)

1. **Remove view clone in render** (src/main.rs:3538)
   - Change: `match &self.current_view { ... }` 
   - Impact: Eliminates ~500+ bytes clone per frame

2. **Use indices for list rendering** (src/main.rs:3463, 5846, 6409, 6723)
   - Change: Store `Vec<usize>` instead of cloning items
   - Impact: Eliminates O(n) clones per frame

3. **Cache relative time strings** (src/main.rs:5888-5896)
   - Change: Store formatted time in entry, update periodically
   - Impact: Eliminates string allocation per item per frame

4. **Use SharedString for static placeholders**
   - Change: `SharedString::from("Search...")` (already done in some places)
   - Impact: Minor but consistent

---

## 10. Memory Profiling Recommendations

### Tools to Use
```bash
# Heap profiling with heaptrack
heaptrack ./target/release/script-kit-gpui

# Allocation counting with valgrind
valgrind --tool=massif ./target/release/script-kit-gpui

# Quick memory check
/usr/bin/time -l ./target/release/script-kit-gpui
```

### Metrics to Track
1. Peak RSS during search with 1000+ scripts
2. Allocations per render frame (target: 0)
3. String allocation count during typing

### Suggested Benchmarks
```rust
#[bench]
fn bench_filtered_results(b: &mut Bencher) {
    let app = setup_app_with_1000_scripts();
    b.iter(|| app.filtered_results())
}

#[bench]
fn bench_render_list_item(b: &mut Bencher) {
    let item = create_test_choice();
    b.iter(|| render_choice(&item))
}
```

---

## Summary of Changes by Priority

### P0 - Critical (Render Performance)
- [ ] Remove `current_view.clone()` in render
- [ ] Use indices instead of clones in uniform_list closures
- [ ] Cache formatted time strings

### P1 - High (Memory Reduction)
- [ ] Use `Arc<ClipboardEntry>` for clipboard history
- [ ] Box large AppView variants
- [ ] Clone request_id once per handler

### P2 - Medium (Code Quality)
- [ ] Replace VibrancySettings.material String with enum
- [ ] Add struct size compile-time assertions
- [ ] Expand SharedString usage

### P3 - Low (Nice to Have)
- [ ] Lazy load app icons on demand
- [ ] Split ScriptListApp into sub-structs
- [ ] Add memory profiling CI

---

**Skills**: [memory-audit] | **Cmds**: [grep, read] | **Changed**: [docs/perf/MEMORY.md] | **Risks**: [none - readonly audit]
