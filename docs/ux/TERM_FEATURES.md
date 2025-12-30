# Terminal Features Audit & Roadmap

## Executive Summary

This document provides a comprehensive audit of the terminal prompt implementation in Script Kit GPUI, including current capabilities, missing features, and a prioritized roadmap for future development.

**Architecture:** The terminal uses Alacritty's terminal emulator backend (`alacritty_terminal`) with `portable-pty` for cross-platform PTY management, rendered through GPUI's retained-mode UI.

**Key Findings:**
- Complete VT100/xterm escape sequence support with full ANSI color palette
- Solid foundational architecture with clear separation of concerns
- Recently added: Paste support (Cmd+V) and visual bell feedback
- Primary gaps: Text selection, scrollback UI, copy functionality
- Alacritty backend has scrollback/selection APIs that need UI wiring

---

## 1. Feature Matrix

### 1.1 Core Terminal Emulation

| Feature | Status | Notes |
|---------|--------|-------|
| VT100/xterm escape sequences | Complete | Full ANSI escape handling |
| 16-color ANSI | Complete | Standard terminal colors |
| 256-color palette | Complete | Extended color support |
| True color (24-bit RGB) | Complete | Full RGB color space |
| Bold text | Complete | Bright color variant |
| Italic text | Complete | Font style support |
| Underline | Complete | Single underline |
| Dim/faint text | Complete | Reduced opacity rendering |
| Inverse/reverse video | Complete | Color swap rendering |
| Strikethrough | Complete | Line-through text |
| Cursor rendering | Complete | Block cursor with color inversion |
| Cursor blinking | Complete | Configurable blink rate |

### 1.2 Input Handling

| Feature | Status | Notes |
|---------|--------|-------|
| Basic text input | Complete | All printable ASCII |
| Ctrl+A-Z sequences | Complete | All control characters |
| Arrow keys | Complete | Up/Down/Left/Right |
| F1-F12 function keys | Complete | Full function key row |
| Home/End | Complete | Line navigation |
| PageUp/PageDown | Complete | Page navigation (in-app only) |
| Insert/Delete | Complete | Editing keys |
| Tab | Complete | Tab character input |
| Escape | Complete | Escape sequence trigger |
| Paste (Cmd+V) | Complete | **Recently implemented** |
| Copy (Cmd+C) | Complete | **Recently implemented** - Copies selection |
| Bracketed paste mode | Complete | **Recently implemented** - Wraps paste in escape sequences |

### 1.3 Display & Rendering

| Feature | Status | Notes |
|---------|--------|-------|
| Dynamic resize | Complete | SIGWINCH signal handling |
| Theme integration | Complete | Colors from theme.json |
| Configurable font size | Complete | Via config.ts `terminalFontSize` |
| 30fps refresh | Complete | Timer-based redraw |
| Batched cell rendering | Complete | Performance optimization |
| Bell visual feedback | Complete | **Recently implemented** - border flash |
| Bell audio | Not Started | Optional audio notification |

### 1.4 Scrollback & Navigation

| Feature | Status | Notes |
|---------|--------|-------|
| 10,000 line buffer | Backend Only | Alacritty stores it, UI doesn't expose |
| Scroll with mouse wheel | Complete | **Recently implemented** - on_scroll_wheel handler |
| Shift+PageUp/Down | Complete | **Recently implemented** - Keyboard scrollback nav |
| Scroll position indicator | Not Started | Visual scrollbar or position |

### 1.5 Selection & Copy

| Feature | Status | Notes |
|---------|--------|-------|
| Mouse text selection | Complete | **Recently implemented** - Click-drag selection |
| Shift+Arrow selection | Not Started | Keyboard selection |
| Double-click word select | Not Started | Word boundary detection |
| Triple-click line select | Not Started | Full line selection |
| Selection highlighting | Complete | **Recently implemented** - Accent color highlighting |
| Copy selected text | Complete | **Recently implemented** - Cmd+C with selection |

### 1.6 Advanced Features

| Feature | Status | Notes |
|---------|--------|-------|
| URL detection | Not Started | Regex pattern matching |
| URL clicking (Cmd+Click) | Not Started | Open in browser |
| Search in scrollback (Cmd+F) | Not Started | Find text in history |
| Mouse click positioning | Not Started | Click to move cursor |
| Mouse reporting modes | Not Started | Application mouse input |
| Alternate screen buffer | Unknown | Needs verification |
| OSC title sequences | Partial | Received but not displayed |

---

## 2. Architecture Overview

### 2.1 File Structure

