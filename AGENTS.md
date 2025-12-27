# Script Kit GPUI

Script KIT GPUI is a complete rewrite of Script Kit into the GPUI framework. The goal is for backwards compatibility with Script Kit scripts, but using a completely new architecture and design principles: GPUI for the app shell and bun for running script with our new SDK.

---

## Agent Quick Start Checklist

**MANDATORY for all AI agents working on this codebase:**

```
□ 1. Read this file completely before making changes
□ 2. Check .hive/issues.jsonl for existing tasks and context
□ 3. Run verification BEFORE committing: cargo check && cargo clippy && cargo test
□ 4. Update bead status when starting/completing work
□ 5. Write tests FIRST (TDD) - see Section 14 for test patterns
□ 6. Include correlation_id in all log entries
□ 7. TEST UI CHANGES via stdin JSON protocol (see "Autonomous Testing Protocol" below)
```

### CRITICAL: How to Test UI Changes

**NEVER pass scripts as command line arguments.** The app uses stdin JSON messages:

```bash
# CORRECT - Use stdin JSON protocol
echo '{"type": "run", "path": "'$(pwd)'/tests/smoke/test-editor-height.ts"}' | ./target/debug/script-kit-gpui 2>&1

# WRONG - Command line args don't work!
./target/debug/script-kit-gpui tests/smoke/hello-world.ts  # THIS DOES NOTHING
```

### Quick Verification Command

```bash
# Run this before every commit
cargo check && cargo clippy --all-targets -- -D warnings && cargo test
```

---

## Quick Reference

| Topic | Key Insight |
|-------|-------------|
| **Layout Order** | Always: Layout (`flex()`) -> Sizing (`w()`, `h()`) -> Spacing (`px()`, `gap()`) -> Visual (`bg()`, `border()`) |
| **List Performance** | Use `uniform_list` with fixed-height items (52px) + `UniformListScrollHandle` |
| **Theme Colors** | Use `theme.colors.*` - NEVER hardcode `rgb(0x...)` values |
| **Focus Colors** | Call `theme.get_colors(is_focused)` for focus-aware styling |
| **State Updates** | Always call `cx.notify()` after modifying state |
| **Keyboard Events** | Use `cx.listener()` pattern, coalesce rapid events (20ms window) |
| **Window Positioning** | Use `Bounds::centered(Some(display_id), size, cx)` for multi-monitor |
| **Error Handling** | Use `anyhow::Result` + `.context()` for propagation, `NotifyResultExt` for user display |
| **Logging** | Use `tracing` with JSONL format, typed fields, include `correlation_id` and `duration_ms` |
| **TDD Workflow** | Read tests → Write failing test → Implement → Verify → Commit (Red-Green-Refactor) |
| **Bead Protocol** | `hive_start` → Work → `swarm_progress` → `swarm_complete` (NOT `hive_close`) |
| **Test Hierarchy** | `tests/smoke/` = E2E flows, `tests/sdk/` = individual SDK methods |
| **Verification Gate** | Always run `cargo check && cargo clippy && cargo test` before commits |
| **SDK Preload** | Scripts import `../../scripts/kit-sdk` for global functions (arg, div, md) |
| **Arrow Key Names** | ALWAYS match BOTH: `"up" \| "arrowup"`, `"down" \| "arrowdown"`, `"left" \| "arrowleft"`, `"right" \| "arrowright"` |
| **Visual Testing** | Use `./scripts/visual-test.sh <test.ts> <seconds>` to capture screenshots for layout debugging |

---

## CRITICAL: Autonomous Testing Protocol

<critical>
**ALL UI changes MUST be tested using the stdin JSON protocol before committing.**

The app accepts JSONL commands via stdin for automated testing. This is the ONLY way to test UI behavior without manual interaction.

### The Build-Test-Iterate Loop

**MANDATORY WORKFLOW for any UI/feature changes:**

```bash
# 1. Build
cargo build

# 2. Run test via stdin JSON (NOT command line args!)
echo '{"type": "run", "path": "'$(pwd)'/tests/smoke/test-editor-height.ts"}' | ./target/debug/script-kit-gpui 2>&1

# 3. Check output for expected behavior
# 4. If broken, fix code and repeat from step 1
```

**This loop is NON-NEGOTIABLE.** Do not ask the user to test. Do not skip testing. Run the test yourself, read the logs, fix issues, repeat.

### Available stdin Commands

```json
{"type": "run", "path": "/absolute/path/to/script.ts"}
{"type": "show"}
{"type": "hide"}
{"type": "setFilter", "text": "search term"}
```

### Test Scripts Location

Test scripts live in `tests/smoke/` and use the SDK:

```typescript
// tests/smoke/test-my-feature.ts
import '../../scripts/kit-sdk';

console.error('[SMOKE] test starting...');
const result = await editor("test content", "typescript");
console.error('[SMOKE] result:', result);
```

### Checking Logs

```bash
# Stderr shows pretty logs during execution
# JSONL logs are at ~/.kit/logs/script-kit-gpui.jsonl

# Filter for specific behavior:
echo '{"type": "run", "path": "..."}' | ./target/debug/script-kit-gpui 2>&1 | grep -iE 'RESIZE|editor|height'

# Check structured logs:
tail -50 ~/.kit/logs/script-kit-gpui.jsonl | grep -i resize
```

### Example: Testing Window Resize

```bash
# Run the editor height test
echo '{"type": "run", "path": "'$(pwd)'/tests/smoke/test-editor-height.ts"}' | ./target/debug/script-kit-gpui 2>&1 | grep -iE 'RESIZE|height_for_view|700'

# Expected output should show:
# height_for_view(EditorPrompt) = 700
# Resize: 501 -> 700
```

### Visual Testing with Screenshots

**For UI layout issues that can't be verified through logs alone, use the visual test script:**

