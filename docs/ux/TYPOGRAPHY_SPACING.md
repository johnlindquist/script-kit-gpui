# Typography & Spacing System Audit

## Executive Summary

This document provides a comprehensive audit of the typography and spacing patterns used across the Script Kit GPUI codebase. The system implements a well-structured design token architecture with configurable font sizes, consistent spacing scales, and design-variant-aware typography.

---

## 1. Font Size Definitions

### 1.1 Config-Driven Font Sizes

The application supports user-configurable font sizes via `~/.sk/kit/config.ts`:

| Setting | Default | Location | Usage |
|---------|---------|----------|-------|
| `editorFontSize` | 14.0px | `src/config.rs` | Code editor prompt |
| `terminalFontSize` | 14.0px | `src/config.rs` | Terminal prompt |

**Implementation:**
```rust
// src/config.rs
pub const DEFAULT_EDITOR_FONT_SIZE: f32 = 14.0;
pub const DEFAULT_TERMINAL_FONT_SIZE: f32 = 14.0;

impl Config {
    pub fn get_editor_font_size(&self) -> f32 {
        self.editor_font_size.unwrap_or(DEFAULT_EDITOR_FONT_SIZE)
    }
    pub fn get_terminal_font_size(&self) -> f32 {
        self.terminal_font_size.unwrap_or(DEFAULT_TERMINAL_FONT_SIZE)
    }
}
```

### 1.2 Design Token Typography Scale

The design system defines a complete typography scale in `src/designs/traits.rs`:

| Token | Default | Minimal | Compact | Retro Terminal | Apple HIG | Material3 |
|-------|---------|---------|---------|----------------|-----------|-----------|
| `font_size_xs` | 10.0 | 10.0 | 9.0 | 10.0 | 11.0 | 11.0 |
| `font_size_sm` | 12.0 | 12.0 | 10.0 | 12.0 | 13.0 | 12.0 |
| `font_size_md` | 14.0 | 16.0 | 11.0 | 13.0 | 15.0 | 14.0 |
| `font_size_lg` | 16.0 | 18.0 | 12.0 | 14.0 | 17.0 | 16.0 |
| `font_size_xl` | 20.0 | 22.0 | 14.0 | 16.0 | 20.0 | 22.0 |
| `font_size_title` | 24.0 | 28.0 | 16.0 | 18.0 | 28.0 | 28.0 |

### 1.3 GPUI Text Size Methods

The codebase uses GPUI's built-in text size methods extensively:

| Method | Approximate Size | Usage Count | Primary Use Cases |
|--------|-----------------|-------------|-------------------|
| `.text_xs()` | ~10-11px | 60+ | Shortcuts, hints, metadata, status text |
| `.text_sm()` | ~12-13px | 40+ | Descriptions, secondary text, labels |
| `.text_base()` | ~14px | 5+ | Standard body text |
| `.text_lg()` | ~16-18px | 25+ | Names, titles, primary content |
| `.text_xl()` | ~20px | 2+ | Main headings |

---

## 2. Line Height Patterns

### 2.1 Line Height Multipliers

| Context | Multiplier | Calculation | Notes |
|---------|------------|-------------|-------|
| Editor | 1.43 | `font_size * 1.43` | ~20px for 14pt font |
| Terminal | 1.3 | `font_size * 1.3` | Room for descenders |
| Default design | 1.5 | `font_size * 1.5` | Normal reading |
| Tight | 1.2 | `font_size * 1.2` | Compact layouts |
| Relaxed | 1.75 | `font_size * 1.75` | Generous spacing |

### 2.2 Design-Variant Line Heights

| Design Variant | Tight | Normal | Relaxed |
|----------------|-------|--------|---------|
| Default | 1.2 | 1.5 | 1.75 |
| Minimal | 1.3 | 1.6 | 1.8 |
| Compact | 1.1 | 1.2 | 1.3 |
| Retro Terminal | 1.1 | 1.3 | 1.5 |
| Brutalist | 1.1 | 1.4 | 1.6 |
| Apple HIG | 1.2 | 1.4 | 1.6 |
| Material3 | 1.2 | 1.5 | 1.75 |
| Paper | 1.3 | 1.6 | 1.8 |

