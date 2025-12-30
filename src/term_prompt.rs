//! Terminal prompt component for GPUI
//!
//! Renders terminal content and handles keyboard input with proper monospace grid,
//! cursor rendering, per-cell colors, and control character handling.

use gpui::{
    div, prelude::*, px, rgb, Context, FocusHandle, Focusable, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Render, ScrollDelta, ScrollWheelEvent, SharedString,
    Timer, Window,
};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, trace, warn};

use crate::config::Config;
use crate::prompts::SubmitCallback;
use crate::terminal::{CellAttributes, TerminalContent, TerminalEvent, TerminalHandle};
use crate::theme::Theme;

const SLOW_RENDER_THRESHOLD_MS: u128 = 16; // 60fps threshold

/// Base font size for calculating ratios
const BASE_FONT_SIZE: f32 = 14.0;
/// Line height multiplier - 1.3 provides room for descenders (g, y, p, q, j)
/// and ascenders while keeping text readable
const LINE_HEIGHT_MULTIPLIER: f32 = 1.3;

/// Terminal cell dimensions at base font size
/// Cell width for Menlo 14pt is 8.4287px (measured). We use a slightly larger value
/// to be conservative and prevent the last character from wrapping to the next line.
/// Using 8.5px ensures we never tell the PTY we have more columns than can render.
const BASE_CELL_WIDTH: f32 = 8.5; // Conservative value for Menlo 14pt (actual: 8.4287px)
/// Default cell height at base font size (used for tests and static calculations)
const BASE_CELL_HEIGHT: f32 = BASE_FONT_SIZE * LINE_HEIGHT_MULTIPLIER; // 18.2px for 14pt

// Aliases for backwards compatibility with tests
#[allow(dead_code)]
const CELL_WIDTH: f32 = BASE_CELL_WIDTH;
#[allow(dead_code)]
const CELL_HEIGHT: f32 = BASE_CELL_HEIGHT;

/// Terminal refresh interval (ms) - 30fps is plenty for terminal output
const REFRESH_INTERVAL_MS: u64 = 33; // ~30fps, reduces CPU load significantly

/// Minimum terminal size
const MIN_COLS: u16 = 20;
const MIN_ROWS: u16 = 5;

/// Duration for bell visual flash
const BELL_FLASH_DURATION_MS: u64 = 150;

