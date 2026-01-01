# Notes Window Coupling Bug Expert Bundle

## Original Goal

> The notes window still only opens when the main window opens. When I press the notes window shortcut, it seems like nothing happens. But it will appear when the main window opens. Something is still binding them together. Let's turn this issue over to an expert.

## Executive Summary

The Notes window is supposed to open independently via Cmd+Shift+N, but it only appears after the main window hotkey is pressed first. The hotkey listener code was recently refactored to spawn before the main window, but the async channel receiver is spawned **inside** `Application::new().run()` after the main window is created at line 1073, which may delay when the listener actually starts processing events.

### Key Problems:

1. **Hotkey listener spawn order**: The Notes hotkey listener at line 1182-1193 is spawned inside `Application::new().run()` but AFTER the main window creation at lines 1073-1089. The `cx.spawn()` call queues an async task that may not run until the main GPUI event loop processes it.

2. **Global hotkey thread vs GPUI async mismatch**: `hotkeys::start_hotkey_listener()` runs at line 1001 (before `Application::new().run()`), registering hotkeys and sending to channels. But the channel receivers (`cx.spawn()`) are inside the GPUI run loop - they may not process events until the main window is shown/activated.

3. **GPUI event loop initialization**: The `cx.spawn()` async tasks may not start processing until the GPUI event loop is fully running, which might require the main window to be activated first.

### Required Fixes:

1. Investigate whether `cx.spawn()` tasks start immediately or only after window activation
2. Consider using a thread-based listener instead of `cx.spawn()` for the Notes hotkey
3. Verify the hotkey channel is being sent to when pressed (add logging in `hotkeys.rs` line 399)
4. Check if `cx.update()` inside the spawned task works before any window exists

### Files Included:
- `src/main.rs`: Main app entry, window creation, hotkey listeners (lines 1001-1193)
- `src/hotkeys.rs`: Global hotkey registration and channel management
- `src/hotkey_pollers.rs`: Alternative hotkey polling implementations (currently unused)
- `src/notes/window.rs`: Notes window creation and management
- `src/notes/mod.rs`: Notes module exports

---

## Code Bundle

This section contains the contents of the repository's files.

### src/main.rs (Key Sections)

**Hotkey listener started at line 1001 (BEFORE GPUI run loop):**
```rust
hotkeys::start_hotkey_listener(loaded_config);
```

**GPUI Application run loop starts at line 1041:**
```rust
Application::new().run(move |cx: &mut App| {
    logging::log("APP", "GPUI Application starting");

    // Initialize gpui-component (theme, context providers)
    gpui_component::init(cx);
    theme::sync_gpui_component_theme(cx);

    // Tray manager initialization...
    let tray_manager = match TrayManager::new() { ... };

    // Window bounds calculation...
    let window_size = size(px(750.), initial_window_height());
    let bounds = calculate_eye_line_bounds_on_mouse_display(window_size);

    // MAIN WINDOW CREATED HERE (line 1073-1089)
    let window: WindowHandle<Root> = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            is_movable: true,
            window_background: WindowBackgroundAppearance::Blurred,
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| ScriptListApp::new(config_for_app, cx));
            *app_entity_for_closure.lock().unwrap() = Some(view.clone());
            cx.new(|cx| Root::new(view, window, cx))
        },
    ).unwrap();
```

**Main window hotkey listener (lines 1117-1179):**
```rust
// Main window hotkey listener - uses Entity<ScriptListApp> instead of WindowHandle
let app_entity_for_hotkey = app_entity.clone();
let window_for_hotkey = window;
cx.spawn(async move |cx: &mut gpui::AsyncApp| {
    logging::log("HOTKEY", "Main hotkey listener started");
    while let Ok(()) = hotkeys::hotkey_channel().1.recv().await {
        // ... toggle main window visibility
    }
}).detach();
```

**Notes hotkey listener (lines 1181-1193) - THIS IS THE PROBLEM AREA:**
```rust
// Notes hotkey listener - spawns independently, doesn't need window handle
cx.spawn(async move |cx: &mut gpui::AsyncApp| {
    logging::log("HOTKEY", "Notes hotkey listener started");
    while let Ok(()) = hotkeys::notes_hotkey_channel().1.recv().await {
        logging::log("HOTKEY", "Notes hotkey triggered - opening notes window");
        let _ = cx.update(|cx: &mut gpui::App| {
            if let Err(e) = notes::open_notes_window(cx) {
                logging::log("HOTKEY", &format!("Failed to open notes window: {}", e));
            }
        });
    }
    logging::log("HOTKEY", "Notes hotkey listener exiting");
}).detach();
```

### src/hotkeys.rs (Key Sections)

**Channel definitions:**
```rust
// NOTES_HOTKEY_CHANNEL: Channel for notes hotkey events
static NOTES_HOTKEY_CHANNEL: OnceLock<(async_channel::Sender<()>, async_channel::Receiver<()>)> =
    OnceLock::new();

pub(crate) fn notes_hotkey_channel(
) -> &'static (async_channel::Sender<()>, async_channel::Receiver<()>) {
    NOTES_HOTKEY_CHANNEL.get_or_init(|| async_channel::bounded(10))
}
```