```
src/
├── term_prompt.rs          # Main GPUI component (~1129 lines)
│   ├── TermPrompt struct   # State and rendering
│   ├── render()            # Cell-by-cell terminal rendering
│   └── handle_key_down()   # Keyboard input processing
│
└── terminal/
    ├── mod.rs              # Module exports, TerminalEvent enum
    ├── alacritty.rs        # Alacritty Term wrapper
    │   ├── AlacrittyBackend # Core terminal emulator
    │   ├── scroll()        # Scrollback API (unused)
    │   └── selection APIs  # Selection handling (unused)
    ├── pty.rs              # PTY management (portable-pty)
    │   └── PtyProcess      # Process spawning, I/O
    └── theme_adapter.rs    # Theme -> Alacritty colors
        └── selection_*     # Selection colors (defined but unused)
```

### 2.2 Data Flow

```
┌─────────────────┐     stdin      ┌─────────────────┐
│   TermPrompt    │ ──────────────>│    PtyProcess   │
│   (GPUI View)   │                │  (portable-pty) │
└────────┬────────┘                └────────┬────────┘
         │                                  │
         │ render                           │ stdout/stderr
         v                                  v
┌─────────────────┐    process     ┌─────────────────┐
│  Terminal Grid  │ <──────────────│ AlacrittyBackend│
│   (GPUI draw)   │                │ (Term<Listener>)│
└─────────────────┘                └─────────────────┘
```

### 2.3 Key Integration Points

**Theme Integration:**
- `theme_adapter.rs` converts Script Kit theme colors to Alacritty's `Colors` struct
- Selection colors (`selection_foreground`, `selection_background`) are defined but never applied during rendering

**Event Handling:**
- `TerminalEvent` enum in `mod.rs` includes `ClipboardStore` and `ClipboardLoad` variants
- These events are received from Alacritty but not fully handled in the UI layer

**Scrollback:**
- Alacritty's `Term` has `scroll(Scroll::Delta(lines))` API
- `history_size()` returns scrollback depth
- Neither is wired to UI controls

---

## 3. Prioritized Roadmap

### 3.1 Quick Wins (< 1 hour each)

#### P0: Terminal Title Display (~15 min)
**Effort:** 15 minutes  
**Impact:** High (user orientation)

The terminal already receives OSC title escape sequences. Display them in the prompt header.

```rust
// In term_prompt.rs, add field:
title: Option<String>,

// In event handling, capture title:
TerminalEvent::Title(title) => {
    self.title = Some(title);
    cx.notify();
}

// In render, show in header
```

#### P1: Bell Audio Option (~30 min)
**Effort:** 30 minutes  
**Impact:** Low (accessibility)

Add optional system beep when bell character received. The visual bell is already implemented.

### 3.2 Medium Effort (1-4 hours)

#### P2: Scrollback Navigation (~2 hours)
**Effort:** 2 hours  
**Impact:** High (essential for logs/output review)

Wire Shift+PageUp/Down to Alacritty's scroll API:

```rust
// In handle_key_down:
("pageup", true, false) => {  // Shift+PageUp
    self.backend.scroll(Scroll::PageUp);
    cx.notify();
}
("pagedown", true, false) => {  // Shift+PageDown
    self.backend.scroll(Scroll::PageDown);
    cx.notify();
}
```

**Subtasks:**
1. Add `scroll()` method to AlacrittyBackend
2. Wire keyboard shortcuts in TermPrompt
3. Add visual indicator for scroll position
4. Handle "snap to bottom" on new output

#### P3: Text Selection (~3 hours)
**Effort:** 3 hours  
**Impact:** High (copy/paste workflow)

Implement mouse-based text selection:

**Subtasks:**
1. Handle mouse down/drag events to track selection
2. Convert pixel coordinates to grid cells
3. Call Alacritty's selection APIs
4. Render selection highlight (colors already defined)
5. Enable Cmd+C when selection exists

#### P4: Bracketed Paste Mode (~1 hour)
**Effort:** 1 hour  
**Impact:** Medium (security for CLI tools)

Wrap pasted text in escape sequences when bracketed paste is enabled:

```rust
// If bracketed paste mode enabled:
let paste_data = format!("\x1b[200~{}\x1b[201~", text);
```

### 3.3 Major Work (Days)

#### P5: Full Mouse Support (2-3 days)
**Effort:** 2-3 days  
**Impact:** High (vim, tmux, interactive CLIs)

**Components:**
1. Click-to-position cursor
2. Mouse wheel scrolling
3. Mouse reporting modes (X10, normal, button, any)
4. Drag selection
5. Mouse hover effects (for URLs)