### 2.3 Explicit Line Height Usage

```rust
// src/list_item.rs - Fixed line heights for list items
.line_height(px(18.))  // Name text
.line_height(px(14.))  // Description text

// src/term_prompt.rs - Dynamic line height
.line_height(px(cell_height))  // Uses calculated cell height

// src/editor.rs - Line height from config
fn line_height(&self) -> f32 {
    self.font_size() * LINE_HEIGHT_MULTIPLIER  // 1.43
}
```

---

## 3. Font Weight Variations

### 3.1 Weight Usage Patterns

| Weight | Constant | Primary Uses |
|--------|----------|--------------|
| THIN (100) | `FontWeight::THIN` | Minimal design default |
| LIGHT (300) | `FontWeight::LIGHT` | Subtle text, hints |
| NORMAL (400) | `FontWeight::NORMAL` | Body text, descriptions |
| MEDIUM (500) | `FontWeight::MEDIUM` | Labels, list item names, buttons |
| SEMIBOLD (600) | `FontWeight::SEMIBOLD` | Headings, selected items |
| BOLD (700) | `FontWeight::BOLD` | Section headers, emphasis |

### 3.2 Design-Variant Weight Mapping

Different designs remap weights for their aesthetic:

| Design | "normal" maps to | "bold" maps to |
|--------|-----------------|----------------|
| Default | NORMAL | BOLD |
| Minimal | THIN | MEDIUM |
| Compact | NORMAL | BOLD |
| Retro Terminal | NORMAL | BOLD |
| Brutalist | MEDIUM | BLACK |

### 3.3 Common Weight Patterns

```rust
// List items - Medium for names
.font_weight(FontWeight::MEDIUM)

// Section headers - Bold for visibility
.font_weight(FontWeight::BOLD)

// Selected items - Conditional weight
.font_weight(if is_selected { FontWeight::MEDIUM } else { FontWeight::NORMAL })
```

---

## 4. Spacing & Padding Patterns

### 4.1 Config-Driven Content Padding

User-configurable padding in `src/config.rs`:

| Setting | Default | Usage |
|---------|---------|-------|
| `padding.top` | 8.0px | Terminal, editor top padding |
| `padding.left` | 12.0px | Terminal, editor left padding |
| `padding.right` | 12.0px | Terminal, editor right padding |

### 4.2 Design Token Spacing Scale

**Padding Scale:**

| Token | Default | Minimal | Compact | Glassmorphism |
|-------|---------|---------|---------|---------------|
| `padding_xs` | 4.0 | 8.0 | 2.0 | 6.0 |
| `padding_sm` | 8.0 | 16.0 | 4.0 | 12.0 |
| `padding_md` | 12.0 | 24.0 | 6.0 | 16.0 |
| `padding_lg` | 16.0 | 32.0 | 8.0 | 20.0 |
| `padding_xl` | 24.0 | 48.0 | 12.0 | 28.0 |

**Gap Scale:**

| Token | Default | Minimal | Compact | Retro Terminal |
|-------|---------|---------|---------|----------------|
| `gap_sm` | 4.0 | 8.0 | 2.0 | 2.0 |
| `gap_md` | 8.0 | 16.0 | 4.0 | 4.0 |
| `gap_lg` | 16.0 | 24.0 | 8.0 | 8.0 |

**Margin Scale:**

| Token | Default | Range Across Designs |
|-------|---------|---------------------|
| `margin_sm` | 4.0 | 2.0 - 8.0 |
| `margin_md` | 8.0 | 4.0 - 16.0 |
| `margin_lg` | 16.0 | 8.0 - 24.0 |

### 4.3 Component-Specific Spacing

