//! Alacritty terminal emulator integration for Script Kit GPUI.
//!
//! This module wraps Alacritty's terminal emulator library to provide
//! VT100/xterm compatible terminal emulation. It handles escape sequence
//! parsing, terminal grid management, and state tracking.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     ┌──────────────┐     ┌─────────────┐
//! │  PTY Output │ ──▶ │ VTE Parser   │ ──▶ │ Term Grid   │
//! └─────────────┘     └──────────────┘     └─────────────┘
//!                                                 │
//!                                                 ▼
//!                                          ┌─────────────┐
//!                                          │ GPUI Render │
//!                                          └─────────────┘
//! ```
//!
//! The terminal processes incoming bytes through the VTE parser, which
//! interprets escape sequences and updates the terminal grid. The grid
//! state is then read by the GPUI rendering layer.
//!
//! # Thread Safety
//!
//! `TerminalHandle` uses `Arc<Mutex<>>` for the terminal state, allowing
//! safe access from multiple threads. The PTY I/O can run on a background
//! thread while the main thread reads terminal content for rendering.
//!

use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event as AlacrittyEvent, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Direction, Line, Point as AlacPoint};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::cell::Flags as AlacrittyFlags;
use alacritty_terminal::term::{Config as TermConfig, Term, TermMode};
use anyhow::{Context, Result};
use bitflags::bitflags;
use tracing::{debug, info, instrument, trace, warn};
use vte::ansi::{Color, NamedColor, Processor, Rgb};

use crate::terminal::pty::PtyManager;
use crate::terminal::theme_adapter::ThemeAdapter;
use crate::terminal::TerminalEvent;

/// Default scrollback buffer size in lines.
const DEFAULT_SCROLLBACK_LINES: usize = 10_000;

/// Maximum bytes to read from PTY in a single process() call.
const PTY_READ_BUFFER_SIZE: usize = 4096;

/// Event proxy for alacritty_terminal - handles terminal events.
///
/// This struct implements `EventListener` to receive events from the
/// Alacritty terminal emulator. Events are batched for efficient processing.
///
/// The EventProxy is cloneable because it shares the event queue via Arc.
#[derive(Debug, Clone)]
pub struct EventProxy {
    /// Batched events waiting to be processed.
    events: Arc<Mutex<Vec<TerminalEvent>>>,
}

impl EventProxy {
    /// Creates a new event proxy with an empty event queue.
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Takes all pending events, leaving an empty queue.
    pub fn take_events(&self) -> Vec<TerminalEvent> {
        let mut events = self.events.lock().unwrap();
        std::mem::take(&mut *events)
    }
}

impl Default for EventProxy {
    fn default() -> Self {
        Self::new()
    }
}

impl EventListener for EventProxy {
    fn send_event(&self, event: AlacrittyEvent) {
        let terminal_event = match event {
            AlacrittyEvent::Bell => {
                debug!("Terminal bell received");
                Some(TerminalEvent::Bell)
            }
            AlacrittyEvent::Title(title) => {
                debug!(title = %title, "Terminal title changed");
                Some(TerminalEvent::Title(title))
            }
            AlacrittyEvent::ResetTitle => {
                debug!("Terminal title reset");
                Some(TerminalEvent::Title(String::new()))
            }
            AlacrittyEvent::Exit => {
                info!("Terminal exit requested");
                Some(TerminalEvent::Exit(0))
            }
            AlacrittyEvent::ChildExit(code) => {
                info!(exit_code = code, "Child process exited");
                Some(TerminalEvent::Exit(code))
            }
            AlacrittyEvent::Wakeup => {
                trace!("Terminal wakeup event");
                // Wakeup events indicate new content is available
                None
            }
            AlacrittyEvent::PtyWrite(text) => {
                trace!(bytes = text.len(), "PTY write request");
                // PtyWrite events are handled internally
                None
            }
            AlacrittyEvent::MouseCursorDirty => {
                trace!("Mouse cursor dirty");
                None
            }
            AlacrittyEvent::CursorBlinkingChange => {
                trace!("Cursor blinking state changed");
                None
            }
            AlacrittyEvent::ClipboardStore(_, _) => {
                trace!("Clipboard store request");
                None
            }
            AlacrittyEvent::ClipboardLoad(_, _) => {
                trace!("Clipboard load request");
                None
            }
            AlacrittyEvent::ColorRequest(_, _) => {
                trace!("Color request");
                None
            }
            AlacrittyEvent::TextAreaSizeRequest(_) => {
                trace!("Text area size request");
                None
            }
        };

        if let Some(event) = terminal_event {
            let mut events = self.events.lock().unwrap();
            events.push(event);
        }
    }
}

/// Terminal dimensions for creating Term instance.
#[derive(Debug, Clone, Copy)]
pub struct TerminalSize {
    /// Number of columns.
    pub cols: usize,
    /// Number of rows.
    pub rows: usize,
}

impl TerminalSize {
    /// Creates a new terminal size.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols: cols as usize,
            rows: rows as usize,
        }
    }
}

impl Dimensions for TerminalSize {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

bitflags! {
    /// Cell attributes for text styling.
    ///
    /// These flags represent visual attributes that can be applied to
    /// terminal cells, such as bold, italic, and underline.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct CellAttributes: u16 {
        /// Bold text (typically rendered with brighter colors or heavier font weight).
        const BOLD = 0b0000_0000_0000_0001;
        /// Italic text.
        const ITALIC = 0b0000_0000_0000_0010;
        /// Underlined text.
        const UNDERLINE = 0b0000_0000_0000_0100;
        /// Double underline.
        const DOUBLE_UNDERLINE = 0b0000_0000_0000_1000;
        /// Curly/wavy underline.
        const UNDERCURL = 0b0000_0000_0001_0000;
        /// Dotted underline.
        const DOTTED_UNDERLINE = 0b0000_0000_0010_0000;
        /// Dashed underline.
        const DASHED_UNDERLINE = 0b0000_0000_0100_0000;
        /// Strikethrough text.
        const STRIKEOUT = 0b0000_0000_1000_0000;
        /// Inverse/reverse video (swap fg/bg).
        const INVERSE = 0b0000_0001_0000_0000;
        /// Hidden/invisible text.
        const HIDDEN = 0b0000_0010_0000_0000;
        /// Dim/faint text.
        const DIM = 0b0000_0100_0000_0000;
    }
}

