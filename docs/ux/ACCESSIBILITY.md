# Accessibility Audit Report

**Application:** Script Kit GPUI  
**Audit Date:** December 2024  
**Framework:** GPUI (Rust) - Native macOS  
**WCAG Target:** 2.1 Level AA  

---

## Executive Summary

This audit evaluates the accessibility implementation of Script Kit GPUI, a Rust-based launcher application using the GPUI framework. GPUI is a native macOS framework that interfaces with macOS accessibility APIs (NSAccessibility) rather than web ARIA standards.

### Overall Assessment: **Needs Improvement**

| Category | Status | Priority |
|----------|--------|----------|
| Screen Reader Support | Critical Gap | P0 |
| Color Contrast | Partial Compliance | P1 |
| Focus Visibility | Good | P2 |
| Text Sizing | Configurable | P3 |
| Motion Reduction | Not Implemented | P2 |
| High Contrast Mode | Not Implemented | P1 |
| Keyboard Navigation | Excellent | - |
| Error Accessibility | Partial | P2 |
| Form Labels | Not Applicable (No Forms) | - |

---

## 1. Screen Reader Support (ARIA-like Patterns in GPUI)

### Current Status: **Critical Gap**

GPUI is a native macOS framework that should integrate with macOS VoiceOver through NSAccessibility. However, the current implementation lacks explicit accessibility annotations.

### Findings

#### Missing Accessibility Labels

**Components without accessibility labels:**
- `ListItem` component (`src/list_item.rs`) - No `.accessibility_label()` or equivalent
- `Button` component (`src/components/button.rs`) - No accessibility role or label
- `Toast` notifications (`src/components/toast.rs`) - No announcements for screen readers
- `ActionsDialog` (`src/actions.rs`) - No modal role or accessibility tree structure
- `EditorPrompt` (`src/editor.rs`) - No document role or text area accessibility
- `TermPrompt` (`src/term_prompt.rs`) - No terminal/document role

#### Element Identification Patterns

The codebase uses `ElementId` for DOM-like targeting:

```rust
// Current pattern (from list_item.rs)
let element_id = if let Some(ref sem_id) = semantic_id {
    ElementId::Name(sem_id.clone().into())
} else {
    ElementId::NamedInteger("list-item".into(), element_idx as u64)
};
```

**Issue:** While semantic IDs exist for testing, they are not exposed as accessibility labels.

#### Positive Patterns Found

1. **Semantic ID Support** - The `ListItem` component supports `semantic_id()` for AI-driven testing:
   ```rust
   pub fn semantic_id(mut self, id: impl Into<String>) -> Self {
       self.semantic_id = Some(id.into());
       self
   }
   ```

2. **Focus Handle Implementation** - Proper focus management exists:
   ```rust
   impl Focusable for ActionsDialog {
       fn focus_handle(&self, _cx: &App) -> FocusHandle {
           self.focus_handle.clone()
       }
   }
   ```

### Recommendations

| Priority | Action | Effort |
|----------|--------|--------|
| P0 | Add accessibility labels to all interactive elements | Medium |
| P0 | Implement accessibility roles for containers (list, dialog, document) | High |
| P0 | Add live region announcements for dynamic content (toasts, prompts) | Medium |
| P1 | Test with VoiceOver and document behavior | Low |

### WCAG Criteria

- **1.1.1 Non-text Content** - FAIL: No text alternatives for icons
- **4.1.2 Name, Role, Value** - FAIL: Interactive elements lack accessible names

---

## 2. Color Contrast Ratios

### Current Status: **Partial Compliance**

### Theme Color Analysis

From `src/theme.rs`, the default dark theme colors:

| Element | Foreground | Background | Contrast Ratio | WCAG AA |
|---------|------------|------------|----------------|---------|
| Primary Text | `#ffffff` | `#1e1e1e` | 15.1:1 | PASS |
| Secondary Text | `#cccccc` | `#1e1e1e` | 10.1:1 | PASS |
| Tertiary Text | `#999999` | `#1e1e1e` | 5.3:1 | PASS |
| Muted Text | `#808080` | `#1e1e1e` | 3.9:1 | FAIL |
| Dimmed Text | `#666666` | `#1e1e1e` | 2.9:1 | FAIL |
| Accent (Selected) | `#fbbf24` | `#1e1e1e` | 9.4:1 | PASS |

### Light Theme Colors

