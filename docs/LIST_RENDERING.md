# List Rendering Guide

This document explains how to customize the appearance of list items and section headers in Script Kit GPUI.

## Key Files

| File | Purpose |
|------|---------|
| `src/list_item.rs` | `ListItem` component, `IconKind` enum, `render_section_header()` |
| `src/designs/mod.rs` | `render_design_item()` - maps SearchResult to ListItem |
| `src/designs/icon_variations.rs` | `IconName` enum, SVG icon paths, `icon_name_from_str()` |
| `src/main.rs` | Main list rendering in `uniform_list` closure |

## List Item Configuration

### Constants

```rust
// src/list_item.rs
pub const LIST_ITEM_HEIGHT: f32 = 40.0;      // Height of each list item
pub const SECTION_HEADER_HEIGHT: f32 = 24.0; // Height hint (actual uses LIST_ITEM_HEIGHT)
pub const ACCENT_BAR_WIDTH: f32 = 3.0;       // Left accent bar when selected
```

### ListItem Builder Methods

```rust
ListItem::new(name, colors)
    .index(0)                           // Item index for hover handling
    .icon("📜")                         // Emoji icon
    .icon_kind(IconKind::Svg("Code"))   // SVG icon by name
    .icon_image(arc_render_image)       // Pre-decoded image (app icons)
    .description("Optional description")
    .shortcut("⌘K")                     // Right-aligned shortcut badge
    .selected(true)                     // Selection state
    .with_accent_bar(true)              // Show 3px left accent when selected
    .semantic_id("choice:0:my-item")    // AI-targeting ID
```

### Icon Types

```rust
pub enum IconKind {
    Emoji(String),           // Text/emoji: "📜", "⚡"
    Image(Arc<RenderImage>), // Pre-decoded PNG (app icons)
    Svg(String),             // SVG by name: "Code", "Terminal", "File"
}
```

## SVG Icons

### Available Icons (22 total)

| Category | Icons |
|----------|-------|
| **Files** | File, FileCode, Folder, FolderOpen |
| **Actions** | Plus, Trash, Copy, Settings, MagnifyingGlass, Terminal, Code |
| **Status** | Check, Star, StarFilled, BoltFilled, BoltOutlined |
| **Arrows** | ArrowRight, ArrowDown, ChevronRight, ChevronDown |
| **Media** | PlayFilled, PlayOutlined |

### Icon Name Aliases

The `icon_name_from_str()` function accepts multiple formats:

```rust
// All of these work:
icon_name_from_str("File")           // Exact
icon_name_from_str("file")           // Lowercase
icon_name_from_str("file-code")      // Kebab-case
icon_name_from_str("file_code")      // Snake_case
icon_name_from_str("file code")      // With spaces

// Aliases:
icon_name_from_str("search")         // -> MagnifyingGlass
icon_name_from_str("gear")           // -> Settings
icon_name_from_str("lightning")      // -> BoltFilled
icon_name_from_str("run")            // -> PlayFilled
icon_name_from_str("delete")         // -> Trash
```

### Adding New SVG Icons

1. Add SVG file to `assets/icons/my_icon.svg`
2. Add variant to `IconName` enum in `src/designs/icon_variations.rs`
3. Add to `IconName::all()`, `name()`, `description()`, `external_path()`, `category()`
4. Add mapping in `icon_name_from_str()`

**SVG Requirements:**
- Use `stroke="currentColor"` for theme-aware coloring
- Standard size: 16x16 viewBox
- No hardcoded colors

## Section Headers

### Current Styling

```rust
// src/list_item.rs - render_section_header()
div()
    .w_full()
    .h_full()                    // Fill parent height (LIST_ITEM_HEIGHT)
    .px(px(16.))                 // Horizontal padding
    .pb(px(4.))                  // Bottom padding
    .flex()
    .flex_col()
    .justify_end()               // Push text to bottom
    .child(
        div()
            .text_xs()           // ~10-11px font
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(colors.text_dimmed))
            .child(label)        // Standard casing (not uppercased)
    )
```

### Customization Options

| Property | Current | Alternatives |
|----------|---------|--------------|
| **Font Size** | `text_xs()` | `text_sm()`, `text_size(px(12.))` |
| **Font Weight** | `BOLD` | `LIGHT`, `MEDIUM`, `SEMIBOLD` |
| **Text Color** | `text_dimmed` | `text_muted`, `text_secondary`, `accent` |
| **Casing** | Standard | `label.to_uppercase()` for ALL CAPS |
| **Position** | Bottom (`justify_end`) | Top (`justify_start`), Center (`justify_center`) |
| **Padding** | `px(16.)` horizontal | Adjust to align with list items |

## Theme Colors

### ListItemColors

```rust
pub struct ListItemColors {
    pub text_primary: u32,         // Main text (selected items)
    pub text_secondary: u32,       // Default text
    pub text_muted: u32,           // Descriptions
    pub text_dimmed: u32,          // Most subtle (section headers)
    pub accent_selected: u32,      // Accent bar color
    pub accent_selected_subtle: u32, // Selection background
    pub background: u32,           // Default background
    pub background_selected: u32,  // Selected item background
}
```

### Creating Colors

```rust
// From theme
let colors = ListItemColors::from_theme(&theme);

// From design tokens
let colors = ListItemColors::from_design(&design_colors);
```

## Rendering Flow

```
main.rs: uniform_list closure
    ├── GroupedListItem::SectionHeader(label)
    │   └── render_section_header(label, colors)
    │
    └── GroupedListItem::Item(idx)
        └── render_design_item(design, result, idx, selected, colors)
            ├── DesignVariant::Minimal → MinimalRenderer
            ├── DesignVariant::RetroTerminal → RetroTerminalRenderer
            └── _ → ListItem::new(...).into_any_element()
```

## Performance Tips

1. **Pre-decode images**: Use `decode_png_to_render_image()` once at load time, not during render
2. **Use uniform_list**: For lists > 20 items, virtualization is essential
3. **Copy-able colors**: `ListItemColors` is `Copy` - efficient for closures
4. **Avoid allocations in render**: Clone data before the closure, not inside

## Testing Changes

```bash
# Build and run
cargo build

# Test with design gallery (shows all icons)
echo '{"type": "triggerBuiltin", "name": "design-gallery"}' | SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1

# Visual verification
# 1. Create test script with captureScreenshot()
# 2. Save to ./.test-screenshots/
# 3. Read file to verify
```