```bash
# Run visual test - launches app, waits for render, captures screenshot, terminates
./scripts/visual-test.sh tests/smoke/test-editor-height.ts 3

# Output:
# Screenshot: .test-screenshots/test-editor-height-20251227-012813.png
# Log: .test-screenshots/test-editor-height-20251227-012813.log
```

**The visual test script (`scripts/visual-test.sh`):**
1. Builds the project
2. Launches the app with your test script via stdin JSON
3. Waits N seconds for the window to render (default: 2s)
4. Captures a screenshot using macOS `screencapture`
5. Terminates the app
6. Saves screenshot and logs to `.test-screenshots/`

**Reading screenshots programmatically:**
```bash
# After running visual test, read the screenshot to analyze UI state
# The screenshot path is output by the script
```

**When to use visual testing:**
- Layout issues (content not filling space, wrong sizes)
- Styling problems (colors, borders, spacing)
- Component visibility issues
- Any UI behavior that logs alone can't verify

**Screenshot analysis workflow:**
1. Run `./scripts/visual-test.sh <test-script.ts> <wait-seconds>`
2. Read the screenshot file to see actual rendered state
3. Compare against expected behavior
4. If broken, fix code and repeat

**Example: Debugging editor height issue:**
```bash
# 1. Run visual test
./scripts/visual-test.sh tests/smoke/test-editor-height.ts 3

# 2. Check the screenshot - does editor fill the 700px window?
# 3. Check the log for height values:
grep -E 'height|700|resize' .test-screenshots/test-editor-height-*.log

# 4. If editor is too small, the issue is in layout code
# 5. Fix and repeat until screenshot shows correct layout
```

### Anti-Patterns

| Wrong | Right |
|-------|-------|
| `./target/debug/script-kit-gpui test.ts` | `echo '{"type":"run",...}' \| ./target/debug/script-kit-gpui` |
| "I can't test this without manual interaction" | Use stdin protocol, add logging, verify in output |
| "The user should test this" | YOU must test it using the stdin protocol |
| Committing without running the test | Run `cargo build && echo '...' \| ./target/debug/...` |
| "I can't see what the UI looks like" | Use `./scripts/visual-test.sh` to capture and read screenshots |
| Guessing at layout issues | Capture screenshot, read it, analyze actual vs expected |

### Why This Matters

- **Keyboard events require a visible window** - the window must be activated via the protocol
- **Focus must be set correctly** - check logs for `focus_handle.is_focused=true`
- **Key names vary by platform** - GPUI may send `"up"` OR `"arrowup"` for arrow keys
- **Layout issues are only visible at runtime** - you MUST run the app to see them
</critical>

---

## CRITICAL: GPUI Keyboard Event Key Names

<critical>
**GPUI sends SHORT key names for arrow keys, not the `arrow*` variants.**

When handling arrow key events, ALWAYS match BOTH possible names:

```rust
// ✅ CORRECT - matches both possible key names
match key.as_str() {
    "up" | "arrowup" => self.move_up(),
    "down" | "arrowdown" => self.move_down(),
    "left" | "arrowleft" => self.move_left(),
    "right" | "arrowright" => self.move_right(),
    // ...
}

// ❌ WRONG - only matches one variant, will fail on some platforms
match key.as_str() {
    "arrowup" => self.move_up(),    // BROKEN - GPUI sends "up"
    "arrowdown" => self.move_down(), // BROKEN - GPUI sends "down"
    // ...
}
```

**Why:** GPUI's `keystroke.key` field contains platform-dependent values. On macOS, arrow keys come through as `"up"`, `"down"`, `"left"`, `"right"`. Always handle both forms.

**Verification:** Search codebase with `grep -n "up.*arrowup\|arrowup.*up" src/*.rs` to see correct patterns.
</critical>

---

## 1. Layout System

### Flexbox Basics

GPUI uses a flexbox-like layout system. Always chain methods in this order:

```rust
div()
    // 1. Layout direction
    .flex()
    .flex_col()           // or .flex_row()
    
    // 2. Sizing
    .w_full()
    .h(px(52.))
    
    // 3. Spacing
    .px(px(16.))
    .py(px(8.))
    .gap_3()
    
    // 4. Visual styling
    .bg(rgb(colors.background.main))
    .border_color(rgb(colors.ui.border))
    .rounded_md()
    
    // 5. Children
    .child(...)
```

### Common Layout Patterns

```rust
// Horizontal row with centered items
div().flex().flex_row().items_center().gap_2()

// Vertical stack, full width
div().flex().flex_col().w_full()

// Centered content
div().flex().items_center().justify_center()

// Fill remaining space
div().flex_1()
```

### Conditional Rendering

```rust
// Use .when() for conditional styles
div()
    .when(is_selected, |d| d.bg(selected_color))
    .when_some(description, |d, desc| d.child(desc))

// Use .map() for transforms
div().map(|d| if loading { d.opacity(0.5) } else { d })
```

---

## 2. List Virtualization

### uniform_list Setup

For long lists, use `uniform_list` with fixed-height items:

```rust
uniform_list(
    "script-list",
    filtered.len(),
    cx.processor(|this, range, _window, _cx| {
        this.render_list_items(range)
    }),
)
.h_full()
.track_scroll(&self.list_scroll_handle)
```

### Scroll Handling

```rust
// Create handle
list_scroll_handle: UniformListScrollHandle::new(),

// Scroll to item
self.list_scroll_handle.scroll_to_item(
    selected_index,
    ScrollStrategy::Nearest,
);
```

### Performance: Event Coalescing

Rapid keyboard scrolling can freeze the UI. Implement a 20ms coalescing window:

```rust
// Track scroll direction and pending events
enum ScrollDirection { Up, Down }

fn process_arrow_key_with_coalescing(&mut self, direction: ScrollDirection) {
    let now = Instant::now();
    let coalesce_window = Duration::from_millis(20);
    
    if now.duration_since(self.last_scroll_time) < coalesce_window
       && self.pending_scroll_direction == Some(direction) {
        self.pending_scroll_delta += 1;
        return;
    }
    
    self.flush_pending_scroll();
    self.pending_scroll_direction = Some(direction);
    self.pending_scroll_delta = 1;
    self.last_scroll_time = now;
}

fn move_selection_by(&mut self, delta: i32) {
    let new_index = (self.selected_index as i32 + delta)
        .max(0)
        .min(self.items.len() as i32 - 1) as usize;
    self.selected_index = new_index;
    cx.notify();
}
```

---

## 3. Theme System

### Architecture

The theme system is in `src/theme.rs`:

```rust
pub struct Theme {
    pub colors: ColorScheme,           // Base colors
    pub focus_aware: Option<FocusAwareColorScheme>,  // Focus/unfocus variants
    pub opacity: Option<BackgroundOpacity>,
    pub drop_shadow: Option<DropShadow>,
    pub vibrancy: Option<VibrancySettings>,
}

pub struct ColorScheme {
    pub background: BackgroundColors,  // main, title_bar, search_box, log_panel
    pub text: TextColors,              // primary, secondary, tertiary, muted, dimmed
    pub accent: AccentColors,          // selected, selected_subtle, button_text
    pub ui: UIColors,                  // border, success
}
```

### Using Theme Colors (CORRECT)

```rust
impl Render for MyComponent {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &self.theme.colors;
        
        div()
            .bg(rgb(colors.background.main))
            .border_color(rgb(colors.ui.border))
            .child(
                div()
                    .text_color(rgb(colors.text.primary))
                    .child("Hello")
            )
    }
}
```

### Anti-Pattern: Hardcoded Colors (WRONG)

```rust
// DON'T DO THIS - breaks theme switching
div()
    .bg(rgb(0x2d2d2d))           // Hardcoded!
    .border_color(rgb(0x3d3d3d)) // Hardcoded!
    .text_color(rgb(0x888888))   // Hardcoded!
```

### Focus-Aware Colors

Windows should dim when unfocused:

```rust
fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let is_focused = self.focus_handle.is_focused(window);
    
    // Track focus changes
    if self.is_window_focused != is_focused {
        self.is_window_focused = is_focused;
        cx.notify();
    }
    
    // Get appropriate colors
    let colors = self.theme.get_colors(is_focused);
    
    // Use colors...
}
```

### Lightweight Color Extraction

For closures, use Copy-able color structs:

```rust
let list_colors = colors.list_item_colors();  // Returns ListItemColors (Copy)

uniform_list(cx, |_this, visible_range, _window, _cx| {
    for ix in visible_range {
        let bg = if is_selected { 
            list_colors.background_selected 
        } else { 
            list_colors.background 
        };
        // ... render
    }
})
```

---

## 4. Event Handling

### Keyboard Events

```rust
// In window setup
window.on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
    let key = event.key.as_ref().map(|k| k.as_str()).unwrap_or("");
    
    match key {
        "ArrowDown" => this.move_selection_down(cx),
        "ArrowUp" => this.move_selection_up(cx),
        "Enter" => this.submit_selection(cx),
        "Escape" => this.clear_filter(cx),
        _ => {}
    }
}));
```

### Focus Management

```rust
pub struct MyApp {
    focus_handle: FocusHandle,
}

impl Focusable for MyApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// Create focus handle
let focus_handle = cx.focus_handle();

// Focus the element
focus_handle.focus(window);

// Check if focused
let is_focused = focus_handle.is_focused(window);
```

### Mouse Events

```rust
div()
    .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
        this.handle_click(event, cx);
    }))
    .on_mouse_down(MouseButton::Right, cx.listener(|this, event, window, cx| {
        this.show_context_menu(event.position, cx);
    }))
```

---

## 5. Window Management

### Multi-Monitor Positioning

Position window on the display containing the mouse:

```rust
fn position_window_on_mouse_display(
    window: &mut Window,
    cx: &mut App,
) {
    let window_size = size(px(500.), px(700.0));
    
    // Get mouse position and find target display
    let mouse_pos = window.mouse_position();
    let window_bounds = window.bounds();
    let absolute_mouse = Point {
        x: window_bounds.origin.x + mouse_pos.x,
        y: window_bounds.origin.y + mouse_pos.y,
    };
    
    let target_display = cx.displays()
        .into_iter()
        .find(|display| display.bounds().contains(&absolute_mouse));
    
    if let Some(display) = target_display {
        let bounds = display.bounds();
        
        // Position at eye-line (upper 1/3)
        let eye_line = bounds.origin.y + bounds.size.height / 3.0;
        
        let positioned = Bounds::centered_at(
            Point { x: bounds.center().x, y: eye_line },
            window_size,
        );
        
        window.set_bounds(WindowBounds::Windowed(positioned), cx);
    }
}
```

### Display APIs

| API | Purpose |
|-----|---------|
| `cx.displays()` | Get all displays |
| `cx.primary_display()` | Get main display |
| `cx.find_display(id)` | Get specific display |
| `display.bounds()` | Full screen area |
| `display.visible_bounds()` | Usable area (no dock/taskbar) |
| `bounds.contains(&point)` | Check if point is in display |

### macOS Floating Panel

Make window float above other applications:

```rust
#[cfg(target_os = "macos")]
fn configure_as_floating_panel() {
    unsafe {
        let app: id = NSApp();
        let window: id = msg_send![app, keyWindow];
        
        if window != nil {
            // NSFloatingWindowLevel = 3
            let floating_level: i32 = 3;
            let _: () = msg_send![window, setLevel:floating_level];
            
            // NSWindowCollectionBehaviorCanJoinAllSpaces = 1
            let collection_behavior: u64 = 1;
            let _: () = msg_send![window, setCollectionBehavior:collection_behavior];
        }
    }
}
```

Call after `cx.activate(true)` in main().

---

## 6. State Management

### Updating State

Always call `cx.notify()` after state changes to trigger re-render:

```rust
fn set_filter(&mut self, filter: String, cx: &mut Context<Self>) {
    self.filter = filter;
    self.update_filtered_results();
    cx.notify();  // REQUIRED - triggers re-render
}
```

### Shared State

Use `Arc<Mutex<T>>` or channels for thread-safe state:

```rust
// For shared mutable state
let shared_state = Arc::new(Mutex::new(MyState::default()));

// For async updates from threads
let (sender, receiver) = mpsc::channel();
std::thread::spawn(move || {
    // Do work...
    sender.send(Update::NewData(data)).ok();
});
```

---

## 7. Code Quality Guidelines

### DO

| Pattern | Example |
|---------|---------|
| Use theme colors | `rgb(colors.background.main)` |
| Call `cx.notify()` after state changes | `self.selected = index; cx.notify();` |
| Use `uniform_list` for long lists | See virtualization section |
| Implement `Focusable` trait | Required for keyboard focus |
| Use `cx.listener()` for events | `on_click(cx.listener(\|...\| {...}))` |
| Log spawn failures | `match Command::new(...).spawn() { Ok(_) => ..., Err(e) => log_error(e) }` |
| Extract shared utilities | `utils::strip_html_tags()` |

### DON'T

| Anti-Pattern | Why It's Bad |
|--------------|--------------|
| Hardcode colors | Breaks theme switching |
| Skip `cx.notify()` | UI won't update |
| Use raw loops for lists | Performance issues with many items |
| Ignore spawn errors | Silent failures are hard to debug |
| Duplicate utilities | Maintenance burden |

### Render Method Size

Keep render methods under ~100 lines. Extract helpers:

```rust
// Instead of one 300-line render method...
fn render(&mut self, ...) -> impl IntoElement {
    div()
        .child(self.render_header(cx))
        .child(self.render_content(cx))
        .child(self.render_footer(cx))
}

fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement { ... }
fn render_content(&self, cx: &mut Context<Self>) -> impl IntoElement { ... }
fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement { ... }
```

---

## 8. Development Workflow

### Hot Reload

```bash
./dev.sh  # Starts cargo-watch for auto-rebuild
```

### What Triggers Reload

| Change Type | Mechanism |
|-------------|-----------|
| `.rs` files | cargo-watch rebuilds |
| `~/.kit/theme.json` | File watcher, live reload |
| `~/.kenv/scripts/` | File watcher, live reload |
| `~/.kit/config.ts` | Requires restart |

### Debugging

- **Logs Panel**: Press `Cmd+L` in the app
- **Log Tags**: `[UI]`, `[EXEC]`, `[KEY]`, `[THEME]`, `[FOCUS]`, `[HOTKEY]`, `[PANEL]`
- **Performance**: Filter for `[KEY_PERF]`, `[SCROLL_TIMING]`, `[PERF_SLOW]`

### Performance Testing

```bash
# Run scroll performance tests
bun run tests/sdk/test-scroll-perf.ts

# Run benchmark
npx tsx scripts/scroll-bench.ts
```

| Metric | Threshold |
|--------|-----------|
| P95 Key Latency | < 50ms |
| Single Key Event | < 16.67ms (60fps) |
| Scroll Operation | < 8ms |

---

## 9. Common Gotchas

### Problem: UI doesn't update after state change
**Solution**: Call `cx.notify()` after modifying any state that affects rendering.

### Problem: Theme changes don't apply
**Solution**: Check for hardcoded `rgb(0x...)` values. Use `theme.colors.*` instead.

### Problem: List scrolling is laggy
**Solution**: Implement event coalescing (20ms window) for rapid key events.

### Problem: Window appears on wrong monitor
**Solution**: Use mouse position to find the correct display, then `Bounds::centered()`.

### Problem: Focus styling doesn't work
**Solution**: Implement `Focusable` trait and track `is_focused` in render.

### Problem: Spawn failures are silent
**Solution**: Match on `Command::spawn()` result and log errors.

---

## 10. File Structure

```
src/
  main.rs       # App entry, window setup, main render loop, ErrorNotification UI
  error.rs      # ScriptKitError enum, ErrorSeverity, NotifyResultExt trait
  theme.rs      # Theme system, ColorScheme, focus-aware colors, error/warning/info colors
  prompts.rs    # ArgPrompt, DivPrompt interactive prompts
  actions.rs    # ActionsDialog popup
  protocol.rs   # JSON message protocol with ParseResult for graceful error handling
  scripts.rs    # Script loading and execution with tracing instrumentation
  config.rs     # Config loading with defaults fallback
  executor.rs   # Script execution with timing spans and structured logging
  watcher.rs    # File watchers with observability instrumentation
  panel.rs      # macOS panel configuration
  perf.rs       # Performance timing utilities
  logging.rs    # Dual-output logging: JSONL to ~/.kit/logs/, pretty to stderr
  lib.rs        # Module exports
  utils.rs      # Shared utilities (strip_html_tags, etc.)
```

### Log File Location

Logs are written to `~/.kit/logs/script-kit-gpui.jsonl` in JSONL format for AI agent consumption.

---

## 11. Error Handling

### Error Type Selection

Use the right error type for the job:

| Crate | When to Use |
|-------|-------------|
| `anyhow` | Application-level errors, CLI tools, error propagation |
| `thiserror` | Library code, domain-specific errors, when callers need to match on error types |

```rust
// Cargo.toml
[dependencies]
anyhow = "1.0"
thiserror = "2.0"
```

### anyhow for Application Errors

Use `anyhow::Result` throughout application code:

```rust
use anyhow::{Context, Result};

fn load_config(path: &Path) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .context("Failed to read config file")?;
    
    let config: Config = serde_json::from_str(&content)
        .context("Failed to parse config JSON")?;
    
    Ok(config)
}

fn main() -> Result<()> {
    let config = load_config(Path::new("~/.kit/config.json"))
        .context("Config initialization failed")?;
    
    // Application logic...
    Ok(())
}
```

