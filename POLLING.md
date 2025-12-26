# Polling Patterns Analysis & async_channel Migration Guide

## Executive Summary

This document catalogs all polling patterns found in the Script Kit GPUI codebase and provides recommendations for converting them to event-driven architectures using `async_channel`. The analysis identified **6 actionable polling patterns** that can be converted to improve CPU efficiency and responsiveness.

**Key Finding:** The codebase already contains an excellent model implementation in the stdin listener that uses `async_channel` correctly. This pattern should be replicated for other polling loops.

### Impact Summary

| Priority | Pattern | Current Overhead | Expected Improvement |
|----------|---------|-----------------|---------------------|
| HIGHEST | Prompt Message Poller | 50ms polling (20 wakeups/sec) | Zero polling |
| HIGH | Terminal Refresh | 8ms polling (~120fps) | True event-driven |
| MEDIUM | Hotkey Trigger | 100ms polling (10 wakeups/sec) | Zero polling |
| MEDIUM | Appearance Watcher | 200ms polling (5 wakeups/sec) | Zero polling |
| MEDIUM | Config File Watcher | 200ms polling (5 wakeups/sec) | Zero polling |
| LOW | Test Command Watcher | 500ms polling (2 wakeups/sec) | Zero polling |

---

## Table of All Polling Patterns

### Patterns to Convert (Priority Order)

| # | Pattern | Location | Interval | Channel Type | Recommendation |
|---|---------|----------|----------|--------------|----------------|
| 1 | Prompt Message Poller | `main.rs:652-668, 3057-3068` | 50ms | `mpsc::channel` | Convert to `async_channel` |
| 2 | Terminal Refresh | `term_prompt.rs:140-173` | 8ms | Timer-based | Use epoll/kqueue or `async_channel` |
| 3 | Hotkey Trigger Poller | `main.rs:401-509` | 100ms | `AtomicBool` | Convert to `async_channel` |
| 4 | Appearance Change Watcher | `main.rs:3023-3036` | 200ms | `crossbeam_channel` | Convert to `async_channel` |
| 5 | Config File Watcher | `main.rs:3039-3053` | 200ms | `crossbeam_channel` | Convert to `async_channel` |
| 6 | Test Command Watcher | `main.rs:3071-3103` | 500ms | File polling | Convert to `async_channel` |

### Patterns to Keep As-Is

| Pattern | Location | Interval | Reason |
|---------|----------|----------|--------|
| Cursor Blink Timer | `main.rs:602-612` | 530ms | Intentional animation timing |
| Debounce Sleeps | `watcher.rs` (various) | 500ms | Intentional event coalescing |
| ConfigWatcher | `watcher.rs` | Event-driven | Already uses `notify` crate correctly |
| ThemeWatcher | `watcher.rs` | Event-driven | Already uses `notify` crate correctly |
| ScriptWatcher | `watcher.rs` | Event-driven | Already uses `notify` crate correctly |

### Model Implementation (Reference)

| Pattern | Location | Implementation |
|---------|----------|----------------|
| stdin Listener | `main.rs:350-389, 3108-3150` | **Correct** `async_channel::recv().await` |

---

## Detailed Analysis

### 1. Prompt Message Poller (HIGHEST PRIORITY)

**Location:** `main.rs:652-668` and `main.rs:3057-3068`

**Current Implementation:**
```rust
// Creates a 50ms polling loop
cx.spawn(|this, mut cx| async move {
    loop {
        Timer::after(Duration::from_millis(50)).await;
        // Try to receive from mpsc channel
        if let Ok(msg) = receiver.try_recv() {
            // Process message
        }
    }
})
```

**Problem:** 
- Wakes up 20 times per second regardless of activity
- Adds 50ms latency to message processing
- Burns CPU cycles checking empty channel

**Recommended Fix:**
```rust
use async_channel::{bounded, Receiver, Sender};

// In setup:
let (tx, rx): (Sender<PromptMessage>, Receiver<PromptMessage>) = bounded(100);

// Background thread sends:
tx.send_blocking(msg).ok();

// GPUI async task - truly event-driven:
cx.spawn(|this, mut cx| async move {
    while let Ok(msg) = rx.recv().await {
        // Wakes ONLY when message arrives - zero polling!
        this.update(&mut cx, |this, cx| {
            this.handle_prompt_message(msg, cx);
        }).ok();
    }
})
```

---

### 2. Terminal Refresh (HIGH PRIORITY)

**Location:** `term_prompt.rs:140-173`

**Current Implementation:**
```rust
// 8ms timer = ~120fps refresh rate
cx.spawn(|this, mut cx| async move {
    loop {
        Timer::after(Duration::from_millis(8)).await;
        this.update(&mut cx, |this, cx| {
            this.refresh_terminal(cx);
        }).ok();
    }
})
```

**Problem:**
- Refreshes 120 times per second even when terminal is idle
- Significant CPU overhead for inactive terminals
- Not synchronized with actual PTY output

