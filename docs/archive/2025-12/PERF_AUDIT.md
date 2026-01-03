# Script Kit GPUI Performance Audit

**Date**: December 2024  
**Status**: Comprehensive analysis complete  
**Total Audit Files**: 9

---

## Executive Summary

This document synthesizes findings from 9 detailed performance audits of the Script Kit GPUI application. The audits cover rendering, memory, list virtualization, async threading, caching, script search, startup, logging, and protocol parsing.

### Overall Assessment

| Area | Status | Key Finding |
|------|--------|-------------|
| **Rendering** | Needs Attention | 78 cx.notify() calls, hover handlers in hot loops |
| **Memory** | Needs Attention | Clones in render paths, unbounded image cache |
| **List Scroll** | Needs Attention | Event coalescing not implemented despite documentation |
| **Async/Threading** | Good | 21 justified threads, event-driven architecture |
| **Caching** | Mixed | Good filter cache, but unbounded image/frecency caches |
| **Script Search** | Needs Attention | No result caching, full re-search per keystroke |
| **Startup** | Critical | Duplicate config load, 500-1500ms cold start |
| **Logging** | Good | Non-blocking I/O, but hot path logging overhead |
| **Protocol** | Needs Attention | Large enum variants, base64 allocations for screenshots |

### Impact Summary

| Priority | Count | Estimated Impact |
|----------|-------|------------------|
| P0 (Critical) | 8 | 40-60% startup reduction, 50%+ render improvement |
| P1 (High) | 12 | 20-30% additional improvements |
| P2 (Medium) | 10 | 10-15% polish optimizations |
| P3 (Low) | 8 | Minor improvements, code quality |

---

## Audit Reports Index

| # | Report | Location | Lines | Focus Area |
|---|--------|----------|-------|------------|
| 1 | [Rendering Audit](docs/perf/RENDERING.md) | `docs/perf/RENDERING.md` | 475 | Frame performance, cx.notify(), virtualization |
| 2 | [Memory Audit](docs/perf/MEMORY.md) | `docs/perf/MEMORY.md` | 440 | Allocations, clones, Arc/Mutex patterns |
| 3 | [List Scroll Audit](docs/perf/LIST_SCROLL.md) | `docs/perf/LIST_SCROLL.md` | 455 | Virtualization, scroll handles, coalescing |
| 4 | [Async/Threading Audit](docs/perf/ASYNC_THREADING.md) | `docs/perf/ASYNC_THREADING.md` | 457 | Threads, channels, Timer overhead |
| 5 | [Caching Audit](docs/perf/CACHING.md) | `docs/perf/CACHING.md` | 421 | Filter cache, image cache, frecency |
| 6 | [Script Search Audit](docs/perf/SCRIPT_SEARCH.md) | `docs/perf/SCRIPT_SEARCH.md` | 467 | Fuzzy search, frecency, file watchers |
| 7 | [Startup Audit](docs/perf/STARTUP.md) | `docs/perf/STARTUP.md` | 288 | Init sequence, cold/warm start |
| 8 | [Logging Audit](docs/perf/LOGGING.md) | `docs/perf/LOGGING.md` | 385 | Dual output, format!() overhead |
| 9 | [Protocol Audit](docs/perf/PROTOCOL.md) | `docs/perf/PROTOCOL.md` | 541 | JSON parsing, enum size, base64 |

---

## Priority Matrix

### P0 - Critical (Fix First)

| # | Issue | Location | Impact | Source |
|---|-------|----------|--------|--------|
| 1 | **Duplicate config::load_config() call** | `main.rs` | -100-300ms startup | Startup |
| 2 | **Hover handler cx.notify() in hot loop** | `main.rs:4507-4521` | 50% render reduction | Rendering |
| 3 | **Event coalescing NOT implemented** | AGENTS.md claims 20ms window | Frame drops during rapid scroll | List Scroll |
| 4 | **Clone of current_view in render** | `main.rs:3538` | ~500+ bytes per frame | Memory |
| 5 | **Clipboard image cache unbounded** | `clipboard_history.rs` | Memory leak (~200MB+) | Caching |
| 6 | **No search result caching** | `scripts.rs` | 2-5ms per keystroke | Script Search |
| 7 | **Base64 screenshot allocation** | `main.rs:2303` | ~4MB per screenshot | Protocol |
| 8 | **Large Message enum variants** | `protocol.rs` | ~400 bytes per message | Protocol |

### P1 - High Priority

