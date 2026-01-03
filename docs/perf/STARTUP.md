# Startup Performance Audit

**Date**: 2025-12-29  
**Auditor**: StartupAuditor Agent (cell--9bnr5-mjqv2hnn3rv)  
**Scope**: READ-ONLY analysis of initialization sequence and startup time

---

## Executive Summary

The script-kit-gpui application has several **blocking operations** during startup that could be optimized. The most significant bottlenecks are:

1. **Config loading** - Spawns 2 subprocess calls to `bun` (called twice!)
2. **Script/scriptlet scanning** - Synchronous filesystem traversal
3. **Theme loading** - May spawn subprocess for system appearance detection

**Estimated cold start time**: 500-1500ms (depending on script count and disk speed)  
**Primary optimization opportunity**: Parallelize blocking I/O and defer non-critical initialization

---

## Initialization Sequence Timeline

### Phase 1: Pre-GPUI Setup (~50-200ms)

| Step | Function | Type | Est. Time | Notes |
|------|----------|------|-----------|-------|
| 1 | `logging::init()` | Sync | <5ms | File creation, tracing setup |
| 2 | `clipboard_history::init_clipboard_history()` | Spawn | ~20-50ms | SQLite DB open, table creation |
| 3 | `config::load_config()` | **BLOCKING** | ~100-300ms | **2 bun subprocess calls** |
| 4 | `start_hotkey_listener()` | Spawn | <5ms | Thread spawn only |
| 5 | `AppearanceWatcher::new() + start()` | Spawn | <10ms | Thread spawn |
| 6 | `ConfigWatcher::new() + start()` | Spawn | <10ms | FSEvents setup |
| 7 | `ScriptWatcher::new() + start()` | Spawn | <10ms | FSEvents setup |

### Phase 2: GPUI Application Init (~50-100ms)

| Step | Function | Type | Est. Time | Notes |
|------|----------|------|-----------|-------|
| 8 | `Application::new()` | Sync | ~20ms | GPUI framework init |
| 9 | `TrayManager::new()` | Sync | ~10-20ms | SVG parsing, icon creation |

### Phase 3: Window & App State (~200-800ms)

| Step | Function | Type | Est. Time | Notes |
|------|----------|------|-----------|-------|
| 10 | `cx.open_window()` | Sync | ~20ms | Native window creation |
| 11 | `scripts::read_scripts()` | **BLOCKING** | ~50-300ms | Glob + metadata extraction |
| 12 | `scripts::read_scriptlets()` | **BLOCKING** | ~20-100ms | Markdown parsing |
| 13 | `theme::load_theme()` | **BLOCKING** | ~10-50ms | JSON parse + subprocess |
| 14 | `config::load_config()` | **BLOCKING** | ~100-300ms | **DUPLICATE CALL!** |
| 15 | `FrecencyStore::load()` | Sync | ~5-20ms | JSON file read |
| 16 | `builtins::get_builtin_entries()` | Sync | <5ms | In-memory generation |
| 17 | App scanning | **Async** | N/A | Background thread (good!) |

### Phase 4: Event Loop Setup (~10-30ms)

| Step | Function | Type | Est. Time | Notes |
|------|----------|------|-----------|-------|
| 18 | Window registration | Sync | <5ms | |
| 19 | Focus handle setup | Sync | <5ms | |
| 20 | Stdin listener spawn | Async | <5ms | |
| 21 | Event handlers setup | Sync | ~10ms | |

---

## Blocking Operations Analysis

### 1. Config Loading (CRITICAL - ~200-600ms total)

**Location**: `src/config.rs`

**Problem**: Called **TWICE** during startup - once in `main()` and again in `ScriptListApp::new()`.

```rust
// First call in main() at line ~8293
let config = config::load_config();

// Second call in ScriptListApp::new() - DUPLICATE
let config = config::load_config();
```

**What it does**:
1. Runs `bun build ~/.sk/kit/config.ts` (~100-200ms)
2. Runs `bun -e "import config..."` to evaluate (~100-200ms)

**Impact**: 200-600ms of startup time, doubled due to duplicate call.

**Recommendation**: 
- Remove duplicate call - pass config from main() to ScriptListApp
- Consider caching compiled config with file modification check
- Explore using native Rust config parsing instead of bun subprocess

---

### 2. Script Scanning (~50-300ms)

**Location**: `src/scripts.rs`

**What it does**:
- `read_scripts()`: Globs `~/.sk/kit/scripts/*.ts`, reads each file for metadata
- `read_scriptlets()`: Reads `~/.sk/kit/scriptlets.md`, parses markdown

**Impact**: Linear with script count. 100 scripts ≈ 200ms.

**Recommendation**:
- Implement script metadata cache (hash-based invalidation)
- Use async parallel file reads with `tokio::spawn`
- Defer full metadata extraction until script is selected

---

### 3. Theme Loading (~10-50ms)

**Location**: `src/theme.rs`

**What it does**:
- Reads `~/.sk/kit/theme.json`
- Calls `defaults read -g AppleInterfaceStyle` for system appearance

**Impact**: Minor, but subprocess adds latency.

**Recommendation**:
- Cache system appearance, update via AppearanceWatcher
- Load theme file async while other init continues

---

### 4. Clipboard History Init (~20-50ms)