```rust
pub fn light_default() -> Self {
    ColorScheme {
        text: TextColors {
            primary: 0x000000,    // Black on white - 21:1 PASS
            secondary: 0x333333,  // Dark gray on white - 12:1 PASS
            tertiary: 0x666666,   // Medium gray - 5.7:1 PASS
            muted: 0x999999,      // Light gray - 2.8:1 FAIL
            dimmed: 0xcccccc,     // Very light - 1.6:1 FAIL
        },
        // ...
    }
}
```

### Critical Issues

1. **Muted/Dimmed Text Fails WCAG AA** (minimum 4.5:1 for normal text)
   - Dark theme: `#808080` (3.9:1), `#666666` (2.9:1)
   - Light theme: `#999999` (2.8:1), `#cccccc` (1.6:1)

2. **Focus-Unfocused Dimming** - The `to_unfocused()` method reduces contrast further:
   ```rust
   pub fn to_unfocused(&self) -> Self {
       fn darken_hex(color: HexColor) -> HexColor {
           // Reduce saturation and brightness: blend 30% toward gray
           let new_r = ((r * 70 + gray * 30) / 100) as u8;
           // ...
       }
   }
   ```

### Recommendations

| Priority | Action | Effort |
|----------|--------|--------|
| P1 | Increase muted text contrast to minimum 4.5:1 | Low |
| P1 | Increase dimmed text contrast to minimum 3:1 (large text) | Low |
| P2 | Ensure unfocused state maintains minimum contrast | Low |
| P2 | Add contrast validation tests | Medium |

### WCAG Criteria

- **1.4.3 Contrast (Minimum)** - PARTIAL: Primary/secondary pass, muted/dimmed fail
- **1.4.6 Contrast (Enhanced)** - FAIL: Enhanced ratio (7:1) not met for secondary text

---

## 3. Focus Visibility

### Current Status: **Good**

### Implementation Analysis

The application has strong focus visibility implementation:

#### Selection Background
```rust
// From list_item.rs
let selected_bg = rgba((colors.accent_selected_subtle << 8) | 0x80);
let hover_bg = rgba((colors.accent_selected_subtle << 8) | 0x40);
```

#### Accent Bar for Selection
```rust
// From list_item.rs
let accent_bar = if self.show_accent_bar {
    let accent_color = rgb(colors.accent_selected);
    div()
        .w(px(ACCENT_BAR_WIDTH))  // 3px
        .h_full()
        .bg(if self.selected { accent_color } else { rgba(0x00000000) })
} else {
    div().w(px(0.)).h(px(0.))
};
```

#### Cursor Visibility
```rust
// From actions.rs - Blinking cursor in search input
.when(self.cursor_visible, |d| d.bg(accent_color))
```

#### Focus State Tracking
```rust
// From main.rs
enum FocusedInput {
    MainFilter,
    ActionsSearch,
    ArgPrompt,
    None,
}
```

### Positive Findings

1. **Clear Visual Focus Indicator** - Selected items have:
   - 3px accent bar on left edge (gold/yellow `#fbbf24`)
   - 50% opacity background highlight
   - Text color change from secondary to primary

2. **Hover State Distinction** - Separate from selection:
   - 25% opacity background (subtle)
   - Does not change text color

3. **Focus Handle Tracking** - Proper focus management across views

### Minor Issues

1. **Focus Ring on Buttons** - No visible focus outline (only hover state)
2. **Dialog Focus Trap** - Not explicitly implemented for ActionsDialog

### Recommendations

| Priority | Action | Effort |
|----------|--------|--------|
| P2 | Add focus ring to Button component | Low |
| P2 | Implement focus trap for modal dialogs | Medium |
| P3 | Ensure focus visible indicator on all interactive elements | Low |

### WCAG Criteria

- **2.4.7 Focus Visible** - PASS: Strong selection indicators
- **2.4.11 Focus Not Obscured (Minimum)** - PASS: Focus indicators are prominent

---

## 4. Text Sizing and Readability

### Current Status: **Configurable** (Good)

### Implementation

Text sizing is configurable through `~/.sk/kit/config.ts`:

```rust
// From src/config.rs (conceptual)
pub fn get_editor_font_size(&self) -> f32 {
    self.editor_font_size.unwrap_or(14.0)
}

pub fn get_terminal_font_size(&self) -> f32 {
    self.terminal_font_size.unwrap_or(14.0)
}
```

### Font Size Constants

| Component | Default Size | Line Height |
|-----------|--------------|-------------|
| Editor | 14pt | 1.43x (20px) |
| Terminal | 14pt | 1.3x (18.2px) |
| List Items | 14px name, 12px desc | ~1.3x |
| Action Items | 14px (text_sm) | Standard |
| Toasts | 14px (text_sm) | Standard |