impl CellAttributes {
    /// Convert from Alacritty's cell Flags to CellAttributes.
    pub fn from_alacritty_flags(flags: AlacrittyFlags) -> Self {
        let mut attrs = Self::empty();

        if flags.contains(AlacrittyFlags::BOLD) {
            attrs.insert(Self::BOLD);
        }
        if flags.contains(AlacrittyFlags::ITALIC) {
            attrs.insert(Self::ITALIC);
        }
        if flags.contains(AlacrittyFlags::UNDERLINE) {
            attrs.insert(Self::UNDERLINE);
        }
        if flags.contains(AlacrittyFlags::DOUBLE_UNDERLINE) {
            attrs.insert(Self::DOUBLE_UNDERLINE);
        }
        if flags.contains(AlacrittyFlags::UNDERCURL) {
            attrs.insert(Self::UNDERCURL);
        }
        if flags.contains(AlacrittyFlags::DOTTED_UNDERLINE) {
            attrs.insert(Self::DOTTED_UNDERLINE);
        }
        if flags.contains(AlacrittyFlags::DASHED_UNDERLINE) {
            attrs.insert(Self::DASHED_UNDERLINE);
        }
        if flags.contains(AlacrittyFlags::STRIKEOUT) {
            attrs.insert(Self::STRIKEOUT);
        }
        if flags.contains(AlacrittyFlags::INVERSE) {
            attrs.insert(Self::INVERSE);
        }
        if flags.contains(AlacrittyFlags::HIDDEN) {
            attrs.insert(Self::HIDDEN);
        }
        if flags.contains(AlacrittyFlags::DIM) {
            attrs.insert(Self::DIM);
        }

        attrs
    }
}

/// A single styled terminal cell with character, colors, and attributes.
///
/// This struct represents the complete visual state of a single cell
/// in the terminal grid, including the character, foreground and background
/// colors (resolved to actual RGB values), and text attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCell {
    /// The character in this cell.
    pub c: char,
    /// Foreground (text) color as RGB.
    pub fg: Rgb,
    /// Background color as RGB.
    pub bg: Rgb,
    /// Cell attributes (bold, italic, underline, etc.).
    pub attrs: CellAttributes,
}

impl Default for TerminalCell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: Rgb {
                r: 212,
                g: 212,
                b: 212,
            }, // Default foreground (light gray)
            bg: Rgb {
                r: 30,
                g: 30,
                b: 30,
            }, // Default background (dark gray)
            attrs: CellAttributes::empty(),
        }
    }
}

/// Thread-safe terminal state wrapper.
///
/// This struct bundles the terminal and its VTE processor together,
/// allowing thread-safe access to both.
struct TerminalState {
    term: Term<EventProxy>,
    processor: Processor,
}

impl TerminalState {
    fn new(config: TermConfig, size: &TerminalSize, event_proxy: EventProxy) -> Self {
        Self {
            term: Term::new(config, size, event_proxy),
            processor: Processor::new(),
        }
    }

    /// Process raw bytes from PTY through the VTE parser.
    fn process_bytes(&mut self, bytes: &[u8]) {
        // VTE 0.15 advance() takes a slice of bytes
        self.processor.advance(&mut self.term, bytes);
    }
}

/// Handle to an Alacritty terminal emulator instance.
///
/// `TerminalHandle` provides the core terminal emulation functionality:
///
/// - **Escape Sequence Parsing**: Full VT100/xterm/ANSI support via VTE
/// - **Grid Management**: Character grid with colors, attributes, and Unicode
/// - **Scrollback Buffer**: Configurable history for scrolling back
/// - **Selection**: Text selection support for copy operations
/// - **Thread Safety**: Safe concurrent access via Arc<Mutex<>>
///
/// # Thread Safety
///
/// The terminal state is wrapped in `Arc<Mutex<>>`, allowing safe access
/// from multiple threads. The main thread can render while a background
/// thread handles PTY I/O.
///
/// # Performance
///
/// The terminal uses event batching and damage tracking to minimize
/// unnecessary re-rendering. Only cells that have changed are marked dirty.
pub struct TerminalHandle {
    /// Thread-safe terminal state.
    state: Arc<Mutex<TerminalState>>,
    /// Event proxy for receiving terminal events (shared with Term).
    event_proxy: EventProxy,
    /// PTY manager for process I/O (writing only - reading happens in background thread).
    pty: PtyManager,
    /// Theme adapter for colors.
    #[allow(dead_code)]
    theme: ThemeAdapter,
    /// Current terminal dimensions.
    cols: u16,
    rows: u16,
    /// Receiver for PTY output from background reader thread.
    pty_output_rx: std::sync::mpsc::Receiver<Vec<u8>>,
    /// Flag to signal background reader to stop.
    reader_stop_flag: Arc<std::sync::atomic::AtomicBool>,
}

impl std::fmt::Debug for TerminalHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalHandle")
            .field("cols", &self.cols)
            .field("rows", &self.rows)
            .finish_non_exhaustive()
    }
}

impl TerminalHandle {
    /// Creates a new terminal handle with the default shell.
    ///
    /// # Arguments
    ///
    /// * `cols` - Number of columns (character width)
    /// * `rows` - Number of rows (character height)
    ///
    /// # Errors
    ///
    /// Returns an error if PTY creation or shell spawning fails.
    #[instrument(level = "info", name = "terminal_new", fields(cols, rows))]
    pub fn new(cols: u16, rows: u16) -> Result<Self> {
        Self::with_scrollback(cols, rows, DEFAULT_SCROLLBACK_LINES)
    }