**Hotkey registration (line 455-500):**
```rust
// Register notes hotkey (Cmd+Shift+N by default)
let notes_config = config.get_notes_hotkey();
let notes_code = match notes_config.key.as_str() {
    "KeyN" => Code::KeyN,
    // ...
    _ => Code::KeyN,
};

let mut notes_modifiers = Modifiers::empty();
for modifier in &notes_config.modifiers {
    match modifier.as_str() {
        "meta" => notes_modifiers |= Modifiers::META,
        "ctrl" => notes_modifiers |= Modifiers::CONTROL,
        "alt" => notes_modifiers |= Modifiers::ALT,
        "shift" => notes_modifiers |= Modifiers::SHIFT,
        _ => {}
    }
}

let notes_hotkey = HotKey::new(Some(notes_modifiers), notes_code);
let notes_hotkey_id = notes_hotkey.id();

if let Err(e) = manager.register(notes_hotkey) {
    logging::log("HOTKEY", &format!("Failed to register notes hotkey {}: {}", notes_display, e));
} else {
    logging::log("HOTKEY", &format!("Registered notes hotkey {} (id: {})", notes_display, notes_hotkey_id));
}
```

**Event dispatch (around line 396-400):**
```rust
// Check if it's the notes hotkey
else if event.id == notes_hotkey_id {
    if notes_hotkey_channel().0.send_blocking(()).is_err() {
        // Channel closed
    }
}
```

### src/notes/window.rs (Key Sections)

**open_notes_window function:**
```rust
pub fn open_notes_window(cx: &mut App) -> Result<()> {
    // Ensure gpui-component theme is initialized before opening window
    ensure_theme_initialized(cx);

    let window_handle = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = window_handle.lock().unwrap();

    // Check if window already exists and is valid
    if let Some(ref handle) = *guard {
        if handle.update(cx, |_, _, cx| cx.notify()).is_ok() {
            info!("Focusing existing notes window");
            return Ok(());
        }
    }

    // Create new window
    info!("Opening new notes window");

    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(gpui::Bounds::centered(
            None,
            size(px(900.), px(700.)),
            cx,
        ))),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some("Script Kit Notes".into()),
            appears_transparent: true,
            ..Default::default()
        }),
        focus: true,
        show: true,
        kind: gpui::WindowKind::Normal,
        ..Default::default()
    };

    let handle = cx.open_window(window_options, |window, cx| {
        let view = cx.new(|cx| NotesApp::new(window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    })?;

    *guard = Some(handle);

    // Configure as floating panel
    configure_notes_as_floating_panel();

    Ok(())
}
```

---

## Implementation Guide

### Hypothesis: GPUI async tasks don't run until event loop is processing

The `cx.spawn()` call inside `Application::new().run()` creates an async task, but GPUI's event loop may not process these tasks until it's actively running. The main window being shown/activated might be what triggers the event loop to start processing.

### Step 1: Add Diagnostic Logging

Add logging to verify when the hotkey channel receives events and when the spawned task starts:

```rust
// File: src/hotkeys.rs
// Location: Around line 396-400 (in the event dispatch section)

// Check if it's the notes hotkey
else if event.id == notes_hotkey_id {
    logging::log("HOTKEY", "Notes hotkey pressed - sending to channel");
    match notes_hotkey_channel().0.try_send(()) {
        Ok(()) => logging::log("HOTKEY", "Notes hotkey event sent to channel successfully"),
        Err(e) => logging::log("HOTKEY", &format!("Notes hotkey channel send failed: {:?}", e)),
    }
}
```

### Step 2: Move Notes Listener to a Dedicated Thread

Instead of using `cx.spawn()` which depends on GPUI's async executor, use a dedicated thread that calls `cx.update()`:

```rust
// File: src/main.rs
// Location: After line 1001 (after hotkeys::start_hotkey_listener), BEFORE Application::new().run()

// Start notes hotkey listener in a dedicated thread (independent of GPUI event loop)
std::thread::spawn(move || {
    logging::log("HOTKEY", "Notes hotkey thread started (dedicated thread)");
    
    // Block on the async channel receiver using std::sync::mpsc pattern
    loop {
        // Use blocking recv since we're in a dedicated thread
        if notes_hotkey_channel().1.recv_blocking().is_ok() {
            logging::log("HOTKEY", "Notes hotkey received in dedicated thread");
            // We need to update GPUI from outside - this requires a different approach
            // Consider using a crossbeam channel to communicate with the GPUI run loop
        }
    }
});
```

**Problem**: We can't call `cx.update()` from outside the GPUI run loop. We need a different approach.

### Step 3: Use a Polling Approach (Recommended Fix)

Since GPUI async tasks may not run until the event loop is active, use a polling timer that starts immediately:

```rust
// File: src/main.rs
// Location: Replace lines 1181-1193 with this:

// Notes hotkey listener - use a timer-based approach to ensure immediate responsiveness
cx.spawn(async move |cx: &mut gpui::AsyncApp| {
    logging::log("HOTKEY", "Notes hotkey listener started (poll-based)");
    
    loop {
        // Poll every 50ms for notes hotkey events
        gpui::Timer::after(std::time::Duration::from_millis(50)).await;
        
        // Non-blocking check for notes hotkey
        while let Ok(()) = hotkeys::notes_hotkey_channel().1.try_recv() {
            logging::log("HOTKEY", "Notes hotkey triggered - opening notes window");
            let _ = cx.update(|cx: &mut gpui::App| {
                if let Err(e) = notes::open_notes_window(cx) {
                    logging::log("HOTKEY", &format!("Failed to open notes window: {}", e));
                }
            });
        }
    }
}).detach();
```

### Step 4: Move Listener BEFORE Window Creation

Move the Notes hotkey listener spawn to immediately after `gpui_component::init(cx)` but BEFORE `cx.open_window()`:

```rust
// File: src/main.rs
// Location: After line 1050 (after theme::sync_gpui_component_theme), BEFORE tray manager

// Sync Script Kit theme with gpui-component's ThemeColor system
theme::sync_gpui_component_theme(cx);

// MOVED: Notes hotkey listener - spawn immediately so it's ready before any windows
cx.spawn(async move |cx: &mut gpui::AsyncApp| {
    logging::log("HOTKEY", "Notes hotkey listener started (early spawn)");
    
    loop {
        // Poll every 50ms - this ensures the task runs even before windows are created
        gpui::Timer::after(std::time::Duration::from_millis(50)).await;
        
        while let Ok(()) = hotkeys::notes_hotkey_channel().1.try_recv() {
            logging::log("HOTKEY", "Notes hotkey triggered - opening notes window");
            let _ = cx.update(|cx: &mut gpui::App| {
                if let Err(e) = notes::open_notes_window(cx) {
                    logging::log("HOTKEY", &format!("Failed to open notes window: {}", e));
                }
            });
        }
    }
}).detach();

// Initialize tray icon and menu (continues at line 1052)
```

### Step 5: Alternative - Use Thread + Flag

If polling is undesirable, use a thread with an atomic flag:

```rust
// File: src/main.rs
// Add at the top with other statics:
static NOTES_REQUESTED: AtomicBool = AtomicBool::new(false);

// In hotkeys.rs event dispatch:
else if event.id == notes_hotkey_id {
    NOTES_REQUESTED.store(true, Ordering::SeqCst);
}

// In main.rs, inside Application::new().run(), spawn a timer that checks the flag:
cx.spawn(async move |cx: &mut gpui::AsyncApp| {
    loop {
        gpui::Timer::after(std::time::Duration::from_millis(50)).await;
        
        if NOTES_REQUESTED.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            let _ = cx.update(|cx| {
                if let Err(e) = notes::open_notes_window(cx) {
                    logging::log("HOTKEY", &format!("Failed to open notes: {}", e));
                }
            });
        }
    }
}).detach();
```

### Testing

1. Build the app: `cargo build`
2. Run with AI logs: `SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1 | grep -i hotkey`
3. Press Cmd+Shift+N **before** pressing the main window hotkey
4. Check logs for:
   - "Notes hotkey pressed - sending to channel" (from hotkeys.rs)
   - "Notes hotkey listener started" (from main.rs)
   - "Notes hotkey triggered - opening notes window"
5. The Notes window should appear immediately without needing to show the main window first

---

## Instructions For The Next AI Agent

You are reading the "Notes Window Coupling Bug Expert Bundle". This file is self-contained and includes all the context you should assume you have.

**Original Goal:**
> The notes window still only opens when the main window opens. When I press the notes window shortcut, it seems like nothing happens. But it will appear when the main window opens. Something is still binding them together.

Your job:

* Design and describe the minimal, safe changes needed to fully resolve the issues described in the Executive Summary and Key Problems.
* Operate **only** on the files and code snippets included in this bundle. If you need additional files or context, clearly say so.

When you propose changes, follow these rules strictly:

1. Always provide **precise code snippets** that can be copy-pasted directly into the repo.
2. Always include **exact file paths** (e.g. `src/main.rs`) and, when possible, line numbers or a clear description of the location (e.g. "replace the existing `cx.spawn()` for notes hotkey at lines 1181-1193").
3. Never describe code changes only in prose. Show the full function or block as it should look **after** the change, or show both "before" and "after" versions.
4. Keep instructions **unmistakable and unambiguous**. A human or tool following your instructions should not need to guess what to do.
5. Assume you cannot see any files outside this bundle. If you must rely on unknown code, explicitly note assumptions and risks.

**Key Investigation Points:**
1. Verify the hotkey is being sent to the channel (add logging in `src/hotkeys.rs`)
2. Verify the `cx.spawn()` task actually starts running (check for "Notes hotkey listener started" log)
3. Consider whether GPUI's async executor requires window activation to start processing tasks
4. The recommended fix is to use a polling timer (`Timer::after()` + `try_recv()`) instead of blocking `recv().await`

When you answer, you do not need to restate this bundle. Work directly with the code and instructions it contains and return a clear, step-by-step plan plus exact code edits.