/// Terminal prompt GPUI component
pub struct TermPrompt {
    pub id: String,
    pub terminal: TerminalHandle,
    pub focus_handle: FocusHandle,
    pub on_submit: SubmitCallback,
    pub theme: Arc<Theme>,
    pub config: Arc<Config>,
    exited: bool,
    exit_code: Option<i32>,
    /// Whether the refresh timer is active
    refresh_timer_active: bool,
    /// Last known terminal size (cols, rows)
    last_size: (u16, u16),
    /// Explicit content height - GPUI entities don't inherit parent flex sizing
    content_height: Option<Pixels>,
    /// Time until which the bell flash should be visible
    bell_flash_until: Option<Instant>,
    /// Terminal title from OSC escape sequences
    title: Option<String>,
    /// Whether mouse is currently dragging for selection
    is_selecting: bool,
    /// Start position of mouse selection (in terminal grid coordinates: col, row)
    selection_start: Option<(usize, usize)>,
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
        config: Arc<Config>,
    ) -> anyhow::Result<Self> {
        Self::with_height(id, command, focus_handle, on_submit, theme, config, None)
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
        config: Arc<Config>,
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
            config,
            exited: false,
            exit_code: None,
            refresh_timer_active: false,
            last_size: (initial_cols, initial_rows),
            content_height,
            bell_flash_until: None,
            title: None,
            is_selecting: false,
            selection_start: None,
        })
    }

    /// Set the content height (for dynamic resizing)
    #[allow(dead_code)]
    pub fn set_height(&mut self, height: Pixels) {
        self.content_height = Some(height);
    }

    /// Get the configured font size
    fn font_size(&self) -> f32 {
        self.config.get_terminal_font_size()
    }

    /// Get cell width scaled to configured font size
    fn cell_width(&self) -> f32 {
        BASE_CELL_WIDTH * (self.font_size() / BASE_FONT_SIZE)
    }

    /// Get cell height scaled to configured font size
    fn cell_height(&self) -> f32 {
        self.font_size() * LINE_HEIGHT_MULTIPLIER
    }

    /// Convert pixel position to terminal grid cell (col, row)
    fn pixel_to_cell(&self, position: gpui::Point<Pixels>) -> (usize, usize) {
        let padding = self.config.get_padding();
        let pos_x: f32 = position.x.into();
        let pos_y: f32 = position.y.into();
        let x = (pos_x - padding.left).max(0.0);
        let y = (pos_y - padding.top).max(0.0);

        let col = (x / self.cell_width()) as usize;
        let row = (y / self.cell_height()) as usize;

        (col, row)
    }

    /// Calculate terminal dimensions from pixel size with padding (uses default cell dimensions)
    /// This version uses the base font size dimensions, suitable for tests and static calculations.
    #[cfg(test)]
    fn calculate_terminal_size(
        width: Pixels,
        height: Pixels,
        padding_left: f32,
        padding_right: f32,
        padding_top: f32,
        padding_bottom: f32,
    ) -> (u16, u16) {
        Self::calculate_terminal_size_with_cells(
            width,
            height,
            padding_left,
            padding_right,
            padding_top,
            padding_bottom,
            CELL_WIDTH,
            CELL_HEIGHT,
        )
    }

    /// Calculate terminal dimensions from pixel size with padding and custom cell dimensions
    #[allow(clippy::too_many_arguments)]
    fn calculate_terminal_size_with_cells(
        width: Pixels,
        height: Pixels,
        padding_left: f32,
        padding_right: f32,
        padding_top: f32,
        padding_bottom: f32,
        cell_width: f32,
        cell_height: f32,
    ) -> (u16, u16) {
        // Subtract padding from available space
        let available_width = f32::from(width) - padding_left - padding_right;
        let available_height = f32::from(height) - padding_top - padding_bottom;

        // Calculate columns and rows
        // Use floor() for cols to ensure we never tell the PTY we have more columns
        // than can actually be rendered. Combined with a conservative cell_width,
        // this prevents the last character from wrapping.
        let cols = (available_width / cell_width).floor() as u16;
        let rows = (available_height / cell_height).floor() as u16;

        // Apply minimum bounds
        let cols = cols.max(MIN_COLS);
        let rows = rows.max(MIN_ROWS);

        (cols, rows)
    }

    /// Resize terminal if needed based on new dimensions
    fn resize_if_needed(&mut self, width: Pixels, height: Pixels) {
        let padding = self.config.get_padding();
        let cell_width = self.cell_width();
        let cell_height = self.cell_height();
        // Note: We use padding.top for bottom padding as well (see render() which uses pb(px(padding.top)))
        let (new_cols, new_rows) = Self::calculate_terminal_size_with_cells(
            width,
            height,
            padding.left,
            padding.right,
            padding.top,
            padding.top,
            cell_width,
            cell_height,
        );

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

                            // Process terminal output - 2 iterations catches bursts without excessive overhead
                            for _ in 0..2 {
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
            "z" => Some(0x1A),  // SIGTSTP
            "[" => Some(0x1B),  // ESC
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
        let selection_bg = rgb(colors.accent.selected_subtle);
        let default_fg = rgb(colors.text.primary);

        // Get dynamic font sizing
        let font_size = self.font_size();
        let cell_height = self.cell_height();
        let cell_width = self.cell_width();

        // Build HashSet for O(1) selection lookup
        let selected: HashSet<(usize, usize)> =
            content.selected_cells.iter().cloned().collect();

        let mut lines_container = div()
            .flex()
            .flex_col()
            .flex_1()
            .size_full() // Both w_full and h_full
            .min_h(px(0.)) // Critical for flex children sizing
            .overflow_hidden()
            .bg(default_bg)
            .font_family("Menlo")
            .text_size(px(font_size))
            .line_height(px(cell_height)); // Use calculated line height for proper descender room

        for (line_idx, cells) in content.styled_lines.iter().enumerate() {
            let is_cursor_line = line_idx == content.cursor_line;

            // Build a row - we'll batch consecutive cells with same styling
            let mut row = div().flex().flex_row().w_full().h(px(cell_height));

            // Batch consecutive cells with same styling
            let mut batch_start = 0;
            while batch_start < cells.len() {
                let first_cell = &cells[batch_start];
                let is_cursor_start = is_cursor_line && batch_start == content.cursor_col;
                let is_selected_start = selected.contains(&(batch_start, line_idx));

                // Get styling for this batch
                let fg_u32 = (first_cell.fg.r as u32) << 16
                    | (first_cell.fg.g as u32) << 8
                    | (first_cell.fg.b as u32);
                let bg_u32 = (first_cell.bg.r as u32) << 16
                    | (first_cell.bg.g as u32) << 8
                    | (first_cell.bg.b as u32);
                let attrs = first_cell.attrs;

                // Find how many consecutive cells have the same styling
                // (excluding cursor position and selection boundaries)
                let mut batch_end = batch_start + 1;

                // If this is the cursor cell, it's always its own batch
                if !is_cursor_start {
                    while batch_end < cells.len() {
                        let cell = &cells[batch_end];
                        let is_cursor_here = is_cursor_line && batch_end == content.cursor_col;
                        let is_selected_here = selected.contains(&(batch_end, line_idx));

                        // Stop if cursor, selection boundary change, or different styling
                        if is_cursor_here || is_selected_here != is_selected_start {
                            break;
                        }

                        let cell_fg =
                            (cell.fg.r as u32) << 16 | (cell.fg.g as u32) << 8 | (cell.fg.b as u32);
                        let cell_bg =
                            (cell.bg.r as u32) << 16 | (cell.bg.g as u32) << 8 | (cell.bg.b as u32);

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

                let batch_width = (batch_end - batch_start) as f32 * cell_width;

                // Determine colors - priority: cursor > selection > custom bg > default
                let (fg_color, bg_color) = if is_cursor_start {
                    // Cursor inverts colors
                    (rgb(bg_u32), cursor_bg)
                } else if is_selected_start {
                    // Selection uses selection background with original foreground
                    (if fg_u32 == 0 { default_fg } else { rgb(fg_u32) }, selection_bg)
                } else if bg_u32 != 0x000000 {
                    // Custom background
                    (if fg_u32 == 0 { default_fg } else { rgb(fg_u32) }, rgb(bg_u32))
                } else {
                    // Default
                    (if fg_u32 == 0 { default_fg } else { rgb(fg_u32) }, default_bg)
                };

                let mut span = div()
                    .w(px(batch_width))
                    .h(px(cell_height))
                    .flex_shrink_0()
                    .bg(bg_color) // Always apply background to prevent bleed-through
                    .text_color(fg_color)
                    .child(SharedString::from(batch_text));

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

        // Process terminal events - minimal polling since timer handles most updates
        // 2 iterations catches any output that arrived since last timer tick
        if !self.exited {
            for _ in 0..2 {
                let events = self.terminal.process();
                for event in events {
                    match event {
                        TerminalEvent::Exit(code) => {
                            self.handle_exit(code);
                            break;
                        }
                        TerminalEvent::Bell => {
                            self.bell_flash_until = Some(
                                Instant::now() + Duration::from_millis(BELL_FLASH_DURATION_MS),
                            );
                            debug!("Terminal bell triggered, flashing border");
                        }
                        TerminalEvent::Title(title) => {
                            self.title = if title.is_empty() { None } else { Some(title) };
                            debug!(title = ?self.title, "Terminal title updated");
                        }
                        TerminalEvent::Output(_) => { /* handled by content() */ }
                    }
                }
                if self.exited {
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
                  _cx: &mut Context<Self>| {
                let key_str = event.keystroke.key.to_lowercase();
                let has_ctrl = event.keystroke.modifiers.control;
                let has_meta = event.keystroke.modifiers.platform;
                let has_shift = event.keystroke.modifiers.shift;

                // Escape always cancels
                if key_str == "escape" {
                    this.submit_cancel();
                    return;
                }

                // Handle Shift+PageUp/PageDown/Home/End for scrollback navigation
                // These work even after terminal exits to review output
                if has_shift {
                    match key_str.as_str() {
                        "pageup" => {
                            this.terminal.scroll_page_up();
                            debug!("Shift+PageUp: scrolling terminal page up");
                            return;
                        }
                        "pagedown" => {
                            this.terminal.scroll_page_down();
                            debug!("Shift+PageDown: scrolling terminal page down");
                            return;
                        }
                        "home" => {
                            this.terminal.scroll_to_top();
                            debug!("Shift+Home: scrolling terminal to top");
                            return;
                        }
                        "end" => {
                            this.terminal.scroll_to_bottom();
                            debug!("Shift+End: scrolling terminal to bottom");
                            return;
                        }
                        _ => {}
                    }
                }

                // Check if terminal is still running before sending input
                if this.exited || !this.terminal.is_running() {
                    trace!(key = %key_str, "Terminal exited, ignoring key input");
                    return;
                }

                // Handle Cmd+V paste (macOS: platform modifier = Command key)
                if has_meta && key_str == "v" {
                    use arboard::Clipboard;
                    if let Ok(mut clipboard) = Clipboard::new() {
                        if let Ok(text) = clipboard.get_text() {
                            // Check if bracketed paste mode is enabled
                            // When enabled, wrap paste in escape sequences so apps know it's pasted
                            let paste_data = if this.terminal.is_bracketed_paste_mode() {
                                debug!(text_len = text.len(), "Pasting with bracketed paste mode");
                                format!("\x1b[200~{}\x1b[201~", text)
                            } else {
                                debug!(text_len = text.len(), "Pasting clipboard text to terminal");
                                text
                            };
                            
                            if let Err(e) = this.terminal.input(paste_data.as_bytes()) {
                                if !this.exited {
                                    warn!(error = %e, "Failed to paste clipboard to terminal");
                                }
                            }
                        }
                    }
                    return;
                }

                // Handle Cmd+C copy (when there's a selection)
                // If no selection exists, fall through to send Ctrl+C (SIGINT) to the terminal
                if has_meta && key_str == "c" {
                    if let Some(selected_text) = this.terminal.selection_to_string() {
                        if !selected_text.is_empty() {
                            use arboard::Clipboard;
                            if let Ok(mut clipboard) = Clipboard::new() {
                                if clipboard.set_text(&selected_text).is_ok() {
                                    debug!(text_len = selected_text.len(), "Copied selection to clipboard");
                                }
                            }
                            // Clear selection after copy (common terminal behavior)
                            this.terminal.clear_selection();
                            return;
                        }
                    }
                    // No selection - fall through to let Ctrl+C logic handle SIGINT
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
                        // No cx.notify() needed - timer handles refresh at 30fps
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
                    // No cx.notify() needed - timer handles refresh at 30fps
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
                        // No cx.notify() needed - timer handles refresh at 30fps
                    }
                }
            },
        );

        // Render terminal content with styled cells
        let colors = &self.theme.colors;
        let terminal_content = self.render_content(&content);

        // Get padding from config
        let padding = self.config.get_padding();

        // Check if bell is flashing
        let is_bell_flashing = self
            .bell_flash_until
            .map(|until| Instant::now() < until)
            .unwrap_or(false);

        // Log slow renders
        let elapsed = start.elapsed().as_millis();
        if elapsed > SLOW_RENDER_THRESHOLD_MS {
            warn!(elapsed_ms = elapsed, "Slow terminal render");
        } else {
            debug!(elapsed_ms = elapsed, "Terminal render");
        }

        // Main container with terminal styling
        // Use explicit height if available, otherwise fall back to size_full
        // Apply padding from config settings (top/left/right/bottom)
        // Use main background color to match window - no visible seam at edges
        let container = div()
            .flex()
            .flex_col()
            .w_full()
            .pl(px(padding.left))
            .pr(px(padding.right))
            .pt(px(padding.top))
            .pb(px(padding.top)) // Use same as top for consistent spacing
            .bg(rgb(colors.background.main))
            .text_color(rgb(colors.text.primary))
            .overflow_hidden() // Clip any overflow
            .key_context("term_prompt")
            .track_focus(&self.focus_handle)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _window, _cx| {
                    let (col, row) = this.pixel_to_cell(event.position);
                    debug!(col, row, "Mouse down at cell - starting selection");
                    this.is_selecting = true;
                    this.selection_start = Some((col, row));
                    this.terminal.start_selection(col, row);
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, _cx| {
                if this.is_selecting {
                    let (col, row) = this.pixel_to_cell(event.position);
                    this.terminal.update_selection(col, row);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, _window, _cx| {
                    if this.is_selecting {
                        let (col, row) = this.pixel_to_cell(event.position);
                        debug!(col, row, "Mouse up at cell - finalizing selection");
                        this.terminal.update_selection(col, row);
                        this.is_selecting = false;
                        
                        // Log the selected text if any
                        if let Some(text) = this.terminal.selection_to_string() {
                            let preview = if text.len() > 50 { 
                                format!("{}...", &text[..50]) 
                            } else { 
                                text.clone() 
                            };
                            debug!(text_len = text.len(), "Selection complete: {:?}", preview);
                        }
                    }
                }),
            )
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _window, cx| {
                // Get scroll direction from delta
                // Lines: direct line count, Pixels: convert based on cell height
                let lines = match event.delta {
                    ScrollDelta::Lines(point) => point.y,
                    ScrollDelta::Pixels(point) => {
                        // Convert pixels to lines by dividing by cell height
                        // Pixels implements Div<Pixels> -> f32
                        let cell_height = px(this.cell_height());
                        point.y / cell_height
                    }
                };
                
                // Convert to integer lines (positive = scroll down, negative = scroll up)
                // In terminal scrollback: negative delta scrolls up into history
                // We invert because terminal scroll() uses positive = scroll up (into history)
                let scroll_lines = -lines.round() as i32;
                
                if scroll_lines != 0 {
                    this.terminal.scroll(scroll_lines);
                    trace!(delta = scroll_lines, "Mouse wheel scroll");
                    cx.notify();
                }
            }))
            .on_key_down(handle_key);

        // Apply bell flash border if active
        let container = if is_bell_flashing {
            container
                .border_2()
                .border_color(rgb(colors.accent.selected))
        } else {
            container
        };

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
            ('a', 0x01),
            ('b', 0x02),
            ('c', 0x03),
            ('d', 0x04),
            ('e', 0x05),
            ('f', 0x06),
            ('g', 0x07),
            ('h', 0x08),
            ('i', 0x09),
            ('j', 0x0A),
            ('k', 0x0B),
            ('l', 0x0C),
            ('m', 0x0D),
            ('n', 0x0E),
            ('o', 0x0F),
            ('p', 0x10),
            ('q', 0x11),
            ('r', 0x12),
            ('s', 0x13),
            ('t', 0x14),
            ('u', 0x15),
            ('v', 0x16),
            ('w', 0x17),
            ('x', 0x18),
            ('y', 0x19),
            ('z', 0x1A),
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

        // Window of 750x500 pixels with default padding (12 left, 12 right, 8 top, 8 bottom)
        // Available width: 750 - 12 - 12 = 726
        // Available height: 500 - 8 - 8 = 484
        let (cols, rows) =
            TermPrompt::calculate_terminal_size(px(750.0), px(500.0), 12.0, 12.0, 8.0, 8.0);

        // Expected: 726 / 8.5 = 85.4 -> 85 cols
        // Expected: 484 / 18.2 = 26.6 -> 26 rows
        assert!(
            (80..=90).contains(&cols),
            "Cols should be around 85, got {}",
            cols
        );
        assert!(
            (24..=28).contains(&rows),
            "Rows should be around 26, got {}",
            rows
        );
    }

    #[test]
    fn test_calculate_terminal_size_minimum() {
        use gpui::px;

        // Very small window should return minimum size
        let (cols, rows) =
            TermPrompt::calculate_terminal_size(px(50.0), px(50.0), 0.0, 0.0, 0.0, 0.0);

        assert_eq!(cols, MIN_COLS, "Should use minimum cols");
        assert_eq!(rows, MIN_ROWS, "Should use minimum rows");
    }

    #[test]
    fn test_calculate_terminal_size_large() {
        use gpui::px;

        // Large window (1920x1080) with no padding
        let (cols, rows) =
            TermPrompt::calculate_terminal_size(px(1920.0), px(1080.0), 0.0, 0.0, 0.0, 0.0);

        // Should be roughly 225 cols x 59 rows
        assert!(
            cols > 200,
            "Large window should have many cols, got {}",
            cols
        );
        assert!(
            rows > 50,
            "Large window should have many rows, got {}",
            rows
        );
    }

    #[test]
    fn test_calculate_terminal_size_conservative() {
        use gpui::px;

        // Test that we use conservative column calculation to prevent wrapping.
        // CELL_WIDTH is 8.5px (slightly larger than actual 8.4287px Menlo width)
        // to ensure we never tell PTY we have more columns than can render.

        // With no padding: 680px / 8.5 = 80.0 -> exactly 80 cols
        let (cols, _rows) =
            TermPrompt::calculate_terminal_size(px(680.0), px(500.0), 0.0, 0.0, 0.0, 0.0);
        assert_eq!(
            cols, 80,
            "680px width should give 80 cols (680/8.5=80), got {}",
            cols
        );

        // 679px / 8.5 = 79.88 -> floors to 79 cols (conservative)
        let (cols2, _) =
            TermPrompt::calculate_terminal_size(px(679.0), px(500.0), 0.0, 0.0, 0.0, 0.0);
        assert_eq!(
            cols2, 79,
            "679px width should give 79 cols (679/8.5=79.88 floors to 79), got {}",
            cols2
        );

        // 500px / 8.5 = 58.82 -> floors to 58 cols
        let (cols3, _) =
            TermPrompt::calculate_terminal_size(px(500.0), px(500.0), 0.0, 0.0, 0.0, 0.0);
        assert_eq!(
            cols3, 58,
            "500px width should give 58 cols (500/8.5=58.82 floors to 58), got {}",
            cols3
        );
    }

    #[test]
    fn test_calculate_terminal_size_with_padding() {
        use gpui::px;

        // Test that padding is properly subtracted from available space
        // 500px width with 12px left and 12px right padding = 476px available
        // 476 / 8.5 = 56.0 -> 56 cols
        let (cols, _) =
            TermPrompt::calculate_terminal_size(px(500.0), px(500.0), 12.0, 12.0, 0.0, 0.0);
        assert_eq!(
            cols, 56,
            "500px with 24px total horizontal padding should give 56 cols, got {}",
            cols
        );

        // 500px height with 8px top padding only = 492px available
        // 492 / 18.2 = 27.0 -> 27 rows
        let (_, rows) =
            TermPrompt::calculate_terminal_size(px(500.0), px(500.0), 0.0, 0.0, 8.0, 0.0);
        assert_eq!(
            rows, 27,
            "500px with 8px top padding only should give 27 rows, got {}",
            rows
        );

        // 500px height with 8px top AND 8px bottom padding = 484px available
        // 484 / 18.2 = 26.6 -> 26 rows
        let (_, rows2) =
            TermPrompt::calculate_terminal_size(px(500.0), px(500.0), 0.0, 0.0, 8.0, 8.0);
        assert_eq!(
            rows2, 26,
            "500px with 8px top+bottom padding should give 26 rows, got {}",
            rows2
        );
    }

    // ========================================================================
    // Padding Symmetry Regression Tests
    //
    // BUG FIXED: calculate_terminal_size only subtracted padding_top from height,
    // but render() applied BOTH top AND bottom padding, causing content cutoff.
    // These tests ensure the fix is never regressed.
    // ========================================================================

    #[test]
    fn test_padding_symmetry_regression_top_and_bottom_must_both_be_subtracted() {
        use gpui::px;

        // REGRESSION TEST: This test would FAIL if padding_bottom is not subtracted.
        //
        // Scenario: 700px window height with 8px top and 8px bottom padding
        // render() applies: pt(8) + pb(8) = 16px total vertical padding
        // calculate_terminal_size MUST subtract BOTH:
        //   available_height = 700 - 8 - 8 = 684px
        //   rows = floor(684 / 18.2) = 37 rows
        //
        // If only padding_top was subtracted (the bug):
        //   available_height = 700 - 8 = 692px
        //   rows = floor(692 / 18.2) = 38 rows
        //   Then 38 * 18.2 = 691.6px > 684px available = CONTENT CUTOFF!

        let padding_top = 8.0;
        let padding_bottom = 8.0;
        let total_height = 700.0;

        let (_, rows) = TermPrompt::calculate_terminal_size(
            px(500.0),
            px(total_height),
            0.0,
            0.0,
            padding_top,
            padding_bottom,
        );

        // Verify the row count accounts for BOTH paddings
        let expected_available_height = total_height - padding_top - padding_bottom;
        let expected_rows = (expected_available_height / CELL_HEIGHT).floor() as u16;

        assert_eq!(
            rows, expected_rows,
            "REGRESSION: padding_bottom not being subtracted! \
            Expected {} rows (684px / 18.2), got {} rows. \
            This means content will be cut off!",
            expected_rows, rows
        );

        // Additional invariant: rendered content must fit within available space
        let content_height = rows as f32 * CELL_HEIGHT;
        let available_height = total_height - padding_top - padding_bottom;
        assert!(
            content_height <= available_height,
            "REGRESSION: Content ({:.1}px = {} rows × {:.1}px) exceeds available height ({:.1}px)!",
            content_height,
            rows,
            CELL_HEIGHT,
            available_height
        );
    }

    #[test]
    fn test_padding_symmetry_invariant_content_plus_padding_never_exceeds_total() {
        use gpui::px;

        // INVARIANT TEST: rows * CELL_HEIGHT + padding_top + padding_bottom <= total_height
        // This must hold for ANY valid padding values.

        let test_cases: Vec<(f32, f32, f32)> = vec![
            // (total_height, padding_top, padding_bottom)
            (700.0, 8.0, 8.0),   // Default case
            (500.0, 8.0, 8.0),   // Smaller window
            (700.0, 16.0, 16.0), // Larger padding
            (700.0, 0.0, 0.0),   // No padding
            (700.0, 20.0, 20.0), // Very large padding
            (400.0, 50.0, 50.0), // Extreme padding ratio
        ];

        for (total_height, padding_top, padding_bottom) in test_cases {
            let (_, rows) = TermPrompt::calculate_terminal_size(
                px(500.0),
                px(total_height),
                0.0,
                0.0,
                padding_top,
                padding_bottom,
            );

            let content_height = rows as f32 * CELL_HEIGHT;
            let total_used = content_height + padding_top + padding_bottom;

            assert!(
                total_used <= total_height,
                "INVARIANT VIOLATED for height={}, top={}, bottom={}: \
                content ({} rows × {:.1}px = {:.1}px) + padding ({:.1}+{:.1}={:.1}px) = {:.1}px > {:.1}px!",
                total_height, padding_top, padding_bottom,
                rows, CELL_HEIGHT, content_height,
                padding_top, padding_bottom, padding_top + padding_bottom,
                total_used, total_height
            );
        }
    }

    #[test]
    fn test_padding_edge_case_padding_exceeds_available_height() {
        use gpui::px;

        // EDGE CASE: What happens when padding is larger than available height?
        // Should return MIN_ROWS to prevent panic/negative values.

        let total_height = 50.0;
        let padding_top = 30.0;
        let padding_bottom = 30.0;
        // Available height = 50 - 30 - 30 = -10px (negative!)

        let (_, rows) = TermPrompt::calculate_terminal_size(
            px(500.0),
            px(total_height),
            0.0,
            0.0,
            padding_top,
            padding_bottom,
        );

        // Should return minimum rows, not crash or return 0
        assert_eq!(
            rows, MIN_ROWS,
            "When padding exceeds height, should return MIN_ROWS ({}), got {}",
            MIN_ROWS, rows
        );
    }

    #[test]
    fn test_padding_symmetry_max_height_scenario() {
        use gpui::px;

        // This test uses the actual MAX_HEIGHT (700px) from window_resize.rs
        // to verify the exact scenario that was causing content cutoff.
        const MAX_HEIGHT: f32 = 700.0;
        const DEFAULT_PADDING: f32 = 8.0; // From config defaults

        let (_, rows) = TermPrompt::calculate_terminal_size(
            px(500.0),
            px(MAX_HEIGHT),
            12.0,
            12.0,            // left/right padding
            DEFAULT_PADDING, // top
            DEFAULT_PADDING, // bottom
        );

        // With 700px and 8+8=16px vertical padding:
        // Available = 684px, rows = floor(684/18.2) = 37
        assert_eq!(
            rows, 37,
            "MAX_HEIGHT (700px) with 8px symmetric padding should give 37 rows, got {}. \
            This was the exact bug scenario!",
            rows
        );

        // Verify no cutoff
        let content_height = rows as f32 * CELL_HEIGHT;
        let available_height = MAX_HEIGHT - DEFAULT_PADDING - DEFAULT_PADDING;

        assert!(
            content_height <= available_height,
            "Content ({:.1}px) exceeds available space ({:.1}px) - cutoff will occur!",
            content_height,
            available_height
        );
    }

    #[test]
    fn test_padding_difference_between_buggy_and_fixed_calculation() {
        use gpui::px;

        // This test explicitly compares the buggy calculation vs the fixed one
        // to demonstrate what the bug was.

        let height = 700.0;
        let padding_top = 8.0;
        let padding_bottom = 8.0;

        // BUGGY calculation (only subtracts top):
        let buggy_available = height - padding_top; // 692px
        let buggy_rows = (buggy_available / CELL_HEIGHT).floor() as u16; // 38 rows

        // FIXED calculation (subtracts both):
        let fixed_available = height - padding_top - padding_bottom; // 684px
        let fixed_rows = (fixed_available / CELL_HEIGHT).floor() as u16; // 37 rows

        // The actual function should use the FIXED calculation
        let (_, actual_rows) = TermPrompt::calculate_terminal_size(
            px(500.0),
            px(height),
            0.0,
            0.0,
            padding_top,
            padding_bottom,
        );

        assert_ne!(
            actual_rows, buggy_rows,
            "REGRESSION: Function returned buggy row count ({})! \
            Should be {} (with both paddings subtracted).",
            buggy_rows, fixed_rows
        );

        assert_eq!(
            actual_rows, fixed_rows,
            "Function should return {} rows (fixed), got {}.",
            fixed_rows, actual_rows
        );

        // Show the difference (for documentation)
        assert_eq!(
            buggy_rows - fixed_rows,
            1,
            "Bug caused 1 extra row, leading to {:.1}px cutoff",
            CELL_HEIGHT
        );
    }
}