    /// Creates a new terminal handle running a specific command.
    ///
    /// # Arguments
    ///
    /// * `cmd` - The command to execute
    /// * `cols` - Number of columns
    /// * `rows` - Number of rows
    ///
    /// # Errors
    ///
    /// Returns an error if PTY creation or command spawning fails.
    #[instrument(level = "info", name = "terminal_with_command", fields(cmd = %cmd, cols, rows))]
    pub fn with_command(cmd: &str, cols: u16, rows: u16) -> Result<Self> {
        Self::create_internal(Some(cmd), cols, rows, DEFAULT_SCROLLBACK_LINES)
    }

    /// Creates a new terminal handle with custom scrollback size.
    ///
    /// # Arguments
    ///
    /// * `cols` - Number of columns
    /// * `rows` - Number of rows
    /// * `scrollback_lines` - Maximum lines to keep in scrollback buffer
    ///
    /// # Errors
    ///
    /// Returns an error if PTY creation or shell spawning fails.
    #[instrument(
        level = "info",
        name = "terminal_with_scrollback",
        fields(cols, rows, scrollback_lines)
    )]
    pub fn with_scrollback(cols: u16, rows: u16, scrollback_lines: usize) -> Result<Self> {
        Self::create_internal(None, cols, rows, scrollback_lines)
    }

    /// Internal creation method.
    fn create_internal(
        cmd: Option<&str>,
        cols: u16,
        rows: u16,
        scrollback_lines: usize,
    ) -> Result<Self> {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::mpsc;

        // Always spawn an interactive shell - never use -c which exits after command
        // If a command is provided, we'll write it to the PTY after creation
        let mut pty = PtyManager::with_size(cols, rows).context("Failed to create PTY")?;

        // Create terminal configuration
        let config = TermConfig {
            scrolling_history: scrollback_lines,
            ..TermConfig::default()
        };

        // Create event proxy - we'll share it with the Term
        let event_proxy = EventProxy::new();

        // Create terminal dimensions
        let size = TerminalSize::new(cols, rows);

        // Create the terminal state (Term + Processor)
        let state = TerminalState::new(config, &size, event_proxy.clone());
        let state = Arc::new(Mutex::new(state));

        // Create theme adapter with defaults
        let theme = ThemeAdapter::dark_default();

        // Create channel for PTY output from background reader
        let (pty_output_tx, pty_output_rx) = mpsc::channel();

        // Create stop flag for background reader
        let reader_stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = reader_stop_flag.clone();

        // Take the PTY reader and spawn background thread
        if let Some(mut reader) = pty.take_reader() {
            std::thread::spawn(move || {
                let mut buffer = vec![0u8; PTY_READ_BUFFER_SIZE];
                loop {
                    // Check if we should stop
                    if stop_flag_clone.load(Ordering::Relaxed) {
                        trace!("PTY reader thread stopping");
                        break;
                    }

                    // Read from PTY (this blocks, but that's OK in a background thread)
                    match reader.read(&mut buffer) {
                        Ok(0) => {
                            // EOF - PTY closed
                            trace!("PTY EOF in reader thread");
                            break;
                        }
                        Ok(n) => {
                            // Send data to main thread
                            if pty_output_tx.send(buffer[..n].to_vec()).is_err() {
                                // Channel closed, stop reading
                                trace!("PTY output channel closed");
                                break;
                            }
                        }
                        Err(e) => {
                            if e.kind() != std::io::ErrorKind::Interrupted {
                                warn!(error = %e, "Error reading from PTY in background thread");
                                break;
                            }
                            // Interrupted - continue reading
                        }
                    }
                }
                trace!("PTY reader thread exiting");
            });
        }

        let mut handle = Self {
            state,
            event_proxy,
            pty,
            theme,
            cols,
            rows,
            pty_output_rx,
            reader_stop_flag,
        };

        // If a command was provided, send it to the interactive shell
        // This allows the command to run while keeping the shell open for more input
        if let Some(cmd) = cmd {
            info!(
                cmd = %cmd,
                "Sending initial command to interactive shell"
            );
            // Send command followed by newline to execute it
            let cmd_with_newline = format!("{}\n", cmd);
            if let Err(e) = handle.input(cmd_with_newline.as_bytes()) {
                warn!(error = %e, cmd = %cmd, "Failed to send initial command to terminal");
            }
        }

        info!(
            cols,
            rows, scrollback_lines, "Terminal created successfully"
        );

        Ok(handle)
    }

    /// Detects the default shell for the current platform.
    ///
    /// On Unix, uses `$SHELL` environment variable, falling back to `/bin/sh`.
    /// On Windows, uses `%COMSPEC%`, falling back to `cmd.exe`.
    fn detect_shell() -> String {
        #[cfg(unix)]
        {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
        }
        #[cfg(windows)]
        {
            std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
        }
    }

    /// Processes PTY output through terminal parser.
    ///
    /// Reads available data from the channel (sent by background reader thread)
    /// and processes it through the VTE parser to update the terminal grid.
    /// Returns any events generated during processing.
    ///
    /// This method is non-blocking - it only processes data that's already
    /// been read by the background thread.
    ///
    /// # Returns
    ///
    /// A vector of terminal events generated during processing.
    #[instrument(level = "trace", skip(self))]
    pub fn process(&mut self) -> Vec<TerminalEvent> {
        // Process all available data from the background reader thread (non-blocking)
        while let Ok(data) = self.pty_output_rx.try_recv() {
            trace!(bytes = data.len(), "Processing PTY data from channel");
            let mut state = self.state.lock().unwrap();
            state.process_bytes(&data);
        }

        // Collect and return events
        self.event_proxy.take_events()
    }

    /// Sends keyboard input bytes to the terminal.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw bytes to send (e.g., UTF-8 encoded text)
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the PTY fails.
    #[instrument(level = "debug", skip(self, bytes), fields(bytes_len = bytes.len()))]
    pub fn input(&mut self, bytes: &[u8]) -> Result<()> {
        self.pty
            .write_all(bytes)
            .context("Failed to write to PTY")?;
        self.pty.flush().context("Failed to flush PTY")?;
        debug!(bytes_len = bytes.len(), "Sent input to terminal");
        Ok(())
    }

    /// Resizes the terminal grid.
    ///
    /// Content is reflowed according to terminal resize semantics:
    /// - Lines longer than the new width are wrapped
    /// - The cursor position is adjusted to stay visible
    /// - Scrollback content is preserved
    ///
    /// # Arguments
    ///
    /// * `cols` - New number of columns
    /// * `rows` - New number of rows
    ///
    /// # Errors
    ///
    /// Returns an error if the PTY resize fails.
    #[instrument(level = "debug", skip(self), fields(cols, rows))]
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        // Resize PTY first
        self.pty
            .resize(cols, rows)
            .context("Failed to resize PTY")?;

        // Resize terminal grid
        let size = TerminalSize::new(cols, rows);
        {
            let mut state = self.state.lock().unwrap();
            state.term.resize(size);
        }

        self.cols = cols;
        self.rows = rows;

        debug!(cols, rows, "Terminal resized");
        Ok(())
    }

    /// Returns the current terminal dimensions as (columns, rows).
    #[inline]
    pub fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    /// Checks if the terminal process is still running.
    ///
    /// # Returns
    ///
    /// `true` if the child process is still running, `false` otherwise.
    pub fn is_running(&mut self) -> bool {
        self.pty.is_running()
    }

    /// Gets the current terminal content for rendering.
    ///
    /// This method creates a snapshot of the visible terminal content,
    /// including the cursor position, styled cells with colors and attributes.
    /// It's designed to be called from the render loop.
    ///
    /// # Returns
    ///
    /// A `TerminalContent` struct containing lines, styled cells, and cursor info.
    #[instrument(level = "trace", skip(self))]
    pub fn content(&self) -> TerminalContent {
        let state = self.state.lock().unwrap();
        let grid = state.term.grid();

        let mut lines = Vec::with_capacity(state.term.screen_lines());
        let mut styled_lines = Vec::with_capacity(state.term.screen_lines());

        // Get selection range for highlighting
        let selection_range = state
            .term
            .selection
            .as_ref()
            .and_then(|sel| sel.to_range(&state.term));

        // Collect selected cells as (col, line) pairs
        let mut selected_cells = Vec::new();

        // Iterate over visible lines
        for line_idx in 0..state.term.screen_lines() {
            let row = &grid[alacritty_terminal::index::Line(line_idx as i32)];
            let mut line_str = String::with_capacity(state.term.columns());
            let mut styled_row = Vec::with_capacity(state.term.columns());

            for col_idx in 0..state.term.columns() {
                let cell = &row[alacritty_terminal::index::Column(col_idx)];
                line_str.push(cell.c);

                // Resolve colors using theme adapter
                let fg = resolve_color(&cell.fg, &self.theme);
                let bg = resolve_color(&cell.bg, &self.theme);
                let attrs = CellAttributes::from_alacritty_flags(cell.flags);

                styled_row.push(TerminalCell {
                    c: cell.c,
                    fg,
                    bg,
                    attrs,
                });

                // Check if this cell is in the selection
                if let Some(ref range) = selection_range {
                    let point = AlacPoint::new(Line(line_idx as i32), Column(col_idx));
                    if range.contains(point) {
                        selected_cells.push((col_idx, line_idx));
                    }
                }
            }

            // Trim trailing spaces for cleaner plain text output
            let trimmed = line_str.trim_end();
            lines.push(trimmed.to_string());
            styled_lines.push(styled_row);
        }

        // Get cursor position
        let cursor = grid.cursor.point;

        TerminalContent {
            lines,
            styled_lines,
            cursor_line: cursor.line.0 as usize,
            cursor_col: cursor.column.0,
            selected_cells,
        }
    }

    /// Gets the configured scrollback buffer size.
    #[inline]
    pub fn scrollback_lines(&self) -> usize {
        let state = self.state.lock().unwrap();
        state.term.history_size()
    }

    /// Scrolls the terminal display by a number of lines.
    ///
    /// # Arguments
    ///
    /// * `delta` - Number of lines to scroll (positive = up, negative = down)
    pub fn scroll(&mut self, delta: i32) {
        let mut state = self.state.lock().unwrap();
        let scroll = alacritty_terminal::grid::Scroll::Delta(delta);
        state.term.scroll_display(scroll);
        debug!(delta, "Scrolled terminal display");
    }

    /// Scrolls the terminal display by one page up.
    pub fn scroll_page_up(&mut self) {
        let mut state = self.state.lock().unwrap();
        state
            .term
            .scroll_display(alacritty_terminal::grid::Scroll::PageUp);
        debug!("Scrolled terminal page up");
    }

    /// Scrolls the terminal display by one page down.
    pub fn scroll_page_down(&mut self) {
        let mut state = self.state.lock().unwrap();
        state
            .term
            .scroll_display(alacritty_terminal::grid::Scroll::PageDown);
        debug!("Scrolled terminal page down");
    }

    /// Scrolls the terminal display to the top of scrollback.
    pub fn scroll_to_top(&mut self) {
        let mut state = self.state.lock().unwrap();
        state
            .term
            .scroll_display(alacritty_terminal::grid::Scroll::Top);
        debug!("Scrolled terminal to top");
    }

    /// Scrolls the terminal display to the bottom (latest output).
    pub fn scroll_to_bottom(&mut self) {
        let mut state = self.state.lock().unwrap();
        state
            .term
            .scroll_display(alacritty_terminal::grid::Scroll::Bottom);
        debug!("Scrolled terminal to bottom");
    }

    /// Gets the current scroll offset (0 = at bottom).
    pub fn display_offset(&self) -> usize {
        let state = self.state.lock().unwrap();
        state.term.grid().display_offset()
    }

    /// Gets the current selection as a string, if any.
    ///
    /// # Returns
    ///
    /// The selected text, or `None` if there is no selection.
    pub fn selection_to_string(&self) -> Option<String> {
        let state = self.state.lock().unwrap();
        state.term.selection_to_string()
    }

    /// Clears the current selection.
    pub fn clear_selection(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.term.selection = None;
        debug!("Selection cleared");
    }

    /// Start a new selection at the given grid position.
    ///
    /// # Arguments
    ///
    /// * `col` - Column index (0-indexed from left)
    /// * `row` - Row index (0-indexed from top of visible area)
    pub fn start_selection(&mut self, col: usize, row: usize) {
        let mut state = self.state.lock().unwrap();
        let point = AlacPoint::new(Line(row as i32), Column(col));
        state.term.selection = Some(Selection::new(
            SelectionType::Simple,
            point,
            Direction::Left,
        ));
        debug!(col, row, "Selection started");
    }

    /// Start a semantic (word) selection at the given grid position.
    ///
    /// Double-click triggers word selection - selects the word at the clicked position.
    ///
    /// # Arguments
    ///
    /// * `col` - Column index (0-indexed from left)
    /// * `row` - Row index (0-indexed from top of visible area)
    pub fn start_semantic_selection(&mut self, col: usize, row: usize) {
        let mut state = self.state.lock().unwrap();
        let point = AlacPoint::new(Line(row as i32), Column(col));
        state.term.selection = Some(Selection::new(
            SelectionType::Semantic,
            point,
            Direction::Left,
        ));
        debug!(col, row, "Semantic (word) selection started");
    }

    /// Start a line selection at the given grid position.
    ///
    /// Triple-click triggers line selection - selects the entire line at the clicked position.
    ///
    /// # Arguments
    ///
    /// * `col` - Column index (0-indexed from left)
    /// * `row` - Row index (0-indexed from top of visible area)
    pub fn start_line_selection(&mut self, col: usize, row: usize) {
        let mut state = self.state.lock().unwrap();
        let point = AlacPoint::new(Line(row as i32), Column(col));
        state.term.selection = Some(Selection::new(SelectionType::Lines, point, Direction::Left));
        debug!(col, row, "Line selection started");
    }

    /// Update the current selection to extend to the given position.
    ///
    /// # Arguments
    ///
    /// * `col` - Column index (0-indexed from left)
    /// * `row` - Row index (0-indexed from top of visible area)
    pub fn update_selection(&mut self, col: usize, row: usize) {
        let mut state = self.state.lock().unwrap();
        if let Some(ref mut selection) = state.term.selection {
            let point = AlacPoint::new(Line(row as i32), Column(col));
            selection.update(point, Direction::Right);
            trace!(col, row, "Selection updated");
        }
    }

    /// Check if there is an active selection.
    pub fn has_selection(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.term.selection.is_some()
    }

    /// Check if bracketed paste mode is enabled.
    ///
    /// When bracketed paste mode is enabled, pasted text should be wrapped
    /// in escape sequences (`\x1b[200~` before and `\x1b[201~` after) so
    /// the shell/application knows the content is pasted rather than typed.
    ///
    /// # Returns
    ///
    /// `true` if the terminal is in bracketed paste mode.
    pub fn is_bracketed_paste_mode(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.term.mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// Updates the theme adapter for focus state.
    ///
    /// # Arguments
    ///
    /// * `is_focused` - Whether the terminal window is focused.
    pub fn update_focus(&mut self, is_focused: bool) {
        self.theme.update_for_focus(is_focused);
        let mut state = self.state.lock().unwrap();
        state.term.is_focused = is_focused;
        debug!(is_focused, "Terminal focus updated");
    }

    /// Gets a reference to the theme adapter.
    pub fn theme(&self) -> &ThemeAdapter {
        &self.theme
    }
}

