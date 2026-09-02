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

        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)?;
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
            pty: Some(pty),
            fixture_input: Vec::new(),
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

    /// Construct the production VT parser/grid without a PTY, process or reader thread.
    pub fn from_bytes(cols: u16, rows: u16, theme: &Theme, bytes: &[u8]) -> Result<Self> {
        anyhow::ensure!(
            cols > 0 && rows > 0 && cols <= 512 && rows <= 256,
            "invalid_terminal_size"
        );
        anyhow::ensure!(
            bytes.len() <= MAX_PROCESS_BYTES_PER_TICK,
            "terminal_fixture_too_large"
        );
        let event_proxy = EventProxy::new();
        let config = TermConfig {
            scrolling_history: DEFAULT_SCROLLBACK_LINES,
            kitty_keyboard: true,
            ..TermConfig::default()
        };
        let mut state =
            TerminalState::new(config, &TerminalSize::new(cols, rows), event_proxy.clone());
        state.process_bytes(bytes);
        let (_tx, pty_output_rx) = std::sync::mpsc::channel();
        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            event_proxy,
            pty: None,
            fixture_input: Vec::new(),
            exit_event_emitted: false,
            theme: ThemeAdapter::from_theme(theme),
            cols,
            rows,
            pty_output_rx,
            reader_stop_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            reader_thread: None,
        })
    }

    /// Feed the same parser used by PTY output. An interactive terminal cannot
    /// accept this fixture-only source, so injected bytes cannot race live I/O.
    pub fn feed_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        anyhow::ensure!(self.pty.is_none(), "terminal_source_is_interactive");
        anyhow::ensure!(
            bytes.len() <= MAX_PROCESS_BYTES_PER_TICK,
            "terminal_fixture_too_large"
        );
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .process_bytes(bytes);
        Ok(())
    }

    pub fn finish_fixture(&mut self, exit_code: i32) -> Result<()> {
        anyhow::ensure!(self.pty.is_none(), "terminal_source_is_interactive");
        anyhow::ensure!(
            !self.exit_event_emitted,
            "terminal_fixture_already_finished"
        );
        self.event_proxy
            .events
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(TerminalEvent::Exit(exit_code));
        self.exit_event_emitted = true;
        Ok(())
    }

    pub fn fixture_input(&self) -> Option<&[u8]> {
        self.pty.is_none().then_some(self.fixture_input.as_slice())
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

#[cfg(test)]
mod injected_source_tests {
    use super::*;

    #[test]
    fn injected_terminal_runs_real_parser_and_has_no_process() {
        let mut terminal =
            TerminalHandle::from_bytes(20, 5, &Theme::default(), b"\x1b[31mred\x1b[0m\r\nnext")
                .unwrap();
        assert!(!terminal.is_running());
        assert!(terminal.reader_thread.is_none());
        assert!(terminal.text_snapshot(10, 1024).text.contains("red\nnext"));
        terminal.input(b"local-input").unwrap();
        assert_eq!(terminal.fixture_input(), Some(b"local-input".as_slice()));
        terminal.feed_bytes(b"\r\nmore").unwrap();
        terminal.resize(25, 6).unwrap();
        assert!(terminal.text_snapshot(10, 1024).text.contains("more"));
        terminal.finish_fixture(7).unwrap();
        assert!(terminal.finish_fixture(7).is_err());
        let (_, events) = terminal.process();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, TerminalEvent::Exit(7)))
                .count(),
            1
        );
        assert!(terminal.process().1.iter().all(|event| !event.is_exit()));
    }

    #[test]
    fn injected_terminal_bounds_precede_allocation() {
        let theme = Theme::default();
        assert!(TerminalHandle::from_bytes(0, 5, &theme, b"").is_err());
        assert!(TerminalHandle::from_bytes(513, 5, &theme, b"").is_err());
        assert!(TerminalHandle::from_bytes(20, 257, &theme, b"").is_err());
        assert!(TerminalHandle::from_bytes(
            20,
            5,
            &theme,
            &vec![0; MAX_PROCESS_BYTES_PER_TICK + 1]
        )
        .is_err());
    }
}
