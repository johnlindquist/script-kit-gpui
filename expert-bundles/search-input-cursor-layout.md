# Search Input Cursor & Placeholder Layout Expert Bundle

## Executive Summary

The main search input in Script Kit's GPUI application has cursor positioning and placeholder text alignment issues. The blinking cursor appears slightly off from where it should be, and the placeholder text seems out of place. This is a GPUI flexbox layout issue involving the interaction between cursor elements, text elements, and flexbox alignment.

### Key Problems:
1. **Cursor vertical alignment**: The cursor uses `CURSOR_HEIGHT_LG = 18.0` with `CURSOR_MARGIN_Y = 2.0` vertical margin, but the `text_lg()` style may have different line-height expectations causing misalignment.
2. **Cursor horizontal positioning**: When empty, cursor uses `.mr(px(design_spacing.padding_xs))` (4px) margin-right before placeholder; when typing, cursor uses `.ml(px(design_visual.border_normal))` (2px) margin-left after text - these inconsistent spacings may cause visual shifting.
3. **Flexbox baseline alignment**: The input uses `.items_center()` for vertical centering, but text and cursor divs may not align properly due to different intrinsic heights.

### Required Fixes:
1. `src/panel.rs`: Review `CURSOR_HEIGHT_LG` and `CURSOR_MARGIN_Y` constants - may need adjustment to match GPUI's `text_lg()` actual line-height.
2. `src/main.rs`: Lines ~7940-7970 - Main search input cursor rendering needs consistent margin values and possibly baseline alignment instead of center alignment.
3. `src/components/prompt_header.rs`: Lines ~305-340 - The reusable PromptHeader component has similar cursor positioning that may need synchronization.

### Files Included:
- `src/panel.rs`: Cursor constants and documentation
- `src/main.rs`: Main search input rendering code (lines ~7917-7970)
- `src/components/prompt_header.rs`: Reusable header component with search input

---

## Source Files

### src/panel.rs (Complete)

