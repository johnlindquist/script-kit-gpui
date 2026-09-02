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

use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event as AlacrittyEvent, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::Flags as AlacrittyFlags;
use alacritty_terminal::term::{Config as TermConfig, Term};
use bitflags::bitflags;
use tracing::{debug, info, trace, warn};
use vte::ansi::Processor;

use crate::terminal::pty::PtyManager;
use crate::terminal::theme_adapter::ThemeAdapter;
use crate::terminal::TerminalEvent;

mod colors;
mod content_types;
mod handle_content;
mod handle_creation;
mod handle_navigation;
mod handle_runtime;
#[cfg(test)]
mod tests;

pub use colors::{resolve_color, resolve_fg_color_with_bold};
#[allow(unused_imports)]
pub use content_types::{CursorPosition, TerminalContent, TerminalTextSnapshot};

/// Default scrollback buffer size in lines.
const DEFAULT_SCROLLBACK_LINES: usize = 10_000;

/// Maximum bytes to read from PTY in a single process() call.
const PTY_READ_BUFFER_SIZE: usize = 4096;

/// Maximum PTY bytes to process per `process()` call.
///
/// This keeps UI frames responsive under heavy terminal output by
/// spreading parsing work across multiple ticks.
const MAX_PROCESS_BYTES_PER_TICK: usize = 1_048_576;

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
        let mut events = self.events.lock().unwrap_or_else(|e| e.into_inner());
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
                crate::terminal::telemetry::log_terminal_child_exit("alacritty::event_proxy", code);
                Some(TerminalEvent::Exit(code))
            }
            AlacrittyEvent::Wakeup => {
                trace!("Terminal wakeup event");
                None
            }
            AlacrittyEvent::PtyWrite(text) => {
                debug!(
                    bytes = text.len(),
                    "PTY write request from terminal emulator"
                );
                Some(TerminalEvent::PtyWrite(text))
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
            let mut events = self.events.lock().unwrap_or_else(|e| e.into_inner());
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
    pub fg: vte::ansi::Rgb,
    /// Background color as RGB.
    pub bg: vte::ansi::Rgb,
    /// Cell attributes (bold, italic, underline, etc.).
    pub attrs: CellAttributes,
}

impl Default for TerminalCell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: vte::ansi::Rgb {
                r: 212,
                g: 212,
                b: 212,
            },
            bg: vte::ansi::Rgb {
                r: 30,
                g: 30,
                b: 30,
            },
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
        self.processor.advance(&mut self.term, bytes);
    }
}

/// Handle to an Alacritty terminal emulator instance.
pub struct TerminalHandle {
    /// Thread-safe terminal state.
    state: Arc<Mutex<TerminalState>>,
    /// Event proxy for receiving terminal events (shared with Term).
    event_proxy: EventProxy,
    /// PTY manager for process I/O (writing only - reading happens in background thread).
    pty: Option<PtyManager>,
    /// Local input bytes accepted by an injected parser-only terminal.
    fixture_input: Vec<u8>,
    /// Prevents the cached child status from emitting duplicate exit events.
    exit_event_emitted: bool,
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
    /// Join handle for the background PTY reader thread.
    reader_thread: Option<std::thread::JoinHandle<()>>,
}

impl std::fmt::Debug for TerminalHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalHandle")
            .field("cols", &self.cols)
            .field("rows", &self.rows)
            .finish_non_exhaustive()
    }
}

impl Drop for TerminalHandle {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering;

        self.reader_stop_flag.store(true, Ordering::Relaxed);

        // Best-effort shutdown: terminate the child to unblock PTY reads, then join.
        if let Some(pty) = self.pty.as_mut() {
            if pty.is_running() {
                if let Err(error) = pty.kill() {
                    warn!(
                        error = %error,
                        "Failed to kill PTY child during terminal drop"
                    );
                }
            }
        }

        if let Some(reader_thread) = self.reader_thread.take() {
            if let Err(join_error) = reader_thread.join() {
                warn!(
                    join_error = ?join_error,
                    "PTY reader thread panicked during terminal drop"
                );
            }
        }

        debug!("TerminalHandle dropped, reader thread joined");
    }
}