| # | Issue | Location | Impact | Source |
|---|-------|----------|--------|--------|
| 1 | Cache design tokens in state | `main.rs:4364-4368` | Avoid per-render computation | Rendering |
| 2 | Use Arc for grouped_items | `main.rs:4439-4440` | Eliminate clone per render | Rendering |
| 3 | Use indices instead of clones in lists | `main.rs:3463, 5846, 6409` | O(n) allocation reduction | Memory |
| 4 | Extract scroll stabilization helper | All list components | 14/15 call sites lack stabilization | List Scroll |
| 5 | Convert poll loops to event-driven | `main.rs:8387-8455` | 0-200ms latency elimination | Async |
| 6 | Add capacity bounds to unbounded channels | `main.rs:540, 1761` | Prevent memory growth | Async |
| 7 | Add LRU eviction to app icons cache | `~/.sk/kit/cache/app-icons/` | Disk space leak | Caching |
| 8 | Implement incremental script loading | `scripts.rs` | 90%+ reload time reduction | Script Search |
| 9 | Parallelize Phase 3 blocking operations | `main.rs` | -200-400ms startup | Startup |
| 10 | Reduce hot path logging | `main.rs, executor.rs` | ~15 log calls per hotkey | Logging |
| 11 | Single-parse graceful handling | `protocol.rs:2182` | Eliminate double parse | Protocol |
| 12 | Reuse line buffer in JsonlReader | `protocol.rs:2280` | Reduce GC pressure | Protocol |

### P2 - Medium Priority

| # | Issue | Location | Impact | Source |
|---|-------|----------|--------|--------|
| 1 | Use const for font family strings | Multiple files (91 occurrences) | Minor memory reduction | Rendering |
| 2 | Cache scrollbar state | `main.rs:4443-4470` | Avoid unnecessary computation | Rendering |
| 3 | Cache relative time strings | `main.rs:5888-5896` | Avoid per-frame allocation | Memory |
| 4 | Connect performance instrumentation | `perf.rs` | Fill monitoring blind spots | List Scroll |
| 5 | Consider RwLock for terminal state | `alacritty.rs:352` | Reduce contention | Async |
| 6 | Cache script metadata with mtime | `scripts.rs` | -50-200ms warm start | Caching |
| 7 | LRU cache for preview (5-10 recent) | `main.rs:781-783` | Faster list navigation | Caching |
| 8 | Move clipboard DB init to background | `clipboard_history.rs` | -20-50ms startup | Startup |
| 9 | Use tracing structured fields | `main.rs, executor.rs` | Avoid heap allocation | Logging |
| 10 | Accept impl Into<String> in constructors | `protocol.rs` | Ergonomics improvement | Protocol |

### P3 - Low Priority (Nice to Have)

| # | Issue | Location | Impact | Source |
|---|-------|----------|--------|--------|
| 1 | Move toast tick to timer | `main.rs:3621` | Consistent with term_prompt | Rendering |
| 2 | Box large AppView variants | `main.rs` | Reduce stack usage | Memory |
| 3 | Fix outdated 52px comment | `list_item.rs:617` | Developer clarity | List Scroll |
| 4 | Document thread shutdown | All spawn locations | Clean app shutdown | Async |
| 5 | Prune frecency entries on load | `frecency.rs` | Cleaner data | Caching |
| 6 | Async script loading | `scripts.rs` | Better UI responsiveness | Script Search |
| 7 | Add log rotation | `logging.rs` | Prevent log growth | Logging |
| 8 | Sampling for high-frequency events | `logging.rs` | Reduce log volume | Logging |

---

## Impact Estimates Table

### Startup Time

| Optimization | Current | Target | Reduction |
|--------------|---------|--------|-----------|
| Remove duplicate config load | ~200-400ms | 0ms | -200-400ms |
| Parallelize Phase 3 | Sequential | Parallel | -200-400ms |
| Move clipboard DB to background | ~20-50ms | 0ms | -20-50ms |
| Script metadata caching | ~100-300ms | ~10ms | -90-290ms |
| **Total Cold Start** | 800-1500ms | 200-400ms | **50-70%** |

### Render Performance

| Optimization | Current | Target | Improvement |
|--------------|---------|--------|-------------|
| Debounce hover state | Many re-renders | 1 per 16ms | 50% reduction |
| Remove view clone | ~500B/frame | 0B/frame | Memory savings |
| Cache design tokens | Per-render compute | Once on change | CPU savings |
| Use indices in lists | O(n) clones | O(1) indices | 80%+ reduction |

### Memory Usage

| Cache | Current Limit | Recommended | Savings |
|-------|---------------|-------------|---------|
| Clipboard Images | Unbounded (~200MB+) | 50-100 images LRU | ~150MB cap |
| App Icons (disk) | Unbounded | 100MB with LRU | Disk cleanup |
| Frecency Store | Unbounded | 500 entries | Minor |
| Preview Cache | 1 item | 5-10 items LRU | Faster navigation |

### Search Performance

| Optimization | Current | Target | Improvement |
|--------------|---------|--------|-------------|
| Result memoization | 2-5ms/keystroke | <1ms repeat | 80%+ for repeats |
| Prefix-based filtering | Full re-search | Filter previous | 50-80% reduction |
| Incremental script load | Full reload | Delta only | 90%+ reduction |

---

## Implementation Roadmap

### Phase 1: Quick Wins (1-2 days)

**Goal**: 40-50% startup improvement, basic render fixes

