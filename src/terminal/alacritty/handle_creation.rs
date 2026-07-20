use anyhow::{Context, Result};
use tracing::{info, instrument, trace, warn};

use crate::theme::Theme;

use super::*;

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

    /// Creates a new terminal handle with the default shell and Script Kit theme.
    #[instrument(
        level = "info",
        name = "terminal_new_with_theme",
        fields(cols, rows),
        skip(theme)
    )]
    pub fn new_with_theme(cols: u16, rows: u16, theme: &Theme) -> Result<Self> {
        Self::create_internal(None, cols, rows, DEFAULT_SCROLLBACK_LINES, Some(theme))
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
        Self::create_internal(Some(cmd), cols, rows, DEFAULT_SCROLLBACK_LINES, None)
    }

    /// Creates a new terminal handle running a command with Script Kit theme colors.
    #[instrument(
        level = "info",
        name = "terminal_with_command_and_theme",
        fields(cmd = %cmd, cols, rows),
        skip(theme)
    )]
    pub fn with_command_and_theme(cmd: &str, cols: u16, rows: u16, theme: &Theme) -> Result<Self> {
        Self::create_internal(Some(cmd), cols, rows, DEFAULT_SCROLLBACK_LINES, Some(theme))
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
        Self::create_internal(None, cols, rows, scrollback_lines, None)
    }

    /// Internal creation method.
    fn create_internal(
        cmd: Option<&str>,
        cols: u16,
        rows: u16,
        scrollback_lines: usize,
        theme: Option<&Theme>,
    ) -> Result<Self> {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::mpsc;

        // Always spawn an interactive shell - never use -c which exits after command.
        // If a command is provided, we'll write it to the PTY after creation.
        let mut pty = PtyManager::with_size(cols, rows).context("Failed to create PTY")?;

        let config = TermConfig {
            scrolling_history: scrollback_lines,
            // Enable Kitty keyboard protocol so TUI apps like Claude Code can
            // negotiate enhanced key encoding. Without this, alacritty_terminal
            // silently ignores CSI > u push/query sequences and apps fall into
            // a broken input state where Enter acts as newline instead of submit.
            kitty_keyboard: true,
            ..TermConfig::default()
        };

        let event_proxy = EventProxy::new();
        let size = TerminalSize::new(cols, rows);
        let state = TerminalState::new(config, &size, event_proxy.clone());
        let state = Arc::new(Mutex::new(state));
        let theme = theme
            .map(ThemeAdapter::from_theme)
            .unwrap_or_else(ThemeAdapter::dark_default);

        let (pty_output_tx, pty_output_rx) = mpsc::channel();

        let reader_stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = reader_stop_flag.clone();

        let reader_thread = pty.take_reader().map(|mut reader| {
            std::thread::spawn(move || {
                let mut buffer = vec![0u8; PTY_READ_BUFFER_SIZE];
                loop {
                    if stop_flag_clone.load(Ordering::Relaxed) {
                        trace!("PTY reader thread stopping");
                        break;
                    }

                    match reader.read(&mut buffer) {
                        Ok(0) => {
                            trace!("PTY EOF in reader thread");
                            break;
                        }
                        Ok(n) => {
                            if pty_output_tx.send(buffer[..n].to_vec()).is_err() {
                                trace!("PTY output channel closed");
                                break;
                            }
                        }
                        Err(e) => {
                            if e.kind() != std::io::ErrorKind::Interrupted {
                                warn!(error = %e, "Error reading from PTY in background thread");
                                break;
                            }
                        }
                    }
                }
                trace!("PTY reader thread exiting");
            })
        });

        let mut handle = Self {
            state,
            event_proxy,
            pty,
            exit_event_emitted: false,
            theme,
            cols,
            rows,
            pty_output_rx,
            reader_stop_flag,
            reader_thread,
        };

        if let Some(cmd) = cmd {
            info!(
                cmd = %cmd,
                "Sending initial command to interactive shell"
            );
            let cmd_with_cr = format!("{}\r", cmd);
            if let Err(e) = handle.input(cmd_with_cr.as_bytes()) {
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
    pub(crate) fn detect_shell() -> String {
        #[cfg(unix)]
        {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
        }
        #[cfg(windows)]
        {
            std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
        }
    }
}
