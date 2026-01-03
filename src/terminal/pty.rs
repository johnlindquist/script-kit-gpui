//! PTY (Pseudo-Terminal) management for Script Kit GPUI.
//!
//! This module provides cross-platform PTY creation and lifecycle management
//! using the `portable-pty` crate. It handles spawning shell processes and
//! managing their I/O streams.
//!
//! # Platform Support
//!
//! - **macOS**: Uses native PTY via `/dev/ptmx`
//! - **Linux**: Uses native PTY via `/dev/ptmx` or `/dev/pts`
//! - **Windows**: Uses ConPTY (Windows 10 1809+)
//!

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{self, Read, Write};
use tracing::{debug, error, info, instrument, warn};

/// Manages a pseudo-terminal session.
///
/// `PtyManager` wraps the portable-pty crate to provide a simplified API
/// for spawning and communicating with shell processes. It handles:
///
/// - PTY pair creation (master/slave)
/// - Shell process spawning with proper environment
/// - Bidirectional I/O with the child process
/// - Terminal size (rows/cols) management
/// - Graceful shutdown and cleanup
///
/// # Thread Safety
///
/// The reader and writer can be used from different threads if cloned,
/// but the `PtyManager` itself should be owned by a single thread.
/// Size changes are synchronized via the master PTY.
pub struct PtyManager {
    /// The master side of the PTY pair
    master: Box<dyn MasterPty + Send>,
    /// The child process running in the PTY
    child: Box<dyn Child + Send + Sync>,
    /// Reader for PTY output (Option to allow taking ownership)
    reader: Option<Box<dyn Read + Send>>,
    /// Writer for PTY input
    writer: Box<dyn Write + Send>,
    /// Current terminal dimensions
    size: PtySize,
}

impl std::fmt::Debug for PtyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtyManager")
            .field("size", &self.size)
            .field("master", &"<MasterPty>")
            .field("child", &"<Child>")
            .finish()
    }
}

impl PtyManager {
    /// Creates a new PTY manager with the default shell.
    ///
    /// On Unix, uses the `$SHELL` environment variable if set,
    /// otherwise falls back to `/bin/sh`. On Windows, uses `cmd.exe`.
    ///
    /// The default size is 80 columns by 24 rows, matching a standard
    /// terminal window.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - PTY creation fails (e.g., resource exhaustion)
    /// - Shell spawning fails
    ///
    #[instrument(level = "info", name = "pty_spawn_default")]
    pub fn new() -> Result<Self> {
        let shell = Self::detect_shell();
        info!(shell = %shell, "Detected default shell");
        Self::with_command(&shell, &[])
    }

    /// Creates a new PTY manager with specified dimensions.
    ///
    /// # Arguments
    ///
    /// * `cols` - Number of columns (character width)
    /// * `rows` - Number of rows (character height)
    ///
    /// # Errors
    ///
    /// Returns an error if PTY creation or shell spawning fails.
    #[instrument(level = "info", name = "pty_spawn_sized", fields(cols, rows))]
    pub fn with_size(cols: u16, rows: u16) -> Result<Self> {
        let shell = Self::detect_shell();
        info!(shell = %shell, cols, rows, "Spawning shell with custom size");
        Self::spawn_internal(&shell, &[], cols, rows)
    }

    /// Creates a new PTY manager running a specific command.
    ///
    /// # Arguments
    ///
    /// * `cmd` - The command to execute
    /// * `args` - Arguments to pass to the command
    ///
    /// # Errors
    ///
    /// Returns an error if PTY creation or command spawning fails.
    ///
    #[instrument(level = "info", name = "pty_spawn_command", fields(cmd = %cmd))]
    pub fn with_command(cmd: &str, args: &[&str]) -> Result<Self> {
        Self::spawn_internal(cmd, args, 80, 24)
    }

    /// Creates a new PTY manager running a specific command with custom dimensions.
    ///
    /// # Arguments
    ///
    /// * `cmd` - The command to execute
    /// * `args` - Arguments to pass to the command
    /// * `cols` - Number of columns
    /// * `rows` - Number of rows
    ///
    /// # Errors
    ///
    /// Returns an error if PTY creation or command spawning fails.
    #[instrument(level = "info", name = "pty_spawn_full", fields(cmd = %cmd, cols, rows))]
    pub fn with_command_and_size(cmd: &str, args: &[&str], cols: u16, rows: u16) -> Result<Self> {
        Self::spawn_internal(cmd, args, cols, rows)
    }

