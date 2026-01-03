# GPUI and gpui-component REM Sizing System

## Overview

The rem (root em) sizing system provides scalable, consistent sizing across the UI. This document explains how gpui-component's rem system works and how Script Kit integrates with it.

## Architecture Flow

```
Script Kit Theme (theme.json)     gpui-component Theme              GPUI Window
       ↓                                 ↓                              ↓
   fonts.ui_size           →      theme.font_size         →    window.rem_size
  (e.g., 16.0)                    (px(16.0))                   (Pixels(16.0))
                                                                      ↓
                                                              Element Rendering
                                                                      ↓
                                                           rem values → pixels
```

## 1. How Root Component Sets Window rem Size

The gpui-component `Root` component is the **critical link** between the theme's font_size and the window's rem_size. It must be the first view in any window.

### Root's Render Method (src/root.rs:347-364)

```rust
impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // THIS IS THE KEY LINE - sets window rem_size from theme
        window.set_rem_size(cx.theme().font_size);

        window_border().child(
            div()
                .id("root")
                .relative()
                .size_full()
                .font_family(cx.theme().font_family.clone())
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(self.view.clone()),
        )
    }
}
```

**Key Points:**
- `window.set_rem_size(cx.theme().font_size)` is called on **every render**
- This ensures the rem size stays synchronized with theme changes
- The `font_size` comes from gpui-component's `Theme` struct (default: 16px)

### Creating a Window with Root

```rust
// REQUIRED pattern - views must be wrapped in Root
let handle = cx.open_window(window_options, |window, cx| {
    let view = cx.new(|cx| MyApp::new(window, cx));
    cx.new(|cx| Root::new(view, window, cx))  // <-- Root wrapper
})?;
```

## 2. How font_size Flows from Script Kit to gpui-component

Script Kit syncs its theme to gpui-component in `src/theme.rs`:

### Theme Synchronization (src/theme.rs:1408-1419)

```rust
// Get font configuration from Script Kit theme
let fonts = sk_theme.get_fonts();

// Apply the custom colors and fonts to the global theme
let theme = GpuiTheme::global_mut(cx);
theme.colors = custom_colors;
theme.mode = ThemeMode::Dark;

// Set monospace font for code editor
theme.mono_font_family = fonts.mono_family.clone().into();
theme.mono_font_size = gpui::px(fonts.mono_size);

// Set UI font - THIS CONTROLS REM SIZE
theme.font_family = fonts.ui_family.clone().into();
theme.font_size = gpui::px(fonts.ui_size);  // e.g., px(16.0)
```

### gpui-component Theme Struct (theme/mod.rs:42-82)

```rust
pub struct Theme {
    pub colors: ThemeColor,
    // ...
    
    /// The base font size for the application, default is 16px.
    /// THIS BECOMES THE WINDOW'S REM SIZE
    pub font_size: Pixels,
    
    /// The monospace font size for the application, default is 13px.
    pub mono_font_size: Pixels,
    // ...
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            font_size: px(16.),  // Default rem base
            mono_font_size: px(13.),
            // ...
        }
    }
}
```

## 3. What rem() Does vs px() in GPUI

### Pixels (px) - Absolute Sizing

```rust
// From gpui/src/geometry.rs:3608-3610
pub const fn px(pixels: f32) -> Pixels {
    Pixels(pixels)
}
```

- **Absolute**: Always renders at exactly the specified size
- **Not scalable**: Ignores window rem_size
- **Use case**: Fixed-size elements, borders, icons with specific pixel requirements

### Rems - Relative Sizing

```rust
// From gpui/src/geometry.rs:3111-3129
/// Represents a length in rems, a unit based on the font-size of the window,
/// which can be assigned with [`Window::set_rem_size`].
#[derive(Clone, Copy, Default, Add, Sub, Mul, Div, Neg, PartialEq)]
pub struct Rems(pub f32);

impl Rems {
    /// Convert this Rem value to pixels.
    pub fn to_pixels(self, rem_size: Pixels) -> Pixels {
        self * rem_size  // e.g., 0.875 * 16px = 14px
    }
}

// Helper function
pub const fn rems(rems: f32) -> Rems {
    Rems(rems)
}
```

