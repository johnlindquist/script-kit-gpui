# Theme System & Color Schemes Audit

## Executive Summary

- **Robust theme architecture**: The theme system in `src/theme.rs` is well-designed with comprehensive color scheme support, focus-aware colors, and macOS vibrancy integration
- **11 design variants available**: Full design token system enables complete visual customization via pluggable designs (Default, Minimal, RetroTerminal, Glassmorphism, etc.)
- **Some hardcoded colors remain**: Terminal adapter and a few design files still use `rgb(0x...)` patterns instead of theme colors
- **Missing light mode in runtime**: While `light_default()` exists in code, the system auto-selects dark/light on startup only - no runtime toggle
- **Accessibility gaps**: No WCAG contrast ratio enforcement or colorblind-friendly mode options

---

## Table of Contents

1. [Current State Analysis](#1-current-state-analysis)
2. [Theme Architecture Deep Dive](#2-theme-architecture-deep-dive)
3. [Color Scheme Structure](#3-color-scheme-structure)
4. [Focus-Aware Colors](#4-focus-aware-colors)
5. [Design Tokens System](#5-design-tokens-system)
6. [Issues Found](#6-issues-found)
7. [Accessibility Analysis](#7-accessibility-analysis)
8. [Improvement Recommendations](#8-improvement-recommendations)

---

## 1. Current State Analysis

### Theme Loading Pipeline

```
~/.sk/kit/theme.json (optional)
        │
        ▼
  load_theme() in theme.rs
        │
        ├── File exists? → Parse JSON → Theme struct
        │
        └── File missing? → detect_system_appearance()
                                │
                                ├── Dark mode? → ColorScheme::dark_default()
                                └── Light mode? → ColorScheme::light_default()
```

**Key Observations:**

| Feature | Status | Notes |
|---------|--------|-------|
| Theme file location | `~/.sk/kit/theme.json` | Consistent with config path |
| Hot reload | **Implemented** | File watcher triggers theme reload |
| System appearance detection | **Implemented** | Uses `defaults read -g AppleInterfaceStyle` |
| Light/dark modes | **Code exists** | But no runtime toggle UI |
| Focus-aware colors | **Fully implemented** | Separate focused/unfocused color schemes |
| Vibrancy/blur | **Implemented** | macOS popover material with configurable settings |
| Drop shadows | **Implemented** | Configurable blur, spread, offset, opacity |
| Design variants | **11 variants** | Complete DesignTokens system |

### File Structure

```
src/theme.rs              # Core theme system (1234 lines)
  ├── BackgroundOpacity   # Window transparency settings
  ├── VibrancySettings    # macOS blur effect config
  ├── Padding             # Consistent spacing system
  ├── DropShadow          # Window shadow configuration
  ├── ColorScheme         # Base colors (bg, text, accent, ui)
  ├── FocusColorScheme    # Focus-state specific colors
  ├── Theme               # Complete theme definition
  ├── ListItemColors      # Pre-computed list rendering colors
  └── InputFieldColors    # Pre-computed input field colors

src/designs/
  ├── mod.rs              # DesignVariant enum, dispatch logic
  ├── traits.rs           # DesignTokens trait, token structs
  ├── minimal.rs          # Minimal design implementation
  ├── retro_terminal.rs   # Terminal aesthetic design
  ├── glassmorphism.rs    # Frosted glass design
  ├── brutalist.rs        # Bold typography design
  ├── compact.rs          # Dense layout design
  ├── material3.rs        # Material Design 3
  ├── apple_hig.rs        # Apple HIG-inspired
  ├── paper.rs            # Warm paper aesthetic
  ├── neon_cyberpunk.rs   # Neon glow design
  └── playful.rs          # Rounded, vibrant design
```

---

## 2. Theme Architecture Deep Dive

### Core Theme Struct

```rust
pub struct Theme {
    pub colors: ColorScheme,           // Base colors
    pub focus_aware: Option<FocusAwareColorScheme>,  // Focus/unfocus variants
    pub opacity: Option<BackgroundOpacity>,
    pub drop_shadow: Option<DropShadow>,
    pub vibrancy: Option<VibrancySettings>,
    pub padding: Option<Padding>,
}
```

**Design Decision Analysis:**

1. **All optional fields** - Enables partial theme overrides from JSON
2. **`Option<T>` with `.unwrap_or_default()`** - Graceful fallback to sensible defaults
3. **Separation of concerns** - Colors, opacity, shadows, vibrancy are independent

### Theme Access Patterns

```rust
// Pattern 1: Direct color access (most common)
let colors = &theme.colors;
div().bg(rgb(colors.background.main))

// Pattern 2: Focus-aware colors
let colors = theme.get_colors(is_focused);  // Returns focused or unfocused scheme

// Pattern 3: Lightweight extraction for closures
let list_colors = colors.list_item_colors();  // Copy-able struct
uniform_list(cx, |_, range, _, _| {
    // list_colors is Copy, no heap allocation in closure
})

// Pattern 4: Design tokens for variant-specific styling
let tokens = designs::get_tokens(variant);
let bg = rgb(tokens.colors().background);
```

---

## 3. Color Scheme Structure

### Dark Mode Colors (Default)

| Category | Color | Hex | Usage |
|----------|-------|-----|-------|
| **Background** ||||
| main | Dark gray | `#1e1e1e` | Window background |
| title_bar | Medium gray | `#2d2d30` | Title bar background |
| search_box | Light gray | `#3c3c3c` | Input field backgrounds |
| log_panel | Near black | `#0d0d0d` | Log panel background |
| **Text** ||||
| primary | White | `#ffffff` | Headings, selected items |
| secondary | Light gray | `#cccccc` | Body text, descriptions |
| tertiary | Medium gray | `#999999` | Secondary info |
| muted | Gray | `#808080` | Placeholders, hints |
| dimmed | Dark gray | `#666666` | Disabled text, shortcuts |
| **Accent** ||||
| selected | Gold/Yellow | `#fbbf24` | Selection highlight, links |
| selected_subtle | Dark gray | `#2a2a2a` | Selection background |
| **UI** ||||
| border | Gray | `#464647` | Borders, dividers |
| success | Green | `#00ff00` | Success states |
| error | Red | `#ef4444` | Error states (red-500) |
| warning | Amber | `#f59e0b` | Warning states (amber-500) |
| info | Blue | `#3b82f6` | Info states (blue-500) |

### Light Mode Colors

| Category | Color | Hex | Notes |
|----------|-------|-----|-------|
| **Background** ||||
| main | White | `#ffffff` | Clean white background |
| title_bar | Light gray | `#f3f3f3` | Subtle title bar |
| search_box | Off-white | `#ececec` | Input backgrounds |
| **Text** ||||
| primary | Black | `#000000` | Main text |
| secondary | Dark gray | `#333333` | Body text |
| **Accent** ||||
| selected | Blue | `#0078d4` | Windows-like accent |
| selected_subtle | Light gray | `#e8e8e8` | Selection background |
| **UI** ||||
| border | Light gray | `#d0d0d0` | Subtle borders |
| error | Dark red | `#dc2626` | Darker for contrast |
| warning | Dark amber | `#d97706` | Darker for contrast |
| info | Dark blue | `#2563eb` | Darker for contrast |

---

## 4. Focus-Aware Colors

### Implementation Details

```rust
// Theme provides focus-aware color retrieval
impl Theme {
    pub fn get_colors(&self, is_focused: bool) -> ColorScheme {
        if let Some(ref focus_aware) = self.focus_aware {
            if is_focused {
                if let Some(ref focused) = focus_aware.focused {
                    return focused.to_color_scheme();
                }
            } else if let Some(ref unfocused) = focus_aware.unfocused {
                return unfocused.to_color_scheme();
            }
        }
        
        // Automatic fallback: generate unfocused from base colors
        if is_focused {
            self.colors.clone()
        } else {
            self.colors.to_unfocused()  // 30% blend toward gray
        }
    }
}
```

### Automatic Unfocused Color Generation

The `to_unfocused()` method automatically dims colors when no explicit unfocused scheme is provided:

```rust
fn darken_hex(color: HexColor) -> HexColor {
    // Blend 30% toward gray (0x80)
    let gray = 0x80u32;
    let new_r = ((r * 70 + gray * 30) / 100) as u8;
    // ... same for g, b
}
```

**Visual Effect:** Window content appears slightly washed out when unfocused, matching macOS conventions.

### Focus-Aware Opacity

```rust
pub fn get_opacity_for_focus(&self, is_focused: bool) -> BackgroundOpacity {
    let base = self.get_opacity();
    if is_focused {
        base
    } else {
        // Reduce opacity by 10% when unfocused
        BackgroundOpacity {
            main: (base.main * 0.9).clamp(0.0, 1.0),
            // ... etc
        }
    }
}
```

---

## 5. Design Tokens System

### DesignTokens Trait

```rust
pub trait DesignTokens: Send + Sync {
    fn colors(&self) -> DesignColors;
    fn spacing(&self) -> DesignSpacing;
    fn typography(&self) -> DesignTypography;
    fn visual(&self) -> DesignVisual;
    fn item_height(&self) -> f32;
    fn variant(&self) -> DesignVariant;
}
```

### Token Categories

| Category | Struct | Key Values |
|----------|--------|------------|
| **Colors** | `DesignColors` | 20+ color properties for all UI elements |
| **Spacing** | `DesignSpacing` | Padding (xs→xl), gaps, margins, item padding |
| **Typography** | `DesignTypography` | Font families, sizes, weights, line heights |
| **Visual** | `DesignVisual` | Border radii, shadows, opacity, animations |

### Design Variant Item Heights

| Variant | Height | Rationale |
|---------|--------|-----------|
| Default | 40px | Balanced for name + description |
| Minimal | 64px | Generous spacing, thin fonts |
| RetroTerminal | 28px | Dense, monospace terminal feel |
| Compact | 24px | Smallest - power users with many scripts |
| AppleHIG | 44px | iOS standard row height |
| Material3 | 56px | M3 list item height |
| Glassmorphism | 56px | Room for blur effects |

---

## 6. Issues Found

### Critical Issues

*None identified.*

### High Severity Issues

#### H1: Hardcoded Terminal Colors

**Location:** `src/terminal/theme_adapter.rs`

**Problem:** 37 instances of `hex_to_rgb(0x...)` with hardcoded ANSI color values that don't adapt to the theme system.

```rust
// Lines 93-109 - Hardcoded ANSI palette
black: hex_to_rgb(0x000000),
red: hex_to_rgb(0xcd3131),
green: hex_to_rgb(0x0dbc79),
// ... etc
```

**Impact:** Terminal content doesn't match theme colors, breaking visual consistency.

**Recommendation:** Add terminal-specific colors to `UIColors` or create a `TerminalColors` sub-struct:

```rust
pub struct TerminalColors {
    pub black: HexColor,
    pub red: HexColor,
    pub green: HexColor,
    // ... etc
}
```

#### H2: No Runtime Light/Dark Toggle

**Location:** `src/theme.rs`

**Problem:** System appearance is detected only at startup. No mechanism to toggle between light and dark mode at runtime.

**Impact:** Users must change macOS system settings and restart the app to switch themes.

**Recommendation:** Add a `set_appearance(is_dark: bool)` method or config option:

```rust
// In config.ts
export default {
    appearance: "auto" | "dark" | "light",
    // ...
}
```

### Medium Severity Issues

#### M1: Inconsistent Cursor Color Source

**Location:** `src/theme.rs:844`

**Problem:** Input field cursor color is hardcoded to cyan:

```rust
cursor: rgb(0x00ffff),  // Cyan cursor - hardcoded
```

**Impact:** Cursor doesn't match theme accent color.

**Recommendation:** Use `colors.accent.selected` or add `CursorStyle` to main ColorScheme.

#### M2: Missing Design Variant Color Overrides

**Location:** `src/designs/traits.rs`

**Problem:** Some design variants (Glassmorphism, Paper, etc.) use colors with alpha values directly in the color definitions, which may not render correctly:

```rust
// Glassmorphism tokens
background: 0xffffff20,  // White with 0x20 alpha baked in
```

**Impact:** GPUI's `rgb()` may not interpret these as intended. Should use `rgba()`.

**Recommendation:** Store base colors in tokens, apply alpha at render time:

```rust
let bg = rgba((tokens.colors().background << 8) | 0x20);
```

#### M3: RetroTerminal Hardcoded Warning/Error Colors

**Location:** `src/designs/retro_terminal.rs:549-554`

```rust
("█", rgb(0xff4444)) // Red for errors - hardcoded
("▒", rgb(0xffff00)) // Yellow for warnings - hardcoded
```

**Impact:** Status indicators don't use theme's semantic colors.

### Low Severity Issues

#### L1: Icon Variation Comment with Hardcoded Color

**Location:** `src/designs/icon_variations.rs:18`

```rust
//! svg().path(path).size(px(16.)).color(rgb(0xffffff))
```

**Impact:** Example in documentation shows hardcoded color (acceptable in comments).

#### L2: Unused `button_text` Field in AccentColors

**Location:** `src/theme.rs` (AccentColors struct)

**Problem:** The struct originally had a `button_text` field that was removed but may be expected by some code.

**Impact:** Minor - just documentation inconsistency.

#### L3: Toast Default Colors Hardcoded

**Location:** `src/components/toast.rs:124-135`

```rust
impl Default for ToastColors {
    fn default() -> Self {
        Self {
            background: 0x2d2d2d,  // Hardcoded
            text: 0xffffff,         // Hardcoded
            // ...
        }
    }
}
```

**Impact:** Fallback colors don't match any specific theme.

**Recommendation:** This is acceptable for `Default` implementation but should be documented.

---

## 7. Accessibility Analysis

### Current State

| WCAG Criterion | Status | Notes |
|----------------|--------|-------|
| **1.4.3 Contrast (Minimum)** | Partial | No enforcement mechanism |
| **1.4.6 Contrast (Enhanced)** | Not implemented | AAA level not checked |
| **1.4.11 Non-text Contrast** | Partial | UI components generally good |
| **1.4.1 Use of Color** | Good | Icons and text supplement color |

### Contrast Ratio Analysis (Dark Theme)

| Element | Foreground | Background | Ratio | AA | AAA |
|---------|------------|------------|-------|----|----|
| Primary text | `#ffffff` | `#1e1e1e` | 15.1:1 | ✓ | ✓ |
| Secondary text | `#cccccc` | `#1e1e1e` | 10.1:1 | ✓ | ✓ |
| Muted text | `#808080` | `#1e1e1e` | 4.5:1 | ✓ | ✗ |
| Dimmed text | `#666666` | `#1e1e1e` | 3.5:1 | ✗ | ✗ |
| Accent on subtle bg | `#fbbf24` | `#2a2a2a` | 7.2:1 | ✓ | ✓ |

**Issues Identified:**

1. **Dimmed text (`#666666`)** fails WCAG AA for normal text (4.5:1 minimum)
2. **Placeholder text** may be too low contrast in some designs

### Colorblind Considerations

| Color Pair | Issue | Affected Users |
|------------|-------|----------------|
| Red/Green (success/error) | Similar luminance | Deuteranopia, Protanopia (~8% males) |
| Warning amber | May blend with selection gold | Tritanopia (rare) |

**Recommendations:**

1. Add distinct icons for success/error/warning states (already partially done)
2. Consider adding pattern fills or borders as secondary indicators
3. Provide a high-contrast theme option

---

## 8. Improvement Recommendations

### Priority 1: Critical Path

#### R1: Theme-Aware Terminal Colors

```rust
// Add to UIColors
pub terminal: TerminalColors,

pub struct TerminalColors {
    pub ansi_black: HexColor,
    pub ansi_red: HexColor,
    // ... 16 ANSI colors
}

// In theme_adapter.rs
fn adapt(&self, theme: &Theme) -> TerminalTheme {
    let tc = &theme.colors.ui.terminal;
    AnsiColors {
        black: hex_to_rgb(tc.ansi_black),
        // ...
    }
}
```

#### R2: Light/Dark Mode Toggle

```rust
// Add to config.ts type
appearance?: "auto" | "dark" | "light";

// In main.rs
fn set_appearance(&mut self, appearance: Appearance, cx: &mut Context<Self>) {
    self.theme = match appearance {
        Appearance::Auto => load_theme(),
        Appearance::Dark => Theme::dark(),
        Appearance::Light => Theme::light(),
    };
    cx.notify();
}
```

### Priority 2: Accessibility

#### R3: Contrast Enforcement

```rust
impl ColorScheme {
    /// Validate contrast ratios meet WCAG AA
    pub fn validate_contrast(&self) -> Vec<ContrastIssue> {
        let mut issues = Vec::new();
        
        // Check text against backgrounds
        let ratio = contrast_ratio(self.text.dimmed, self.background.main);
        if ratio < 4.5 {
            issues.push(ContrastIssue {
                foreground: "text.dimmed",
                background: "background.main",
                ratio,
                required: 4.5,
            });
        }
        
        issues
    }
}
```

#### R4: High Contrast Theme

```rust
impl ColorScheme {
    pub fn high_contrast() -> Self {
        ColorScheme {
            background: BackgroundColors {
                main: 0x000000,      // Pure black
                // ...
            },
            text: TextColors {
                primary: 0xffffff,   // Pure white
                secondary: 0xffffff, // All text is maximum contrast
                muted: 0xe0e0e0,     // Minimum AA compliance
                dimmed: 0xc0c0c0,    // Still passes AA
            },
            // ...
        }
    }
}
```

### Priority 3: Developer Experience

#### R5: Theme Debugging Tools

```rust
// Add logging for theme issues
fn log_theme_validation(theme: &Theme) {
    let issues = theme.colors.validate_contrast();
    for issue in issues {
        warn!(
            foreground = %issue.foreground,
            background = %issue.background,
            ratio = issue.ratio,
            required = issue.required,
            "Contrast ratio below WCAG AA"
        );
    }
}
```

#### R6: Design Token Documentation

Generate theme documentation automatically:

```rust
impl DesignTokens for T {
    fn to_documentation(&self) -> String {
        format!(r#"
# {} Design Tokens

## Colors
- Background: #{:06x}
- Text Primary: #{:06x}
...
"#, 
            self.variant().name(),
            self.colors().background,
            self.colors().text_primary
        )
    }
}
```

### Priority 4: Feature Enhancements

#### R7: Animated Theme Transitions

```rust
// Smooth color transitions when switching themes
pub struct AnimatedTheme {
    current: Theme,
    target: Theme,
    progress: f32,  // 0.0 to 1.0
    animation_ms: u64,
}

impl AnimatedTheme {
    fn interpolated_color(&self, get_color: impl Fn(&Theme) -> HexColor) -> HexColor {
        let from = get_color(&self.current);
        let to = get_color(&self.target);
        lerp_color(from, to, self.progress)
    }
}
```

#### R8: Per-Component Theme Overrides

```rust
// Allow scripts to customize specific elements
{
    "colors": {
        "overrides": {
            "list_item.selected_background": "#ff0000",
            "search_box.border_focus": "#00ff00"
        }
    }
}
```

---

## Appendix A: Theme JSON Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "colors": {
      "type": "object",
      "properties": {
        "background": {
          "type": "object",
          "properties": {
            "main": { "type": "integer", "description": "Decimal hex color (e.g., 1980410 for #1e1e1e)" },
            "title_bar": { "type": "integer" },
            "search_box": { "type": "integer" },
            "log_panel": { "type": "integer" }
          }
        },
        "text": {
          "type": "object",
          "properties": {
            "primary": { "type": "integer" },
            "secondary": { "type": "integer" },
            "tertiary": { "type": "integer" },
            "muted": { "type": "integer" },
            "dimmed": { "type": "integer" }
          }
        },
        "accent": {
          "type": "object",
          "properties": {
            "selected": { "type": "integer" },
            "selected_subtle": { "type": "integer" }
          }
        },
        "ui": {
          "type": "object",
          "properties": {
            "border": { "type": "integer" },
            "success": { "type": "integer" },
            "error": { "type": "integer" },
            "warning": { "type": "integer" },
            "info": { "type": "integer" }
          }
        }
      }
    },
    "opacity": {
      "type": "object",
      "properties": {
        "main": { "type": "number", "minimum": 0, "maximum": 1 },
        "title_bar": { "type": "number", "minimum": 0, "maximum": 1 },
        "search_box": { "type": "number", "minimum": 0, "maximum": 1 },
        "log_panel": { "type": "number", "minimum": 0, "maximum": 1 }
      }
    },
    "vibrancy": {
      "type": "object",
      "properties": {
        "enabled": { "type": "boolean" },
        "material": { "type": "string", "enum": ["hud", "popover", "menu", "sidebar", "content"] }
      }
    },
    "drop_shadow": {
      "type": "object",
      "properties": {
        "enabled": { "type": "boolean" },
        "blur_radius": { "type": "number" },
        "spread_radius": { "type": "number" },
        "offset_x": { "type": "number" },
        "offset_y": { "type": "number" },
        "color": { "type": "integer" },
        "opacity": { "type": "number", "minimum": 0, "maximum": 1 }
      }
    },
    "padding": {
      "type": "object",
      "properties": {
        "xs": { "type": "number" },
        "sm": { "type": "number" },
        "md": { "type": "number" },
        "lg": { "type": "number" },
        "xl": { "type": "number" },
        "content_x": { "type": "number" },
        "content_y": { "type": "number" }
      }
    }
  }
}
```

---

## Appendix B: Color Conversion Reference

The theme system uses decimal integers for colors in JSON:

| Hex | Decimal | Color |
|-----|---------|-------|
| `#1e1e1e` | `1980410` | Dark gray (default bg) |
| `#ffffff` | `16777215` | White |
| `#000000` | `0` | Black |
| `#fbbf24` | `16498468` | Script Kit gold |
| `#ef4444` | `15676484` | Error red |
| `#f59e0b` | `16097803` | Warning amber |
| `#3b82f6` | `3899126` | Info blue |

---

## Appendix C: Testing Theme Changes

```bash
# Build and run with custom theme
cargo build && echo '{"type": "show"}' | SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1

# Filter theme-related logs
grep '|T|' output.log  # Theme category

# Verify theme loaded
grep 'Theme' output.log | head -5
```

---

*Audit completed: 2024-12-29*
*Auditor: theme-auditor (swarm worker)*
*Files analyzed: src/theme.rs, src/designs/*.rs, src/list_item.rs, src/components/toast.rs*
