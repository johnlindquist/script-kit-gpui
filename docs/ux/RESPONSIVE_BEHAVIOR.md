# Responsive & Resize Behavior Audit

**Audit Date:** December 2024  
**Scope:** Window resize handling, content reflow, dynamic height calculations, terminal/editor sizing  
**Status:** Comprehensive Analysis Complete

---

## Executive Summary

Script Kit GPUI implements a **fixed-width, dynamic-height** window model optimized for a launcher-style UI. The window width is constant (750px), while height adjusts based on the current view type. This approach prioritizes:

1. **Consistency** - Predictable window placement and appearance
2. **Speed** - Minimal resize operations for better performance
3. **User Experience** - Content-aware sizing that matches Raycast/Alfred patterns

### Key Findings

| Aspect | Status | Notes |
|--------|--------|-------|
| View-based height | Implemented | Three fixed heights: 120px, 500px, 700px |
| Resize debouncing | Removed | Resizes are now rare enough that debouncing is unnecessary |
| Terminal sizing | Implemented | Dynamic grid calculation with padding-aware rows/cols |
| Editor height | Implemented | Explicit height passing due to GPUI entity constraints |
| Animation smoothing | Not implemented | Instant resizes via `animate:false` |
| Multi-monitor | Implemented | Eye-line positioning on cursor's display |

---

## 1. Window Resize Architecture

### 1.1 Core Module: `src/window_resize.rs`

The resize system uses a **view-type-based approach** rather than content-driven dynamic sizing:

```rust
/// View types for height calculation
pub enum ViewType {
    ScriptList,           // Fixed 500px - has preview panel
    ArgPromptWithChoices, // Fixed 500px - has preview panel
    ArgPromptNoChoices,   // Compact 120px - input only
    DivPrompt,            // Standard 500px - matches main window
    EditorPrompt,         // Full 700px - code editing
    TermPrompt,           // Full 700px - terminal output
}
```

### 1.2 Height Constants

| Constant | Value | Usage |
|----------|-------|-------|
| `MIN_HEIGHT` | 120px | Input-only prompts (ArgPromptNoChoices) |
| `STANDARD_HEIGHT` | 500px | Script list, arg with choices, div prompts |
| `MAX_HEIGHT` | 700px | Editor, terminal prompts |

### 1.3 Resize Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    Resize Decision Flow                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  View Change (e.g., script list → editor)                       │
│       │                                                          │
│       ▼                                                          │
│  update_window_size() in ScriptListApp                          │
│       │                                                          │
│       ▼                                                          │
│  height_for_view(ViewType, item_count) → Pixels                 │
│       │                                                          │
│       ▼                                                          │
│  resize_first_window_to_height(target_height)                   │
│       │                                                          │
│       ▼                                                          │
│  Native macOS setFrame:display:animate: (instant, no animation) │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 1.4 Deferred Resize Pattern

To avoid GPUI RefCell borrow conflicts during render cycles, resizes are deferred:

```rust
pub fn defer_resize_to_view<T: Render>(view_type: ViewType, item_count: usize, cx: &mut Context<T>) {
    let target_height = height_for_view(view_type, item_count);
    
    cx.spawn(async move |_this, _cx| {
        // 16ms delay (~1 frame at 60fps) ensures GPUI render cycle completes
        Timer::after(Duration::from_millis(16)).await;
        
        if window_manager::get_main_window().is_some() {
            resize_first_window_to_height(target_height);
        }
    }).detach();
}
```

**Key Insight:** The 16ms delay is strategically chosen to align with a single frame at 60fps, preventing borrow conflicts while maintaining perceived responsiveness.

---

## 2. Top-Edge Fixed Positioning

### 2.1 macOS Coordinate System Handling

The resize implementation keeps the **top edge fixed** while the window grows/shrinks from the bottom:

```rust
// macOS coordinate system: Y=0 at bottom, increases upward
// To keep the TOP of the window fixed, adjust origin.y
let height_delta = height_f64 - current_height;
let new_origin_y = current_frame.origin.y - height_delta;

let new_frame = NSRect::new(
    NSPoint::new(current_frame.origin.x, new_origin_y),
    NSSize::new(current_frame.size.width, height_f64),
);
```

**Why This Matters:** Launcher apps like Raycast and Alfred maintain top-edge stability because users visually anchor to the search input at the top. If the top moved, it would be disorienting.

### 2.2 Skip Redundant Resizes

An optimization prevents unnecessary resize operations:

```rust
// Skip if height is already correct (within 1px tolerance)
if (current_height - height_f64).abs() < 1.0 {
    return;
}
```

---

## 3. Content Reflow Behavior

### 3.1 Script List

The script list uses `uniform_list` for virtualized rendering:

- **Fixed item height:** 40px (`LIST_ITEM_HEIGHT`)
- **Section headers:** 24px (`SECTION_HEADER_HEIGHT`)
- **Window height:** Always 500px (STANDARD_HEIGHT)
- **Scroll handling:** `UniformListScrollHandle` with `ScrollStrategy::Nearest`

```rust
// From main.rs - scroll to selected item
self.list_scroll_handle.scroll_to_item(target, ScrollStrategy::Nearest);
```

### 3.2 Arg Prompt

The arg prompt list mirrors script list behavior:

- Uses same `uniform_list` pattern
- Separate scroll handle: `arg_list_scroll_handle`
- Choices container: `flex_1()` to fill available space
- Overflow handling: `overflow_y_hidden()` clips at boundary

### 3.3 Div Prompt

Content display without list virtualization:

```rust
div()
    .h_full()           // Fill container height completely
    .min_h(px(0.))      // Allow proper flex behavior
    .child(content)     // Single content child
```

---

## 4. Dynamic Height Calculations

### 4.1 height_for_view Function

The core height calculation is intentionally simple:

```rust
pub fn height_for_view(view_type: ViewType, _item_count: usize) -> Pixels {
    match view_type {
        ViewType::ScriptList | ViewType::ArgPromptWithChoices | ViewType::DivPrompt => {
            STANDARD_HEIGHT  // 500px
        }
        ViewType::ArgPromptNoChoices => {
            MIN_HEIGHT  // 120px
        }
        ViewType::EditorPrompt | ViewType::TermPrompt => {
            MAX_HEIGHT  // 700px
        }
    }
}
```

**Design Decision:** The `_item_count` parameter is intentionally unused. Earlier iterations attempted content-based sizing, but this was abandoned for simplicity and predictability.

### 4.2 Window Size Update Trigger Points

```rust
// In ScriptListApp
fn update_window_size(&self) {
    let (view_type, item_count) = match &self.current_view {
        AppView::ScriptList => { /* ... */ (ViewType::ScriptList, count) }
        AppView::ArgPrompt { choices, .. } => {
            if filtered.is_empty() && choices.is_empty() {
                (ViewType::ArgPromptNoChoices, 0)
            } else {
                (ViewType::ArgPromptWithChoices, filtered.len())
            }
        }
        AppView::EditorPrompt { .. } => (ViewType::EditorPrompt, 0),
        AppView::TermPrompt { .. } => (ViewType::TermPrompt, 0),
        // ... other views
    };
    
    let target_height = height_for_view(view_type, item_count);
    resize_first_window_to_height(target_height);
}
```

---

## 5. List Item Density

### 5.1 Fixed Density Model

List items use fixed heights regardless of window size:

| Component | Height | Rationale |
|-----------|--------|-----------|
| `LIST_ITEM_HEIGHT` | 40px | Balances compact layout with touch targets |
| `SECTION_HEADER_HEIGHT` | 24px | Minimal but readable |
| Item padding | 6px vertical, 12px horizontal | Comfortable spacing |

### 5.2 No Responsive Density

There is **no dynamic density adjustment** based on window size. This is intentional:

- **Consistency:** Items look the same regardless of context
- **Performance:** No recalculation needed
- **Predictability:** Users develop muscle memory for item positions

### 5.3 Potential Enhancement

A future enhancement could add density levels:

```rust
enum ItemDensity {
    Compact,  // 32px height
    Default,  // 40px height (current)
    Relaxed,  // 52px height
}
```

This would be a **config option**, not a responsive behavior.

---

## 6. Editor Height Handling

### 6.1 GPUI Entity Height Constraint

GPUI entities don't inherit parent flex sizing. The editor must receive explicit height:

```rust
/// Create a new EditorPrompt with explicit height
/// 
/// This is necessary because GPUI entities don't inherit parent flex sizing.
/// When rendered as a child of a sized container, h_full() doesn't resolve
/// to the parent's height. We must pass an explicit height.
pub fn with_height(
    id: String,
    content: String,
    language: String,
    focus_handle: FocusHandle,
    on_submit: SubmitCallback,
    theme: Arc<Theme>,
    config: Arc<Config>,
    content_height: Option<gpui::Pixels>,
) -> Self
```

### 6.2 Editor Area Calculation

```rust
// Status bar height constant
const STATUS_BAR_HEIGHT: f32 = 28.0;

// Calculate editor area height: use explicit height if available
let editor_area = if let Some(total_height) = self.content_height {
    let editor_height = total_height - gpui::px(STATUS_BAR_HEIGHT);
    // ... render with explicit height
} else {
    // Fallback: use flex (may not work in all GPUI contexts)
    // ... render with flex_1()
};
```

### 6.3 Font-Based Line Height

Editor line height scales with configured font size:

```rust
const LINE_HEIGHT_MULTIPLIER: f32 = 1.43; // 20/14 ≈ 1.43

fn line_height(&self) -> f32 {
    self.font_size() * LINE_HEIGHT_MULTIPLIER
}
```

---

## 7. Terminal Sizing

### 7.1 Cell Dimension Calculations

Terminal sizing is the most complex due to grid-based rendering:

```rust
/// Base font size for calculating ratios
const BASE_FONT_SIZE: f32 = 14.0;
/// Line height multiplier - 1.3 provides room for descenders
const LINE_HEIGHT_MULTIPLIER: f32 = 1.3;
/// Cell width for Menlo 14pt (conservative value)
const BASE_CELL_WIDTH: f32 = 8.5;  // Actual: 8.4287px
```

### 7.2 Terminal Size Calculation

```rust
fn calculate_terminal_size_with_cells(
    width: Pixels, 
    height: Pixels, 
    padding_left: f32, padding_right: f32, 
    padding_top: f32, padding_bottom: f32, 
    cell_width: f32, cell_height: f32
) -> (u16, u16) {
    // Subtract padding from available space
    let available_width = f32::from(width) - padding_left - padding_right;
    let available_height = f32::from(height) - padding_top - padding_bottom;
    
    // Use floor() to prevent last character wrapping
    let cols = (available_width / cell_width).floor() as u16;
    let rows = (available_height / cell_height).floor() as u16;
    
    // Apply minimum bounds
    (cols.max(MIN_COLS), rows.max(MIN_ROWS))
}
```

### 7.3 Padding Symmetry (Bug Fix Documented)

A previous bug only subtracted `padding_top` but not `padding_bottom`, causing content cutoff:

```rust
// FIXED calculation (subtracts both):
let available_height = height - padding_top - padding_bottom;

// BUGGY calculation (only subtracted top):
// let available_height = height - padding_top;  // WRONG!
```

The fix includes regression tests to prevent recurrence.

### 7.4 Dynamic Resize on Window Change

```rust
fn resize_if_needed(&mut self, width: Pixels, height: Pixels) {
    let (new_cols, new_rows) = Self::calculate_terminal_size_with_cells(
        width, height, 
        padding.left, padding.right, 
        padding.top, padding.top,  // Same padding top/bottom
        cell_width, cell_height
    );
    
    if (new_cols, new_rows) != self.last_size {
        if let Err(e) = self.terminal.resize(new_cols, new_rows) {
            warn!(error = %e, "Failed to resize terminal");
        } else {
            self.last_size = (new_cols, new_rows);
        }
    }
}
```

---

## 8. Resize Animation (Not Implemented)

### 8.1 Current State

Window resizes are **instant** with no animation:

```rust
let _: () = msg_send![window, setFrame:new_frame display:true animate:false];
```

### 8.2 Why No Animation

1. **Speed Priority:** Launcher apps should feel instant
2. **Frequency:** Resizes are rare (only on view type change)
3. **Simplicity:** Animation requires timing coordination

### 8.3 Potential Future Enhancement

If animation is desired, macOS provides built-in support:

```rust
// NOT IMPLEMENTED - for reference only
let _: () = msg_send![window, setFrame:new_frame display:true animate:true];
```

Animation duration would be ~0.25s (macOS default). This could be added as a config option.

---

## 9. Resize Event Debouncing

### 9.1 Current State: Removed

The debouncing mechanism was removed because resizes are now rare:

```rust
/// Force reset the debounce timer (kept for API compatibility)
pub fn reset_resize_debounce() {
    // No-op - we removed debouncing since resizes are now rare
}
```

### 9.2 Historical Context

Earlier implementations had more frequent resizes (e.g., per-keystroke), requiring debouncing. The current view-type-based approach eliminated this need.

### 9.3 Recommendation

If content-based dynamic sizing is re-introduced, debouncing should be re-implemented:

```rust
// Recommended debounce pattern (not currently implemented)
const RESIZE_DEBOUNCE_MS: u64 = 100;

struct ResizeDebouncer {
    last_resize: Instant,
    pending: Option<Pixels>,
}
```

---

## 10. Multi-Monitor Window Positioning

### 10.1 Eye-Line Positioning

Window appears at "eye-line" height (upper 14% of screen) on the display containing the cursor:

```rust
fn calculate_eye_line_bounds_on_mouse_display(
    window_size: gpui::Size<Pixels>,
    _cx: &App,
) -> Bounds<Pixels> {
    let displays = get_macos_displays();
    
    // Find display containing mouse cursor
    let target_display = if let Some((mouse_x, mouse_y)) = get_global_mouse_position() {
        displays.iter().find(|display| {
            mouse_x >= display.origin_x && mouse_x < display.origin_x + display.width &&
            mouse_y >= display.origin_y && mouse_y < display.origin_y + display.height
        }).cloned()
    } else {
        None
    };
    
    // Position at eye-line (upper 14%)
    let eye_line_y = display.origin_y + display.height * 0.14;
    let center_x = display.origin_x + (display.width - window_width) / 2.0;
    
    Bounds { origin: point(px(center_x), px(eye_line_y)), size: window_size }
}
```