```rust
// macOS Panel Configuration Module
// This module configures GPUI windows as macOS floating panels
// that appear above other applications with native vibrancy effects
//
// Also provides placeholder configuration for input fields

#![allow(dead_code)]

/// Vibrancy configuration for GPUI window background appearance
///
/// GPUI supports three WindowBackgroundAppearance values:
/// - Opaque: Solid, no transparency
/// - Transparent: Fully transparent
/// - Blurred: macOS vibrancy effect (recommended for Spotlight/Raycast-like feel)
///
/// The actual vibrancy effect is achieved through:
/// 1. Setting `WindowBackgroundAppearance::Blurred` in WindowOptions (done in main.rs)
/// 2. Using semi-transparent background colors (controlled by theme opacity settings)
///
/// The blur shows through the transparent portions of the window background,
/// creating the native macOS vibrancy effect similar to Spotlight and Raycast.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum WindowVibrancy {
    /// Solid, opaque background - no vibrancy effect
    Opaque,
    /// Transparent background without blur
    Transparent,
    /// macOS vibrancy/blur effect - the native feel
    /// This is the recommended setting for Spotlight/Raycast-like appearance
    #[default]
    Blurred,
}

impl WindowVibrancy {
    /// Check if this vibrancy setting enables the blur effect
    pub fn is_blurred(&self) -> bool {
        matches!(self, WindowVibrancy::Blurred)
    }

    /// Check if this vibrancy setting is fully opaque
    pub fn is_opaque(&self) -> bool {
        matches!(self, WindowVibrancy::Opaque)
    }
}

#[cfg(target_os = "macos")]
/// Configure the current key window as a floating panel window that appears above other apps.
pub fn configure_as_floating_panel() {
    crate::logging::log("PANEL", "Panel configuration (implemented in main.rs)");
}

#[cfg(not(target_os = "macos"))]
/// No-op on non-macOS platforms
pub fn configure_as_floating_panel() {}

// ============================================================================
// Input Placeholder Configuration
// ============================================================================

/// Default placeholder text for the main search input
pub const DEFAULT_PLACEHOLDER: &str = "Script Kit";

/// Configuration for input field placeholder behavior
///
/// When using this configuration:
/// - Cursor should be positioned at FAR LEFT (index 0) when input is empty
/// - Placeholder text appears dimmed/muted when no user input
/// - Placeholder disappears immediately when user starts typing
#[derive(Debug, Clone)]
pub struct PlaceholderConfig {
    /// The placeholder text to display when input is empty
    pub text: String,
    /// Whether cursor should appear at left (true) or right (false) of placeholder
    pub cursor_at_left: bool,
}

impl Default for PlaceholderConfig {
    fn default() -> Self {
        Self {
            text: DEFAULT_PLACEHOLDER.to_string(),
            cursor_at_left: true,
        }
    }
}

impl PlaceholderConfig {
    /// Create a new placeholder configuration with custom text
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            cursor_at_left: true,
        }
    }

    /// Log when placeholder state changes (for observability)
    pub fn log_state_change(&self, is_showing_placeholder: bool) {
        crate::logging::log(
            "PLACEHOLDER",
            &format!(
                "Placeholder state changed: showing={}, text='{}', cursor_at_left={}",
                is_showing_placeholder, self.text, self.cursor_at_left
            ),
        );
    }

    /// Log cursor position on input focus (for observability)
    pub fn log_cursor_position(&self, position: usize, is_empty: bool) {
        crate::logging::log(
            "PLACEHOLDER",
            &format!(
                "Cursor position on focus: pos={}, input_empty={}, expected_left={}",
                position,
                is_empty,
                is_empty && self.cursor_at_left
            ),
        );
    }
}

// ============================================================================
// Cursor Styling Constants
// ============================================================================

/// Standard cursor width in pixels for text input fields
///
/// This matches the standard cursor width used in editor.rs and provides
/// visual consistency across all input fields.
pub const CURSOR_WIDTH: f32 = 2.0;

/// Cursor height for large text (.text_lg() / 18px font)
///
/// This value is calculated to align properly with GPUI's .text_lg() text rendering:
/// - GPUI's text_lg() uses ~18px font size
/// - With natural line height (~1.55), this gives ~28px line height
/// - Cursor should be 18px with 5px top/bottom spacing for vertical centering
///
/// NOTE: This value differs from `font_size_lg * line_height_normal` in design tokens
/// because GPUI's .text_lg() has different line-height than our token calculations.
/// Using this constant ensures proper cursor-text alignment.
pub const CURSOR_HEIGHT_LG: f32 = 18.0;

/// Cursor height for small text (.text_sm() / 12px font)
pub const CURSOR_HEIGHT_SM: f32 = 14.0;

/// Cursor height for medium text (.text_md() / 14px font)
pub const CURSOR_HEIGHT_MD: f32 = 16.0;

/// Vertical margin for cursor centering within text line
///
/// Apply this as `.my(px(CURSOR_MARGIN_Y))` to vertically center the cursor
/// within its text line. This follows the editor.rs pattern.
pub const CURSOR_MARGIN_Y: f32 = 2.0;

/// Configuration for input cursor styling
///
/// Use this struct to ensure consistent cursor appearance across all input fields.
/// The cursor should:
/// 1. Use a fixed height matching the text size (not calculated from design tokens)
/// 2. Use vertical margin for centering within the line
/// 3. Be rendered as an always-present div to prevent layout shift, with bg toggled
#[derive(Debug, Clone, Copy)]
pub struct CursorStyle {
    /// Cursor width in pixels
    pub width: f32,
    /// Cursor height in pixels (should match text size, not line height)
    pub height: f32,
    /// Vertical margin for centering
    pub margin_y: f32,
}

impl Default for CursorStyle {
    fn default() -> Self {
        Self::large()
    }
}

impl CursorStyle {
    /// Cursor style for large text (.text_lg())
    pub const fn large() -> Self {
        Self {
            width: CURSOR_WIDTH,
            height: CURSOR_HEIGHT_LG,
            margin_y: CURSOR_MARGIN_Y,
        }
    }

    /// Cursor style for medium text (.text_md())
    pub const fn medium() -> Self {
        Self {
            width: CURSOR_WIDTH,
            height: CURSOR_HEIGHT_MD,
            margin_y: CURSOR_MARGIN_Y,
        }
    }

    /// Cursor style for small text (.text_sm())
    pub const fn small() -> Self {
        Self {
            width: CURSOR_WIDTH,
            height: CURSOR_HEIGHT_SM,
            margin_y: CURSOR_MARGIN_Y,
        }
    }
}
```

### src/main.rs - Main Search Input (Lines 7917-7970)