### Positive Findings

1. **Dynamic Font Sizing** - Editor and terminal scale based on config
2. **Sufficient Line Height** - Good vertical spacing for readability
3. **Monospace for Code** - Appropriate font family (Menlo)

### Issues

1. **No UI Scale Setting** - `uiScale` in config exists but may not be fully implemented
2. **No Responsive Text Scaling** - Fixed pixel sizes don't adapt to system text scaling
3. **Minimum Text Size** - Some text uses `text_xs` (~10-11px) which may be too small

### Recommendations

| Priority | Action | Effort |
|----------|--------|--------|
| P3 | Implement uiScale config option | Medium |
| P3 | Respect macOS system text scaling preferences | High |
| P3 | Increase minimum text size to 12px | Low |

### WCAG Criteria

- **1.4.4 Resize Text** - PARTIAL: Config supports font size, no runtime scaling
- **1.4.12 Text Spacing** - PASS: Good line heights and letter spacing

---

## 5. Motion Reduction Preferences

### Current Status: **Not Implemented**

### Analysis

No handling of `prefers-reduced-motion` or macOS "Reduce Motion" setting.

#### Current Animations (Implicit)

1. **Cursor Blinking** - 530ms interval
2. **Scroll Animations** - Via `UniformListScrollHandle`
3. **Toast Auto-Dismiss** - Timer-based removal (5 seconds)
4. **Window Positioning** - Animated moves possible

### Missing Implementation

```rust
// Not found in codebase:
// - reduce_motion preference check
// - NSWorkspace.accessibilityDisplayShouldReduceMotion
// - Animation duration configuration
```

### Recommendations

| Priority | Action | Effort |
|----------|--------|--------|
| P2 | Query macOS `accessibilityDisplayShouldReduceMotion` | Low |
| P2 | Disable cursor blinking when motion reduced | Low |
| P2 | Use instant scroll instead of animated scroll | Medium |
| P3 | Disable toast animations | Low |

### WCAG Criteria

- **2.3.3 Animation from Interactions** - FAIL: No motion preference support

---

## 6. High Contrast Mode Support

### Current Status: **Not Implemented**

### Analysis

No detection or handling of macOS high contrast mode.

#### Current Theme System

```rust
// From theme.rs
pub fn detect_system_appearance() -> bool {
    match Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.to_lowercase().contains("dark")
        }
        Err(_) => false
    }
}
```

**Issue:** Only detects dark/light mode, not high contrast.

### Missing Features

1. No query for `accessibilityDisplayShouldIncreaseContrast`
2. No high-contrast color scheme variant
3. No thickened focus indicators for high contrast

### Recommendations

| Priority | Action | Effort |
|----------|--------|--------|
| P1 | Query macOS `accessibilityDisplayShouldIncreaseContrast` | Low |
| P1 | Define high-contrast color scheme with 7:1+ ratios | Medium |
| P2 | Increase border widths in high contrast mode | Low |
| P2 | Add theme variant switching for high contrast | Medium |

### WCAG Criteria

- **1.4.11 Non-text Contrast** - PARTIAL: UI components have 3:1+ contrast
- **1.4.6 Contrast (Enhanced)** - FAIL: No enhanced contrast option

---

## 7. Keyboard-Only Navigation

### Current Status: **Excellent**

### Implementation Analysis

Comprehensive keyboard navigation throughout the application.

#### Global Hotkey
```rust
// From main.rs - Global hotkey toggle
static HOTKEY_CHANNEL: OnceLock<(async_channel::Sender<()>, async_channel::Receiver<()>)>
```

#### List Navigation
```rust
// From main.rs
fn move_selection_up(&mut self, cx: &mut Context<Self>) {
    if self.selected_index > 0 {
        let mut new_index = self.selected_index - 1;
        // Skip section headers when moving up
        while new_index > 0 {
            if let Some(GroupedListItem::SectionHeader(_)) = grouped_items.get(new_index) {
                new_index -= 1;
            } else {
                break;
            }
        }
        // ...
    }
}
```

#### Editor Navigation
```rust
// From editor.rs - Comprehensive key handling
match (key.as_str(), cmd, shift, alt) {
    ("left" | "arrowleft", false, _, false) => self.move_left(shift),
    ("right" | "arrowright", false, _, false) => self.move_right(shift),
    ("up" | "arrowup", false, _, false) => self.move_up(shift),
    ("down" | "arrowdown", false, _, false) => self.move_down(shift),
    ("left" | "arrowleft", false, _, true) => self.move_word_left(shift),
    ("right" | "arrowright", false, _, true) => self.move_word_right(shift),
    // ... 20+ more key bindings
}
```