#### P6: URL Detection & Clicking (1-2 days)
**Effort:** 1-2 days  
**Impact:** Medium (quality of life)

**Components:**
1. Regex-based URL detection in terminal content
2. Underline/highlight URLs on hover
3. Cmd+Click to open in default browser
4. Context menu with "Copy URL" option

#### P7: Search in Scrollback (1 day)
**Effort:** 1 day  
**Impact:** Medium (debugging, log review)

**Components:**
1. Cmd+F opens search overlay
2. Incremental search with highlighting
3. Next/Previous match navigation
4. Search wrapping option

---

## 4. Implementation Notes

### 4.1 Selection Implementation Guide

The Alacritty backend already has selection infrastructure:

```rust
// In alacritty.rs, Term has:
term.selection = Some(Selection::new(
    SelectionType::Simple,
    start_point,
    Side::Left,
));

// Extend selection on drag:
term.selection.as_mut().map(|s| s.update(end_point, Side::Right));

// Get selected text:
let text = term.selection_to_string();
```

Theme adapter already defines selection colors:

```rust
// In theme_adapter.rs:
selection_foreground: Some(Rgb { r, g, b }),
selection_background: Some(Rgb { r, g, b }),
```

### 4.2 Scrollback Implementation Guide

```rust
// In alacritty.rs:
use alacritty_terminal::grid::Scroll;

impl AlacrittyBackend {
    pub fn scroll(&mut self, scroll: Scroll) {
        self.term.lock().scroll_display(scroll);
    }
    
    pub fn scroll_to_bottom(&mut self) {
        self.term.lock().scroll_display(Scroll::Bottom);
    }
    
    pub fn display_offset(&self) -> usize {
        self.term.lock().grid().display_offset()
    }
}
```

### 4.3 Mouse Coordinate Conversion

```rust
fn pixel_to_cell(&self, position: Point<Pixels>) -> alacritty_terminal::index::Point {
    let col = (position.x.0 / self.cell_width()) as usize;
    let row = (position.y.0 / self.cell_height()) as usize;
    
    alacritty_terminal::index::Point {
        line: Line(row as i32),
        column: Column(col),
    }
}
```

### 4.4 Handling ClipboardStore Events

Currently unhandled in the event loop:

```rust
// In terminal event processing:
TerminalEvent::ClipboardStore(clipboard_type, content) => {
    // Write to system clipboard
    cx.write_to_clipboard(ClipboardItem::new(content));
}

TerminalEvent::ClipboardLoad(clipboard_type, format_callback) => {
    // Read from system clipboard and send to terminal
    if let Some(content) = cx.read_from_clipboard() {
        format_callback(content.text());
    }
}
```

---

## 5. Testing Recommendations

### 5.1 Manual Test Cases

For each new feature, verify with:

1. **Basic shells:** bash, zsh, fish
2. **Interactive programs:** vim, nano, htop, less
3. **Build tools:** cargo, npm with colored output
4. **SSH sessions:** Remote terminal behavior
5. **tmux/screen:** Multiplexer compatibility

### 5.2 Automated Testing

Create smoke tests for terminal features:

```typescript
// tests/smoke/test-terminal-scrollback.ts
import '../../scripts/kit-sdk';

const result = await terminal("bash");
// Send commands that produce scrollable output
// Verify scroll position changes

process.exit(0);
```

### 5.3 Edge Cases to Test

- Very long lines (horizontal scrolling)
- Rapid output (stress test rendering)
- Unicode/emoji in output
- Right-to-left text
- Combining characters
- Wide characters (CJK)

---

## 6. References

- [Alacritty Terminal](https://github.com/alacritty/alacritty) - Terminal emulator backend
- [portable-pty](https://docs.rs/portable-pty) - Cross-platform PTY library
- [GPUI Documentation](https://docs.rs/gpui) - UI framework
- [VT100 Escape Codes](https://vt100.net/docs/vt100-ug/) - Terminal protocol reference
- [XTerm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html) - Extended sequences

---

## Changelog

| Date | Change |
|------|--------|
| 2024-12-30 | Initial audit and roadmap creation |
| 2024-12-30 | Documented paste support (Cmd+V) implementation |
| 2024-12-30 | Documented visual bell implementation |
| 2024-12-30 | Implemented P0 (terminal title), P2 (scrollback nav), P3 (text selection + copy) |
| 2024-12-30 | Implemented P4 (bracketed paste mode), P5a (mouse wheel scrolling) |