    /// Internal spawn implementation.
    fn spawn_internal(cmd: &str, args: &[&str], cols: u16, rows: u16) -> Result<Self> {
        let pty_system = native_pty_system();

        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        debug!(
            cols = size.cols,
            rows = size.rows,
            "Creating PTY with dimensions"
        );

        // Open a PTY pair
        let pair = pty_system
            .openpty(size)
            .context("Failed to open PTY pair")?;

        // Build the command
        let mut command = CommandBuilder::new(cmd);
        for arg in args {
            command.arg(*arg);
        }

        // Set up environment for interactive shell
        #[cfg(unix)]
        {
            command.env("TERM", "xterm-256color");
            command.env("COLORTERM", "truecolor"); // Enable 24-bit color support
            command.env("CLICOLOR_FORCE", "1"); // Force colors even if not a TTY
            if let Ok(home) = std::env::var("HOME") {
                command.env("HOME", home);
            }
            if let Ok(user) = std::env::var("USER") {
                command.env("USER", user);
            }
            if let Ok(path) = std::env::var("PATH") {
                command.env("PATH", path);
            }
            // Inherit other important env vars for shell tools
            if let Ok(shell) = std::env::var("SHELL") {
                command.env("SHELL", shell);
            }
        }

        info!(cmd = %cmd, args = ?args, "Spawning child process");

        // Spawn the child in the PTY
        let child = pair
            .slave
            .spawn_command(command)
            .context("Failed to spawn child process in PTY")?;

        // Get reader and writer from master
        let reader = pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;
        let writer = pair
            .master
            .take_writer()
            .context("Failed to take PTY writer")?;

        info!("PTY spawned successfully");

        Ok(Self {
            master: pair.master,
            child,
            reader: Some(reader),
            writer,
            size,
        })
    }

    /// Takes ownership of the PTY reader for use in a background thread.
    ///
    /// After calling this, `read()` will return an error.
    /// This is used to move the reader to a background thread for non-blocking I/O.
    pub fn take_reader(&mut self) -> Option<Box<dyn Read + Send>> {
        self.reader.take()
    }

    /// Detects the default shell for the current platform.
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

    /// Resizes the PTY to new dimensions.
    ///
    /// This sends a SIGWINCH signal to the child process on Unix platforms,
    /// allowing applications like vim or less to redraw correctly.
    ///
    /// # Arguments
    ///
    /// * `cols` - New number of columns
    /// * `rows` - New number of rows
    ///
    /// # Errors
    ///
    /// Returns an error if the resize operation fails.
    ///
    #[instrument(level = "debug", name = "pty_resize", skip(self), fields(cols, rows))]
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        let new_size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        debug!(
            old_cols = self.size.cols,
            old_rows = self.size.rows,
            new_cols = cols,
            new_rows = rows,
            "Resizing PTY"
        );

        self.master
            .resize(new_size)
            .context("Failed to resize PTY")?;

        self.size = new_size;
        info!(cols, rows, "PTY resized successfully");

        Ok(())
    }

    /// Returns the current PTY dimensions as (columns, rows).
    #[inline]
    pub fn size(&self) -> (u16, u16) {
        (self.size.cols, self.size.rows)
    }

    /// Reads output from the PTY.
    ///
    /// This reads data that the child process has written to stdout/stderr.
    /// The read may block if no data is available.
    ///
    /// # Arguments
    ///
    /// * `buf` - Buffer to read data into
    ///
    /// # Returns
    ///
    /// The number of bytes read, or 0 if the PTY is closed.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the read fails.
    #[inline]
    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.reader {
            Some(reader) => {
                let result = reader.read(buf);
                if let Ok(n) = &result {
                    if *n > 0 {
                        debug!(bytes = n, "Read from PTY");
                    }
                }
                result
            }
            None => Err(io::Error::other(
                "PTY reader has been taken for background thread",
            )),
        }
    }

    /// Writes input to the PTY.
    ///
    /// This writes data to the child process's stdin.
    ///
    /// # Arguments
    ///
    /// * `data` - Data to write
    ///
    /// # Returns
    ///
    /// The number of bytes written.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the write fails.
    #[inline]
    pub fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let result = self.writer.write(data);
        if let Ok(n) = &result {
            debug!(bytes = n, "Wrote to PTY");
        }
        result
    }

    /// Writes all data to the PTY.
    ///
    /// This is a convenience method that ensures all data is written.
    ///
    /// # Arguments
    ///
    /// * `data` - Data to write
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the write fails.
    pub fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        self.writer.write_all(data)?;
        debug!(bytes = data.len(), "Wrote all data to PTY");
        Ok(())
    }

    /// Flushes the PTY writer.
    ///
    /// Ensures all buffered data is sent to the child process.
    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    /// Checks if the child process is still running.
    ///
    /// # Returns
    ///
    /// `true` if the child is still running, `false` otherwise.
    pub fn is_running(&mut self) -> bool {
        // try_wait returns Ok(Some(status)) if exited, Ok(None) if still running
        match self.child.try_wait() {
            Ok(Some(status)) => {
                debug!(exit_status = ?status, "Child process has exited");
                false
            }
            Ok(None) => true,
            Err(e) => {
                warn!(error = %e, "Failed to check child process status");
                false
            }
        }
    }

    /// Waits for the child process to exit and returns the exit status.
    ///
    /// This blocks until the child process terminates.
    ///
    /// # Returns
    ///
    /// The exit status of the child process, or an error if waiting fails.
    #[instrument(level = "info", name = "pty_wait", skip(self))]
    pub fn wait(&mut self) -> Result<portable_pty::ExitStatus> {
        info!("Waiting for child process to exit");
        let status = self.child.wait().context("Failed to wait for child")?;
        info!(exit_status = ?status, "Child process exited");
        Ok(status)
    }

    /// Kills the child process.
    ///
    /// Sends a termination signal to the child process. On Unix, this
    /// sends SIGKILL. On Windows, this terminates the process.
    ///
    /// # Errors
    ///
    /// Returns an error if the kill operation fails.
    #[instrument(level = "info", name = "pty_kill", skip(self))]
    pub fn kill(&mut self) -> Result<()> {
        info!("Killing child process");
        self.child.kill().context("Failed to kill child process")?;
        info!("Child process killed");
        Ok(())
    }

    /// Gets a reference to the master PTY.
    ///
    /// This can be used for advanced operations not covered by the
    /// PtyManager API.
    pub fn master(&self) -> &dyn MasterPty {
        self.master.as_ref()
    }
}

