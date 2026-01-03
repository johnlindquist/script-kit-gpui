# Responsive & Resize Behavior Audit

**Auditor:** ResizeAuditor  
**Date:** 2025-12-29  
**Scope:** Window resizing, content-driven sizing, responsive patterns  
**Files Analyzed:**
- `src/window_resize.rs` (296 lines)
- `src/prompts.rs` (530 lines) 
- `src/editor.rs` (1276 lines)
- `src/term_prompt.rs` (1013 lines)
- `src/config.rs` (1158 lines)
- `src/main.rs` (resize-related sections)

---

## Executive Summary

Script Kit GPUI uses a **fixed-tier height system** for window resizing rather than fully dynamic content-driven sizing. Three height tiers (120px, 500px, 700px) cover all view types. This approach prioritizes **predictability and performance** over pixel-perfect content fit. Terminal and editor prompts implement **dynamic internal resizing** based on available space.

**Overall Assessment:** ✅ Well-architected | ⚠️ Some edge cases need attention

---

## 1. Content-Driven Sizing Architecture

### 1.1 Height Tier System

The application uses a three-tier fixed height system defined in `src/window_resize.rs`:

| Constant | Value | Purpose |
|----------|-------|---------|
| `MIN_HEIGHT` | 120px | Input-only prompts (no choices) |
| `STANDARD_HEIGHT` | 500px | Script list, arg prompts with choices, div prompts |
| `MAX_HEIGHT` | 700px | Editor, terminal (full-content views) |

**Code Reference:**
```rust
// src/window_resize.rs:24-36
pub mod layout {
    pub const MIN_HEIGHT: Pixels = px(120.0);
    pub const STANDARD_HEIGHT: Pixels = px(500.0);
    pub const MAX_HEIGHT: Pixels = px(700.0);
}
```

### 1.2 View Type Classification

The `ViewType` enum maps views to height tiers:

| ViewType | Height Tier | Rationale |
|----------|-------------|-----------|
| `ScriptList` | STANDARD (500px) | Preview panel needs space |
| `ArgPromptWithChoices` | STANDARD (500px) | List + preview |
| `ArgPromptNoChoices` | MIN (120px) | Input field only |
| `DivPrompt` | STANDARD (500px) | HTML content display |
| `EditorPrompt` | MAX (700px) | Code editing needs vertical space |
| `TermPrompt` | MAX (700px) | Terminal output scrollback |

### 1.3 Height Calculation Logic

**Key Function: `height_for_view()`**

```rust
// src/window_resize.rs:63-98
pub fn height_for_view(view_type: ViewType, _item_count: usize) -> Pixels {
    let height = match view_type {
        ViewType::ScriptList | ViewType::ArgPromptWithChoices | ViewType::DivPrompt => {
            STANDARD_HEIGHT
        }
        ViewType::ArgPromptNoChoices => {
            MIN_HEIGHT
        }
        ViewType::EditorPrompt | ViewType::TermPrompt => {
            MAX_HEIGHT
        }
    };
    height
}
```

**Design Decision:** The `_item_count` parameter is intentionally **unused**. Early iterations attempted dynamic height based on list item count, but this was abandoned in favor of fixed tiers. This decision:
- ✅ Prevents jarring height changes during filtering
- ✅ Simplifies window animation
- ⚠️ May waste space for small lists

---

## 2. Resize Debouncing & Performance

### 2.1 Current Debounce Implementation

**Key Finding:** Resize debouncing is **disabled** (no-op function):

```rust
// src/window_resize.rs:141-144
pub fn reset_resize_debounce() {
    // No-op - we removed debouncing since resizes are now rare
}
```

**Rationale:** Since heights are fixed tiers (not dynamic), resize events are:
- Infrequent (only on view type change)
- Predictable (same target height each time)
- Unlikely to cause rapid-fire events

### 2.2 Deferred Resize Pattern

To avoid RefCell borrow conflicts during GPUI's render cycle, resizes are **deferred by one frame**:

```rust
// src/window_resize.rs:105-139
pub fn defer_resize_to_view<T: Render>(view_type: ViewType, item_count: usize, cx: &mut Context<T>) {
    let target_height = height_for_view(view_type, item_count);
    
    cx.spawn(async move |_this, _cx| {
        // 16ms delay (~1 frame at 60fps) ensures GPUI render cycle completes
        Timer::after(Duration::from_millis(16)).await;
        
        if window_manager::get_main_window().is_some() {
            resize_first_window_to_height(target_height);
        }
    })
    .detach();
}
```

**Performance Characteristics:**
- ✅ 16ms delay matches 60fps frame time
- ✅ Window existence check prevents crashes
- ⚠️ Detached spawn means no error propagation

### 2.3 Height Optimization (Skip Redundant Resizes)

```rust
// src/window_resize.rs:174-187
// Skip if height is already correct (within 1px tolerance)
if (current_height - height_f64).abs() < 1.0 {
    // Skip resize
    return;
}
```

---

## 3. Min/Max Constraints

### 3.1 Window Height Constraints

| Constraint | Value | Enforced By |
|------------|-------|-------------|
| Minimum Window | 120px | `MIN_HEIGHT` constant |
| Maximum Window | 700px | `MAX_HEIGHT` constant |
| No intermediate | N/A | Fixed tier system |

### 3.2 Terminal-Specific Constraints

The terminal implements its own min size constraints:

```rust
// src/term_prompt.rs:44-45
const MIN_COLS: u16 = 20;
const MIN_ROWS: u16 = 5;
```

**Grid-Based Sizing:**
```rust
// src/term_prompt.rs:152-170
fn calculate_terminal_size_with_cells(...) -> (u16, u16) {
    let available_width = f32::from(width) - padding_left - padding_right;
    let available_height = f32::from(height) - padding_top - padding_bottom;
    
    let cols = (available_width / cell_width).floor() as u16;
    let rows = (available_height / cell_height).floor() as u16;
    
    let cols = cols.max(MIN_COLS);
    let rows = rows.max(MIN_ROWS);
    
    (cols, rows)
}
```

### 3.3 Content Padding Configuration

Padding is configurable via `~/.sk/kit/config.ts`:

```rust
// src/config.rs:6-9
pub const DEFAULT_PADDING_TOP: f32 = 8.0;
pub const DEFAULT_PADDING_LEFT: f32 = 12.0;
pub const DEFAULT_PADDING_RIGHT: f32 = 12.0;
```

**Usage Pattern:**
```rust
let padding = self.config.get_padding();
// padding.top, padding.left, padding.right
```

---

## 4. Responsive Patterns

### 4.1 Flexbox Layout System

All prompts use GPUI's flexbox-based layout:

```rust
// Common pattern from src/prompts.rs:381-399
div()
    .id(gpui::ElementId::Name("window:arg".into()))
    .flex()
    .flex_col()
    .w_full()
    .h_full()            // Fill container height
    .min_h(px(0.))       // Allow proper flex behavior
    .bg(main_bg)
    // ...
```

**Key Patterns:**
| Pattern | Purpose |
|---------|---------|
| `.flex().flex_col()` | Vertical stacking |
| `.h_full().min_h(px(0.))` | Fill parent, allow shrinking |
| `.flex_1()` | Grow to fill remaining space |
| `.overflow_hidden()` | Clip content at boundary |

### 4.2 GPUI Entity Height Limitation

**Critical Design Constraint:** GPUI entities don't inherit parent flex sizing. Components must receive **explicit heights**:

```rust
// src/editor.rs:169-178
pub fn with_height(
    id: String,
    content: String,
    // ...
    content_height: Option<gpui::Pixels>,  // Explicit height parameter
) -> Self {
    // ...
    content_height,
}
```

**Workaround Pattern:**
```rust
// src/main.rs:5541-5559 (TermPrompt wrapper)
let content_height = window_resize::layout::MAX_HEIGHT;

div()
    .flex()
    .flex_col()
    .w_full()
    .h(content_height)  // Explicit height on wrapper
    .overflow_hidden()
    .child(
        div()
            .size_full()
            .child(entity)  // Entity inside sized wrapper
    )
```

### 4.3 Truncation vs Wrapping