### thiserror for Domain Errors

Use `thiserror` when callers need to handle specific error variants:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScriptError {
    #[error("Script not found: {path}")]
    NotFound { path: String },
    
    #[error("Script execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Invalid script format: {0}")]
    InvalidFormat(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// Callers can match on specific errors
match run_script(&path) {
    Ok(output) => handle_output(output),
    Err(ScriptError::NotFound { path }) => show_not_found_dialog(&path),
    Err(ScriptError::ExecutionFailed(msg)) => log_execution_error(&msg),
    Err(e) => show_generic_error(e),
}
```

### Result Propagation with Context

Always add context when propagating errors up the call stack:

```rust
// GOOD: Adds context at each level
fn load_theme() -> Result<Theme> {
    let path = get_theme_path()
        .context("Failed to determine theme path")?;
    
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read theme file: {}", path.display()))?;
    
    let theme: Theme = serde_json::from_str(&content)
        .context("Failed to parse theme JSON")?;
    
    Ok(theme)
}

// BAD: Loses context
fn load_theme_bad() -> Result<Theme> {
    let path = get_theme_path()?;  // What failed?
    let content = std::fs::read_to_string(&path)?;  // Which file?
    let theme: Theme = serde_json::from_str(&content)?;  // What was wrong?
    Ok(theme)
}
```

### User Notification Pattern (Toast)

Display errors to users with auto-dismissing toasts:

```rust
/// Extension trait for ergonomic error display
pub trait NotifyResultExt<T> {
    fn notify_err(self, cx: &mut Context<impl Render>) -> Option<T>;
}

impl<T, E: std::fmt::Display> NotifyResultExt<T> for Result<T, E> {
    fn notify_err(self, cx: &mut Context<impl Render>) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(e) => {
                // Always log first
                tracing::error!(error = %e, "Operation failed");
                
                // Show toast to user
                show_toast(cx, ToastOptions {
                    message: e.to_string(),
                    level: ToastLevel::Error,
                    auto_dismiss_ms: 5000,  // 5 seconds
                });
                
                None
            }
        }
    }
}

// Usage
fn handle_save(&mut self, cx: &mut Context<Self>) {
    if let Some(saved) = self.save_file().notify_err(cx) {
        self.show_success_message(&saved);
    }
    // Error already displayed to user if save_file() failed
}
```

### Error Handling Best Practices

| Pattern | Description |
|---------|-------------|
| Log before display | Always `tracing::error!()` before showing to user |
| Context at boundaries | Add `.context()` at function call boundaries |
| Typed errors for APIs | Use `thiserror` for public library APIs |
| anyhow for apps | Use `anyhow` for application/CLI code |
| Don't panic | Use `Result` instead of `.unwrap()` or `.expect()` |
| Auto-dismiss toasts | 5 seconds is standard, 10 for critical errors |

---

## 12. Logging & Observability

### Tracing Crate Setup

Use the `tracing` ecosystem for structured logging:

```rust
// Cargo.toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tracing-appender = "0.2"
```

### Initialization

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn init_logging() {
    // JSONL to file (for AI agents and log analysis)
    let file_appender = tracing_appender::rolling::daily("logs", "app.jsonl");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    let json_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(non_blocking);
    
    // Pretty output to stdout (for humans)
    let stdout_layer = tracing_subscriber::fmt::layer()
        .pretty()
        .with_target(true);
    
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("script_kit=debug".parse().unwrap()))
        .with(json_layer)
        .with(stdout_layer)
        .init();
}
```

### JSONL Format

Logs are written as one JSON object per line:

```json
{"timestamp":"2024-01-15T10:30:45.123Z","level":"INFO","target":"script_kit::executor","message":"Script executed","fields":{"script_name":"hello.ts","duration_ms":142,"exit_code":0}}
{"timestamp":"2024-01-15T10:30:45.265Z","level":"ERROR","target":"script_kit::theme","message":"Theme load failed","fields":{"path":"/Users/x/.kit/theme.json","error":"Invalid JSON"}}
```

### Using tracing Macros

```rust
use tracing::{info, warn, error, debug, trace, instrument, span, Level};

// Basic logging with typed fields
info!(script_name = %name, duration_ms = elapsed, "Script completed");
warn!(attempt = retry_count, max_attempts = 3, "Retrying operation");
error!(error = ?e, path = %file_path, "Failed to load file");

// Debug for development, trace for very verbose
debug!(selected_index = idx, total_items = len, "Selection changed");
trace!(raw_event = ?event, "Received keyboard event");
```

### Spans for Timing

Use spans to track operation duration:

```rust
use tracing::{instrument, info_span, Instrument};

// Automatic span with #[instrument]
#[instrument(skip(self, cx), fields(script_count = scripts.len()))]
fn load_scripts(&mut self, scripts: Vec<Script>, cx: &mut Context<Self>) {
    // Duration automatically recorded when function exits
    for script in scripts {
        self.process_script(script);
    }
}

// Manual span for async or partial timing
async fn execute_script(&self, script: &Script) -> Result<Output> {
    let span = info_span!("execute_script", 
        script_name = %script.name,
        script_path = %script.path.display()
    );
    
    async {
        let start = Instant::now();
        let result = self.run_process(script).await?;
        
        info!(
            duration_ms = start.elapsed().as_millis() as u64,
            exit_code = result.exit_code,
            "Script execution complete"
        );
        
        Ok(result)
    }.instrument(span).await
}
```

### Correlation IDs

Track related operations across the codebase:

```rust
use uuid::Uuid;

fn handle_user_action(&mut self, action: Action, cx: &mut Context<Self>) {
    let correlation_id = Uuid::new_v4().to_string();
    
    let span = info_span!("user_action", 
        correlation_id = %correlation_id,
        action_type = ?action.action_type()
    );
    let _guard = span.enter();
    
    info!("Action started");
    
    // All nested logs will include correlation_id
    self.validate_action(&action)?;
    self.execute_action(&action)?;
    self.update_ui(cx);
    
    info!("Action completed");
}
```