```rust
div()
    .w_full()
    .px(px(header_padding_x))
    .py(px(header_padding_y))
    .flex()
    .flex_row()
    .items_center()
    .gap(px(header_gap))
    // Search input with blinking cursor
    // Cursor appears at LEFT when input is empty (before placeholder text)
    .child(
        div()
            .flex_1()
            .flex()
            .flex_row()
            .items_center()
            .text_lg()
            .text_color(if filter_is_empty {
                rgb(text_muted)
            } else {
                rgb(text_primary)
            })
            // When empty: cursor FIRST (at left), then placeholder
            // When typing: text, then cursor at end
            // ALWAYS render cursor div to prevent layout shift, but only show bg when focused + visible
            .when(filter_is_empty, |d| {
                d.child(
                    div()
                        .w(px(design_visual.border_normal))  // Uses design_visual.border_normal (2px)
                        .h(px(CURSOR_HEIGHT_LG))             // 18px from panel.rs
                        .my(px(CURSOR_MARGIN_Y))             // 2px vertical margin
                        .mr(px(design_spacing.padding_xs))   // 4px margin-right before placeholder
                        .when(
                            self.focused_input == FocusedInput::MainFilter
                                && self.cursor_visible,
                            |d| d.bg(rgb(text_primary)),
                        ),
                )
            })
            .child(filter_display)  // Placeholder or typed text
            .when(!filter_is_empty, |d| {
                d.child(
                    div()
                        .w(px(design_visual.border_normal))  // 2px width
                        .h(px(CURSOR_HEIGHT_LG))             // 18px height
                        .my(px(CURSOR_MARGIN_Y))             // 2px vertical margin
                        .ml(px(design_visual.border_normal)) // 2px margin-left after text
                        .when(
                            self.focused_input == FocusedInput::MainFilter
                                && self.cursor_visible,
                            |d| d.bg(rgb(text_primary)),
                        ),
                )
            }),
    )
```

### src/components/prompt_header.rs - Reusable Header (Lines 261-342)

```rust
/// Render the search input area with cursor
fn render_input_area(&self) -> impl IntoElement {
    let colors = self.colors;
    let filter_is_empty = self.config.filter_text.is_empty();
    let cursor_visible = self.config.cursor_visible && self.config.is_focused;

    // Display text: filter text or placeholder
    let display_text: SharedString = if filter_is_empty {
        self.config.placeholder.clone().into()
    } else {
        self.config.filter_text.clone().into()
    };

    // Text color: muted for placeholder, primary for input
    let text_color = if filter_is_empty {
        rgb(colors.text_muted)
    } else {
        rgb(colors.text_primary)
    };

    // Build input container
    let mut input = div()
        .flex_1()
        .flex()
        .flex_row()
        .items_center()
        .text_lg()
        .text_color(text_color);

    // Path prefix (if present)
    if let Some(ref prefix) = self.config.path_prefix {
        input = input.child(
            div()
                .text_color(rgb(colors.text_muted))
                .child(prefix.clone()),
        );
    }

    // Cursor position:
    // - When empty: cursor LEFT (before placeholder)
    // - When typing: cursor RIGHT (after text)

    // Left cursor (when empty)
    // Use conditional background instead of .when() to avoid type inference issues
    if filter_is_empty {
        let cursor_bg = if cursor_visible {
            rgb(colors.text_primary)
        } else {
            rgba(0x00000000)
        };
        input = input.child(
            div()
                .w(px(CURSOR_WIDTH))       // 2px from local constant
                .h(px(CURSOR_HEIGHT_LG))   // 20px - DIFFERENT from panel.rs!
                .my(px(CURSOR_MARGIN_Y))   // 2px
                .mr(px(4.))                // 4px margin-right
                .bg(cursor_bg),
        );
    }

    // Display text
    input = input.child(display_text);

    // Right cursor (when not empty)
    if !filter_is_empty {
        let cursor_bg = if cursor_visible {
            rgb(colors.text_primary)
        } else {
            rgba(0x00000000)
        };
        input = input.child(
            div()
                .w(px(CURSOR_WIDTH))       // 2px
                .h(px(CURSOR_HEIGHT_LG))   // 20px - DIFFERENT from panel.rs!
                .my(px(CURSOR_MARGIN_Y))   // 2px
                .ml(px(2.))                // 2px margin-left
                .bg(cursor_bg),
        );
    }

    input
}
```

**CRITICAL DISCREPANCY FOUND**: 
- `src/panel.rs` defines `CURSOR_HEIGHT_LG = 18.0`
- `src/components/prompt_header.rs` has local `const CURSOR_HEIGHT_LG: f32 = 20.0`

This inconsistency could be causing the cursor alignment issues!

---

## Implementation Guide

### Step 1: Fix Cursor Height Inconsistency

The `prompt_header.rs` component has its own local `CURSOR_HEIGHT_LG = 20.0` constant that differs from the canonical `panel.rs` constant of `18.0`. This needs to be unified.

```rust
// File: src/components/prompt_header.rs
// Location: Lines 28-33

// BEFORE:
/// Height of the cursor in the main filter input
const CURSOR_HEIGHT_LG: f32 = 20.0;
/// Vertical margin for cursor
const CURSOR_MARGIN_Y: f32 = 2.0;
/// Cursor width
const CURSOR_WIDTH: f32 = 2.0;

// AFTER:
// Remove local constants and import from panel.rs
use crate::panel::{CURSOR_HEIGHT_LG, CURSOR_MARGIN_Y, CURSOR_WIDTH};
```