**List Items (`src/list_item.rs`):**
```rust
pub const LIST_ITEM_HEIGHT: f32 = 40.0;
pub const SECTION_HEADER_HEIGHT: f32 = 24.0;
pub const ACCENT_BAR_WIDTH: f32 = 3.0;

// Item padding
.px(px(12.))  // Horizontal padding
.py(px(6.))   // Vertical padding
.gap(px(8.))  // Icon-to-text gap
```

**Editor (`src/editor.rs`):**
```rust
const GUTTER_WIDTH: f32 = 50.0;
const STATUS_BAR_HEIGHT: f32 = 28.0;
// Uses config-driven padding
.pt(px(padding.top))
.pl(px(padding.left))
.pr(px(padding.right))
```

**Terminal (`src/term_prompt.rs`):**
```rust
const MIN_COLS: u16 = 20;
const MIN_ROWS: u16 = 5;
// Cell dimensions scale with font
const BASE_CELL_WIDTH: f32 = 8.5;  // For Menlo 14pt
```

---

## 5. Gap & Margin Usage

### 5.1 GPUI Gap Methods

| Method | Size | Common Uses |
|--------|------|-------------|
| `.gap_1()` | 4px | Tight inline elements |
| `.gap_2()` | 8px | Standard element spacing |
| `.gap_3()` | 12px | Section spacing |
| `.gap_4()` | 16px | Large separations |
| `.gap(px(N))` | Custom | Design token values |

### 5.2 Gap Usage by Context

```rust
// Header elements
.gap_2()  // Search box icons

// List layouts
.gap_3()  // Main content sections

// Status bars
.gap_4()  // Status bar items (editor.rs)

// Design token usage
.gap(px(spacing.gap_md))  // Theme-aware gaps
```

### 5.3 Margin Patterns

The codebase prefers padding over margins, but uses:
- `.mx(px(4.))` - Horizontal margins for inline spacing
- `.my(px(2.))` - Cursor vertical margins
- `.mr(px(N))` - Right margins for trailing elements

---

## 6. Typography Hierarchy

### 6.1 Heading Levels (Implied)

| Level | Size | Weight | Use Case |
|-------|------|--------|----------|
| H1 | `.text_xl()` | BOLD | Main titles |
| H2 | `.text_lg()` + SEMIBOLD | SEMIBOLD | Section headings |
| H3 | `.text_lg()` | MEDIUM | Subsection headings |
| Body | `.text_base()` / `.text_sm()` | NORMAL | Content text |
| Caption | `.text_xs()` | NORMAL/MUTED | Metadata, hints |

### 6.2 List Item Hierarchy

```rust
// Primary text (name)
.text_size(px(14.))
.font_weight(FontWeight::MEDIUM)
.line_height(px(18.))

// Secondary text (description)
.text_size(px(12.))
.line_height(px(14.))
.text_color(rgb(colors.text_muted))

// Tertiary text (shortcut badge)
.text_size(px(11.))
.text_color(rgb(colors.text_dimmed))
```

### 6.3 Editor Typography

```rust
// Line numbers
.text_color(rgb(colors.text.muted))
.px_2()

// Code content
.font_family("Menlo")
.text_size(px(font_size))  // Config-driven

// Status bar
.text_xs()
.text_color(rgb(colors.text.secondary))
```

---

## 7. Font Family Patterns

### 7.1 System Fonts

| Usage | Font Family | Notes |
|-------|-------------|-------|
| UI Text | `.AppleSystemUIFont` | macOS system font |
| Code/Terminal | `Menlo` | Monospace for code |
| Alternative Mono | `SF Mono` | Compact design |
| Paper Design | `Georgia` | Serif for paper aesthetic |
| Brutalist | `Helvetica Neue` | Clean sans-serif |

### 7.2 Design Token Fonts

```rust
// Default typography tokens
font_family: ".AppleSystemUIFont",
font_family_mono: "Menlo",

// Retro Terminal - all monospace
font_family: "Menlo",
font_family_mono: "Menlo",

// Paper Design - serif
font_family: "Georgia",
font_family_mono: "Courier New",
```

---