### Log Levels Guide

| Level | When to Use | Example |
|-------|-------------|---------|
| `error!` | Operation failed, needs attention | Script crash, file not found |
| `warn!` | Unexpected but handled | Retry succeeded, fallback used |
| `info!` | Important business events | Script executed, theme loaded |
| `debug!` | Development troubleshooting | Selection changed, filter applied |
| `trace!` | Very verbose, rarely enabled | Raw events, internal state dumps |

### Performance Logging Pattern

```rust
use std::time::Instant;
use tracing::{info, warn};

const SLOW_THRESHOLD_MS: u64 = 100;

fn render_list(&self, range: Range<usize>) -> Vec<impl IntoElement> {
    let start = Instant::now();
    
    let items = self.build_list_items(range);
    
    let duration_ms = start.elapsed().as_millis() as u64;
    
    if duration_ms > SLOW_THRESHOLD_MS {
        warn!(
            duration_ms,
            item_count = items.len(),
            threshold_ms = SLOW_THRESHOLD_MS,
            "Slow render detected"
        );
    } else {
        debug!(duration_ms, item_count = items.len(), "Render complete");
    }
    
    items
}
```

### Log Tags for Filtering

Use consistent target names for easy filtering:

```rust
// In different modules, logs automatically get module path as target
// script_kit::ui, script_kit::executor, script_kit::theme

// Filter examples:
// RUST_LOG=script_kit::ui=debug  # Only UI logs
// RUST_LOG=script_kit=info       # All info+ logs
// RUST_LOG=script_kit::executor=trace  # Verbose executor logs
```

### Logging Best Practices

| Pattern | Description |
|---------|-------------|
| Use typed fields | `duration_ms = 42` not `"duration: 42ms"` |
| Include correlation IDs | Track related operations across functions |
| Use spans for timing | Automatic duration tracking |
| Non-blocking file writes | Use `tracing_appender::non_blocking` |
| Dual output | JSONL to file, pretty to stdout |
| Structured over interpolation | `info!(count = 5, "Items")` not `info!("Items: {}", 5)` |

---

## 13. Agent Workflow Protocol

### TDD-First Development

AI agents MUST follow Test-Driven Development (TDD) for all code changes:

```
┌─────────────────────────────────────────────────────────────┐
│                    TDD CYCLE FOR AGENTS                     │
├─────────────────────────────────────────────────────────────┤
│  1. READ existing tests to understand expected behavior     │
│  2. WRITE a failing test for the new feature/fix           │
│  3. IMPLEMENT the minimum code to pass the test            │
│  4. VERIFY with cargo check && cargo clippy && cargo test  │
│  5. REFACTOR if needed (tests still pass)                  │
│  6. COMMIT only after verification passes                  │
└─────────────────────────────────────────────────────────────┘
```

### Red-Green-Refactor Pattern

| Phase | Action | Verification |
|-------|--------|--------------|
| **Red** | Write failing test | `cargo test` shows failure |
| **Green** | Implement minimum code | `cargo test` passes |
| **Refactor** | Clean up code | `cargo test` still passes |

### Verification Gate (MANDATORY)

**Run this before EVERY commit:**

```bash
cargo check && cargo clippy --all-targets -- -D warnings && cargo test
```

| Check | Purpose | Failure Action |
|-------|---------|----------------|
| `cargo check` | Type errors, borrow checker | Fix compilation errors |
| `cargo clippy` | Lints, anti-patterns | Address warnings |
| `cargo test` | Unit + integration tests | Fix failing tests |

### Agent Session Workflow

```
Session Start:
  1. swarmmail_init(project_path, task_description)
  2. Query semantic memory for past learnings
  3. Load relevant skills (skills_list / skills_use)
  4. Reserve files with swarmmail_reserve()

During Work:
  5. Read → Test → Implement → Verify cycle
  6. Report progress at 25%, 50%, 75% with swarm_progress()
  7. Use swarm_checkpoint() before risky operations

Session End:
  8. Store learnings in semantic memory
  9. Complete with swarm_complete() (NOT hive_close)
```

### Progress Reporting

Report progress at key milestones:

```typescript
swarm_progress({
  project_key: "/Users/johnlindquist/dev/script-kit-gpui",
  agent_name: "your-agent-name",
  bead_id: "cell--xxxxx",
  status: "in_progress",  // or "blocked", "completed"
  progress_percent: 50,
  message: "Completed X, now working on Y",
  files_touched: ["src/main.rs", "src/theme.rs"]
})
```

---

## 14. Testing Infrastructure

### Test Directory Structure

```
tests/
├── smoke/                    # End-to-end integration tests
│   ├── hello-world.ts        # Basic sanity check
│   ├── hello-world-args.ts   # Interactive prompts
│   ├── test-window-reset.ts  # Window state reset
│   ├── test-process-cleanup.ts
│   └── README.md
├── sdk/                      # Individual SDK method tests
│   ├── test-arg.ts           # arg() prompt tests
│   ├── test-div.ts           # div() display tests
│   ├── test-editor.ts        # editor() tests
│   ├── test-fields.ts        # fields() form tests
│   ├── test-hotkey.ts        # hotkey() capture tests
│   └── README.md
```

### Test Types

| Type | Location | Purpose | Run Command |
|------|----------|---------|-------------|
| **Smoke Tests** | `tests/smoke/` | Full E2E flows | `./target/debug/script-kit-gpui tests/smoke/hello-world.ts` |
| **SDK Tests** | `tests/sdk/` | Individual API methods | `bun run tests/sdk/test-arg.ts` |
| **Rust Unit Tests** | `src/*.rs` | Internal Rust functions | `cargo test` |

### SDK Preload Pattern

All test scripts import the SDK for global functions:

```typescript
// At the top of every test file
import '../../scripts/kit-sdk';

// This makes these globals available:
// - arg(placeholder, choices) -> Promise<string>
// - div(html, tailwind?) -> Promise<void>
// - md(markdown) -> string
// - editor(content?, language?) -> Promise<string>
// - fields(fieldDefs) -> Promise<string[]>
// ... and more
```

### Test Output Format (JSONL)

Tests output structured JSONL for machine parsing:

```json
{"test": "arg-string-choices", "status": "running", "timestamp": "2024-..."}
{"test": "arg-string-choices", "status": "pass", "result": "Apple", "duration_ms": 45}
```

| Status | Meaning |
|--------|---------|
| `running` | Test started |
| `pass` | Test completed successfully |
| `fail` | Test failed (includes `error` field) |
| `skip` | Test skipped (includes `reason` field) |

### Writing New Tests

Follow this pattern for SDK tests:

```typescript
// Name: SDK Test - myFunction()
// Description: Tests myFunction() behavior

import '../../scripts/kit-sdk';

interface TestResult {
  test: string;
  status: 'running' | 'pass' | 'fail' | 'skip';
  timestamp: string;
  result?: unknown;
  error?: string;
  duration_ms?: number;
}

function logTest(name: string, status: TestResult['status'], extra?: Partial<TestResult>) {
  const result: TestResult = {
    test: name,
    status,
    timestamp: new Date().toISOString(),
    ...extra
  };
  console.log(JSON.stringify(result));
}

// Test implementation
const testName = 'my-function-basic';
logTest(testName, 'running');
const start = Date.now();

try {
  const result = await myFunction('input');
  
  if (result === expectedValue) {
    logTest(testName, 'pass', { result, duration_ms: Date.now() - start });
  } else {
    logTest(testName, 'fail', { 
      error: `Expected "${expectedValue}", got "${result}"`,
      duration_ms: Date.now() - start 
    });
  }
} catch (err) {
  logTest(testName, 'fail', { error: String(err), duration_ms: Date.now() - start });
}
```

### Running Tests

```bash
# Run all SDK tests
bun run scripts/test-runner.ts

# Run single SDK test
bun run scripts/test-runner.ts tests/sdk/test-arg.ts

# Run with full GPUI integration
cargo build && ./target/debug/script-kit-gpui tests/sdk/test-arg.ts

# Run smoke tests
./target/debug/script-kit-gpui tests/smoke/hello-world.ts

# Run Rust unit tests
cargo test

# Performance benchmark
npx tsx scripts/scroll-bench.ts
```

### Performance Thresholds

| Metric | Threshold | Test |
|--------|-----------|------|
| P95 Key Latency | < 50ms | `tests/sdk/test-scroll-perf.ts` |
| Single Key Event | < 16.67ms (60fps) | Manual profiling |
| Scroll Operation | < 8ms | `scripts/scroll-bench.ts` |

---

## 15. Hive/Beads Task Management

### Overview

The `.hive/` directory contains task tracking in JSONL format, designed for AI agent workflows.

```
.hive/
├── issues.jsonl     # Task tracking (epics, tasks, bugs)
└── memories.jsonl   # Semantic memory for learnings
```

### JSONL Format

Each line in `issues.jsonl` is a JSON object:

```json
{
  "id": "cell--9bnr5-mjjg2p0an0j",
  "title": "GPUI Script Kit PoC",
  "description": "Build a proof-of-concept...",
  "status": "closed",
  "priority": 1,
  "issue_type": "epic",
  "created_at": "2025-12-24T03:18:51.418Z",
  "updated_at": "2025-12-24T03:32:06.214Z",
  "closed_at": "2025-12-24T03:32:06.214Z",
  "parent_id": null,
  "dependencies": [],
  "labels": [],
  "comments": []
}
```

### Issue Types

| Type | Purpose |
|------|---------|
| `epic` | Large feature with subtasks |
| `task` | Individual work item |
| `bug` | Defect to fix |
| `feature` | New functionality |
| `chore` | Maintenance work |

### Status Values

| Status | Meaning |
|--------|---------|
| `open` | Not started |
| `in_progress` | Currently being worked on |
| `blocked` | Waiting on something |
| `closed` | Completed |

### Priority Levels

| Priority | Meaning |
|----------|---------|
| 0 | Critical - do first |
| 1 | High - important |
| 2 | Medium - normal |
| 3 | Low - nice to have |

### Bead Management Commands

**IMPORTANT: Use MCP tools, not CLI commands directly**

```typescript
// Query beads
hive_query({ status: "open", type: "task" })
hive_ready()  // Get next unblocked, highest priority

// Create beads
hive_create({ title: "Fix bug", type: "bug", priority: 1 })
hive_create_epic({ 
  epic_title: "New Feature",
  subtasks: [{ title: "Subtask 1", files: ["src/main.rs"] }]
})

// Update beads
hive_start({ id: "cell--xxxxx" })  // Mark as in_progress
hive_update({ id: "cell--xxxxx", status: "blocked", description: "Waiting for X" })

// Complete beads (MANDATORY pattern)
swarm_complete({
  project_key: "/path/to/project",
  agent_name: "worker-name",
  bead_id: "cell--xxxxx",
  summary: "What was accomplished",
  files_touched: ["src/main.rs"]
})  // NOT hive_close!
```

### Epic/Subtask Pattern

```typescript
// Create epic with subtasks
hive_create_epic({
  epic_title: "Add search functionality",
  epic_description: "Implement fuzzy search for script list",
  subtasks: [
    { title: "Add search input UI", files: ["src/main.rs"], priority: 0 },
    { title: "Implement fuzzy matching", files: ["src/scripts.rs"], priority: 1 },
    { title: "Add keyboard navigation", files: ["src/main.rs"], priority: 1 }
  ]
})
```

### Mandatory Bead Protocol for Agents