impl Drop for PtyManager {
    fn drop(&mut self) {
        debug!("PtyManager dropping, cleaning up resources");

        // Try to kill the child if it's still running
        if self.is_running() {
            if let Err(e) = self.kill() {
                error!(error = %e, "Failed to kill child process during cleanup");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell() {
        let shell = PtyManager::detect_shell();
        assert!(!shell.is_empty(), "Shell should not be empty");

        #[cfg(unix)]
        {
            // On Unix, should be a valid path
            assert!(
                shell.starts_with('/') || shell == "sh",
                "Unix shell should be absolute path or 'sh'"
            );
        }

        #[cfg(windows)]
        {
            // On Windows, should contain cmd or powershell
            let lower = shell.to_lowercase();
            assert!(
                lower.contains("cmd") || lower.contains("powershell"),
                "Windows shell should be cmd or powershell"
            );
        }
    }

    #[test]
    fn test_pty_size_default() {
        // Create with echo command that exits immediately
        let pty = PtyManager::with_command("echo", &["test"]);

        // Skip if PTY creation fails (e.g., in CI without PTY support)
        if let Ok(pty) = pty {
            assert_eq!(pty.size(), (80, 24), "Default size should be 80x24");
        }
    }

    #[test]
    fn test_pty_size_custom() {
        let pty = PtyManager::with_command_and_size("echo", &["test"], 120, 40);

        if let Ok(pty) = pty {
            assert_eq!(pty.size(), (120, 40), "Custom size should be 120x40");
        }
    }

    #[test]
    fn test_pty_spawn_and_exit() {
        // Use a command that exits immediately
        let pty = PtyManager::with_command("echo", &["hello"]);

        if let Ok(mut pty) = pty {
            // Read output
            let mut buf = [0u8; 1024];
            let mut output = Vec::new();

            // Read until EOF or timeout
            loop {
                match pty.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => output.extend_from_slice(&buf[..n]),
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }

            // Wait for exit
            let status = pty.wait();
            assert!(status.is_ok(), "Wait should succeed");

            // Check output contains hello
            let output_str = String::from_utf8_lossy(&output);
            assert!(
                output_str.contains("hello"),
                "Output should contain 'hello', got: {}",
                output_str
            );
        }
    }

    #[test]
    fn test_pty_write() {
        // Use cat which echoes input (but need to be careful about timing)
        let pty = PtyManager::with_command("cat", &[]);

        if let Ok(mut pty) = pty {
            // Write some data
            let write_result = pty.write(b"test input\n");
            assert!(write_result.is_ok(), "Write should succeed");

            // Flush to ensure data is sent
            let flush_result = pty.flush();
            assert!(flush_result.is_ok(), "Flush should succeed");

            // Kill the cat process
            let _ = pty.kill();
        }
    }

    #[test]
    fn test_pty_resize() {
        let pty = PtyManager::with_command("sleep", &["0.1"]);

        if let Ok(mut pty) = pty {
            // Resize
            let resize_result = pty.resize(100, 50);
            assert!(resize_result.is_ok(), "Resize should succeed");
            assert_eq!(pty.size(), (100, 50), "Size should be updated");

            // Kill the process
            let _ = pty.kill();
        }
    }

    #[test]
    fn test_pty_is_running() {
        let pty = PtyManager::with_command("sleep", &["10"]);

        if let Ok(mut pty) = pty {
            // Should be running initially
            assert!(pty.is_running(), "Process should be running");

            // Kill it
            let _ = pty.kill();

            // Give it a moment to exit
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Should no longer be running
            assert!(
                !pty.is_running(),
                "Process should not be running after kill"
            );
        }
    }

    #[test]
    fn test_pty_manager_debug() {
        let pty = PtyManager::with_command("echo", &["test"]);

        if let Ok(pty) = pty {
            // Debug should not panic and should contain size info
            let debug_str = format!("{:?}", pty);
            assert!(debug_str.contains("PtyManager"));
            assert!(debug_str.contains("size"));
        }
    }
}