### 10.2 Display Detection

Uses native macOS APIs (NSScreen) because GPUI's display bounds have incorrect origins for secondary displays:

```rust
fn get_macos_displays() -> Vec<DisplayBounds> {
    unsafe {
        let screens: id = msg_send![class!(NSScreen), screens];
        // ... iterate screens, convert coordinates
    }
}
```

---

## 11. Minimum/Maximum Window Sizes

### 11.1 Current Constraints

| Dimension | Value | Enforced By |
|-----------|-------|-------------|
| Width | 750px | Fixed in code |
| Min Height | 120px | `layout::MIN_HEIGHT` |
| Max Height | 700px | `layout::MAX_HEIGHT` |

### 11.2 No OS-Level Constraints

Window constraints are not set at the macOS level. The app manages sizing entirely in code.

### 11.3 Recommendation

Consider adding macOS-level constraints for robustness:

```rust
// NOT IMPLEMENTED - recommended for future
let _: () = msg_send![window, setMinSize:NSSize::new(750.0, 120.0)];
let _: () = msg_send![window, setMaxSize:NSSize::new(750.0, 700.0)];
```

---

## 12. Configuration Options

### 12.1 Relevant Config Settings (`~/.sk/kit/config.ts`)

```typescript
export default {
  padding: {
    top: 8,      // default: 8
    left: 12,    // default: 12
    right: 12    // default: 12
  },
  editorFontSize: 16,      // default: 14
  terminalFontSize: 14,    // default: 14
  uiScale: 1.0,            // default: 1.0 (not currently used for resize)
} satisfies Config;
```

### 12.2 Config Impact on Sizing

| Setting | Affects | How |
|---------|---------|-----|
| `padding` | Terminal rows/cols | Subtracted from available space |
| `editorFontSize` | Editor line height | Multiplied by 1.43 |
| `terminalFontSize` | Terminal cell dimensions | Scales proportionally |
| `uiScale` | Nothing currently | Future: could scale all dimensions |

---

## 13. Test Coverage

### 13.1 Unit Tests in `window_resize.rs`

```rust
#[test]
fn test_script_list_fixed_height() { /* ... */ }
#[test]
fn test_arg_with_choices_fixed_height() { /* ... */ }
#[test]
fn test_arg_no_choices_compact() { /* ... */ }
#[test]
fn test_full_height_views() { /* ... */ }
#[test]
fn test_div_prompt_standard_height() { /* ... */ }
#[test]
fn test_initial_window_height() { /* ... */ }
#[test]
fn test_height_constants() { /* ... */ }
```

### 13.2 Terminal Sizing Tests in `term_prompt.rs`

Includes regression tests for the padding symmetry bug:

```rust
#[test]
fn test_padding_symmetry_regression_top_and_bottom_must_both_be_subtracted() { /* ... */ }
#[test]
fn test_padding_symmetry_invariant_content_plus_padding_never_exceeds_total() { /* ... */ }
#[test]
fn test_padding_difference_between_buggy_and_fixed_calculation() { /* ... */ }
```

---

## 14. Recommendations

### 14.1 Short-Term

1. **Add OS-level window constraints** to prevent accidental oversizing
2. **Document the 16ms defer delay** more prominently for future maintainers
3. **Add telemetry** for resize frequency to validate debouncing removal

### 14.2 Medium-Term

1. **Consider animation option** with configurable enable/disable
2. **Add density config** for compact/default/relaxed item heights
3. **Test on ultra-wide monitors** for edge cases

### 14.3 Long-Term

1. **Investigate uiScale implementation** for accessibility
2. **Consider responsive width** for larger displays
3. **Profile resize performance** under heavy script output

---

## 15. Appendix: Key Code Locations

| Feature | File | Line Range (approx) |
|---------|------|---------------------|
| View type enum | `src/window_resize.rs` | 39-53 |
| Height calculation | `src/window_resize.rs` | 63-98 |
| Deferred resize | `src/window_resize.rs` | 123-139 |
| Top-edge positioning | `src/window_resize.rs` | 203-216 |
| Terminal cell sizing | `src/term_prompt.rs` | 129-142 |
| Terminal resize logic | `src/term_prompt.rs` | 172-195 |
| Editor height handling | `src/editor.rs` | 168-178, 1080-1144 |
| Window size update | `src/main.rs` | 1390-1468 |
| Eye-line positioning | `src/main.rs` | 300-393 |
| Config padding | `src/config.rs` | 58-87 |
| List item height | `src/list_item.rs` | 27-32 |

---

*End of Audit*