```
┌─────────────────────────────────────────────────────────────┐
│                    BEAD LIFECYCLE                           │
├─────────────────────────────────────────────────────────────┤
│  1. swarmmail_init()     - Register with coordination       │
│  2. hive_start(id)       - Mark bead as in_progress        │
│  3. swarm_progress()     - Report at 25/50/75%             │
│  4. swarm_complete()     - Close bead + release resources  │
├─────────────────────────────────────────────────────────────┤
│  ⚠️  NEVER use hive_close() directly - use swarm_complete() │
│      swarm_complete() handles: UBS scan, reservation        │
│      release, learning signals, coordinator notification    │
└─────────────────────────────────────────────────────────────┘
```

---

## 16. Agent Observability

### Correlation IDs

Every agent session should use correlation IDs to track related operations:

```rust
use uuid::Uuid;

fn handle_task(&mut self, task: Task, cx: &mut Context<Self>) {
    let correlation_id = Uuid::new_v4().to_string();
    
    let span = info_span!("agent_task", 
        correlation_id = %correlation_id,
        task_id = %task.id,
        task_type = %task.task_type
    );
    let _guard = span.enter();
    
    info!("Task started");
    // All nested logs will include correlation_id
    self.execute_task(&task)?;
    info!("Task completed");
}
```

### JSONL Log Format

Logs are written to `~/.kit/logs/script-kit-gpui.jsonl`:

```json
{"timestamp":"2024-01-15T10:30:45.123Z","level":"INFO","target":"script_kit::executor","message":"Script executed","fields":{"correlation_id":"abc-123","script_name":"hello.ts","duration_ms":142}}
```

### Log Queries for Agents

```bash
# Find all logs for a correlation ID
grep '"correlation_id":"abc-123"' ~/.kit/logs/script-kit-gpui.jsonl

# Find slow operations (>100ms)
grep '"duration_ms":' ~/.kit/logs/script-kit-gpui.jsonl | \
  jq 'select(.fields.duration_ms > 100)'

# Find errors in last hour
grep '"level":"ERROR"' ~/.kit/logs/script-kit-gpui.jsonl | \
  jq 'select(.timestamp > "2024-01-15T09:30:00")'

# Extract timing metrics
grep '"duration_ms":' ~/.kit/logs/script-kit-gpui.jsonl | \
  jq -r '.fields.duration_ms' | sort -n | tail -10
```

### Performance Monitoring

```rust
use std::time::Instant;
use tracing::{info, warn};

const SLOW_THRESHOLD_MS: u64 = 100;

fn monitored_operation(&self) {
    let start = Instant::now();
    
    // ... do work ...
    
    let duration_ms = start.elapsed().as_millis() as u64;
    
    if duration_ms > SLOW_THRESHOLD_MS {
        warn!(
            duration_ms,
            threshold_ms = SLOW_THRESHOLD_MS,
            operation = "operation_name",
            "Slow operation detected"
        );
    } else {
        info!(duration_ms, "Operation completed");
    }
}
```

### Required Log Fields for Agent Tracing

| Field | Purpose | Example |
|-------|---------|---------|
| `correlation_id` | Track related operations | `"abc-123-def"` |
| `duration_ms` | Performance tracking | `142` |
| `bead_id` | Link to task | `"cell--9bnr5-xxx"` |
| `agent_name` | Identify worker | `"worker-theme"` |
| `files_touched` | Track changes | `["src/main.rs"]` |

---

## 17. Agent Anti-Patterns and Gotchas

### Common Mistakes

| Anti-Pattern | Why It's Bad | Correct Approach |
|--------------|--------------|------------------|
| Skip `swarmmail_init()` | Work not tracked, completion fails | Always init first |
| Use `hive_close()` directly | Doesn't release reservations | Use `swarm_complete()` |
| Skip verification gate | Broken code gets committed | Run check/clippy/test before commit |
| Edit unreserved files | Causes merge conflicts | Reserve files with `swarmmail_reserve()` |
| No progress reports | Coordinator can't track work | Report at 25/50/75% |
| Skip TDD | Harder to verify correctness | Write failing test first |
| Hardcode correlation_id | Can't trace operations | Generate UUID per session |
| Ignore blocked status | Work on wrong priorities | Use `hive_ready()` for next task |

### File Reservation Protocol

```typescript
// CORRECT: Reserve before editing
swarmmail_reserve({
  paths: ["src/main.rs", "src/theme.rs"],
  reason: "cell--xxxxx: Implement feature X",
  exclusive: true
})

// Work on files...

// Release happens automatically via swarm_complete()

// WRONG: Edit without reservation
// This causes conflicts if other agents are editing the same files!
```

### When Blocked

```typescript
// Report block immediately
swarmmail_send({
  to: ["coordinator"],
  subject: "BLOCKED: cell--xxxxx",
  body: "Cannot proceed because: <specific reason>",
  importance: "high"
})

// Update bead status
hive_update({
  id: "cell--xxxxx",
  status: "blocked",
  description: "Blocked on: <reason>"
})

// Wait for coordinator response before continuing
```

### Scope Change Protocol

If you discover additional work needed:

```typescript
// DON'T just expand scope - request first
swarmmail_send({
  to: ["coordinator"],
  subject: "Scope change request: cell--xxxxx",
  body: "Original task was X. Found that Y is also needed. Request permission to expand scope.",
  importance: "high"
})

// Wait for approval before expanding beyond files_owned
```

### Pre-Commit Checklist

```
Before every commit, verify:
□ cargo check passes
□ cargo clippy --all-targets -- -D warnings passes
□ cargo test passes
□ Only reserved files were modified
□ Bead status updated (in_progress → completed)
□ Progress reported at milestones
□ Correlation ID in relevant log entries
□ Tests written for new functionality
```

---

## References

- **GPUI Docs**: https://docs.rs/gpui/latest/gpui/
- **Zed Source**: https://github.com/zed-industries/zed/tree/main/crates/gpui
- **Project Research**: See `GPUI_RESEARCH.md`, `GPUI_IMPROVEMENTS_REPORT.md`

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd sync
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
