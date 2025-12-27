//! Terminal prompt component for GPUI
//!
//! Renders terminal content and handles keyboard input with proper monospace grid,
//! cursor rendering, per-cell colors, and control character handling.

use gpui::{
    div, prelude::*, px, rgb, Context, FocusHandle, Focusable, Pixels, Render, SharedString, Timer, Window,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, trace, warn};

use crate::terminal::{CellAttributes, TerminalContent, TerminalEvent, TerminalHandle};
use crate::theme::Theme;
use crate::prompts::SubmitCallback;

const SLOW_RENDER_THRESHOLD_MS: u128 = 16; // 60fps threshold

/// Terminal font configuration
const FONT_SIZE: f32 = 14.0;
/// Line height multiplier - 1.3 provides room for descenders (g, y, p, q, j)
/// and ascenders while keeping text readable
const LINE_HEIGHT_MULTIPLIER: f32 = 1.3;

/// Terminal cell dimensions
/// Cell width is approximately 0.6 × font_size for monospace fonts like Menlo
const CELL_WIDTH: f32 = FONT_SIZE * 0.6;  // ~8.4px for 14pt
/// Cell height = font_size × line_height_multiplier
const CELL_HEIGHT: f32 = FONT_SIZE * LINE_HEIGHT_MULTIPLIER;  // 18.2px for 14pt

/// Terminal refresh interval (ms) - 30fps is plenty for terminal output
const REFRESH_INTERVAL_MS: u64 = 33; // ~30fps, reduces CPU load significantly

/// Minimum terminal size
const MIN_COLS: u16 = 20;
const MIN_ROWS: u16 = 5;

/// Padding around terminal content (pixels) - now 0 to fill edge-to-edge
const TERMINAL_PADDING: f32 = 0.0;

/// Terminal prompt GPUI component
pub struct TermPrompt {
    pub id: String,
    pub terminal: TerminalHandle,
    pub focus_handle: FocusHandle,
    pub on_submit: SubmitCallback,
    pub theme: Arc<Theme>,
    exited: bool,
    exit_code: Option<i32>,
    /// Whether the refresh timer is active
    refresh_timer_active: bool,
    /// Last known terminal size (cols, rows)
    last_size: (u16, u16),
    /// Explicit content height - GPUI entities don't inherit parent flex sizing
    content_height: Option<Pixels>,
}

impl TermPrompt {
    /// Create new terminal prompt
    #[allow(dead_code)]
    pub fn new(
        id: String,
        command: Option<String>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<Theme>,
    ) -> anyhow::Result<Self> {
        Self::with_height(id, command, focus_handle, on_submit, theme, None)
    }
    
    /// Create new terminal prompt with explicit height
    /// 
    /// This is necessary because GPUI entities don't inherit parent flex sizing.
    /// When rendered as a child of a sized container, h_full() doesn't resolve
    /// to the parent's height. We must pass an explicit height.
    pub fn with_height(
        id: String,
        command: Option<String>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<Theme>,
        content_height: Option<Pixels>,
    ) -> anyhow::Result<Self> {
        // Start with a reasonable default size; will be resized dynamically
        let initial_cols = 80;
        let initial_rows = 24;
        
        let terminal = match command {
            Some(cmd) => TerminalHandle::with_command(&cmd, initial_cols, initial_rows)?,
            None => TerminalHandle::new(initial_cols, initial_rows)?,
        };

        info!(
            id = %id,
            content_height = ?content_height,
            "TermPrompt::with_height created"
        );

        Ok(Self {
            id,
            terminal,
            focus_handle,
            on_submit,
            theme,
            exited: false,
            exit_code: None,
            refresh_timer_active: false,
            last_size: (initial_cols, initial_rows),
            content_height,
        })
    }
    
    /// Set the content height (for dynamic resizing)
    #[allow(dead_code)]
    pub fn set_height(&mut self, height: Pixels) {
        self.content_height = Some(height);
    }
    
