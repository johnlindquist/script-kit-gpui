# Editor Implementation Plan for Script Kit GPUI

## Executive Summary

This document outlines the comprehensive plan for implementing an `editor()` prompt in Script Kit GPUI that maintains API compatibility with the original Monaco-based editor while using native GPUI components for performance and consistency.

**Goal**: Implement a full-featured code editor that supports the existing Script Kit `editor()` API:

```typescript
// Basic usage
let content = await editor();

// With initial content
let content = await editor("Hello world!");

// With content and language
let content = await editor(initialCode, "typescript");

// With actions
await editor("Hello World", [
  { name: "Exclaim", shortcut: "cmd+2", onAction: () => editor.append("!") }
]);
```

---

## Table of Contents

1. [Current State Analysis](#1-current-state-analysis)
2. [API Requirements](#2-api-requirements)
3. [Architecture Options](#3-architecture-options)
4. [Recommended Approach](#4-recommended-approach)
5. [Implementation Phases](#5-implementation-phases)
6. [Technical Design](#6-technical-design)
7. [File Changes](#7-file-changes)
8. [Testing Strategy](#8-testing-strategy)
9. [Timeline Estimate](#9-timeline-estimate)
10. [Risks and Mitigations](#10-risks-and-mitigations)

---

## 1. Current State Analysis

### What Exists

| Component | Status | Location |
|-----------|--------|----------|
| Protocol message type | Defined | `src/protocol.rs:176-187` |
| SDK function | Implemented | `scripts/kit-sdk.ts:1141-1161` |
| SDK types | Defined | `scripts/kit-sdk.ts:216-221` |
| Syntax highlighting | Working | `src/syntax.rs` (syntect-based) |
| Test file | Exists | `tests/sdk/test-editor.ts` |
| GPUI handler | **Missing** | `src/main.rs` - no editor case |

### Protocol Definition (Already Exists)

```rust
// src/protocol.rs
#[serde(rename = "editor")]
Editor {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(rename = "onInit", skip_serializing_if = "Option::is_none")]
    on_init: Option<String>,
    #[serde(rename = "onSubmit", skip_serializing_if = "Option::is_none")]
    on_submit: Option<String>,
}
```

### SDK Implementation (Already Exists)

```typescript
// scripts/kit-sdk.ts
globalThis.editor = async function editor(
  content: string = '',
  language: string = 'text'
): Promise<string> {
  const id = nextId();

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      resolve(msg.value ?? '');
    });

    const message: EditorMessage = {
      type: 'editor',
      id,
      content,
      language,
    };

    send(message);
  });
};
```

### Syntax Highlighting (Already Exists)

The `src/syntax.rs` module provides:
- `highlight_code_lines(code, language)` - Returns `Vec<HighlightedLine>`
- `highlight_code(code, language)` - Returns flat `Vec<HighlightedSpan>`
- Supports: TypeScript, JavaScript, Markdown, JSON, Rust, Python, HTML, CSS, Shell, YAML

---

## 2. API Requirements

### Script Kit v1 `editor()` API

From `docs/API-GENERATED.md`:

```typescript
// Signature
function editor(content?: string, language?: string): Promise<string>;

// Extended signature with actions
function editor(content: string, actions?: Action[]): Promise<string>;

// Action interface
interface Action {
  name: string;
  shortcut?: string;
  visible?: boolean;
  onAction: () => void;
}

// Editor static methods (available during editor session)
editor.append(text: string): void;
editor.setValue(text: string): void;
editor.getValue(): string;
editor.focus(): void;
```

### Required Features

| Feature | Priority | Complexity |
|---------|----------|------------|
| Display code with syntax highlighting | P0 | Low |
| Basic text editing (insert, delete) | P0 | Medium |
| Cursor navigation (arrows, home/end) | P0 | Medium |
| Selection (shift+arrows, cmd+a) | P0 | Medium |
| Copy/Paste (cmd+c, cmd+v, cmd+x) | P0 | Low |
| Undo/Redo (cmd+z, cmd+shift+z) | P1 | Medium |
| Line numbers | P1 | Low |
| Find/Replace (cmd+f) | P2 | High |
| Multi-cursor | P3 | Very High |
| Scroll with virtualization | P0 | Medium |
| Submit (cmd+enter) | P0 | Low |
| Cancel (escape) | P0 | Low |
| Actions bar | P2 | Medium |

---

## 3. Architecture Options

### Option A: GPUI Native with Ropey + Syntect

Build a minimal editor using:
- **ropey** crate for efficient text buffer (rope data structure)
- **syntect** (existing) for syntax highlighting
- GPUI's `StyledText` for rendering

**Pros:**
- Full control, optimized for Script Kit's needs
- No external dependencies beyond existing crates
- Best performance
- No licensing issues

**Cons:**
- 3-4 weeks development for full editing
- Must implement cursor, selection, undo/redo

### Option B: Tree-sitter Migration

Replace syntect with tree-sitter for:
- Better TypeScript/TSX support
- Incremental parsing (faster on large files)
- Language injection support

**Pros:**
- Industry-standard parsing
- Better accuracy for TypeScript
- Zed uses this approach

**Cons:**
- Additional dependencies (~5 crates)
- Slightly more complex setup

### Option C: Zed Editor Fork

Extract Zed's editor crates and adapt.

**Pros:**
- Proven, battle-tested code
- All features already implemented

**Cons:**
- GPL-3.0 license compatibility
- Massive dependency graph
- Maintenance burden tracking Zed changes

### Option D: WebView with Monaco

Embed Monaco editor via WebView.

**Pros:**
- Full Monaco feature set immediately
- Exact parity with Script Kit v1

**Cons:**
- GPUI doesn't have WebView (would need wry)
- Poor performance
- Memory overhead
- Breaks native look and feel

---

## 4. Recommended Approach

**Option A with phased tree-sitter migration** - Build native GPUI editor with ropey + existing syntect, then optionally migrate to tree-sitter later.

### Rationale

1. **Leverages existing code**: syntect highlighting already works in `src/syntax.rs`
2. **Minimal new dependencies**: Only add `ropey` crate
3. **Performance**: Native GPUI = fastest possible rendering
4. **Control**: Can optimize for Script Kit's specific use case
5. **No licensing issues**: ropey is MIT licensed

### Key Dependencies

```toml
# Add to Cargo.toml
ropey = "1.6"  # Efficient rope-based text buffer
```

---

## 5. Implementation Phases

### Phase 1: Read-Only Code Viewer (Week 1)

Create `EditorPrompt` component that displays code with:
- Syntax highlighting (using existing syntect)
- Line numbers
- Scrolling with virtualization
- Submit (cmd+enter) and Cancel (escape)

**Deliverables:**
- `src/editor.rs` - New module with `EditorPrompt` struct
- Handle `Message::Editor` in `src/main.rs`
- Basic test: display code, press cmd+enter to submit

### Phase 2: Basic Editing (Week 2)

Add text manipulation:
- Insert characters (keyboard input)
- Delete (backspace, delete keys)
- Cursor movement (arrows, home, end, cmd+arrows)
- Basic undo/redo stack

**Deliverables:**
- Text buffer with ropey
- Cursor rendering
- Keyboard event handling

### Phase 3: Selection & Clipboard (Week 3)

Add selection support:
- Shift+arrows for selection
- Cmd+A for select all
- Cmd+C/V/X for copy/paste/cut
- Mouse click to position cursor
- Mouse drag to select

**Deliverables:**
- Selection range tracking
- Clipboard integration
- Mouse event handling

### Phase 4: Polish & Actions (Week 4)

Add remaining features:
- Actions bar (like ArgPrompt)
- Find/Replace (optional, P2)
- Tab handling (insert spaces/tab)
- Auto-indent on newline
- Theme integration

**Deliverables:**
- Actions support
- Theme-aware styling
- Complete SDK parity for basic use cases

---

## 6. Technical Design

### 6.1 EditorPrompt Struct

```rust
// src/editor.rs

use gpui::{*, prelude::*};
use ropey::Rope;
use std::sync::Arc;
use std::ops::Range;

use crate::syntax::{highlight_code_lines, HighlightedLine};
use crate::theme::Theme;

pub type SubmitCallback = Arc<dyn Fn(String, Option<String>) + Send + Sync>;

/// Edit operation for undo/redo
#[derive(Clone, Debug)]
pub enum EditOp {
    Insert { pos: usize, text: String },
    Delete { pos: usize, text: String },
}

/// Editor prompt - Monaco-style code editor
pub struct EditorPrompt {
    // Identity
    pub id: String,
    
    // Content
    buffer: Rope,
    language: String,
    
    // Cursor & Selection
    cursor: usize,                      // Character offset in buffer
    anchor: Option<usize>,              // Selection anchor (None = no selection)
    
    // Display
    highlighted_lines: Vec<HighlightedLine>,
    scroll_handle: UniformListScrollHandle,
    line_height: Pixels,
    
    // History
    undo_stack: Vec<EditOp>,
    redo_stack: Vec<EditOp>,
    
    // GPUI
    focus_handle: FocusHandle,
    on_submit: SubmitCallback,
    theme: Arc<Theme>,
}

impl EditorPrompt {
    pub fn new(
        id: String,
        content: String,
        language: String,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<Theme>,
    ) -> Self {
        let buffer = Rope::from_str(&content);
        let highlighted_lines = highlight_code_lines(&content, &language);
        
        Self {
            id,
            buffer,
            language,
            cursor: 0,
            anchor: None,
            highlighted_lines,
            scroll_handle: UniformListScrollHandle::new(),
            line_height: px(20.),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            focus_handle,
            on_submit,
            theme,
        }
    }
    
    /// Get the current selection range, if any
    pub fn selection(&self) -> Option<Range<usize>> {
        self.anchor.map(|anchor| {
            if anchor < self.cursor {
                anchor..self.cursor
            } else {
                self.cursor..anchor
            }
        })
    }
    
    /// Get the full content as a string
    pub fn content(&self) -> String {
        self.buffer.to_string()
    }
    
    /// Insert text at cursor, updating history
    pub fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        // Delete selection first if any
        if let Some(sel) = self.selection() {
            self.delete_range(sel.clone(), cx);
        }
        
        self.buffer.insert(self.cursor, text);
        self.undo_stack.push(EditOp::Insert { 
            pos: self.cursor, 
            text: text.to_string() 
        });
        self.redo_stack.clear();
        self.cursor += text.chars().count();
        self.anchor = None;
        self.rehighlight();
        cx.notify();
    }
    
    /// Delete a range of text
    fn delete_range(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        let deleted: String = self.buffer.slice(range.clone()).to_string();
        self.buffer.remove(range.clone());
        self.undo_stack.push(EditOp::Delete { 
            pos: range.start, 
            text: deleted 
        });
        self.redo_stack.clear();
        self.cursor = range.start;
        self.anchor = None;
        self.rehighlight();
        cx.notify();
    }
    
    /// Undo last operation
    pub fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(op) = self.undo_stack.pop() {
            match &op {
                EditOp::Insert { pos, text } => {
                    let end = *pos + text.chars().count();
                    self.buffer.remove(*pos..end);
                    self.cursor = *pos;
                }
                EditOp::Delete { pos, text } => {
                    self.buffer.insert(*pos, text);
                    self.cursor = *pos + text.chars().count();
                }
            }
            self.redo_stack.push(op);
            self.anchor = None;
            self.rehighlight();
            cx.notify();
        }
    }
    
    /// Redo last undone operation
    pub fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(op) = self.redo_stack.pop() {
            match &op {
                EditOp::Insert { pos, text } => {
                    self.buffer.insert(*pos, text);
                    self.cursor = *pos + text.chars().count();
                }
                EditOp::Delete { pos, text } => {
                    let end = *pos + text.chars().count();
                    self.buffer.remove(*pos..end);
                    self.cursor = *pos;
                }
            }
            self.undo_stack.push(op);
            self.anchor = None;
            self.rehighlight();
            cx.notify();
        }
    }
    
    /// Re-run syntax highlighting after content change
    fn rehighlight(&mut self) {
        let content = self.buffer.to_string();
        self.highlighted_lines = highlight_code_lines(&content, &self.language);
    }
    
    /// Move cursor, optionally extending selection
    pub fn move_cursor(&mut self, delta: isize, extend_selection: bool, cx: &mut Context<Self>) {
        let new_pos = ((self.cursor as isize) + delta)
            .max(0)
            .min(self.buffer.len_chars() as isize) as usize;
        
        if extend_selection {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
        } else {
            self.anchor = None;
        }
        
        self.cursor = new_pos;
        cx.notify();
    }
    
    /// Handle keyboard events
    pub fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let cmd = event.keystroke.modifiers.platform;
        let shift = event.keystroke.modifiers.shift;
        
        match (key, cmd, shift) {
            // Navigation
            ("left", false, extend) => self.move_cursor(-1, extend, cx),
            ("right", false, extend) => self.move_cursor(1, extend, cx),
            ("up", false, extend) => self.move_cursor_line(-1, extend, cx),
            ("down", false, extend) => self.move_cursor_line(1, extend, cx),
            ("home", false, extend) => self.move_to_line_start(extend, cx),
            ("end", false, extend) => self.move_to_line_end(extend, cx),
            
            // Word navigation
            ("left", true, _) => self.move_cursor_word(-1, shift, cx),
            ("right", true, _) => self.move_cursor_word(1, shift, cx),
            
            // Editing
            ("backspace", false, _) => self.delete_backward(cx),
            ("delete", false, _) => self.delete_forward(cx),
            
            // Clipboard
            ("c", true, _) => self.copy(cx),
            ("x", true, _) => self.cut(cx),
            ("v", true, _) => self.paste(cx),
            ("a", true, _) => self.select_all(cx),
            
            // History
            ("z", true, false) => self.undo(cx),
            ("z", true, true) => self.redo(cx),
            
            // Submit/Cancel
            ("enter", true, _) => self.submit(cx),
            ("escape", false, _) => self.cancel(cx),
            
            // Text input
            _ => {
                if let Some(ime_key) = &event.keystroke.ime_key {
                    self.insert(ime_key, cx);
                } else if key == "enter" && !cmd {
                    self.insert("\n", cx);
                } else if key == "tab" {
                    self.insert("  ", cx); // 2 spaces for tab
                }
            }
        }
    }
    
    fn submit(&self, _cx: &mut Context<Self>) {
        let content = self.buffer.to_string();
        (self.on_submit)(self.id.clone(), Some(content));
    }
    
    fn cancel(&self, _cx: &mut Context<Self>) {
        (self.on_submit)(self.id.clone(), None);
    }
    
    // ... additional helper methods for cursor movement, selection, etc.
}
```

### 6.2 Render Implementation

```rust
impl Render for EditorPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &self.theme.colors;
        let line_count = self.highlighted_lines.len().max(1);
        let cursor_line = self.cursor_line();
        
        div()
            .id("editor-prompt")
            .key_context("EditorPrompt")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event, _, cx| {
                this.handle_key(event, cx);
            }))
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(colors.background.main))
            .child(
                // Editor content area
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        uniform_list(
                            "editor-lines",
                            line_count,
                            cx.handler(|this, range, _window, _cx| {
                                this.render_lines(range)
                            }),
                        )
                        .track_scroll(&self.scroll_handle)
                        .h_full()
                    )
            )
            .child(
                // Status bar
                self.render_status_bar(colors)
            )
    }
}

impl EditorPrompt {
    fn render_lines(&self, range: Range<usize>) -> Vec<impl IntoElement> {
        let colors = &self.theme.colors;
        let gutter_width = px(50.);
        
        range.map(|line_idx| {
            let line = self.highlighted_lines.get(line_idx);
            let is_cursor_line = line_idx == self.cursor_line();
            
            div()
                .id(("line", line_idx))
                .flex()
                .flex_row()
                .h(self.line_height)
                .when(is_cursor_line, |d| d.bg(rgb(colors.background.search_box)))
                .child(
                    // Line number gutter
                    div()
                        .w(gutter_width)
                        .flex_shrink_0()
                        .text_color(rgb(colors.text.muted))
                        .text_sm()
                        .px_2()
                        .justify_end()
                        .child(format!("{}", line_idx + 1))
                )
                .child(
                    // Code content
                    div()
                        .flex_1()
                        .px_2()
                        .font_family("monospace")
                        .children(
                            line.map(|l| {
                                l.spans.iter().map(|span| {
                                    div()
                                        .text_color(rgb(span.color))
                                        .child(span.text.clone())
                                }).collect::<Vec<_>>()
                            }).unwrap_or_default()
                        )
                        // Cursor overlay
                        .when(is_cursor_line, |d| {
                            d.child(self.render_cursor())
                        })
                )
        }).collect()
    }
    
    fn render_cursor(&self) -> impl IntoElement {
        div()
            .absolute()
            .left(self.cursor_x_offset())
            .w(px(2.))
            .h(self.line_height)
            .bg(rgb(self.theme.colors.accent.selected))
    }
    
    fn render_status_bar(&self, colors: &ColorScheme) -> impl IntoElement {
        let cursor_line = self.cursor_line() + 1;
        let cursor_col = self.cursor_column() + 1;
        
        div()
            .flex()
            .flex_row()
            .h(px(24.))
            .px_4()
            .items_center()
            .justify_between()
            .bg(rgb(colors.background.title_bar))
            .border_t_1()
            .border_color(rgb(colors.ui.border))
            .child(
                div()
                    .text_color(rgb(colors.text.secondary))
                    .text_xs()
                    .child(format!("Ln {}, Col {}", cursor_line, cursor_col))
            )
            .child(
                div()
                    .text_color(rgb(colors.text.secondary))
                    .text_xs()
                    .child(format!("{} | Cmd+Enter to submit", self.language))
            )
    }
}
```

### 6.3 Main.rs Integration

```rust
// In handle_script_message() match statement

Message::Editor { id, content, language, .. } => {
    let content_str = content.unwrap_or_default();
    let lang = language.unwrap_or_else(|| "text".to_string());
    
    logging::log("EDITOR", &format!("Editor prompt: id={}, lang={}, len={}", 
        id, lang, content_str.len()));
    
    let editor = EditorPrompt::new(
        id.clone(),
        content_str,
        lang,
        window.focus_handle(cx),
        submit_handler.clone(),
        theme.clone(),
    );
    
    *current_prompt = Some(PromptState::Editor(editor));
    cx.notify();
}
```

---

## 7. File Changes

### New Files

| File | Purpose |
|------|---------|
| `src/editor.rs` | EditorPrompt component (~500 lines) |

### Modified Files

| File | Changes |
|------|---------|
| `src/main.rs` | Add `mod editor;`, handle `Message::Editor` |
| `src/lib.rs` | Export `editor` module |
| `Cargo.toml` | Add `ropey = "1.6"` dependency |

### No Changes Required

| File | Reason |
|------|--------|
| `src/protocol.rs` | Editor message already defined |
| `scripts/kit-sdk.ts` | editor() already implemented |
| `src/syntax.rs` | Already provides highlighting |

---

## 8. Testing Strategy

### Unit Tests

```rust
// src/editor.rs
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_insert_text() {
        // ... test buffer insert
    }
    
    #[test]
    fn test_delete_backward() {
        // ... test backspace
    }
    
    #[test]
    fn test_undo_redo() {
        // ... test history
    }
    
    #[test]
    fn test_selection() {
        // ... test selection range
    }
}
```

### SDK Integration Tests

Update `tests/sdk/test-editor.ts`:

```typescript
// Test 1: Basic editor
const test1 = 'editor-basic';
const result = await editor();
// Verify: editor opens, can type, cmd+enter submits

// Test 2: With content
const test2 = 'editor-with-content';
const result = await editor("Hello World!", "typescript");
// Verify: content displayed with syntax highlighting

// Test 3: Edit and submit
const test3 = 'editor-edit';
const result = await editor("original");
// Verify: can edit content, submitted value includes edits
```

### Manual Testing Checklist

- [ ] Editor displays with syntax highlighting
- [ ] Can type text
- [ ] Backspace/Delete work
- [ ] Arrow keys move cursor
- [ ] Shift+arrows select text
- [ ] Cmd+C/V/X work
- [ ] Cmd+Z/Shift+Z undo/redo
- [ ] Cmd+Enter submits
- [ ] Escape cancels
- [ ] Line numbers display correctly
- [ ] Scroll works for long files
- [ ] Cursor visible at current position

---

## 9. Timeline Estimate

| Phase | Duration | Deliverable |
|-------|----------|-------------|
| Phase 1: Read-Only Viewer | 3-4 days | Display code, submit/cancel |
| Phase 2: Basic Editing | 4-5 days | Insert, delete, cursor movement |
| Phase 3: Selection & Clipboard | 3-4 days | Selection, copy/paste |
| Phase 4: Polish | 3-4 days | Actions, theme integration |
| **Total** | **~3 weeks** | Full editor parity |

---

## 10. Risks and Mitigations

### Risk 1: Complex Cursor Positioning

**Issue**: Mapping pixel positions to character offsets with variable-width fonts.

**Mitigation**: Use monospace font exclusively, calculate positions based on character count.

### Risk 2: IME/International Input

**Issue**: Complex input method handling for CJK languages.

**Mitigation**: Use GPUI's `EntityInputHandler` trait for proper IME support.

### Risk 3: Performance on Large Files

**Issue**: Syntax highlighting and rendering may be slow for large files.

**Mitigation**: 
- Use `uniform_list` for virtualization
- Only re-highlight visible lines + buffer
- Consider incremental highlighting with tree-sitter in future

### Risk 4: Multi-line Selection Rendering

**Issue**: Rendering selection across multiple lines is complex.

**Mitigation**: Render selection as a series of rectangles per line, simplifying the math.

---

## Appendix A: Monaco API Reference

The original Monaco editor in Script Kit v1 provided these APIs:

```typescript
// Global editor object during session
editor.append(text: string): void;      // Append text at cursor
editor.setValue(text: string): void;    // Replace all content
editor.getValue(): string;               // Get current content
editor.focus(): void;                    // Focus the editor
editor.setSelection(range): void;        // Set selection range
```

For Phase 1, we focus on basic submit/cancel. Static methods can be added in Phase 4.

---

## Appendix B: GPUI Text Handling Reference

Key GPUI APIs for text editing:

```rust
// Text rendering
StyledText::new(text)
    .with_highlights(&text_style, highlights)

// Input handling
impl EntityInputHandler for EditorPrompt {
    fn text_for_range(&mut self, range: Range<usize>) -> Option<String>;
    fn selected_text_range(&mut self) -> Option<Range<usize>>;
    fn replace_text_in_range(&mut self, range: Option<Range<usize>>, text: &str);
    // ... other methods for IME support
}

// Focus
.track_focus(&self.focus_handle)
.on_key_down(cx.listener(|this, event, _, cx| { ... }))
```

---

## Appendix C: Dependency Addition

```toml
# Cargo.toml
[dependencies]
# ... existing deps ...

# Text buffer for editor (efficient rope data structure)
ropey = "1.6"
```

The ropey crate provides:
- O(log n) insertions and deletions
- O(1) length queries
- Efficient line/char iteration
- Memory-efficient for large files
- MIT licensed

---

## Summary

This plan provides a clear path from the current state (protocol defined, SDK ready, syntax highlighting working) to a fully functional editor prompt. The phased approach allows for incremental progress with working milestones at each phase.

**Next Steps:**
1. Add `ropey` to Cargo.toml
2. Create `src/editor.rs` with `EditorPrompt` struct
3. Add `Message::Editor` handling in `src/main.rs`
4. Run `tests/sdk/test-editor.ts` to validate