**Recommended Fix (Option A - async_channel):**
```rust
// PTY reader thread signals when data available
let (tx, rx) = async_channel::bounded::<()>(1);

std::thread::spawn(move || {
    loop {
        // Block until PTY has data (use epoll/kqueue internally)
        if pty.wait_for_data().is_ok() {
            tx.send_blocking(()).ok();
        }
    }
});

// GPUI task - refresh only when PTY has new data
cx.spawn(|this, mut cx| async move {
    while rx.recv().await.is_ok() {
        this.update(&mut cx, |this, cx| {
            this.refresh_terminal(cx);
        }).ok();
    }
})
```

**Recommended Fix (Option B - Debounced refresh):**
```rust
// Coalesce rapid updates with debouncing
use std::time::Instant;

const MIN_REFRESH_INTERVAL: Duration = Duration::from_millis(16); // 60fps max

cx.spawn(|this, mut cx| async move {
    let mut last_refresh = Instant::now();
    
    while let Ok(()) = rx.recv().await {
        // Coalesce rapid updates
        if last_refresh.elapsed() >= MIN_REFRESH_INTERVAL {
            this.update(&mut cx, |this, cx| {
                this.refresh_terminal(cx);
            }).ok();
            last_refresh = Instant::now();
        }
    }
})
```

---

### 3. Hotkey Trigger Poller (MEDIUM PRIORITY)

**Location:** `main.rs:401-509`

**Current Implementation:**
```rust
// Uses AtomicBool with 100ms polling
static HOTKEY_TRIGGERED: AtomicBool = AtomicBool::new(false);

cx.spawn(|this, mut cx| async move {
    loop {
        Timer::after(Duration::from_millis(100)).await;
        if HOTKEY_TRIGGERED.swap(false, Ordering::SeqCst) {
            // Handle hotkey
        }
    }
})
```

**Problem:**
- 100ms latency on hotkey response
- 10 wakeups per second when idle
- Uses atomic flag instead of proper signaling

**Recommended Fix:**
```rust
use async_channel::{bounded, Sender, Receiver};

// Global channel for hotkey events
lazy_static! {
    static ref HOTKEY_CHANNEL: (Sender<HotkeyEvent>, Receiver<HotkeyEvent>) = bounded(10);
}

// In hotkey callback:
fn on_hotkey_triggered(event: HotkeyEvent) {
    HOTKEY_CHANNEL.0.send_blocking(event).ok();
}

// GPUI async task - instant response:
cx.spawn(|this, mut cx| async move {
    while let Ok(event) = HOTKEY_CHANNEL.1.recv().await {
        this.update(&mut cx, |this, cx| {
            this.handle_hotkey(event, cx);
        }).ok();
    }
})
```

---

### 4. Appearance Change Watcher (MEDIUM PRIORITY)

**Location:** `main.rs:3023-3036`

**Current Implementation:**
```rust
// crossbeam_channel with 200ms try_recv polling
cx.spawn(|this, mut cx| async move {
    loop {
        Timer::after(Duration::from_millis(200)).await;
        if let Ok(appearance) = appearance_rx.try_recv() {
            // Handle appearance change
        }
    }
})
```

**Problem:**
- 200ms latency on theme changes
- 5 unnecessary wakeups per second

**Recommended Fix:**
```rust
use async_channel::{bounded, Receiver};

// Replace crossbeam with async_channel
let (tx, rx): (_, Receiver<Appearance>) = bounded(1);

// Watcher thread:
std::thread::spawn(move || {
    // On macOS, use NSDistributedNotificationCenter instead of polling
    // For now, keep notify-based watching but use async_channel
    for event in watcher.recv_blocking() {
        tx.send_blocking(event).ok();
    }
});

// GPUI task:
cx.spawn(|this, mut cx| async move {
    while let Ok(appearance) = rx.recv().await {
        this.update(&mut cx, |this, cx| {
            this.handle_appearance_change(appearance, cx);
        }).ok();
    }
})
```

**macOS-Specific Optimization:**
```rust
#[cfg(target_os = "macos")]
fn watch_appearance_native(tx: Sender<Appearance>) {
    // Use NSDistributedNotificationCenter for instant appearance change detection
    // "AppleInterfaceThemeChangedNotification"
}
```

---

### 5. Config File Watcher (MEDIUM PRIORITY)

**Location:** `main.rs:3039-3053`

**Current Implementation:**
```rust
// crossbeam_channel with 200ms try_recv polling
cx.spawn(|this, mut cx| async move {
    loop {
        Timer::after(Duration::from_millis(200)).await;
        if let Ok(config) = config_rx.try_recv() {
            // Handle config change
        }
    }
})
```

**Recommended Fix:**
Same pattern as Appearance Watcher - replace `crossbeam_channel` with `async_channel`.

---

### 6. Test Command File Watcher (LOW PRIORITY)

**Location:** `main.rs:3071-3103`

**Current Implementation:**
```rust
// Polls file existence every 500ms
cx.spawn(|this, mut cx| async move {
    loop {
        Timer::after(Duration::from_millis(500)).await;
        if Path::new("/tmp/test-command").exists() {
            // Process test command
        }
    }
})
```

