use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tracing::{debug, info, instrument};

use super::*;

impl PtyManager {
    /// Creates a new PTY manager with the default shell.
    #[instrument(level = "info", name = "pty_spawn_default")]
    pub fn new() -> Result<Self> {
        let shell = Self::detect_shell();
        info!(shell = %shell, "Detected default shell");
        Self::with_command(&shell, &[])
    }

    /// Creates a new PTY manager with specified dimensions.
    #[instrument(level = "info", name = "pty_spawn_sized", fields(cols, rows))]
    pub fn with_size(cols: u16, rows: u16) -> Result<Self> {
        let shell = Self::detect_shell();
        info!(shell = %shell, cols, rows, "Spawning shell with custom size");
        Self::spawn_internal(&shell, &[], cols, rows)
    }

    /// Creates a new PTY manager running a specific command.
    #[instrument(level = "info", name = "pty_spawn_command", fields(cmd = %cmd))]
    pub fn with_command(cmd: &str, args: &[&str]) -> Result<Self> {
        Self::spawn_internal(cmd, args, 80, 24)
    }

    /// Creates a new PTY manager running a specific command with custom dimensions.
    #[instrument(level = "info", name = "pty_spawn_full", fields(cmd = %cmd, cols, rows))]
    pub fn with_command_and_size(cmd: &str, args: &[&str], cols: u16, rows: u16) -> Result<Self> {
        Self::spawn_internal(cmd, args, cols, rows)
    }

    /// Internal spawn implementation.
    fn spawn_internal(cmd: &str, args: &[&str], cols: u16, rows: u16) -> Result<Self> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)?;
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

        let pair = pty_system
            .openpty(size)
            .context("Failed to open PTY pair")?;

        let mut command = CommandBuilder::new(cmd);
        for arg in args {
            command.arg(*arg);
        }

        #[cfg(unix)]
        {
            let env_vars = Self::unix_spawn_env_allowlist();
            debug!(
                allowlisted_env_count = env_vars.len(),
                "Scrubbing inherited PTY environment before spawn"
            );
            command.env_clear();
            for (key, value) in env_vars {
                command.env(key, value);
            }
        }

        info!(cmd = %cmd, args = ?args, "Spawning child process");

        let child = pair
            .slave
            .spawn_command(command)
            .context("Failed to spawn child process in PTY")?;

        let reader = pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;
        let writer = pair
            .master
            .take_writer()
            .context("Failed to take PTY writer")?;

        info!("PTY spawned successfully");
        crate::terminal::telemetry::log_pty_spawned(
            "pty::spawn_internal",
            cmd,
            child.process_id(),
            cols,
            rows,
        );

        let registration = child
            .process_id()
            .map(|pid| crate::process_manager::ChildRegistration::register(pid, cmd));

        Ok(Self {
            master: pair.master,
            child,
            exit_status: None,
            reader: Some(reader),
            writer,
            size,
            _registration: registration,
        })
    }

    #[cfg(unix)]
    fn unix_spawn_env_allowlist() -> Vec<(&'static str, String)> {
        let mut env_vars = vec![
            ("TERM", "xterm-256color".to_string()),
            ("COLORTERM", "truecolor".to_string()),
            ("CLICOLOR_FORCE", "1".to_string()),
            // Hide zsh's PROMPT_SP partial-line marker glyph. Pairs with
            // ZDOTDIR below — `PROMPT_EOL_MARK=""` makes the marker invisible
            // for shells where the shim isn't installed, while ZDOTDIR lets us
            // fully `unsetopt PROMPT_SP` to also drop the trailing spaces+CR
            // (which otherwise wrap to a blank row above the real prompt).
            // bash/fish ignore PROMPT_EOL_MARK harmlessly.
            ("PROMPT_EOL_MARK", String::new()),
        ];

        // Intentionally do NOT forward TERM_PROGRAM. Our embedded terminal
        // uses a legacy key encoder (manual \r, \x1b[A, etc.), not a full
        // Kitty-protocol-aware frontend. Forwarding the host terminal name
        // (e.g. "ghostty") causes TUI apps like Claude Code to enable Kitty
        // keyboard mode, which breaks input on our non-Kitty terminal.
        for key in ["HOME", "USER", "PATH", "SHELL", "TMPDIR", "LANG"] {
            if let Ok(value) = std::env::var(key) {
                env_vars.push((key, value));
            }
        }

        // For zsh shells, point ZDOTDIR at a Script-Kit-owned shim that
        // sources the user's real ~/.zshenv / ~/.zshrc and then unsets
        // PROMPT_SP / PROMPT_CR. This is the only way to fully suppress
        // the `%` partial-line marker AND its blank-row trailing artifact
        // without modifying the user's own zsh config.
        if Self::detect_shell().ends_with("zsh") {
            if let Some(shim_dir) = Self::ensure_zsh_quick_terminal_shim() {
                env_vars.push(("ZDOTDIR", shim_dir.to_string_lossy().into_owned()));
            }
        }

        env_vars
    }

    /// Ensure `~/.scriptkit/quick-terminal-zsh/` exists with `.zshenv` and
    /// `.zshrc` files that source the user's real configs and then disable
    /// zsh's PROMPT_SP / PROMPT_CR options. Idempotent — safe to call on
    /// every spawn. Returns the shim directory path, or `None` if the home
    /// directory can't be resolved or the shim can't be written.
    #[cfg(unix)]
    fn ensure_zsh_quick_terminal_shim() -> Option<std::path::PathBuf> {
        let home = std::env::var("HOME").ok()?;
        let dir = std::path::PathBuf::from(&home)
            .join(".scriptkit")
            .join("quick-terminal-zsh");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(error = %e, dir = %dir.display(), "Failed to create zsh shim dir");
            return None;
        }

        // .zshenv is sourced for ALL zsh invocations (login, interactive,
        // script). Forward the user's ~/.zshenv so PATH and friends are set.
        let zshenv = "# Auto-generated by Script Kit Quick Terminal — do not edit.\n\
                      [ -r \"$HOME/.zshenv\" ] && . \"$HOME/.zshenv\"\n";

        // .zshrc is sourced for interactive shells. Source the user's real
        // ~/.zshrc FIRST, then unsetopt so this wins regardless of what they
        // set. PROMPT_SP causes the partial-line marker; PROMPT_CR adds the
        // leading carriage return — both contribute to the artifact above
        // the first prompt.
        let zshrc = "# Auto-generated by Script Kit Quick Terminal — do not edit.\n\
                     [ -r \"$HOME/.zshrc\" ] && . \"$HOME/.zshrc\"\n\
                     unsetopt PROMPT_SP 2>/dev/null\n\
                     unsetopt PROMPT_CR 2>/dev/null\n";

        Self::write_if_changed(&dir.join(".zshenv"), zshenv);
        Self::write_if_changed(&dir.join(".zshrc"), zshrc);

        Some(dir)
    }

    #[cfg(unix)]
    fn write_if_changed(path: &std::path::Path, contents: &str) {
        let needs_write = match std::fs::read_to_string(path) {
            Ok(existing) => existing != contents,
            Err(_) => true,
        };
        if needs_write {
            if let Err(e) = std::fs::write(path, contents) {
                tracing::warn!(error = %e, path = %path.display(), "Failed to write zsh shim file");
            }
        }
    }

    /// Takes ownership of the PTY reader for use in a background thread.
    ///
    /// After calling this, `read()` will return an error.
    pub fn take_reader(&mut self) -> Option<Box<dyn std::io::Read + Send>> {
        self.reader.take()
    }

    /// Detects the default shell for the current platform.
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

