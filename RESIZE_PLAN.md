# Dynamic Window Resizing Plan

## Overview

This plan implements dynamic window resizing for the Script Kit GPUI app, matching the behavior of the original Script Kit where the window shrinks/expands based on the number of visible choices.

**Goal**: When the `arg` prompt has no choices (or empty filtered results), the window should shrink to show only the input field. When choices are available, it expands to show them.

---

## Current State Analysis

### Fixed Window Size (Current Implementation)

```rust
// src/main.rs:488, 3158
let window_size = size(px(750.), px(500.0));
```

The window is always 750x500 pixels, regardless of content.

### Window Positioning Locations

| Location | File:Line | Purpose |
|----------|-----------|---------|
| Initial open | `main.rs:3157-3159` | Window creation via `cx.open_window()` |
| Hotkey show | `main.rs:487-502` | Repositions window on hotkey toggle |
| External command show | `main.rs:3346-3348` | Repositions when running script via stdin |

### Content-Affecting Components

| Component | Condition | Expected Height |
|-----------|-----------|-----------------|
| Search input header | Always visible | ~52px |
| Divider | Always visible | 1px |
| Choice list | `filtered_choices.len() > 0` | `N * LIST_ITEM_HEIGHT` (52px each) |
| Empty state | `filtered_choices.len() == 0` | ~40px ("No choices match") |
| Footer | Always visible | ~38px |

---

## Target Behavior

### Height Calculation Formula

```
window_height = header_height + divider + list_height + footer_height

where:
  header_height = 52px (fixed)
  divider = 1px
  list_height = min(visible_choices * LIST_ITEM_HEIGHT, max_list_height)
  footer_height = 38px
  
constraints:
  min_height = header + divider + footer = ~91px (input-only mode)
  max_height = 500px (or configurable)
  max_visible_choices = floor((max_height - min_height) / LIST_ITEM_HEIGHT) ≈ 7-8 items
```

### Visual States

1. **Input Only** (0 choices): ~91px tall - just header + divider + footer
2. **Few Choices** (1-7 choices): scales with content
3. **Many Choices** (8+ choices): capped at max height with scroll

---

## Implementation Plan

### Phase 1: Core Resize Infrastructure

#### Task 1.1: Create WindowResizer Module

**File**: `src/window_resize.rs` (new)

```rust
//! Dynamic window resizing based on content

use gpui::{px, Pixels, Size, Bounds, point};

/// Configuration for window sizing
pub struct WindowSizeConfig {
    /// Fixed window width
    pub width: Pixels,
    /// Minimum window height (input-only mode)
    pub min_height: Pixels,
    /// Maximum window height
    pub max_height: Pixels,
    /// Height of each list item
    pub item_height: Pixels,
    /// Height of header (search input area)
    pub header_height: Pixels,
    /// Height of footer
    pub footer_height: Pixels,
    /// Divider height
    pub divider_height: Pixels,
}

impl Default for WindowSizeConfig {
    fn default() -> Self {
        Self {
            width: px(750.),
            min_height: px(91.),      // header + divider + footer
            max_height: px(500.),
            item_height: px(52.),     // LIST_ITEM_HEIGHT
            header_height: px(52.),
            footer_height: px(38.),
            divider_height: px(1.),
        }
    }
}

impl WindowSizeConfig {
    /// Calculate window height based on number of visible items
    pub fn calculate_height(&self, item_count: usize) -> Pixels {
        let base_height = self.header_height + self.divider_height + self.footer_height;
        
        if item_count == 0 {
            base_height
        } else {
            let list_height = self.item_height * (item_count as f32);
            let total = base_height + list_height;
            
            // Clamp to min/max
            if total < self.min_height {
                self.min_height
            } else if total > self.max_height {
                self.max_height
            } else {
                total
            }
        }
    }
    
    /// Calculate window size for given item count
    pub fn calculate_size(&self, item_count: usize) -> Size<Pixels> {
        Size {
            width: self.width,
            height: self.calculate_height(item_count),
        }
    }
}

/// Calculate the maximum number of visible items before scrolling kicks in
pub fn max_visible_items(config: &WindowSizeConfig) -> usize {
    let available_height = config.max_height 
        - config.header_height 
        - config.divider_height 
        - config.footer_height;
    
    (available_height.0 / config.item_height.0).floor() as usize
}
```

#### Task 1.2: Add Module to lib.rs

**File**: `src/lib.rs`

Add: `pub mod window_resize;`

---

### Phase 2: AppView-Specific Sizing

#### Task 2.1: Calculate Size Based on Current View

**File**: `src/main.rs`

Add method to `ScriptListApp`:

```rust
impl ScriptListApp {
    /// Calculate appropriate window size based on current view and content
    fn calculate_dynamic_window_size(&self) -> Size<Pixels> {
        use crate::window_resize::WindowSizeConfig;
        
        let config = WindowSizeConfig::default();
        
        match &self.current_view {
            AppView::ScriptList => {
                // Script list always uses max size for now
                // Could be dynamic based on filtered_results().len()
                config.calculate_size(config.max_visible_items())
            }
            AppView::ArgPrompt { choices, .. } => {
                // Filter choices and calculate based on visible count
                let filtered = self.filtered_arg_choices();
                config.calculate_size(filtered.len())
            }
            AppView::DivPrompt { .. } => {
                // Div prompts use max size (HTML content is variable)
                Size {
                    width: config.width,
                    height: config.max_height,
                }
            }
            AppView::TermPrompt { .. } => {
                // Terminal uses max size
                Size {
                    width: config.width,
                    height: config.max_height,
                }
            }
            AppView::ActionsDialog => {
                // Actions dialog is a popup overlay, doesn't affect main window
                Size {
                    width: config.width,
                    height: config.max_height,
                }
            }
        }
    }
}
```

---

### Phase 3: Trigger Resize on State Changes

#### Task 3.1: Resize on View Change

When transitioning to `AppView::ArgPrompt`, calculate and apply new size:

```rust
fn show_arg_prompt(&mut self, id: String, placeholder: String, choices: Vec<Choice>, cx: &mut Context<Self>) {
    self.current_view = AppView::ArgPrompt { id, placeholder, choices };
    self.arg_input_text.clear();
    self.arg_selected_index = 0;
    
    // Calculate and apply new window size
    self.resize_window_to_content(cx);
    
    cx.notify();
}

fn resize_window_to_content(&mut self, cx: &mut Context<Self>) {
    let new_size = self.calculate_dynamic_window_size();
    
    // Get current window bounds and update height only (keep position stable)
    // This requires access to the window, may need to be done differently
    // See Task 3.2 for implementation details
}
```

#### Task 3.2: Implement Window Resize Function

The challenge: GPUI doesn't provide a direct `window.set_size()` API. We need to use macOS native APIs similar to existing `move_first_window_to()`.

**New function in `main.rs`:**

```rust
/// Resize the first window to new dimensions, keeping top-left origin stable
fn resize_first_window_to_height(new_height: f64) {
    unsafe {
        let app: id = NSApp();
        let windows: id = msg_send![app, windows];
        let count: usize = msg_send![windows, count];
        
        if count > 0 {
            let window: id = msg_send![windows, objectAtIndex:0usize];
            
            if window != nil {
                let current_frame: NSRect = msg_send![window, frame];
                
                // Get primary screen height for coordinate conversion
                let screens: id = msg_send![class!(NSScreen), screens];
                let main_screen: id = msg_send![screens, firstObject];
                let main_screen_frame: NSRect = msg_send![main_screen, frame];
                let primary_screen_height = main_screen_frame.size.height;
                
                // macOS uses bottom-left origin. To keep top-left stable when resizing,
                // we need to adjust the origin.y
                let current_top = current_frame.origin.y + current_frame.size.height;
                let height_delta = new_height - current_frame.size.height;
                
                // New origin.y keeps the top edge at the same position
                let new_origin_y = current_frame.origin.y - height_delta;
                
                let new_frame = NSRect::new(
                    NSPoint::new(current_frame.origin.x, new_origin_y),
                    NSSize::new(current_frame.size.width, new_height),
                );
                
                logging::log("RESIZE", &format!(
                    "Resizing window: old_h={:.0} new_h={:.0} delta={:.0}",
                    current_frame.size.height, new_height, height_delta
                ));
                
                // Animate the resize for smooth UX
                let _: () = msg_send![window, setFrame:new_frame display:true animate:true];
            }
        }
    }
}
```

#### Task 3.3: Trigger Resize on Filter Change

When the user types in the arg prompt, filtered choices change:

```rust
// In handle_char for arg prompt
fn handle_arg_char(&mut self, ch: char, cx: &mut Context<Self>) {
    self.arg_input_text.push(ch);
    self.arg_selected_index = 0;
    
    // Resize window based on new filtered count
    self.resize_window_to_content(cx);
    
    cx.notify();
}

// Similarly for backspace
fn handle_arg_backspace(&mut self, cx: &mut Context<Self>) {
    if !self.arg_input_text.is_empty() {
        self.arg_input_text.pop();
        self.arg_selected_index = 0;
        
        // Resize window based on new filtered count
        self.resize_window_to_content(cx);
        
        cx.notify();
    }
}
```

---

### Phase 4: Positioning Integration

#### Task 4.1: Update Eye-Line Calculation

Modify `calculate_eye_line_bounds_on_mouse_display` to accept dynamic height:

```rust
fn calculate_eye_line_bounds_on_mouse_display(
    window_size: gpui::Size<Pixels>,  // Already parameterized!
    _cx: &App,
) -> Bounds<Pixels> {
    // Existing implementation works - just pass different size
}
```