**Location**: `src/clipboard_history.rs`

**What it does**:
- Opens/creates SQLite database
- Creates tables if not exist
- Starts monitoring thread

**Impact**: Blocking DB operations on main thread before spawn.

**Recommendation**:
- Move DB initialization entirely to background thread
- Use lazy initialization - only init when clipboard history is accessed

---

## Lazy Loading Opportunities

### Already Lazy (Good!)

| Component | Implementation |
|-----------|----------------|
| App Launcher | `scan_applications()` runs in background thread |
| SDK Extraction | `ensure_sdk_extracted()` called on first script run |
| Stdin Processing | Event-driven via async_channel |
| Hotkey Events | Event-driven via async_channel |

### Should Be Lazy

| Component | Current | Proposed |
|-----------|---------|----------|
| Script metadata | Loaded at startup | Load on first filter/search |
| Scriptlets | Loaded at startup | Load when scriptlets tab accessed |
| Clipboard history DB | Init at startup | Init on first clipboard access |
| Full theme | Loaded at startup | Load minimal colors, defer full theme |

---

## Cold Start vs Warm Start

### Cold Start (First Launch After Boot)

- **Disk cache cold**: +100-300ms for file reads
- **bun not in memory**: +50-100ms for subprocess spawn
- **SQLite cold**: +20-50ms for DB open
- **Total estimate**: 800-1500ms

### Warm Start (Subsequent Launches)

- **Disk cache warm**: Files in OS buffer cache
- **bun potentially warm**: Faster subprocess
- **Total estimate**: 400-700ms

### Hot Reload (App Already Running, Window Re-shown)

- **No initialization**: All state in memory
- **Total**: <50ms (just window show animation)

---

## Optimization Recommendations

### Priority 1: Quick Wins (High Impact, Low Effort)

1. **Remove duplicate config::load_config() call**
   - Impact: -100-300ms
   - Effort: 1 line change
   - Pass config from main() to ScriptListApp::new()

2. **Parallelize Phase 3 blocking operations**
   - Impact: -200-400ms (parallel vs sequential)
   - Effort: Medium
   - Use `tokio::join!` or `rayon::join` for scripts + theme + frecency

### Priority 2: Medium Wins (Medium Impact, Medium Effort)

3. **Script metadata caching**
   - Impact: -50-200ms on warm start
   - Effort: Medium
   - Cache script metadata with file mtime/hash validation

4. **Move clipboard DB init to background**
   - Impact: -20-50ms
   - Effort: Low
   - Lazy init on first clipboard history access

### Priority 3: Larger Refactors (High Impact, High Effort)

5. **Native config parsing**
   - Impact: -200-500ms
   - Effort: High
   - Parse TypeScript config with tree-sitter or simplified format

6. **Startup splash / progressive loading**
   - Impact: Perceived performance
   - Effort: Medium
   - Show window immediately with loading state, populate async

---

## Startup Timeline Visualization

```
0ms     100ms    200ms    300ms    400ms    500ms    600ms    700ms
|--------|--------|--------|--------|--------|--------|--------|
[LOGGING ]
[CLIPBOARD---DB-INIT---]
[CONFIG-LOAD-1---------BUN-BUILD---------BUN-EVAL--------]
                                                          [HOTKEY]
                                                          [WATCHERS]
                                                          [GPUI-INIT--]
                                                                      [WINDOW]
                                                                      [SCRIPTS-SCAN-------]
                                                                      [SCRIPTLETS--]
                                                                      [THEME--]
                                                                      [CONFIG-2-DUPLICATE!----]
                                                                      [FRECENCY]
                                                                                    [APP-SCAN-BG...]
```

**Critical Path**: CONFIG-LOAD-1 → GPUI-INIT → SCRIPTS-SCAN → CONFIG-2

---

## Metrics to Track

For ongoing performance monitoring, instrument these metrics:

| Metric | Target | Current (Est.) |
|--------|--------|----------------|
| Time to window visible | <200ms | ~400-600ms |
| Time to interactive | <500ms | ~700-1200ms |
| Time to full script list | <800ms | ~800-1500ms |
| Config load time | <50ms | ~200-400ms |
| Script scan time | <100ms | ~100-300ms |

---

## Appendix: Source File References

| File | Key Functions | Lines |
|------|---------------|-------|
| `src/main.rs` | `main()`, `ScriptListApp::new()` | 8284, ~800 |
| `src/config.rs` | `load_config()` | Full file |
| `src/scripts.rs` | `read_scripts()`, `read_scriptlets()` | Full file |
| `src/theme.rs` | `load_theme()`, `get_system_appearance()` | Full file |
| `src/clipboard_history.rs` | `init_clipboard_history()` | Full file |
| `src/executor.rs` | `ensure_sdk_extracted()` | Full file |
| `src/logging.rs` | `init()` | Full file |
| `src/tray.rs` | `TrayManager::new()` | Full file |

---

## Conclusion

The most impactful optimization is **removing the duplicate config::load_config() call**, which alone could save 100-300ms. Combined with parallelizing the remaining blocking operations in Phase 3, startup time could be reduced by 40-60%.

The application already demonstrates good async patterns (app scanning, event-driven architecture). Extending these patterns to script loading and config parsing would bring startup time closer to the <500ms target for perceived instant launch.