- **Relative**: Scales based on `window.rem_size()`
- **Scalable**: Changing theme.font_size scales all rem-based elements proportionally
- **Use case**: Text, spacing, component sizes that should scale with UI zoom

### Conversion at Render Time

When GPUI renders an element, it converts rem values to pixels:

```rust
// From gpui/src/geometry.rs:3199-3225
impl AbsoluteLength {
    pub fn to_pixels(self, rem_size: Pixels) -> Pixels {
        match self {
            AbsoluteLength::Pixels(pixels) => pixels,  // Pass through
            AbsoluteLength::Rems(rems) => rems.to_pixels(rem_size),  // Convert
        }
    }
}
```

The window provides its rem_size during layout:

```rust
// From gpui/src/window.rs:1955-1966
pub fn rem_size(&self) -> Pixels {
    self.rem_size_override_stack
        .last()
        .copied()
        .unwrap_or(self.rem_size)
}

pub fn set_rem_size(&mut self, rem_size: impl Into<Pixels>) {
    self.rem_size = rem_size.into();
}
```

## 4. Rem-Aware Styling Methods

### GPUI's Tailwind-Style Text Methods

These methods from `gpui/src/styled.rs` use rem values:

```rust
/// Sets the text size to 'extra small' (0.75rem = 12px at 16px base)
fn text_xs(mut self) -> Self {
    self.text_style().font_size = Some(rems(0.75).into());
    self
}

/// Sets the text size to 'small' (0.875rem = 14px at 16px base)
fn text_sm(mut self) -> Self {
    self.text_style().font_size = Some(rems(0.875).into());
    self
}

/// Sets the text size to 'base' (1.0rem = 16px at 16px base)
fn text_base(mut self) -> Self {
    self.text_style().font_size = Some(rems(1.0).into());
    self
}

/// Sets the text size to 'large' (1.125rem = 18px at 16px base)
fn text_lg(mut self) -> Self {
    self.text_style().font_size = Some(rems(1.125).into());
    self
}

/// Sets the text size to 'extra large' (1.25rem = 20px at 16px base)
fn text_xl(mut self) -> Self {
    self.text_style().font_size = Some(rems(1.25).into());
    self
}

/// Sets the text size to '2xl' (1.5rem = 24px at 16px base)
fn text_2xl(mut self) -> Self {
    self.text_style().font_size = Some(rems(1.5).into());
    self
}

/// Sets the text size to '3xl' (1.875rem = 30px at 16px base)
fn text_3xl(mut self) -> Self {
    self.text_style().font_size = Some(rems(1.875).into());
    self
}
```

### gpui-component's StyleSized Trait

The `StyleSized` trait (from `styled.rs`) provides size-aware styling that uses the text methods above:

```rust
pub trait StyleSized<T: Styled> {
    fn input_text_size(self, size: Size) -> Self;
    fn input_size(self, size: Size) -> Self;
    fn button_text_size(self, size: Size) -> Self;
    fn list_size(self, size: Size) -> Self;
    // ...
}

impl<T: Styled> StyleSized<T> for T {
    fn input_text_size(self, size: Size) -> Self {
        match size {
            Size::XSmall => self.text_xs(),   // 0.75rem
            Size::Small => self.text_sm(),    // 0.875rem
            Size::Medium => self.text_sm(),   // 0.875rem
            Size::Large => self.text_base(),  // 1.0rem
            Size::Size(size) => self.text_size(size * 0.875),
        }
    }

    fn button_text_size(self, size: Size) -> Self {
        match size {
            Size::XSmall => self.text_xs(),   // 0.75rem
            Size::Small => self.text_sm(),    // 0.875rem
            _ => self.text_base(),            // 1.0rem
        }
    }
}
```

### Size Enum Reference

| Size | text_xs | text_sm | text_base | Rem Value |
|------|---------|---------|-----------|-----------|
| XSmall | ✓ | | | 0.75rem |
| Small | | ✓ | | 0.875rem |
| Medium | | ✓ | | 0.875rem |
| Large | | | ✓ | 1.0rem |

