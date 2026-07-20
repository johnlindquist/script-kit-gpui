use anyhow::{Context, Result};
use portable_pty::{ExitStatus, MasterPty, PtySize};
use std::io;
use tracing::{debug, info, instrument, warn};

use super::*;

impl PtyManager {
    /// Resizes the PTY to new dimensions.
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
        crate::terminal::telemetry::log_terminal_resized("pty::resize", cols, rows);

        Ok(())
    }

    /// Returns the current PTY dimensions as (columns, rows).
    #[inline]
    pub fn size(&self) -> (u16, u16) {
        (self.size.cols, self.size.rows)
    }

    /// Reads output from the PTY.
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
    #[inline]
    pub fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let result = self.writer.write(data);
        if let Ok(n) = &result {
            debug!(bytes = n, "Wrote to PTY");
        }
        result
    }

    /// Writes all data to the PTY.
    pub fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        self.writer.write_all(data)?;
        debug!(bytes = data.len(), "Wrote all data to PTY");
        Ok(())
    }

    /// Flushes the PTY writer.
    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    /// Polls for child completion without discarding a previously observed status.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        if let Some(status) = &self.exit_status {
            return Ok(Some(status.clone()));
        }

        let status = self
            .child
            .try_wait()
            .context("Failed to check child process status")?;
        if let Some(status) = &status {
            debug!(exit_status = ?status, "Child process has exited");
            self.exit_status = Some(status.clone());
        }
        Ok(status)
    }

    /// Checks if the child process is still running.
    pub fn is_running(&mut self) -> bool {
        match self.try_wait() {
            Ok(status) => status.is_none(),
            Err(error) => {
                warn!(%error, "Unable to observe child process status; assuming it is still running");
                true
            }
        }
    }

    /// Waits for the child process to exit and returns the exit status.
    #[instrument(level = "info", name = "pty_wait", skip(self))]
    pub fn wait(&mut self) -> Result<ExitStatus> {
        if let Some(status) = &self.exit_status {
            return Ok(status.clone());
        }

        info!("Waiting for child process to exit");
        let status = self.child.wait().context("Failed to wait for child")?;
        info!(exit_status = ?status, "Child process exited");
        crate::terminal::telemetry::log_pty_exited("pty::wait", &format!("{status:?}"));
        self.exit_status = Some(status.clone());
        Ok(status)
    }

    /// Kills the child process.
    #[instrument(level = "info", name = "pty_kill", skip(self))]
    pub fn kill(&mut self) -> Result<()> {
        info!("Killing child process");
        self.child.kill().context("Failed to kill child process")?;
        info!("Child process killed");
        crate::terminal::telemetry::log_pty_killed("pty::kill");
        Ok(())
    }

    /// Gets a reference to the master PTY.
    pub fn master(&self) -> &dyn MasterPty {
        self.master.as_ref()
    }
}