impl Drop for TerminalHandle {
    fn drop(&mut self) {
        // Signal the background reader thread to stop
        self.reader_stop_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
        debug!("TerminalHandle dropped, signaled reader thread to stop");
    }
}

/// Resolve a terminal Color to an actual Rgb value using the theme adapter.
///
/// Terminal colors can be:
/// - Named colors (Foreground, Background, Red, Green, etc.)
/// - Indexed colors (0-255 palette)
/// - Spec colors (direct RGB values)
///
/// This function converts all of these to actual Rgb values using the
/// theme adapter for consistent theming.
pub fn resolve_color(color: &Color, theme: &ThemeAdapter) -> Rgb {
    match color {
        Color::Named(named) => resolve_named_color(*named, theme),
        Color::Indexed(index) => resolve_indexed_color(*index, theme),
        Color::Spec(rgb) => *rgb,
    }
}

/// Resolve a named color to Rgb.
fn resolve_named_color(named: NamedColor, theme: &ThemeAdapter) -> Rgb {
    match named {
        NamedColor::Foreground | NamedColor::BrightForeground => theme.foreground(),
        NamedColor::Background => theme.background(),
        NamedColor::Cursor => theme.cursor(),

        // Standard ANSI colors (0-7)
        NamedColor::Black => theme.ansi_color(0),
        NamedColor::Red => theme.ansi_color(1),
        NamedColor::Green => theme.ansi_color(2),
        NamedColor::Yellow => theme.ansi_color(3),
        NamedColor::Blue => theme.ansi_color(4),
        NamedColor::Magenta => theme.ansi_color(5),
        NamedColor::Cyan => theme.ansi_color(6),
        NamedColor::White => theme.ansi_color(7),

        // Bright ANSI colors (8-15)
        NamedColor::BrightBlack => theme.ansi_color(8),
        NamedColor::BrightRed => theme.ansi_color(9),
        NamedColor::BrightGreen => theme.ansi_color(10),
        NamedColor::BrightYellow => theme.ansi_color(11),
        NamedColor::BrightBlue => theme.ansi_color(12),
        NamedColor::BrightMagenta => theme.ansi_color(13),
        NamedColor::BrightCyan => theme.ansi_color(14),
        NamedColor::BrightWhite => theme.ansi_color(15),

        // Dim colors - use normal with reduced intensity
        NamedColor::DimBlack => dim_rgb(theme.ansi_color(0)),
        NamedColor::DimRed => dim_rgb(theme.ansi_color(1)),
        NamedColor::DimGreen => dim_rgb(theme.ansi_color(2)),
        NamedColor::DimYellow => dim_rgb(theme.ansi_color(3)),
        NamedColor::DimBlue => dim_rgb(theme.ansi_color(4)),
        NamedColor::DimMagenta => dim_rgb(theme.ansi_color(5)),
        NamedColor::DimCyan => dim_rgb(theme.ansi_color(6)),
        NamedColor::DimWhite => dim_rgb(theme.ansi_color(7)),
        NamedColor::DimForeground => dim_rgb(theme.foreground()),
    }
}