1. Remove duplicate `config::load_config()` call (1 line change)
2. Add debounce to hover handler (~20 lines)
3. Remove `current_view.clone()` in render (~5 lines)
4. Use indices instead of clones in uniform_list closures (~30 lines)
5. Add capacity bounds to unbounded channels (~2 lines each)

**Verification**: Time app startup, measure render frame times

### Phase 2: Core Optimizations (3-5 days)

**Goal**: Implement event coalescing, fix caching gaps

1. Implement 20ms event coalescing for scroll (~50 lines)
2. Add LRU eviction to clipboard image cache (~100 lines)
3. Implement search result memoization (~80 lines)
4. Cache script metadata with mtime validation (~150 lines)
5. Convert poll loops to event-driven (~50 lines)
6. Extract scroll stabilization helper, apply to all lists (~100 lines)

**Verification**: Run scroll benchmarks, measure cache hit rates

### Phase 3: Advanced Optimizations (1 week)

**Goal**: Protocol efficiency, startup parallelization

1. Box large Message enum variants (~30 lines)
2. Implement streaming base64 for screenshots (~100 lines)
3. Single-parse graceful message handling (~30 lines)
4. Parallelize Phase 3 startup operations (~80 lines)
5. Add script metadata cache with file watcher integration (~200 lines)
6. Reduce hot path logging, use structured fields (~100 lines)

**Verification**: Profile full user flows, measure memory usage

### Phase 4: Polish (Ongoing)

**Goal**: Code quality, monitoring, edge cases

1. Add log rotation
2. Prune stale frecency/cache entries
3. Add periodic cache stats logging
4. Document thread lifecycle and shutdown
5. Fix outdated comments
6. Add performance benchmarks to CI

---

## Metrics to Track

### Startup Metrics

| Metric | Current | Target | How to Measure |
|--------|---------|--------|----------------|
| Time to window visible | ~400-600ms | <200ms | Instrument init phases |
| Time to interactive | ~700-1200ms | <500ms | Log first input response |
| Time to full script list | ~800-1500ms | <800ms | Log render complete |
| Config load time | ~200-400ms | <50ms | Profile bun subprocess |

### Runtime Metrics

| Metric | Current | Target | How to Measure |
|--------|---------|--------|----------------|
| P95 Key Latency | Unknown | <50ms | `perf::KeyEventTracker` |
| Single Key Event | Unknown | <16.67ms | `perf::TimingGuard` |
| Scroll Operation | Unknown | <8ms | `perf::ScrollTimer` |
| cx.notify() per second | ~20-50 | <10 | Add counter |
| Frame drop rate | Unknown | <1% | `perf::FrameTimer` |

### Memory Metrics

| Metric | Target | How to Measure |
|--------|--------|----------------|
| Peak RSS with 1000 scripts | <200MB | `/usr/bin/time -l` |
| Allocations per render frame | 0 | Heap profiler |
| Clipboard image cache size | <100MB | Log cache.len() |

---

## Cross-Cutting Concerns

### Patterns to Adopt Codebase-Wide

1. **Timer-based refresh** (like `term_prompt.rs`) instead of cx.notify() spam
2. **Debounced/coalesced events** for rapid user input
3. **Arc/indices instead of clones** in uniform_list closures
4. **OnceLock caching** for static data (syntax sets, font families)
5. **Dirty flags** for lazy persistence (like frecency)
6. **LRU eviction** for all unbounded caches

### Anti-Patterns to Avoid

1. `cx.notify()` in mouse event handlers without debouncing
2. `format!()` in hot paths - use tracing structured fields
3. Cloning large structs in render methods
4. Unbounded caches without eviction
5. Blocking I/O in the main thread
6. Duplicate initialization calls

---

## Conclusion

The Script Kit GPUI application has a solid architectural foundation with proper virtualization, event-driven IPC, and non-blocking file I/O. However, several performance regressions have accumulated:

1. **Startup is 2-3x slower than necessary** due to duplicate initialization and sequential blocking
2. **Render performance is degraded** by excessive cx.notify() calls and cloning in hot paths
3. **Memory can grow unbounded** due to missing cache eviction policies
4. **Search is inefficient** with no result caching or incremental updates

Implementing the P0 optimizations alone would provide a 40-60% improvement in startup time and significantly smoother UI. The full roadmap would bring the application to near-optimal performance.

---

## Appendix: File Counts

| File | Total Lines | Audit Coverage |
|------|-------------|----------------|
| `src/main.rs` | 8500+ | All audits |
| `src/scripts.rs` | 4000+ | Search, Caching |
| `src/protocol.rs` | 4965 | Protocol |
| `src/logging.rs` | 1070+ | Logging |
| `src/executor.rs` | 800+ | Startup, Logging |
| `src/clipboard_history.rs` | 400+ | Memory, Caching |
| `src/terminal/alacritty.rs` | 700+ | Async, Memory |
| `src/perf.rs` | 548 | Rendering, List Scroll |
| `src/frecency.rs` | 280 | Search, Caching |

---

*Generated by SynthesisAgent from 9 audit reports totaling 3,929 lines of analysis*