Also add to the imports at the top of `prompt_header.rs`:
```rust
// File: src/components/prompt_header.rs
// Location: Top of file, in the use statements

use crate::panel::{CURSOR_HEIGHT_LG, CURSOR_MARGIN_Y};

// Define only CURSOR_WIDTH locally if not in panel.rs, or import it too
```

### Step 2: Normalize Cursor Margins

The main.rs search input uses different margins:
- Empty state: `.mr(px(design_spacing.padding_xs))` = 4px
- Typing state: `.ml(px(design_visual.border_normal))` = 2px

This asymmetry may cause visual shifting. Consider using consistent values:

```rust
// File: src/main.rs
// Location: Lines 7942-7969

// BEFORE (lines 7947-7948):
.mr(px(design_spacing.padding_xs))  // 4px

// AFTER:
.mr(px(2.))  // Consistent 2px margin

// The margin values should match:
// - Empty: cursor has 2px margin-right before placeholder
// - Typing: cursor has 2px margin-left after text
```

### Step 3: Consider Baseline Alignment

If vertical alignment is still off after Step 1-2, try using `.items_baseline()` instead of `.items_center()`:

```rust
// File: src/main.rs
// Location: Line 7932

// BEFORE:
.items_center()

// AFTER (if needed):
.items_baseline()
```

**Note**: This change affects all children in the flex row, so test carefully.

### Step 4: Adjust Cursor Height for text_lg()

If the cursor still appears misaligned, GPUI's `text_lg()` may have a different actual line height. Test with:

```rust
// File: src/panel.rs
// Location: Line 152

// BEFORE:
pub const CURSOR_HEIGHT_LG: f32 = 18.0;

// TRY:
pub const CURSOR_HEIGHT_LG: f32 = 20.0;  // Match the font's visual height
```

The cursor height should match the font's cap-height or x-height for proper visual alignment, not necessarily the font size.

---

## Testing

### Manual Testing Steps:
1. Build: `cargo build`
2. Run with AI logs: `SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1`
3. Observe the main search input with placeholder "Script Kit"
4. Type some text and observe cursor position
5. Delete text and observe cursor return to left position

### Visual Testing:
Create a test script that captures screenshots:

```typescript
// tests/smoke/test-cursor-alignment.ts
import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

// Wait for app to render
await new Promise(r => setTimeout(r, 1000));

// Capture empty state
const empty = await captureScreenshot();
const dir = join(process.cwd(), 'test-screenshots');
mkdirSync(dir, { recursive: true });
writeFileSync(join(dir, 'cursor-empty.png'), Buffer.from(empty.data, 'base64'));
console.error('[SCREENSHOT] cursor-empty.png saved');

process.exit(0);
```

Run: `echo '{"type":"run","path":"'$(pwd)'/tests/smoke/test-cursor-alignment.ts"}' | SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1`

### Expected Results:
- Cursor should be vertically centered with the text
- Cursor should have consistent spacing from text in both empty and typing states
- No visible "jump" when transitioning between empty and typing states

---

## Instructions For The Next AI Agent

You are reading the "Search Input Cursor & Placeholder Layout Expert Bundle". This file is self-contained and includes all the context you should assume you have.

Your job:

* Design and describe the minimal, safe changes needed to fully resolve the issues described in the Executive Summary and Key Problems.
* Operate **only** on the files and code snippets included in this bundle. If you need additional files or context, clearly say so.

When you propose changes, follow these rules strictly:

1. Always provide **precise code snippets** that can be copy-pasted directly into the repo.
2. Always include **exact file paths** (e.g. `src/main.rs`) and, when possible, line numbers or a clear description of the location (e.g. "replace the existing cursor div in the empty state").
3. Never describe code changes only in prose. Show the full function or block as it should look **after** the change, or show both "before" and "after" versions.
4. Keep instructions **unmistakable and unambiguous**. A human or tool following your instructions should not need to guess what to do.
5. Assume you cannot see any files outside this bundle. If you must rely on unknown code, explicitly note assumptions and risks.

**Key finding to address**: The `prompt_header.rs` component uses `CURSOR_HEIGHT_LG = 20.0` while `panel.rs` defines it as `18.0`. This inconsistency is the most likely cause of the cursor positioning issues. Unify these values by importing from `panel.rs`.

When you answer, you do not need to restate this bundle. Work directly with the code and instructions it contains and return a clear, step-by-step plan plus exact code edits.