**Problem:**
- Filesystem polling is inefficient
- 500ms latency for test commands

**Recommended Fix:**
```rust
use notify::{Watcher, RecursiveMode};
use async_channel::bounded;

let (tx, rx) = bounded(10);

// Use notify crate for filesystem events
let mut watcher = notify::recommended_watcher(move |event| {
    if let Ok(event) = event {
        tx.send_blocking(event).ok();
    }
})?;

watcher.watch(Path::new("/tmp"), RecursiveMode::NonRecursive)?;

// GPUI task:
cx.spawn(|this, mut cx| async move {
    while let Ok(event) = rx.recv().await {
        if event.paths.iter().any(|p| p.ends_with("test-command")) {
            this.update(&mut cx, |this, cx| {
                this.handle_test_command(cx);
            }).ok();
        }
    }
})
```

---

## Patterns to Keep As-Is

### Cursor Blink Timer

**Location:** `main.rs:602-612`

```rust
// 530ms cursor blink - this is intentional animation timing
cx.spawn(|this, mut cx| async move {
    loop {
        Timer::after(Duration::from_millis(530)).await;
        this.update(&mut cx, |this, cx| {
            this.cursor_visible = !this.cursor_visible;
            cx.notify();
        }).ok();
    }
})
```

**Reason to keep:** This is intentional animation timing, not polling for external events. The 530ms interval creates the standard cursor blink effect.

### Debounce Sleeps in Watcher

**Location:** `watcher.rs` (various)

```rust
// 500ms debounce after file change detection
std::thread::sleep(Duration::from_millis(500));
```

**Reason to keep:** Intentional event coalescing to prevent rapid re-processing during file save operations (editors often write multiple times).

### File Watchers Using notify Crate

**Location:** `watcher.rs`

The `ConfigWatcher`, `ThemeWatcher`, and `ScriptWatcher` implementations already use the `notify` crate with blocking `recv()` calls. These are event-driven and efficient.

---

## Cargo.toml Addition

Add the `async-channel` dependency:

```toml
[dependencies]
async-channel = "2.0"
```

---

## Recommended Implementation Order

### Phase 1: High-Impact Quick Wins
1. **Prompt Message Poller** - Highest frequency (50ms), straightforward conversion
2. **Hotkey Trigger Poller** - Improves UX responsiveness

### Phase 2: Medium-Impact Conversions
3. **Appearance Change Watcher** - Simple channel swap
4. **Config File Watcher** - Same pattern as appearance watcher

### Phase 3: Complex Optimizations
5. **Terminal Refresh** - Requires PTY integration work
6. **Test Command Watcher** - Low priority, test-only code

---

## Migration Checklist

For each polling pattern conversion:

- [ ] Add `async-channel = "2.0"` to Cargo.toml (once)
- [ ] Replace channel type (`mpsc`/`crossbeam` -> `async_channel`)
- [ ] Update sender: `send_blocking()` for sync contexts
- [ ] Update receiver: `recv().await` in GPUI spawn
- [ ] Remove `Timer::after()` polling loop
- [ ] Test that events still trigger correctly
- [ ] Verify CPU usage reduction with profiler

---

## Model Implementation Reference

The stdin listener in `main.rs:350-389, 3108-3150` demonstrates the correct pattern:

```rust
use async_channel::{bounded, Receiver, Sender};

// Setup
let (tx, rx): (Sender<Command>, Receiver<Command>) = bounded(100);

// Background thread - blocks on I/O, sends on channel
std::thread::spawn(move || {
    let stdin = std::io::stdin();
    for line in stdin.lock().lines().flatten() {
        if let Ok(cmd) = serde_json::from_str::<Command>(&line) {
            // send_blocking() for sync context
            let _ = tx.send_blocking(cmd);
        }
    }
});

// GPUI async task - truly event-driven
cx.spawn(async move |cx: &mut AsyncApp| {
    // recv().await suspends task until data arrives
    // NO POLLING - wakes only on channel send
    while let Ok(cmd) = rx.recv().await {
        cx.update(|app, cx| {
            handle_command(app, cmd, cx);
        }).ok();
    }
}).detach();
```

**Key Points:**
1. `bounded(100)` - Creates backpressure if consumer is slow
2. `send_blocking()` - Use in sync thread contexts
3. `recv().await` - Suspends without polling
4. No `Timer::after()` - True event-driven behavior

---

## Performance Expectations

After full migration:

| Metric | Before | After |
|--------|--------|-------|
| Idle CPU wakeups | ~160/sec | ~2/sec (cursor blink only) |
| Message latency | 50ms avg | <1ms |
| Hotkey latency | 100ms avg | <5ms |
| Theme change latency | 200ms avg | <10ms |

---

## References

- [async-channel crate](https://docs.rs/async-channel/latest/async_channel/)
- [GPUI async patterns](https://docs.rs/gpui/latest/gpui/)
- [notify crate for file watching](https://docs.rs/notify/latest/notify/)
