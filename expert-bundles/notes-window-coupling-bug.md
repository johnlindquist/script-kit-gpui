# Expert Bundle: Notes Window Only Opens After Main Window Hotkey

## Bug Description

The Notes window (Cmd+Shift+N) only opens **after** the main window hotkey (Cmd+;) is pressed first. If you launch the app and immediately press Cmd+Shift+N, nothing happens. But once you press Cmd+; to show the main window, then Cmd+Shift+N works.

The same issue likely affects the AI window (Cmd+Shift+Space).

## Expected Behavior

Notes and AI windows should open immediately when their hotkeys are pressed, regardless of whether the main window has been shown.

## Already Attempted (Did NOT Fix)

1. **Moved hotkey listener spawn earlier** - Moved `cx.spawn()` for Notes/AI listeners before main window creation. Did not help.

2. **Changed from blocking `recv().await` to polling** - Replaced:
   ```rust
   while let Ok(()) = hotkeys::notes_hotkey_channel().1.recv().await {
   ```
   With:
   ```rust
   loop {
       Timer::after(Duration::from_millis(50)).await;
       while let Ok(()) = hotkeys::notes_hotkey_channel().1.try_recv() {
   ```
   Did not help.

## Architecture Overview

### Hotkey Registration (Works Correctly)

`start_hotkey_listener()` runs in a **separate OS thread** (spawned at line 966, BEFORE `Application::new().run()`):

```rust
// src/main.rs line 966 - BEFORE Application::new().run()
hotkeys::start_hotkey_listener(loaded_config);

// ... later at line 1006
Application::new().run(move |cx: &mut App| {
```

Inside `start_hotkey_listener()` (src/hotkeys.rs):
- Creates `GlobalHotKeyManager` 
- Registers all hotkeys (main, notes, AI, script shortcuts)
- Runs event loop: `GlobalHotKeyEvent::receiver().recv()`
- When Notes hotkey pressed, sends to channel: `notes_hotkey_channel().0.send_blocking(())`

**The hotkey registration and detection works correctly** - logs show "meta+shift+N pressed (notes)" when hotkey is pressed.

### Channel Architecture

```rust
// src/hotkeys.rs
static NOTES_HOTKEY_CHANNEL: OnceLock<(async_channel::Sender<()>, async_channel::Receiver<()>)>

pub(crate) fn notes_hotkey_channel() -> &'static (Sender<()>, Receiver<()>) {
    NOTES_HOTKEY_CHANNEL.get_or_init(|| async_channel::bounded(10))
}
```

- Uses `async_channel` (async-compatible mpmc channel)
- Hotkey thread sends via `send_blocking()` (blocks if channel full)
- GPUI async task receives via `try_recv()` (non-blocking)

### Listener in GPUI (The Problem Area)

```rust
// src/main.rs lines 1149-1166
cx.spawn(async move |cx: &mut gpui::AsyncApp| {
    logging::log("HOTKEY", "Notes hotkey listener started (poll-based)");
    loop {
        Timer::after(std::time::Duration::from_millis(50)).await;
        
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

## Key Files

### src/main.rs (relevant sections)

**Hotkey listener start (line 966):**
```rust
hotkeys::start_hotkey_listener(loaded_config);
```

**Application run (line 1006):**
```rust
Application::new().run(move |cx: &mut App| {
```

**Main window hotkey listener (lines 1085-1144):**
```rust
cx.spawn(async move |cx: &mut gpui::AsyncApp| {
    logging::log("HOTKEY", "Main hotkey listener started");
    while let Ok(()) = hotkeys::hotkey_channel().1.recv().await {
        // Toggle main window visibility
        // ...
    }
}).detach();
```

**Notes hotkey listener (lines 1149-1166):**
```rust
cx.spawn(async move |cx: &mut gpui::AsyncApp| {
    logging::log("HOTKEY", "Notes hotkey listener started (poll-based)");
    loop {
        Timer::after(std::time::Duration::from_millis(50)).await;
        while let Ok(()) = hotkeys::notes_hotkey_channel().1.try_recv() {
            // Open notes window
        }
    }
}).detach();
```

### src/hotkeys.rs (full file)

```rust
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use crate::{config, logging, scripts, shortcuts};

// HOTKEY_CHANNEL: Event-driven async_channel for hotkey events
static HOTKEY_CHANNEL: OnceLock<(async_channel::Sender<()>, async_channel::Receiver<()>)> =
    OnceLock::new();

pub(crate) fn hotkey_channel() -> &'static (async_channel::Sender<()>, async_channel::Receiver<()>)
{
    HOTKEY_CHANNEL.get_or_init(|| async_channel::bounded(10))
}

// NOTES_HOTKEY_CHANNEL: Channel for notes hotkey events
static NOTES_HOTKEY_CHANNEL: OnceLock<(async_channel::Sender<()>, async_channel::Receiver<()>)> =
    OnceLock::new();

pub(crate) fn notes_hotkey_channel(
) -> &'static (async_channel::Sender<()>, async_channel::Receiver<()>) {
    NOTES_HOTKEY_CHANNEL.get_or_init(|| async_channel::bounded(10))
}

// AI_HOTKEY_CHANNEL: Channel for AI hotkey events  
static AI_HOTKEY_CHANNEL: OnceLock<(async_channel::Sender<()>, async_channel::Receiver<()>)> =
    OnceLock::new();

pub(crate) fn ai_hotkey_channel(
) -> &'static (async_channel::Sender<()>, async_channel::Receiver<()>) {
    AI_HOTKEY_CHANNEL.get_or_init(|| async_channel::bounded(10))
}