| Component | Behavior | Implementation |
|-----------|----------|----------------|
| Script list items | Truncated | Fixed 52px item height |
| Editor lines | Horizontal scroll | `.overflow_hidden()` on line div |
| Terminal | Character grid | Exact col/row calculation |
| Div content | Vertical scroll | `.flex_1()` + `.overflow_y_hidden()` |

### 4.4 Scroll Appearance

The application uses `uniform_list` for virtualized scrolling:

```rust
// src/editor.rs:1113-1122
uniform_list(
    "editor-lines",
    line_count,
    cx.processor(|this, range: Range<usize>, _window, cx| {
        this.render_lines(range, cx)
    }),
)
.track_scroll(&self.scroll_handle)
.size_full()
```

**Scroll Behavior:**
- ✅ Virtualized (only visible items rendered)
- ✅ Smooth scrolling via `UniformListScrollHandle`
- ⚠️ No visible scrollbar styling (uses native)

---

## 5. Component-Specific Sizing

### 5.1 Editor Sizing

**Font-Based Calculations:**
```rust
// src/editor.rs:34-40
const BASE_CHAR_WIDTH: f32 = 8.4;
const BASE_FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT_MULTIPLIER: f32 = 1.43;

fn line_height(&self) -> f32 {
    self.font_size() * LINE_HEIGHT_MULTIPLIER  // 20px for 14pt font
}
```

**Status Bar:**
- Fixed 28px height
- Deducted from content area

### 5.2 Terminal Sizing

**Cell-Based Calculations:**
```rust
// src/term_prompt.rs:29-32
const BASE_CELL_WIDTH: f32 = 8.5;   // Conservative for Menlo 14pt
const BASE_CELL_HEIGHT: f32 = BASE_FONT_SIZE * LINE_HEIGHT_MULTIPLIER;  // 18.2px
```

**Dynamic Resize on Window Change:**
```rust
// src/term_prompt.rs:444-446 (in render method)
let window_bounds = window.bounds();
self.resize_if_needed(window_bounds.size.width, window_bounds.size.height);
```

### 5.3 Arg Prompt Sizing

**Choice Item Height:**
- Uses `flex_1()` to fill available space
- No fixed per-item height (variable based on description)

### 5.4 Div Prompt Sizing

**Content Area:**
```rust
// src/prompts.rs:505-526
div()
    .id(gpui::ElementId::Name("window:div".into()))
    .flex()
    .flex_col()
    .w_full()
    .h_full()
    .child(
        div()
            .flex_1()            // Grow to fill
            .min_h(px(0.))       // Allow shrinking
            .overflow_y_hidden() // Clip overflow
            .child(display_text)
    )
```

---

## 6. Performance Analysis

### 6.1 Layout Recalculation Frequency

| Trigger | Frequency | Impact |
|---------|-----------|--------|
| View type change | Low | Single layout pass |
| Filter text change | High | List filtering only, no resize |
| Terminal output | 30fps | Optimized refresh timer |
| Window resize by user | Rare | Full layout recalc |

### 6.2 Resize Event Coalescing

**Terminal Refresh Rate:**
```rust
// src/term_prompt.rs:41
const REFRESH_INTERVAL_MS: u64 = 33;  // ~30fps
```

**Scroll Event Coalescing:**
The logging module has infrastructure for this:
```rust
// src/logging.rs:1001-1016
pub fn log_scroll_batch(batch_size: usize, coalesced_from: usize) {
    // Tracks when multiple scroll events are coalesced
}
```

### 6.3 60fps Target

**Slow Render Detection:**
```rust
// src/term_prompt.rs:18
const SLOW_RENDER_THRESHOLD_MS: u128 = 16;  // 60fps threshold

// Usage in render():
if elapsed > SLOW_RENDER_THRESHOLD_MS {
    warn!(elapsed_ms = elapsed, "Slow terminal render");
}
```

---

## 7. Known Issues & Regression Tests

### 7.1 Padding Symmetry Bug (FIXED)

A regression test exists to prevent reintroduction of a bug where only top padding was subtracted from available height:

```rust
// src/term_prompt.rs:809-857
#[test]
fn test_padding_symmetry_regression_top_and_bottom_must_both_be_subtracted() {
    // Ensures both top AND bottom padding are subtracted from available height
    // Without this, content would be cut off at the bottom
}
```

### 7.2 Arrow Key Matching (CRITICAL)

```rust
// src/editor.rs:1234-1274
#[test]
fn test_arrow_key_patterns_match_both_forms() {
    // GPUI sends "up" or "arrowup" depending on platform
    // Must match BOTH: "up" | "arrowup"
}
```

---

## 8. Configuration Options

### 8.1 User-Configurable Settings

Via `~/.sk/kit/config.ts`:

```typescript
export default {
  padding: {
    top: 8,      // Default: 8
    left: 12,    // Default: 12
    right: 12    // Default: 12
  },
  editorFontSize: 16,      // Default: 14
  terminalFontSize: 14,    // Default: 14
  uiScale: 1.0,            // Default: 1.0
} satisfies Config;
```

### 8.2 Runtime Access

```rust
// In components:
let padding = self.config.get_padding();
let font_size = self.config.get_editor_font_size();
```

---

## 9. Recommendations

### 9.1 Potential Improvements

| Issue | Recommendation | Priority |
|-------|----------------|----------|
| No aspect ratio lock | Consider for screenshot features | Low |
| Fixed height tiers | Could add content-aware for DivPrompt | Medium |
| No max-width constraint | 500px implicit, but could overflow | Low |
| Terminal resize on every render | Cache window bounds | Low |

### 9.2 Test Coverage

| Area | Coverage | Recommendation |
|------|----------|----------------|
| Height tiers | ✅ Full | Maintain |
| Terminal sizing | ✅ Extensive | Good coverage |
| Padding symmetry | ✅ Regression tests | Critical to keep |
| Editor height | ⚠️ Indirect | Add explicit tests |
| DivPrompt overflow | ❌ Missing | Add visual tests |

### 9.3 Documentation Gaps

- Width constraints not documented (assumed 500px)
- Window positioning vs sizing relationship unclear
- Multi-monitor resize behavior needs clarification

---

## 10. Architecture Summary

```
┌─────────────────────────────────────────────────────────────────┐
│                     Window Resize Flow                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  View Change    ──►  update_window_size()                       │
│                           │                                     │
│                           ▼                                     │
│                   height_for_view(ViewType)                     │
│                           │                                     │
│                           ▼                                     │
│              ┌────────────┴────────────┐                       │
│              │    Fixed Height Tiers    │                       │
│              ├──────────────────────────┤                       │
│              │  MIN: 120px (input only) │                       │
│              │  STD: 500px (with list)  │                       │
│              │  MAX: 700px (editor/term)│                       │
│              └──────────────────────────┘                       │
│                           │                                     │
│                           ▼                                     │
│              defer_resize_to_view()                             │
│                           │                                     │
│                   (16ms delay)                                  │
│                           │                                     │
│                           ▼                                     │
│              resize_first_window_to_height()                    │
│                           │                                     │
│              (Skip if within 1px tolerance)                     │
│                           │                                     │
│                           ▼                                     │
│              Native macOS setFrame:display:animate:             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Appendix A: Height Tier Usage Matrix

| Caller | ViewType | Height |
|--------|----------|--------|
| Script list load | `ScriptList` | 500px |
| `arg()` with choices | `ArgPromptWithChoices` | 500px |
| `arg()` without choices | `ArgPromptNoChoices` | 120px |
| `div()` | `DivPrompt` | 500px |
| `editor()` | `EditorPrompt` | 700px |
| `term()` | `TermPrompt` | 700px |
| Clipboard History | `ScriptList` | 500px |
| App Launcher | `ScriptList` | 500px |
| Window Switcher | `ScriptList` | 500px |

---

## Appendix B: Related AGENTS.md References

From `AGENTS.md`:
- "Resize debouncing important for performance"
- "Content-driven sizing needs accurate height_for_view"
- "Use `height_for_view(ViewType, item_count)` for window sizing"

---

*Audit completed: 2025-12-29*