    /// Calculate terminal dimensions from pixel size
    fn calculate_terminal_size(width: Pixels, height: Pixels) -> (u16, u16) {
        // Subtract padding from available space
        let available_width = f32::from(width) - (TERMINAL_PADDING * 2.0);
        let available_height = f32::from(height) - (TERMINAL_PADDING * 2.0);
        
        // Calculate columns and rows
        let cols = (available_width / CELL_WIDTH).floor() as u16;
        let rows = (available_height / CELL_HEIGHT).floor() as u16;
        
        // Apply minimum bounds
        let cols = cols.max(MIN_COLS);
        let rows = rows.max(MIN_ROWS);
        
        (cols, rows)
    }
    
    /// Resize terminal if needed based on new dimensions
    fn resize_if_needed(&mut self, width: Pixels, height: Pixels) {
        let (new_cols, new_rows) = Self::calculate_terminal_size(width, height);
        
        if (new_cols, new_rows) != self.last_size {
            debug!(
                old_cols = self.last_size.0,
                old_rows = self.last_size.1,
                new_cols,
                new_rows,
                "Resizing terminal"
            );
            
            if let Err(e) = self.terminal.resize(new_cols, new_rows) {
                warn!(error = %e, "Failed to resize terminal");
            } else {
                self.last_size = (new_cols, new_rows);
            }
        }
    }

    /// Handle terminal exit
    fn handle_exit(&mut self, code: i32) {
        info!(code, "Terminal exited");
        self.exited = true;
        self.exit_code = Some(code);
        // Call submit callback with exit code
        (self.on_submit)(self.id.clone(), Some(code.to_string()));
    }

    /// Submit/cancel
    fn submit_cancel(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }

    /// Start the refresh timer for periodic terminal output updates
    fn start_refresh_timer(&mut self, cx: &mut Context<Self>) {
        if self.refresh_timer_active || self.exited {
            return;
        }
        self.refresh_timer_active = true;

        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(Duration::from_millis(REFRESH_INTERVAL_MS)).await;

                let should_stop = cx
                    .update(|cx| {
                        this.update(cx, |term_prompt, cx| {
                            if term_prompt.exited {
                                term_prompt.refresh_timer_active = false;
                                return true; // Stop polling
                            }
                            
                            // Process multiple times per frame to catch up on output
                            for _ in 0..4 {
                                let events = term_prompt.terminal.process();
                                for event in events {
                                    if let TerminalEvent::Exit(code) = event {
                                        term_prompt.handle_exit(code);
                                        return true;
                                    }
                                }
                            }
                            
                            cx.notify(); // Trigger re-render
                            false
                        })
                        .unwrap_or(true)
                    })
                    .unwrap_or(true);

                if should_stop {
                    break;
                }
            }
        })
        .detach();
    }

    /// Convert a Ctrl+key press to the corresponding control character byte
    /// Returns None if the key is not a valid control character
    fn ctrl_key_to_byte(key: &str) -> Option<u8> {
        // Control characters are ASCII 0x00-0x1F
        // Ctrl+A = 0x01, Ctrl+B = 0x02, ..., Ctrl+Z = 0x1A
        // Special cases:
        // Ctrl+C = 0x03 (SIGINT)
        // Ctrl+D = 0x04 (EOF)
        // Ctrl+Z = 0x1A (SIGTSTP)
        // Ctrl+L = 0x0C (form feed / clear)
        // Ctrl+[ = 0x1B (ESC)
        // Ctrl+\ = 0x1C (SIGQUIT)
        match key.to_lowercase().as_str() {
            "a" => Some(0x01),
            "b" => Some(0x02),
            "c" => Some(0x03), // SIGINT
            "d" => Some(0x04), // EOF
            "e" => Some(0x05),
            "f" => Some(0x06),
            "g" => Some(0x07), // BEL
            "h" => Some(0x08), // BS
            "i" => Some(0x09), // TAB
            "j" => Some(0x0A), // LF
            "k" => Some(0x0B), // VT
            "l" => Some(0x0C), // FF (clear)
            "m" => Some(0x0D), // CR
            "n" => Some(0x0E),
            "o" => Some(0x0F),
            "p" => Some(0x10),
            "q" => Some(0x11), // XON
            "r" => Some(0x12),
            "s" => Some(0x13), // XOFF
            "t" => Some(0x14),
            "u" => Some(0x15), // NAK (kill line)
            "v" => Some(0x16),
            "w" => Some(0x17), // kill word
            "x" => Some(0x18),
            "y" => Some(0x19),
            "z" => Some(0x1A), // SIGTSTP
            "[" => Some(0x1B), // ESC
            "\\" => Some(0x1C), // SIGQUIT
            "]" => Some(0x1D),
            "^" => Some(0x1E),
            "_" => Some(0x1F),
            _ => None,
        }
    }

    /// Render terminal content efficiently by batching consecutive cells with same style.
    /// Instead of creating 2400+ divs (80x30), we batch runs of same-styled text,
    /// typically reducing to ~50-100 elements per frame.
    fn render_content(&self, content: &TerminalContent) -> impl IntoElement {
        let colors = &self.theme.colors;
        // Use main background color to match window - no visible seam
        let default_bg = rgb(colors.background.main);
        let cursor_bg = rgb(colors.accent.selected);
        let default_fg = rgb(colors.text.primary);
        
        let mut lines_container = div()
            .flex()
            .flex_col()
            .flex_1()
            .size_full()  // Both w_full and h_full
            .min_h(px(0.)) // Critical for flex children sizing
            .overflow_hidden()
            .bg(default_bg)
            .font_family("Menlo")
            .text_size(px(FONT_SIZE))
            .line_height(px(CELL_HEIGHT)); // Use calculated line height for proper descender room

        for (line_idx, cells) in content.styled_lines.iter().enumerate() {
            let is_cursor_line = line_idx == content.cursor_line;
            
            // Build a row - we'll batch consecutive cells with same styling
            let mut row = div()
                .flex()
                .flex_row()
                .w_full()
                .h(px(CELL_HEIGHT));

            // Batch consecutive cells with same styling
            let mut batch_start = 0;
            while batch_start < cells.len() {
                let first_cell = &cells[batch_start];
                let is_cursor_start = is_cursor_line && batch_start == content.cursor_col;
                
                // Get styling for this batch
                let fg_u32 = (first_cell.fg.r as u32) << 16 | (first_cell.fg.g as u32) << 8 | (first_cell.fg.b as u32);
                let bg_u32 = (first_cell.bg.r as u32) << 16 | (first_cell.bg.g as u32) << 8 | (first_cell.bg.b as u32);
                let attrs = first_cell.attrs;
                
                // Find how many consecutive cells have the same styling (excluding cursor position)
                let mut batch_end = batch_start + 1;
                
                // If this is the cursor cell, it's always its own batch
                if !is_cursor_start {
                    while batch_end < cells.len() {
                        let cell = &cells[batch_end];
                        let is_cursor_here = is_cursor_line && batch_end == content.cursor_col;
                        
                        // Stop if cursor or different styling
                        if is_cursor_here {
                            break;
                        }
                        
                        let cell_fg = (cell.fg.r as u32) << 16 | (cell.fg.g as u32) << 8 | (cell.fg.b as u32);
                        let cell_bg = (cell.bg.r as u32) << 16 | (cell.bg.g as u32) << 8 | (cell.bg.b as u32);
                        
                        if cell_fg != fg_u32 || cell_bg != bg_u32 || cell.attrs != attrs {
                            break;
                        }
                        
                        batch_end += 1;
                    }
                }
                
                // Build the text for this batch
                let batch_text: String = cells[batch_start..batch_end]
                    .iter()
                    .map(|c| if c.c == '\0' { ' ' } else { c.c })
                    .collect();
                
                let batch_width = (batch_end - batch_start) as f32 * CELL_WIDTH;
                
                // Determine colors
                let fg_color = if is_cursor_start {
                    rgb(bg_u32) // Invert for cursor
                } else {
                    rgb(fg_u32)
                };
                
                let has_custom_bg = is_cursor_start || (bg_u32 != 0x000000 && bg_u32 != colors.background.main);
                
                let mut span = div()
                    .w(px(batch_width))
                    .h(px(CELL_HEIGHT))
                    .flex_shrink_0()
                    .text_color(if fg_u32 == 0 { default_fg } else { fg_color })
                    .child(SharedString::from(batch_text));
                
                // Apply background only if needed
                if is_cursor_start {
                    span = span.bg(cursor_bg);
                } else if has_custom_bg {
                    span = span.bg(rgb(bg_u32));
                }
                
                // Apply text attributes
                if attrs.contains(CellAttributes::BOLD) {
                    span = span.font_weight(gpui::FontWeight::BOLD);
                }
                if attrs.contains(CellAttributes::UNDERLINE) {
                    span = span.text_decoration_1();
                }
                
                row = row.child(span);
                batch_start = batch_end;
            }

            lines_container = lines_container.child(row);
        }

        lines_container
    }
}