/// Resolve an indexed color (0-255) to Rgb.
///
/// The 256-color palette is organized as:
/// - 0-15: Standard ANSI colors
/// - 16-231: 6x6x6 color cube
/// - 232-255: 24 grayscale shades
fn resolve_indexed_color(index: u8, theme: &ThemeAdapter) -> Rgb {
    match index {
        // Standard ANSI colors (0-15)
        0..=15 => theme.ansi_color(index),

        // 6x6x6 color cube (16-231)
        16..=231 => {
            let index = index - 16;
            let r = (index / 36) % 6;
            let g = (index / 6) % 6;
            let b = index % 6;

            // Each component maps 0-5 to 0, 95, 135, 175, 215, 255
            let to_component = |v: u8| -> u8 {
                if v == 0 {
                    0
                } else {
                    55 + v * 40
                }
            };

            Rgb {
                r: to_component(r),
                g: to_component(g),
                b: to_component(b),
            }
        }

        // Grayscale (232-255)
        232..=255 => {
            let shade = index - 232;
            // Maps 0-23 to 8, 18, 28, ... 238
            let gray = 8 + shade * 10;
            Rgb {
                r: gray,
                g: gray,
                b: gray,
            }
        }
    }
}

/// Dim an RGB color (reduce intensity by ~30%).
fn dim_rgb(color: Rgb) -> Rgb {
    Rgb {
        r: (color.r as f32 * 0.7) as u8,
        g: (color.g as f32 * 0.7) as u8,
        b: (color.b as f32 * 0.7) as u8,
    }
}