#### Terminal Navigation
```rust
// From term_prompt.rs - Terminal key support
let bytes: Option<&[u8]> = match key_str.as_str() {
    "enter" => Some(b"\r"),
    "backspace" => Some(b"\x7f"),
    "tab" => Some(b"\t"),
    "up" | "arrowup" => Some(b"\x1b[A"),
    "down" | "arrowdown" => Some(b"\x1b[B"),
    // ... function keys, page up/down, etc.
};
```

### Positive Findings

1. **Complete Arrow Key Support** - Handles both `"up"` and `"arrowup"` variants
2. **Modifier Keys** - Cmd, Shift, Alt combinations for word/line navigation
3. **Section Header Skipping** - Keyboard navigation skips non-interactive headers
4. **Escape to Cancel** - Consistent escape handling across all prompts
5. **Enter to Submit** - Standard confirmation pattern

### Minor Issues

1. **No Skip Links** - No way to skip sections of the list
2. **Tab Navigation** - Limited tab key support between UI regions

### WCAG Criteria

- **2.1.1 Keyboard** - PASS: All functionality keyboard accessible
- **2.1.2 No Keyboard Trap** - PASS: Escape always available
- **2.1.4 Character Key Shortcuts** - PASS: No single-key shortcuts without modifiers

---

## 8. Error Message Accessibility

### Current Status: **Partial**

### Implementation Analysis

#### Toast Error Display
```rust
// From components/toast.rs
impl ToastVariant {
    pub fn icon(&self) -> &'static str {
        match self {
            ToastVariant::Success => "?",
            ToastVariant::Warning => "?",
            ToastVariant::Error => "?",
            ToastVariant::Info => "?",
        }
    }
}
```

#### Error Notification Structure
```rust
// From main.rs
struct ErrorNotification {
    message: String,
    severity: ErrorSeverity,
    created_at: std::time::Instant,
}
```

#### Toast Colors by Severity
```rust
// From toast.rs
let (icon_color, border_color) = match variant {
    ToastVariant::Success => (colors.ui.success, colors.ui.success),
    ToastVariant::Warning => (colors.ui.warning, colors.ui.warning),
    ToastVariant::Error => (colors.ui.error, colors.ui.error),
    ToastVariant::Info => (colors.ui.info, colors.ui.info),
};
```

### Issues

1. **No Live Region Announcements** - Errors not announced to screen readers
2. **Color-Only Error Indication** - Icons help but rely on color (red/yellow/green)
3. **Auto-Dismiss** - 5-second default may be too fast for some users
4. **No Error Focus** - Errors don't receive keyboard focus

### Positive Findings

1. **Icon + Color + Text** - Multiple modalities for error type
2. **Expandable Details** - Stack traces available for debugging
3. **Dismissible** - User can close errors manually

### Recommendations

| Priority | Action | Effort |
|----------|--------|--------|
| P2 | Add live region announcements for errors | Medium |
| P2 | Ensure icons are distinguishable without color | Low |
| P2 | Make auto-dismiss duration configurable | Low |
| P3 | Add option to focus error notifications | Low |

### WCAG Criteria

- **1.4.1 Use of Color** - PARTIAL: Icons provide shape, but similar shapes
- **4.1.3 Status Messages** - FAIL: No ARIA live regions

---

## 9. Form Label Associations

### Current Status: **Not Applicable**

The application does not use traditional HTML forms. Input is handled through:

1. **Text Filter Input** - Inline typing in focused list
2. **Arg Prompt Input** - Direct character handling
3. **Editor Prompt** - Code editor with cursor
4. **Terminal Prompt** - Terminal emulator

### Analysis

While no `<label>` elements exist, accessible input patterns should still be considered:

#### Current Input Pattern
```rust
// From prompts.rs - ArgPrompt
let input_container = div()
    .id(gpui::ElementId::Name("input:filter".into()))
    .w_full()
    // ... no accessibility label
    .child(input_display);
```

### Recommendations

| Priority | Action | Effort |
|----------|--------|--------|
| P2 | Add accessibility labels to input containers | Low |
| P2 | Associate placeholder text as accessible description | Low |
| P3 | Add input role to text entry areas | Low |

### WCAG Criteria

- **1.3.1 Info and Relationships** - PARTIAL: Structure exists but not exposed
- **2.4.6 Headings and Labels** - PARTIAL: Visual labels exist, not programmatic

---

## macOS Accessibility API Integration

### Required macOS APIs