## 8. Responsive Considerations

### 8.1 Dynamic Sizing

The system uses dynamic sizing based on font configuration:

```rust
// Terminal cell sizing scales with font
fn cell_width(&self) -> f32 {
    BASE_CELL_WIDTH * (self.font_size() / BASE_FONT_SIZE)
}

fn cell_height(&self) -> f32 {
    self.font_size() * LINE_HEIGHT_MULTIPLIER
}
```

### 8.2 UI Scale Factor

```rust
// Config supports global UI scaling
pub const DEFAULT_UI_SCALE: f32 = 1.0;

pub fn get_ui_scale(&self) -> f32 {
    self.ui_scale.unwrap_or(DEFAULT_UI_SCALE)
}
```

### 8.3 Flexible Height Patterns

```rust
// Critical pattern for proper sizing
.flex_1()      // Grow to fill available space
.min_h(px(0.)) // Allow shrinking (prevents overflow)
.h_full()      // Fill parent completely
```

---

## 9. Fixed Dimension Constants

### 9.1 Component Heights

| Component | Height | Location |
|-----------|--------|----------|
| List Item | 40.0px | `list_item.rs` |
| Section Header | 24.0px | `list_item.rs` |
| Status Bar | 28.0px | `editor.rs` |
| Input Field | 24.0px | `main.rs` |
| Action Buttons | 28.0px | `main.rs` |
| Cursor (lg) | 18.0px | `traits.rs` |
| Cursor (input) | 14.0px | `main.rs` |

### 9.2 Component Widths

| Component | Width | Location |
|-----------|-------|----------|
| Toast | 380.0px | `main.rs` |
| Accent Bar | 3.0px | `list_item.rs` |
| Cursor | 2.0px | `editor.rs` |
| Gutter | 50.0px | `editor.rs` |
| Icon Container | 20.0px | `list_item.rs` |

---

## 10. Identified Patterns & Recommendations

### 10.1 Strengths

1. **Design Token System**: Well-structured `DesignTypography` struct with complete scale
2. **Config-Driven Sizing**: User-configurable font sizes for editor/terminal
3. **Variant Support**: 10 design variants with customized typography
4. **Consistent Constants**: Fixed heights for list items, headers, etc.
5. **Responsive Calculations**: Cell sizes scale with font configuration

### 10.2 Potential Improvements

1. **Magic Numbers**: Some hardcoded px values (e.g., `px(28.0)`, `px(14.)`) could be design tokens
2. **Line Height Inconsistency**: Mix of explicit `line_height(px(N))` and multipliers
3. **Text Size Methods**: Mix of `.text_sm()` and `.text_size(px(12.))` - could standardize
4. **Font Weight Centralization**: Weights used directly rather than via design tokens in some places

### 10.3 Key Patterns to Follow

```rust
// Preferred: Use design tokens
let spacing = tokens.spacing();
let typography = tokens.typography();
.gap(px(spacing.gap_md))
.text_size(px(typography.font_size_sm))

// Preferred: Use config for user settings
let font_size = self.config.get_editor_font_size();
let padding = self.config.get_padding();

// Preferred: Dynamic line height
fn line_height(&self) -> f32 {
    self.font_size() * LINE_HEIGHT_MULTIPLIER
}
```

---

## 11. Summary Statistics

| Category | Count/Range |
|----------|-------------|
| Font size tokens | 6 per design variant |
| Font weight tokens | 6 per design variant |
| Line height tokens | 3 per design variant |
| Spacing tokens | 14 per design variant |
| Design variants | 10 |
| Config font settings | 2 (editor, terminal) |
| Config padding settings | 3 (top, left, right) |
| Fixed component heights | 8+ |
| `.text_*()` method usages | 100+ across codebase |
| `.gap*()` method usages | 100+ across codebase |

---

*Audit generated: 2024*
*Files analyzed: config.rs, designs/traits.rs, editor.rs, list_item.rs, term_prompt.rs, prompts.rs, main.rs, actions.rs, components/*