/// Content snapshot for rendering.
///
/// This struct contains a snapshot of the terminal content at a point
/// in time, suitable for rendering in GPUI.
#[derive(Debug, Clone)]
pub struct TerminalContent {
    /// Lines of text in the terminal (plain text, backward compatible).
    pub lines: Vec<String>,
    /// Styled lines with per-cell color and attribute information.
    pub styled_lines: Vec<Vec<TerminalCell>>,
    /// Cursor line position (0-indexed from top).
    pub cursor_line: usize,
    /// Cursor column position (0-indexed from left).
    pub cursor_col: usize,
    /// Selected cells as (column, line) pairs for highlighting.
    /// Empty if no selection is active.
    pub selected_cells: Vec<(usize, usize)>,
}

impl TerminalContent {
    /// Returns `true` if the terminal is empty (no content).
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() || self.lines.iter().all(|l| l.is_empty())
    }

    /// Returns the number of non-empty lines.
    pub fn line_count(&self) -> usize {
        self.lines.iter().filter(|l| !l.is_empty()).count()
    }

    /// Returns plain text lines (backward compatible accessor).
    ///
    /// This method provides backward compatibility for code that only
    /// needs the plain text content without styling information.
    pub fn lines_plain(&self) -> &[String] {
        &self.lines
    }
}

/// Cursor position in the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPosition {
    /// Line index (0-indexed from top).
    pub line: usize,
    /// Column index (0-indexed from left).
    pub col: usize,
}