impl Focusable for TermPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TermPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let start = Instant::now();

        // Start refresh timer if not already active
        self.start_refresh_timer(cx);
        
        // Get window bounds and resize terminal if needed
        let window_bounds = window.bounds();
        self.resize_if_needed(window_bounds.size.width, window_bounds.size.height);

        // Process terminal events - read multiple times to catch all output
        if !self.exited {
            for _ in 0..8 {  // Read up to 8 buffers worth of data per render
                let events = self.terminal.process();
                let mut got_exit = false;
                for event in events {
                    match event {
                        TerminalEvent::Exit(code) => {
                            self.handle_exit(code);
                            got_exit = true;
                        }
                        TerminalEvent::Bell => { /* could flash screen */ }
                        TerminalEvent::Title(_) => { /* could update title */ }
                        TerminalEvent::Output(_) => { /* handled by content() */ }
                    }
                }
                if got_exit {
                    break;
                }
            }
        }

        // Get terminal content
        let content = self.terminal.content();

        // Handle keyboard with Ctrl+key support
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  _window: &mut Window,
                  cx: &mut Context<Self>| {
                let key_str = event.keystroke.key.to_lowercase();
                let has_ctrl = event.keystroke.modifiers.control;

                // Escape always cancels
                if key_str == "escape" {
                    this.submit_cancel();
                    return;
                }

                // Check if terminal is still running before sending input
                if this.exited || !this.terminal.is_running() {
                    trace!(key = %key_str, "Terminal exited, ignoring key input");
                    return;
                }

                // Handle Ctrl+key combinations first
                if has_ctrl {
                    if let Some(ctrl_byte) = Self::ctrl_key_to_byte(&key_str) {
                        debug!(key = %key_str, byte = ctrl_byte, "Sending Ctrl+key");
                        if let Err(e) = this.terminal.input(&[ctrl_byte]) {
                            // Only warn if unexpected error
                            if !this.exited {
                                warn!(error = %e, "Failed to send Ctrl+key to terminal");
                            }
                        }
                        cx.notify();
                        return;
                    }
                }

                // Forward regular input to terminal
                if let Some(key_char) = &event.keystroke.key_char {
                    if let Err(e) = this.terminal.input(key_char.as_bytes()) {
                        if !this.exited {
                            warn!(error = %e, "Failed to send input to terminal");
                        }
                    }
                    cx.notify();
                } else {
                    // Handle special keys
                    let bytes: Option<&[u8]> = match key_str.as_str() {
                        "enter" => Some(b"\r"),
                        "backspace" => Some(b"\x7f"),
                        "tab" => Some(b"\t"),
                        "up" | "arrowup" => Some(b"\x1b[A"),
                        "down" | "arrowdown" => Some(b"\x1b[B"),
                        "right" | "arrowright" => Some(b"\x1b[C"),
                        "left" | "arrowleft" => Some(b"\x1b[D"),
                        "home" => Some(b"\x1b[H"),
                        "end" => Some(b"\x1b[F"),
                        "pageup" => Some(b"\x1b[5~"),
                        "pagedown" => Some(b"\x1b[6~"),
                        "delete" => Some(b"\x1b[3~"),
                        "insert" => Some(b"\x1b[2~"),
                        "f1" => Some(b"\x1bOP"),
                        "f2" => Some(b"\x1bOQ"),
                        "f3" => Some(b"\x1bOR"),
                        "f4" => Some(b"\x1bOS"),
                        "f5" => Some(b"\x1b[15~"),
                        "f6" => Some(b"\x1b[17~"),
                        "f7" => Some(b"\x1b[18~"),
                        "f8" => Some(b"\x1b[19~"),
                        "f9" => Some(b"\x1b[20~"),
                        "f10" => Some(b"\x1b[21~"),
                        "f11" => Some(b"\x1b[23~"),
                        "f12" => Some(b"\x1b[24~"),
                        _ => None,
                    };

                    if let Some(bytes) = bytes {
                        if let Err(e) = this.terminal.input(bytes) {
                            if !this.exited {
                                warn!(error = %e, "Failed to send special key to terminal");
                            }
                        }
                        cx.notify();
                    }
                }
            },
        );

        // Render terminal content with styled cells
        let colors = &self.theme.colors;
        let terminal_content = self.render_content(&content);

        // Log slow renders
        let elapsed = start.elapsed().as_millis();
        if elapsed > SLOW_RENDER_THRESHOLD_MS {
            warn!(elapsed_ms = elapsed, "Slow terminal render");
        } else {
            debug!(elapsed_ms = elapsed, "Terminal render");
        }

        // Main container with terminal styling
        // Use explicit height if available, otherwise fall back to size_full
        // NO padding - terminal content should fill edge-to-edge
        // Use main background color to match window - no visible seam at edges
        let container = div()
            .flex()
            .flex_col()
            .w_full()
            .bg(rgb(colors.background.main))
            .text_color(rgb(colors.text.primary))
            .overflow_hidden() // Clip any overflow
            .key_context("term_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key);
        
        // Apply height - use explicit if set, otherwise use h_full (may not work in all contexts)
        let container = if let Some(h) = self.content_height {
            debug!(content_height = ?h, "TermPrompt using explicit height");
            container.h(h)
        } else {
            container.h_full().min_h(px(0.))
        };
        
        container.child(terminal_content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Ctrl+Key Handling Tests (TDD)
    // ========================================================================

    #[test]
    fn test_ctrl_c_returns_sigint() {
        // Ctrl+C should return 0x03 (ETX - End of Text / SIGINT)
        assert_eq!(TermPrompt::ctrl_key_to_byte("c"), Some(0x03));
        assert_eq!(TermPrompt::ctrl_key_to_byte("C"), Some(0x03));
    }

    #[test]
    fn test_ctrl_d_returns_eof() {
        // Ctrl+D should return 0x04 (EOT - End of Transmission / EOF)
        assert_eq!(TermPrompt::ctrl_key_to_byte("d"), Some(0x04));
        assert_eq!(TermPrompt::ctrl_key_to_byte("D"), Some(0x04));
    }

    #[test]
    fn test_ctrl_z_returns_sigtstp() {
        // Ctrl+Z should return 0x1A (SUB - Substitute / SIGTSTP)
        assert_eq!(TermPrompt::ctrl_key_to_byte("z"), Some(0x1A));
        assert_eq!(TermPrompt::ctrl_key_to_byte("Z"), Some(0x1A));
    }

    #[test]
    fn test_ctrl_l_returns_clear() {
        // Ctrl+L should return 0x0C (FF - Form Feed / clear screen)
        assert_eq!(TermPrompt::ctrl_key_to_byte("l"), Some(0x0C));
        assert_eq!(TermPrompt::ctrl_key_to_byte("L"), Some(0x0C));
    }

    #[test]
    fn test_ctrl_a_through_z() {
        // Test all Ctrl+letter combinations
        let expected: [(char, u8); 26] = [
            ('a', 0x01), ('b', 0x02), ('c', 0x03), ('d', 0x04),
            ('e', 0x05), ('f', 0x06), ('g', 0x07), ('h', 0x08),
            ('i', 0x09), ('j', 0x0A), ('k', 0x0B), ('l', 0x0C),
            ('m', 0x0D), ('n', 0x0E), ('o', 0x0F), ('p', 0x10),
            ('q', 0x11), ('r', 0x12), ('s', 0x13), ('t', 0x14),
            ('u', 0x15), ('v', 0x16), ('w', 0x17), ('x', 0x18),
            ('y', 0x19), ('z', 0x1A),
        ];

        for (ch, expected_byte) in expected {
            let result = TermPrompt::ctrl_key_to_byte(&ch.to_string());
            assert_eq!(
                result,
                Some(expected_byte),
                "Ctrl+{} should return 0x{:02X}",
                ch,
                expected_byte
            );
        }
    }

    #[test]
    fn test_ctrl_bracket_returns_esc() {
        // Ctrl+[ should return 0x1B (ESC)
        assert_eq!(TermPrompt::ctrl_key_to_byte("["), Some(0x1B));
    }

    #[test]
    fn test_ctrl_backslash_returns_sigquit() {
        // Ctrl+\ should return 0x1C (SIGQUIT)
        assert_eq!(TermPrompt::ctrl_key_to_byte("\\"), Some(0x1C));
    }

    #[test]
    fn test_ctrl_special_chars() {
        // Test other special control characters
        assert_eq!(TermPrompt::ctrl_key_to_byte("]"), Some(0x1D));
        assert_eq!(TermPrompt::ctrl_key_to_byte("^"), Some(0x1E));
        assert_eq!(TermPrompt::ctrl_key_to_byte("_"), Some(0x1F));
    }

    #[test]
    fn test_ctrl_invalid_key_returns_none() {
        // Non-control keys should return None
        assert_eq!(TermPrompt::ctrl_key_to_byte("1"), None);
        assert_eq!(TermPrompt::ctrl_key_to_byte("!"), None);
        assert_eq!(TermPrompt::ctrl_key_to_byte("@"), None);
        assert_eq!(TermPrompt::ctrl_key_to_byte(" "), None);
        assert_eq!(TermPrompt::ctrl_key_to_byte("enter"), None);
        assert_eq!(TermPrompt::ctrl_key_to_byte("escape"), None);
    }

    // ========================================================================
    // Cell Dimension Tests
    // ========================================================================

    #[test]
    fn test_cell_dimensions_are_reasonable() {
        // Menlo 14pt should have reasonable cell dimensions
        const _: () = assert!(CELL_WIDTH > 5.0 && CELL_WIDTH < 15.0);
        const _: () = assert!(CELL_HEIGHT > 10.0 && CELL_HEIGHT < 25.0);
    }

    #[test]
    fn test_refresh_interval_is_reasonable() {
        // Refresh can be up to 120fps (8ms) for smoother terminal output
        const _: () = assert!(REFRESH_INTERVAL_MS >= 4);
        const _: () = assert!(REFRESH_INTERVAL_MS <= 100);
    }
    
    // ========================================================================
    // Terminal Size Calculation Tests
    // ========================================================================
    
    #[test]
    fn test_calculate_terminal_size_basic() {
        use gpui::px;
        
        // Window of 750x500 pixels
        let (cols, rows) = TermPrompt::calculate_terminal_size(px(750.0), px(500.0));
        
        // Expected: (750 - 16) / 8.4 = 87.38 -> 87 cols
        // Expected: (500 - 16) / 17 = 28.47 -> 28 rows
        assert!((80..=95).contains(&cols), "Cols should be around 87, got {}", cols);
        assert!((25..=35).contains(&rows), "Rows should be around 28, got {}", rows);
    }
    
    #[test]
    fn test_calculate_terminal_size_minimum() {
        use gpui::px;
        
        // Very small window should return minimum size
        let (cols, rows) = TermPrompt::calculate_terminal_size(px(50.0), px(50.0));
        
        assert_eq!(cols, MIN_COLS, "Should use minimum cols");
        assert_eq!(rows, MIN_ROWS, "Should use minimum rows");
    }
    
    #[test]
    fn test_calculate_terminal_size_large() {
        use gpui::px;
        
        // Large window (1920x1080)
        let (cols, rows) = TermPrompt::calculate_terminal_size(px(1920.0), px(1080.0));
        
        // Should be roughly 226 cols x 62 rows
        assert!(cols > 200, "Large window should have many cols, got {}", cols);
        assert!(rows > 50, "Large window should have many rows, got {}", rows);
    }
}
