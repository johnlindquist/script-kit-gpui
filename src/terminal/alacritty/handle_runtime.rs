use alacritty_terminal::term::TermMode;
use anyhow::{Context, Result};
use tracing::{debug, instrument, trace};

use crate::theme::Theme;

use super::*;

impl TerminalHandle {
    /// Processes PTY output through terminal parser.
    ///
    /// Reads available data from the channel (sent by background reader thread)
    /// and processes it through the VTE parser to update the terminal grid.
    /// Returns a tuple of (had_output, events) where had_output is true if any
    /// data was processed (grid may have changed), and events are terminal events
    /// like Bell, Title changes, or Exit.
    ///
    /// This method is non-blocking - it only processes data that's already
    /// been read by the background thread.
    ///
    /// # Returns
    ///
    /// A tuple of (had_output: bool, events: Vec<TerminalEvent>)
    #[instrument(level = "trace", skip(self))]
    pub fn process(&mut self) -> (bool, Vec<TerminalEvent>) {
        let mut had_output = false;
        let mut processed_bytes = 0usize;
        let mut pending_chunks = Vec::new();

        while processed_bytes < MAX_PROCESS_BYTES_PER_TICK {
            match self.pty_output_rx.try_recv() {
                Ok(data) => {
                    processed_bytes += data.len();
                    pending_chunks.push(data);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }

        if !pending_chunks.is_empty() {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            for data in pending_chunks {
                trace!(bytes = data.len(), "Processing PTY data from channel");
                state.process_bytes(&data);
                had_output = true;
            }
        }

        if processed_bytes >= MAX_PROCESS_BYTES_PER_TICK {
            trace!(
                processed_bytes,
                max_bytes_per_tick = MAX_PROCESS_BYTES_PER_TICK,
                "Reached PTY processing budget for this tick"
            );
        }

        let mut events = self.event_proxy.take_events();

        if events.iter().any(TerminalEvent::is_exit) {
            self.exit_event_emitted = true;
        } else if !self.exit_event_emitted {
            match self.pty.try_wait() {
                Ok(Some(status)) => {
                    let exit_code = status.exit_code() as i32;
                    debug!(exit_code, "Emitting terminal child exit status");
                    crate::terminal::telemetry::log_terminal_child_exit(
                        "alacritty::process",
                        exit_code,
                    );
                    events.push(TerminalEvent::Exit(exit_code));
                    self.exit_event_emitted = true;
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(%error, "Failed to poll terminal child exit status");
                }
            }
        }

        // Process PtyWrite events: write terminal emulator responses back to the PTY.
        // This handles protocol responses like device attributes and Kitty keyboard
        // mode queries that alacritty_terminal generates in response to escape sequences
        // from the child process.
        events.retain(|event| {
            if let TerminalEvent::PtyWrite(ref text) = event {
                let escaped: String = text
                    .chars()
                    .map(|c| {
                        if c == '\x1b' {
                            "ESC".to_string()
                        } else if c.is_control() {
                            format!("\\x{:02x}", c as u32)
                        } else {
                            c.to_string()
                        }
                    })
                    .collect();
                tracing::info!(
                    target: "script_kit::terminal",
                    response = %escaped,
                    bytes = text.len(),
                    "Writing terminal emulator response back to PTY"
                );
                if let Err(e) = self.pty.write_all(text.as_bytes()) {
                    tracing::warn!(error = %e, "Failed to write terminal response to PTY");
                }
                let _ = self.pty.flush();
                false // Remove PtyWrite from the returned events
            } else {
                true
            }
        });

        (had_output, events)
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
        if bytes.is_empty() {
            return Ok(());
        }

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
        let cols = cols.max(1);
        let rows = rows.max(1);

        if self.cols == cols && self.rows == rows {
            trace!(cols, rows, "Skipping resize; dimensions unchanged");
            return Ok(());
        }

        self.pty
            .resize(cols, rows)
            .context("Failed to resize PTY")?;

        let size = TerminalSize::new(cols, rows);
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
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

    /// Checks if the terminal child process is still alive.
    pub fn is_alive(&mut self) -> bool {
        self.pty.is_running()
    }

    /// Kill the PTY child process and signal the reader thread to stop.
    ///
    /// After this call, `is_running()` will return `false` and the
    /// background reader thread will exit on its next iteration.
    /// Safe to call multiple times — returns `Ok(())` if already stopped.
    pub fn kill(&mut self) -> Result<()> {
        self.reader_stop_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if self.pty.is_running() {
            self.pty
                .kill()
                .context("Failed to kill PTY child process")?;
        }
        Ok(())
    }

    /// Gets the configured scrollback buffer size.
    #[inline]
    pub fn scrollback_lines(&self) -> usize {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.term.history_size()
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
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.term.mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// Check if application cursor mode (DECCKM) is enabled.
    ///
    /// When application cursor mode is enabled, arrow keys send different
    /// escape sequences:
    /// - Normal mode: `\x1b[A` (up), `\x1b[B` (down), `\x1b[C` (right), `\x1b[D` (left)
    /// - Application mode: `\x1bOA` (up), `\x1bOB` (down), `\x1bOC` (right), `\x1bOD` (left)
    ///
    /// Many terminal applications (vim, less, htop, fzf) enable this mode
    /// to properly receive arrow key input.
    ///
    /// # Returns
    ///
    /// `true` if the terminal is in application cursor mode.
    pub fn is_application_cursor_mode(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.term.mode().contains(TermMode::APP_CURSOR)
    }

    /// Check if any mouse reporting mode is enabled.
    ///
    /// TUI applications (Claude Code, vim, htop, etc.) enable mouse mode to
    /// receive mouse events including scroll wheel. When mouse mode is active,
    /// scroll events must be sent as escape sequences to the PTY instead of
    /// scrolling the terminal display buffer.
    pub fn is_mouse_mode(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.term.mode().intersects(TermMode::MOUSE_MODE)
    }

    /// Check if SGR mouse encoding is enabled.
    ///
    /// When enabled, mouse events use the SGR format (`\x1b[<...M` / `\x1b[<...m`)
    /// instead of the legacy X10/normal format.
    pub fn is_sgr_mouse(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.term.mode().contains(TermMode::SGR_MOUSE)
    }

    /// Check if the terminal is using the alternate screen buffer.
    ///
    /// TUI applications switch to the alternate screen buffer for their UI.
    /// In alt screen mode, there is no scrollback history, so scroll events
    /// should be converted to arrow key sequences when `ALTERNATE_SCROLL` is set.
    pub fn is_alt_screen(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.term.mode().contains(TermMode::ALT_SCREEN)
    }

    /// Check if alternate scroll mode is enabled (enabled by default).
    ///
    /// When on the alternate screen and not in mouse mode, scroll events
    /// should be converted to arrow key sequences (up/down) so the TUI
    /// application can handle them.
    pub fn is_alternate_scroll(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.term.mode().contains(TermMode::ALTERNATE_SCROLL)
    }

    /// Updates the theme adapter for focus state.
    ///
    /// # Arguments
    ///
    /// * `is_focused` - Whether the terminal window is focused.
    pub fn update_focus(&mut self, is_focused: bool) {
        self.theme.update_for_focus(is_focused);
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.term.is_focused = is_focused;
        debug!(is_focused, "Terminal focus updated");
    }

    /// Gets a reference to the theme adapter.
    pub fn theme(&self) -> &ThemeAdapter {
        &self.theme
    }

    /// Updates the terminal theme from a new Theme.
    ///
    /// This allows updating terminal colors when the application theme changes.
    /// Preserves the current focus state.
    pub fn update_theme(&mut self, theme: &Theme) {
        self.theme.update_from_theme(theme);
        debug!("Terminal theme updated");
    }
}