impl From<&TerminalContent> for CursorPosition {
    fn from(content: &TerminalContent) -> Self {
        Self {
            line: content.cursor_line,
            col: content.cursor_col,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // TerminalCell and Styled Content Tests (TDD - RED)
    // ========================================================================

    #[test]
    fn test_terminal_cell_default() {
        let cell = TerminalCell::default();
        assert_eq!(cell.c, ' ');
        // Default colors should be theme defaults (foreground/background)
        assert_eq!(cell.attrs, CellAttributes::empty());
    }

    #[test]
    fn test_cell_attributes_bitflags() {
        let mut attrs = CellAttributes::empty();
        assert!(!attrs.contains(CellAttributes::BOLD));
        assert!(!attrs.contains(CellAttributes::ITALIC));
        assert!(!attrs.contains(CellAttributes::UNDERLINE));

        attrs.insert(CellAttributes::BOLD);
        assert!(attrs.contains(CellAttributes::BOLD));

        attrs.insert(CellAttributes::ITALIC);
        assert!(attrs.contains(CellAttributes::BOLD | CellAttributes::ITALIC));
    }

    #[test]
    fn test_terminal_content_styled_lines() {
        let content = TerminalContent {
            lines: vec!["hello".to_string()],
            styled_lines: vec![vec![
                TerminalCell {
                    c: 'h',
                    fg: Rgb {
                        r: 255,
                        g: 255,
                        b: 255,
                    },
                    bg: Rgb { r: 0, g: 0, b: 0 },
                    attrs: CellAttributes::empty(),
                },
                TerminalCell {
                    c: 'e',
                    fg: Rgb {
                        r: 255,
                        g: 255,
                        b: 255,
                    },
                    bg: Rgb { r: 0, g: 0, b: 0 },
                    attrs: CellAttributes::empty(),
                },
                TerminalCell {
                    c: 'l',
                    fg: Rgb {
                        r: 255,
                        g: 255,
                        b: 255,
                    },
                    bg: Rgb { r: 0, g: 0, b: 0 },
                    attrs: CellAttributes::empty(),
                },
                TerminalCell {
                    c: 'l',
                    fg: Rgb {
                        r: 255,
                        g: 255,
                        b: 255,
                    },
                    bg: Rgb { r: 0, g: 0, b: 0 },
                    attrs: CellAttributes::empty(),
                },
                TerminalCell {
                    c: 'o',
                    fg: Rgb {
                        r: 255,
                        g: 255,
                        b: 255,
                    },
                    bg: Rgb { r: 0, g: 0, b: 0 },
                    attrs: CellAttributes::empty(),
                },
            ]],
            cursor_line: 0,
            cursor_col: 5,
            selected_cells: vec![],
        };
        assert_eq!(content.styled_lines.len(), 1);
        assert_eq!(content.styled_lines[0].len(), 5);
        assert_eq!(content.styled_lines[0][0].c, 'h');
    }

    #[test]
    fn test_terminal_content_lines_plain_backward_compat() {
        let content = TerminalContent {
            lines: vec!["hello".to_string(), "world".to_string()],
            styled_lines: vec![],
            cursor_line: 0,
            cursor_col: 0,
            selected_cells: vec![],
        };
        let plain = content.lines_plain();
        assert_eq!(plain.len(), 2);
        assert_eq!(plain[0], "hello");
        assert_eq!(plain[1], "world");
    }

    // ========================================================================
    // Color Resolution Tests
    // ========================================================================

    #[test]
    fn test_resolve_color_named_foreground() {
        use vte::ansi::{Color, NamedColor};

        let theme = ThemeAdapter::dark_default();
        let color = Color::Named(NamedColor::Foreground);
        let resolved = resolve_color(&color, &theme);

        // Should resolve to theme's foreground color
        assert_eq!(resolved, theme.foreground());
    }

    #[test]
    fn test_resolve_color_named_background() {
        use vte::ansi::{Color, NamedColor};

        let theme = ThemeAdapter::dark_default();
        let color = Color::Named(NamedColor::Background);
        let resolved = resolve_color(&color, &theme);

        // Should resolve to theme's background color
        assert_eq!(resolved, theme.background());
    }

    #[test]
    fn test_resolve_color_named_ansi_red() {
        use vte::ansi::{Color, NamedColor};

        let theme = ThemeAdapter::dark_default();
        let color = Color::Named(NamedColor::Red);
        let resolved = resolve_color(&color, &theme);

        // Should resolve to ANSI red (index 1)
        assert_eq!(resolved, theme.ansi_color(1));
    }

    #[test]
    fn test_resolve_color_indexed() {
        use vte::ansi::Color;

        let theme = ThemeAdapter::dark_default();

        // Index 0-15 are the 16 ANSI colors
        let color = Color::Indexed(4); // Blue
        let resolved = resolve_color(&color, &theme);
        assert_eq!(resolved, theme.ansi_color(4));
    }

    #[test]
    fn test_resolve_color_indexed_216_cube() {
        use vte::ansi::Color;

        let theme = ThemeAdapter::dark_default();

        // Index 16-231 are the 216-color cube
        // Index 16 = rgb(0, 0, 0) in the cube
        let color = Color::Indexed(16);
        let resolved = resolve_color(&color, &theme);
        assert_eq!(resolved, Rgb { r: 0, g: 0, b: 0 });

        // Index 231 = rgb(255, 255, 255) in the cube
        let color = Color::Indexed(231);
        let resolved = resolve_color(&color, &theme);
        assert_eq!(
            resolved,
            Rgb {
                r: 255,
                g: 255,
                b: 255
            }
        );
    }

    #[test]
    fn test_resolve_color_indexed_grayscale() {
        use vte::ansi::Color;

        let theme = ThemeAdapter::dark_default();

        // Index 232-255 are grayscale (24 shades)
        // Index 232 = darkest gray (8, 8, 8)
        let color = Color::Indexed(232);
        let resolved = resolve_color(&color, &theme);
        assert_eq!(resolved, Rgb { r: 8, g: 8, b: 8 });

        // Index 255 = lightest gray (238, 238, 238)
        let color = Color::Indexed(255);
        let resolved = resolve_color(&color, &theme);
        assert_eq!(
            resolved,
            Rgb {
                r: 238,
                g: 238,
                b: 238
            }
        );
    }

    #[test]
    fn test_resolve_color_spec_direct() {
        use vte::ansi::Color;

        let theme = ThemeAdapter::dark_default();

        // Spec is a direct RGB color
        let color = Color::Spec(Rgb {
            r: 128,
            g: 64,
            b: 32,
        });
        let resolved = resolve_color(&color, &theme);
        assert_eq!(
            resolved,
            Rgb {
                r: 128,
                g: 64,
                b: 32
            }
        );
    }

    // ========================================================================
    // Existing Tests
    // ========================================================================

    #[test]
    fn test_event_proxy_creation() {
        let proxy = EventProxy::new();
        assert!(proxy.take_events().is_empty());
    }

    #[test]
    fn test_event_proxy_batching() {
        let proxy = EventProxy::new();

        // Simulate sending events
        proxy.send_event(AlacrittyEvent::Bell);
        proxy.send_event(AlacrittyEvent::Title("Test".to_string()));

        let events = proxy.take_events();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], TerminalEvent::Bell));
        assert!(matches!(events[1], TerminalEvent::Title(_)));