#### Task 4.2: Update All Window Show Locations

| Location | Change Required |
|----------|-----------------|
| `main.rs:488` (hotkey show) | Calculate size based on current view |
| `main.rs:3158` (initial) | Use max size for initial script list |
| `main.rs:3347` (stdin cmd) | Calculate size based on view |

---

### Phase 5: Animation & Polish

#### Task 5.1: Smooth Resize Animation

macOS `setFrame:display:animate:` with `animate:true` provides smooth resize.

Consider adding easing or using GPUI's animation system if available.

#### Task 5.2: Debounce Rapid Resize

Typing rapidly shouldn't cause janky resizing:

```rust
struct ResizeState {
    last_resize_time: Instant,
    pending_height: Option<f64>,
}

const RESIZE_DEBOUNCE_MS: u64 = 50;

fn maybe_resize(&mut self, new_height: f64, cx: &mut Context<Self>) {
    let now = Instant::now();
    
    if now.duration_since(self.last_resize_time) < Duration::from_millis(RESIZE_DEBOUNCE_MS) {
        self.pending_height = Some(new_height);
        // Schedule deferred resize
        return;
    }
    
    self.last_resize_time = now;
    resize_first_window_to_height(new_height);
}
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_height_calculation_zero_items() {
        let config = WindowSizeConfig::default();
        let height = config.calculate_height(0);
        assert_eq!(height, config.min_height);
    }
    
    #[test]
    fn test_height_calculation_few_items() {
        let config = WindowSizeConfig::default();
        let height = config.calculate_height(3);
        let expected = config.header_height + config.divider_height + config.footer_height
            + (config.item_height * 3.0);
        assert_eq!(height, expected);
    }
    
    #[test]
    fn test_height_calculation_capped_at_max() {
        let config = WindowSizeConfig::default();
        let height = config.calculate_height(100);
        assert_eq!(height, config.max_height);
    }
}
```

### SDK Tests

Create `tests/sdk/test-resize.ts`:

```typescript
// Name: SDK Test - Window Resize
// Description: Tests dynamic window resizing based on choice count

import '../../scripts/kit-sdk';

// Test 1: Empty choices - window should be minimal
await arg("No choices", []);
// Verify: window is ~91px tall

// Test 2: Few choices - window scales
await arg("Few choices", ["One", "Two", "Three"]);
// Verify: window is ~247px tall (91 + 3*52)

// Test 3: Many choices - window capped
await arg("Many choices", Array.from({length: 20}, (_, i) => `Item ${i+1}`));
// Verify: window is 500px tall (max)

// Test 4: Filter reduces choices - window shrinks
// Type in filter to reduce visible items, verify resize
```

---

## File Change Summary

| File | Change Type | Description |
|------|-------------|-------------|
| `src/window_resize.rs` | **NEW** | Core resize calculation module |
| `src/lib.rs` | EDIT | Add `mod window_resize` |
| `src/main.rs` | EDIT | Add `resize_first_window_to_height()`, update view transitions |
| `tests/sdk/test-resize.ts` | **NEW** | SDK test for resize behavior |

---

## Dependencies & Risks

### Dependencies

1. **macOS Native APIs**: Resize uses `NSWindow.setFrame:display:animate:`
2. **GPUI Bounds**: Need to understand how GPUI tracks window bounds internally
3. **Coordinate System**: macOS bottom-left origin requires careful Y adjustment

### Risks

| Risk | Mitigation |
|------|------------|
| GPUI state out of sync after native resize | Test thoroughly, may need to notify GPUI |
| Animation jank during rapid typing | Debounce resize operations |
| Multi-monitor edge cases | Test with various monitor configurations |
| Height calculation drift | Use constants for all component heights |

---

## Implementation Order

1. **Task 1.1-1.2**: Create `window_resize.rs` module *(foundation)*
2. **Task 3.2**: Implement `resize_first_window_to_height()` *(native integration)*
3. **Task 2.1**: Add `calculate_dynamic_window_size()` to `ScriptListApp`
4. **Task 3.1, 3.3**: Wire up resize triggers on view/filter changes
5. **Task 4.1-4.2**: Update positioning to use dynamic sizes
6. **Task 5.1-5.2**: Polish with animation and debouncing
7. **Testing**: Unit tests + SDK integration tests

---

## Success Criteria

- [ ] Window shrinks to ~91px when arg prompt has 0 choices
- [ ] Window scales with 1-7 choices (each choice adds ~52px)
- [ ] Window caps at 500px for 8+ choices
- [ ] Typing filter causes smooth resize as choices filter
- [ ] Backspace/clear causes smooth resize as choices expand
- [ ] Window position (top-left) stays stable during resize
- [ ] No flicker or jank during rapid typing
- [ ] Multi-monitor positioning still works correctly
- [ ] Script list view uses max size (no regression)
- [ ] Div and Term prompts use max size (no regression)