#[cfg(all(test, unix))]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::PtyManager;

    #[test]
    fn test_unix_spawn_env_allowlist_contains_expected_keys_when_environment_present() {
        let env_vars = PtyManager::unix_spawn_env_allowlist();
        let env_map: HashMap<&str, &str> = env_vars
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();

        assert_eq!(env_map.get("TERM"), Some(&"xterm-256color"));
        assert_eq!(env_map.get("COLORTERM"), Some(&"truecolor"));
        assert_eq!(env_map.get("CLICOLOR_FORCE"), Some(&"1"));
        assert_eq!(env_map.get("PROMPT_EOL_MARK"), Some(&""));

        let allowed: HashSet<&str> = [
            "TERM",
            "COLORTERM",
            "CLICOLOR_FORCE",
            "PROMPT_EOL_MARK",
            "ZDOTDIR",
            "HOME",
            "USER",
            "PATH",
            "SHELL",
            "TMPDIR",
            "LANG",
        ]
        .into_iter()
        .collect();

        for key in env_map.keys() {
            assert!(
                allowed.contains(key),
                "found unexpected env key in PTY allowlist: {key}"
            );
        }

        for key in ["HOME", "USER", "PATH", "SHELL", "TMPDIR", "LANG"] {
            match std::env::var(key) {
                Ok(expected) => assert_eq!(env_map.get(key), Some(&expected.as_str())),
                Err(_) => assert!(!env_map.contains_key(key)),
            }
        }
    }
}