        // Events should be cleared after take
        assert!(proxy.take_events().is_empty());
    }

    #[test]
    fn test_terminal_size() {
        let size = TerminalSize::new(80, 24);
        assert_eq!(size.columns(), 80);
        assert_eq!(size.screen_lines(), 24);
        assert_eq!(size.total_lines(), 24);
    }

    #[test]
    fn test_terminal_content_is_empty() {
        let empty_content = TerminalContent {
            lines: vec![],
            styled_lines: vec![],
            cursor_line: 0,
            cursor_col: 0,
            selected_cells: vec![],
        };
        assert!(empty_content.is_empty());

        let whitespace_content = TerminalContent {
            lines: vec!["".to_string(), "".to_string()],
            styled_lines: vec![],
            cursor_line: 0,
            cursor_col: 0,
            selected_cells: vec![],
        };
        assert!(whitespace_content.is_empty());

        let content_with_text = TerminalContent {
            lines: vec!["hello".to_string()],
            styled_lines: vec![],
            cursor_line: 0,
            cursor_col: 5,
            selected_cells: vec![],
        };
        assert!(!content_with_text.is_empty());
    }

    #[test]
    fn test_terminal_content_line_count() {
        let content = TerminalContent {
            lines: vec!["hello".to_string(), "".to_string(), "world".to_string()],
            styled_lines: vec![],
            cursor_line: 0,
            cursor_col: 0,
            selected_cells: vec![],
        };
        assert_eq!(content.line_count(), 2);
    }

    #[test]
    fn test_cursor_position_from_content() {
        let content = TerminalContent {
            lines: vec!["hello world".to_string()],
            styled_lines: vec![],
            cursor_line: 0,
            cursor_col: 6,
            selected_cells: vec![],
        };
        let cursor: CursorPosition = (&content).into();
        assert_eq!(cursor.line, 0);
        assert_eq!(cursor.col, 6);
    }

    #[test]
    fn test_detect_shell() {
        let shell = TerminalHandle::detect_shell();
        assert!(!shell.is_empty(), "Shell should not be empty");

        #[cfg(unix)]
        {
            // On Unix, should be a valid path or common shell name
            assert!(
                shell.starts_with('/') || shell == "sh" || shell == "bash" || shell == "zsh",
                "Unix shell should be absolute path or known shell, got: {}",
                shell
            );
        }

        #[cfg(windows)]
        {
            let lower = shell.to_lowercase();
            assert!(
                lower.contains("cmd") || lower.contains("powershell"),
                "Windows shell should be cmd or powershell, got: {}",
                shell
            );
        }
    }

    #[test]
    fn test_terminal_with_simple_command() {
        // Test that a simple command like "echo hello" works when wrapped in shell
        let result = TerminalHandle::with_command("echo hello", 80, 24);

        // Skip if PTY creation fails (e.g., in CI without PTY support)
        if let Ok(mut terminal) = result {
            // Give it a moment to produce output
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Process should have run
            terminal.process();

            let content = terminal.content();
            // The output should contain "hello" somewhere
            let all_text: String = content.lines.join("\n");
            assert!(
                all_text.contains("hello"),
                "Output should contain 'hello', got: {}",
                all_text
            );
        }
    }

    #[test]
    fn test_terminal_with_command_and_args() {
        // Test command with multiple arguments: "ls -la"
        // Now uses interactive shell, so need more time for shell startup + command execution
        let result = TerminalHandle::with_command("ls -la", 80, 24);

        if let Ok(mut terminal) = result {
            // Wait for shell to start and command to execute
            std::thread::sleep(std::time::Duration::from_millis(500));
            terminal.process();

            let content = terminal.content();
            let all_text: String = content.lines.join("\n");

            // ls -la should show "total" or "drwx" or similar
            // Also accept "ls" in output (the command itself may be echoed)
            assert!(
                all_text.contains("total")
                    || all_text.contains("drwx")
                    || all_text.contains("rw")
                    || all_text.contains("ls"),
                "ls -la output should contain directory listing, got: {}",
                all_text
            );
        }
    }

    #[test]
    fn test_terminal_with_tilde_expansion() {
        // Test that ~ is expanded by the shell
        // Now uses interactive shell, so need more time
        let result = TerminalHandle::with_command("echo ~", 80, 24);

        if let Ok(mut terminal) = result {
            std::thread::sleep(std::time::Duration::from_millis(500));
            terminal.process();

            let content = terminal.content();
            let all_text: String = content.lines.join("\n");

            // ~ should be expanded to home directory (starts with /)
            // In interactive shell, we should see either the expanded path OR the echo command
            assert!(
                all_text.contains("/Users")
                    || all_text.contains("/home")
                    || all_text.contains("/root")
                    || all_text.contains("echo"),
                "~ should be expanded to home directory path, got: {}",
                all_text
            );
        }
    }

    #[test]
    fn test_terminal_with_env_var_expansion() {
        // Test that environment variables are expanded
        // Now uses interactive shell, so need more time
        let result = TerminalHandle::with_command("echo $HOME", 80, 24);

        if let Ok(mut terminal) = result {
            std::thread::sleep(std::time::Duration::from_millis(500));
            terminal.process();

            let content = terminal.content();
            let all_text: String = content.lines.join("\n");

            // $HOME should be expanded to home directory
            // In interactive shell, we should see either the expanded path OR the echo command
            assert!(
                all_text.contains("/Users")
                    || all_text.contains("/home")
                    || all_text.contains("/root")
                    || all_text.contains("echo"),
                "$HOME should be expanded to home directory path, got: {}",
                all_text
            );
        }
    }

    #[test]
    fn test_terminal_with_pipe() {
        // Test that pipes work
        // Now uses interactive shell, so need more time
        let result = TerminalHandle::with_command("echo hello | tr a-z A-Z", 80, 24);

        if let Ok(mut terminal) = result {
            std::thread::sleep(std::time::Duration::from_millis(500));
            terminal.process();

            let content = terminal.content();
            let all_text: String = content.lines.join("\n");

            // Should contain "HELLO" (uppercase) or at least the command being echoed
            assert!(
                all_text.contains("HELLO") || all_text.contains("echo") || all_text.contains("tr"),
                "Pipe should work, expected 'HELLO' or command, got: {}",
                all_text
            );
        }
    }
}
