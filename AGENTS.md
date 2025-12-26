# Script Kit GPUI - Agent Knowledge Base

> **Auto-loaded at conversation start.** This file contains project-specific GPUI patterns, gotchas, and best practices discovered during development.

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
  main.rs       # App entry, window setup, main render loop
  theme.rs      # Theme system, ColorScheme, focus-aware colors
  prompts.rs    # ArgPrompt, DivPrompt interactive prompts
  actions.rs    # ActionsDialog popup
  protocol.rs   # JSON message protocol
  scripts.rs    # Script loading and execution
  panel.rs      # macOS panel configuration
  perf.rs       # Performance timing utilities
  logging.rs    # Log functions
  utils.rs      # Shared utilities (strip_html_tags, etc.)
```

---

## References

- **GPUI Docs**: https://docs.rs/gpui/latest/gpui/
- **Zed Source**: https://github.com/zed-industries/zed/tree/main/crates/gpui
- **Project Research**: See `GPUI_RESEARCH.md`, `GPUI_IMPROVEMENTS_REPORT.md`