GPUI should integrate with these macOS accessibility APIs:

| API | Purpose | Current Status |
|-----|---------|----------------|
| `NSAccessibility` | Element labeling | Not Found |
| `accessibilityLabel` | Screen reader text | Not Found |
| `accessibilityRole` | Element type | Not Found |
| `accessibilityValue` | Current value | Not Found |
| `accessibilityDisplayShouldIncreaseContrast` | High contrast mode | Not Found |
| `accessibilityDisplayShouldReduceMotion` | Motion preference | Not Found |
| `accessibilityDisplayShouldReduceTransparency` | Transparency preference | Not Found |

### VoiceOver Compatibility

Current VoiceOver support is unknown. Testing required with:

1. VoiceOver navigation through list items
2. Announcement of selection changes
3. Reading of prompt placeholders
4. Error notification announcements

---

## Implementation Priority Matrix

### P0 - Critical (Blocks Users)

| Issue | Effort | Impact |
|-------|--------|--------|
| Add accessibility labels to all interactive elements | Medium | High |
| Implement accessibility roles for containers | High | High |
| Add live region announcements for dynamic content | Medium | High |

### P1 - Important (Significant Barriers)

| Issue | Effort | Impact |
|-------|--------|--------|
| Increase muted/dimmed text contrast to 4.5:1 | Low | Medium |
| Implement high contrast mode support | Medium | Medium |
| Query macOS accessibility preferences | Low | Medium |

### P2 - Moderate (Usability Issues)

| Issue | Effort | Impact |
|-------|--------|--------|
| Implement prefers-reduced-motion support | Medium | Low |
| Add focus ring to Button component | Low | Low |
| Ensure error icons distinguishable by shape | Low | Medium |
| Add accessible labels to input areas | Low | Medium |

### P3 - Minor (Enhancements)

| Issue | Effort | Impact |
|-------|--------|--------|
| Implement uiScale config option | Medium | Low |
| Increase minimum text size to 12px | Low | Low |
| Add option to focus error notifications | Low | Low |

---

## Testing Recommendations

### Manual Testing

1. **VoiceOver Testing**
   - Navigate entire UI with keyboard only
   - Verify all elements are announced
   - Check selection change announcements
   - Test error notification announcements

2. **High Contrast Testing**
   - Enable macOS "Increase Contrast"
   - Verify all UI elements remain visible
   - Check focus indicators are prominent

3. **Reduced Motion Testing**
   - Enable macOS "Reduce Motion"
   - Verify animations are disabled
   - Check cursor blinking behavior

### Automated Testing

```rust
#[cfg(test)]
mod accessibility_tests {
    // Contrast ratio tests
    #[test]
    fn test_text_contrast_ratios() {
        let dark_theme = ColorScheme::dark_default();
        assert!(contrast_ratio(dark_theme.text.muted, dark_theme.background.main) >= 4.5);
    }
    
    // Focus indicator tests
    #[test]
    fn test_focus_indicator_visible() {
        let colors = ListItemColors::from_theme(&theme);
        assert!(colors.accent_selected != colors.background);
    }
}
```

---

## References

- [WCAG 2.1 Guidelines](https://www.w3.org/WAI/WCAG21/quickref/)
- [Apple Accessibility Programming Guide](https://developer.apple.com/library/archive/documentation/Accessibility/Conceptual/AccessibilityMacOSX/)
- [NSAccessibility Protocol](https://developer.apple.com/documentation/appkit/nsaccessibility)
- [GPUI Framework](https://github.com/zed-industries/zed/tree/main/crates/gpui)

---

## Appendix: Color Contrast Calculations

### Formula
```
Contrast Ratio = (L1 + 0.05) / (L2 + 0.05)
Where L1 = lighter relative luminance, L2 = darker
```

### Key Contrast Ratios

| Foreground | Background | Ratio | Required | Status |
|------------|------------|-------|----------|--------|
| `#ffffff` | `#1e1e1e` | 15.1:1 | 4.5:1 | PASS |
| `#cccccc` | `#1e1e1e` | 10.1:1 | 4.5:1 | PASS |
| `#999999` | `#1e1e1e` | 5.3:1 | 4.5:1 | PASS |
| `#808080` | `#1e1e1e` | 3.9:1 | 4.5:1 | FAIL |
| `#666666` | `#1e1e1e` | 2.9:1 | 4.5:1 | FAIL |
| `#fbbf24` | `#1e1e1e` | 9.4:1 | 4.5:1 | PASS |

---

*This audit should be revisited after accessibility improvements are implemented.*