## 5. Best Practices for Custom Components

### DO: Use rem-Based Sizing for Scalable Elements

```rust
fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
        .text_sm()                           // 0.875rem - scales with theme
        .p(rems(0.5))                         // 0.5rem padding - scales
        .gap(rems(0.25))                      // 0.25rem gap - scales
        .child("Scalable content")
}
```

### DO: Use StyleSized for Component Variants

```rust
use gpui_component::{Size, Sizable, StyleSized};

impl Render for MyButton {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .button_text_size(self.size)      // Automatically uses rem
            .input_px(self.size)              // Size-appropriate padding
            .child(&self.label)
    }
}

impl Sizable for MyButton {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}
```

### DO: Use px() for Fixed-Size Elements

```rust
fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
        .border_1()                           // 1px border - fixed
        .w(px(24.))                           // 24px icon container - fixed
        .h(px(24.))
        .child(Icon::new(IconName::Check))
}
```

### DON'T: Hardcode Sizes That Should Scale

```rust
// ❌ BAD - won't scale with theme
div()
    .text_size(px(14.))                       // Fixed at 14px
    .p(px(8.))                                // Fixed 8px padding

// ✓ GOOD - scales with theme
div()
    .text_sm()                                // 0.875rem = 14px at 16px base
    .p(rems(0.5))                             // 0.5rem = 8px at 16px base
```

### DON'T: Mix rem and px Inconsistently

```rust
// ❌ BAD - inconsistent scaling
div()
    .text_base()                              // rem-based
    .py(px(12.))                              // px-based
    .gap_2()                                  // rem-based (2 * 0.25rem = 0.5rem)

// ✓ GOOD - consistent approach
div()
    .text_base()                              // rem-based
    .py(rems(0.75))                           // rem-based
    .gap_2()                                  // rem-based
```

### Reading rem_size at Runtime

When you need to convert rem values to pixels for calculations:

```rust
fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let rem_size = window.rem_size();
    let icon_size_px = rems(1.5).to_pixels(rem_size);  // 24px at 16px base
    
    div()
        .w(icon_size_px)
        .h(icon_size_px)
        .child(Icon::new(IconName::Star))
}
```

## Reference Tables

### Common rem Values (at 16px base)

| rem | Pixels | Use Case |
|-----|--------|----------|
| 0.25 | 4px | Small gaps, micro-spacing |
| 0.5 | 8px | Standard gap, small padding |
| 0.75 | 12px | XS text size, medium padding |
| 0.875 | 14px | SM text size |
| 1.0 | 16px | Base text size |
| 1.125 | 18px | LG text size |
| 1.25 | 20px | XL text size |
| 1.5 | 24px | 2XL text size, large icons |
| 1.875 | 30px | 3XL text size |
| 2.0 | 32px | Headers, large spacing |

### GPUI Spacing Helpers (rem-based)

| Method | rem Value | Pixels (16px base) |
|--------|-----------|-------------------|
| `.gap_0_5()` | 0.125rem | 2px |
| `.gap_1()` | 0.25rem | 4px |
| `.gap_2()` | 0.5rem | 8px |
| `.gap_3()` | 0.75rem | 12px |
| `.gap_4()` | 1rem | 16px |
| `.p_1()` | 0.25rem | 4px |
| `.p_2()` | 0.5rem | 8px |
| `.p_3()` | 0.75rem | 12px |
| `.p_4()` | 1rem | 16px |

## Integration Checklist

When adding new gpui-component-based windows to Script Kit:

1. **Wrap your view in Root**
   ```rust
   cx.new(|cx| Root::new(my_view, window, cx))
   ```

2. **Ensure theme sync happens before window creation**
   ```rust
   sync_gpui_component_theme(cx);
   ```

3. **Use rem-based sizing** for text and spacing that should scale

4. **Use px-based sizing** for borders, icons, and fixed-size elements

5. **Test with different font_size values** to ensure proper scaling