static HOTKEY_TRIGGER_COUNT: AtomicU64 = AtomicU64::new(0);

pub(crate) fn start_hotkey_listener(config: config::Config) {
    std::thread::spawn(move || {
        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                logging::log("HOTKEY", &format!("Failed to create hotkey manager: {}", e));
                return;
            }
        };

        // ... hotkey registration code ...
        
        // Register notes hotkey (Cmd+Shift+N)
        let notes_hotkey = HotKey::new(Some(notes_modifiers), notes_code);
        let notes_hotkey_id = notes_hotkey.id();
        manager.register(notes_hotkey)?;

        let receiver = GlobalHotKeyEvent::receiver();

        loop {
            if let Ok(event) = receiver.recv() {
                if event.state != HotKeyState::Pressed {
                    continue;
                }

                if event.id == main_hotkey_id {
                    hotkey_channel().0.send_blocking(()).is_err();
                }
                else if event.id == notes_hotkey_id {
                    logging::log("HOTKEY", &format!("{} pressed (notes)", notes_display));
                    if notes_hotkey_channel().0.send_blocking(()).is_err() {
                        logging::log("HOTKEY", "Notes hotkey channel closed, cannot send");
                    }
                }
                // ... more hotkey handling ...
            }
        }
    });
}
```

### src/notes/window.rs (open_notes_window function)

```rust
use gpui::{App, WindowHandle, WindowOptions, ...};
use gpui_component::Root;
use std::sync::OnceLock;

static NOTES_WINDOW: OnceLock<std::sync::Mutex<Option<WindowHandle<Root>>>> = OnceLock::new();

pub fn open_notes_window(cx: &mut App) -> anyhow::Result<WindowHandle<Root>> {
    let window_lock = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = window_lock.lock().unwrap();
    
    // If window exists, just focus it
    if let Some(ref handle) = *guard {
        if handle.is_valid() {
            cx.activate(true);
            let _ = handle.update(cx, |_root, window, _cx| {
                window.activate_window();
            });
            return Ok(handle.clone());
        }
    }
    
    // Create new window
    let window_options = WindowOptions {
        titlebar: None,
        window_background: WindowBackgroundAppearance::Blurred,
        // ...
    };
    
    let handle = cx.open_window(window_options, |window, cx| {
        let view = cx.new(|cx| NotesApp::new(window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    })?;
    
    *guard = Some(handle.clone());
    
    cx.activate(true);
    handle.update(cx, |_root, window, _cx| {
        window.activate_window();
    })?;
    
    Ok(handle)
}
```

## Hypothesis

The issue is likely that **GPUI's async executor doesn't process spawned tasks until the event loop is actively running**. 

Key observations:
1. `start_hotkey_listener()` runs in a separate OS thread - this works fine
2. The hotkey is detected and logged correctly
3. The channel send succeeds (no "channel closed" error)
4. But the GPUI `cx.spawn()` task doesn't receive the message

The main window hotkey works because when you press it:
1. It triggers window show/activation
2. This kicks the GPUI event loop into active processing
3. Now ALL spawned tasks start running (including Notes listener)
4. Subsequent Notes hotkey presses work

## Possible Solutions to Investigate

1. **Use a different async runtime** - Instead of `cx.spawn()`, use `tokio::spawn()` or `std::thread::spawn()` with a callback mechanism

2. **Force event loop wake** - Call some GPUI function that forces the event loop to process pending tasks

3. **Use platform-native callback** - Use macOS's dispatch queue to call back into GPUI

4. **Polling timer that starts immediately** - Maybe the Timer itself doesn't tick until event loop is active?

5. **Global window handle** - Create an invisible window that's always present to receive events

## Log Output When Bug Occurs

When pressing Cmd+Shift+N BEFORE main window hotkey:
```
[HOTKEY] meta+shift+N pressed (notes)
```
(No "Notes hotkey triggered" log - the listener never receives the message)

When pressing Cmd+; first, then Cmd+Shift+N:
```
[HOTKEY] meta+; pressed (trigger #1)
[VISIBILITY] HOTKEY TRIGGERED - TOGGLE WINDOW
[HOTKEY] Window shown and activated
[HOTKEY] meta+shift+N pressed (notes)
[HOTKEY] Notes hotkey triggered - opening notes window
```

## Questions for Expert

1. Does GPUI's `cx.spawn()` require the event loop to be actively processing window events before async tasks run?

2. Is there a way to force the GPUI async executor to start processing without showing a window?

3. Should we use a different approach entirely - e.g., `std::thread` with `cx.update_window()` callback?

4. Is there a GPUI-idiomatic way to handle global hotkeys for secondary windows?
