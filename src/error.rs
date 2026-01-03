use thiserror::Error;
use tracing::{error, warn};

/// Error severity for UI display
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Info,     // Blue - informational
    Warning,  // Yellow - recoverable
    Error,    // Red - operation failed
    Critical, // Red + modal - requires user action
}

/// Domain-specific errors for Script Kit
#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum ScriptKitError {
    #[error("Script execution failed: {message}")]
    ScriptExecution {
        message: String,
        script_path: Option<String>,
    },

    #[error("Failed to parse protocol message: {0}")]
    ProtocolParse(#[from] serde_json::Error),

    #[error("Theme loading failed for '{path}': {source}")]
    ThemeLoad {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Process spawn failed: {0}")]
    ProcessSpawn(String),

    #[error("File watch error: {0}")]
    FileWatch(String),

    #[error("Window operation failed: {0}")]
    Window(String),
}

#[allow(dead_code)]
impl ScriptKitError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::ScriptExecution { .. } => ErrorSeverity::Error,
            Self::ProtocolParse(_) => ErrorSeverity::Warning,
            Self::ThemeLoad { .. } => ErrorSeverity::Warning,
            Self::Config(_) => ErrorSeverity::Warning,
            Self::ProcessSpawn(_) => ErrorSeverity::Error,
            Self::FileWatch(_) => ErrorSeverity::Warning,
            Self::Window(_) => ErrorSeverity::Error,
        }
    }

    pub fn user_message(&self) -> String {
        match self {
            Self::ScriptExecution { message, .. } => message.clone(),
            Self::ProtocolParse(e) => format!("Invalid message format: {}", e),
            Self::ThemeLoad { path, .. } => format!("Could not load theme from {}", path),
            Self::Config(msg) => format!("Configuration issue: {}", msg),
            Self::ProcessSpawn(msg) => format!("Could not start process: {}", msg),
            Self::FileWatch(msg) => format!("File watcher issue: {}", msg),
            Self::Window(msg) => msg.clone(),
        }
    }
}

#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, ScriptKitError>;

/// Extension trait for ergonomic error logging
#[allow(dead_code)]
pub trait NotifyResultExt<T> {
    fn log_err(self) -> Option<T>;
    fn warn_on_err(self) -> Option<T>;
}

impl<T, E: std::fmt::Debug> NotifyResultExt<T> for std::result::Result<T, E> {
    fn log_err(self) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(e) => {
                error!(error = ?e, "Operation failed");
                None
            }
        }
    }

    fn warn_on_err(self) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(e) => {
                warn!(error = ?e, "Operation warning");
                None
            }
        }
    }
}

/// Extension trait for silent error logging with caller location tracking.
/// Use when the operation is recoverable and user doesn't need to know.
///
/// This is an enhanced version that includes file/line information using
/// `#[track_caller]` for better debugging. Follows the Zed error handling pattern.
///
#[allow(dead_code)]
pub trait ResultExt<T> {
    /// Log error with caller location and return None. Use for recoverable failures.
    fn log_err(self) -> Option<T>;
    /// Log as warning with caller location and return None. Use for expected failures.
    fn warn_on_err(self) -> Option<T>;
}

impl<T, E: std::fmt::Debug> ResultExt<T> for std::result::Result<T, E> {
    #[track_caller]
    fn log_err(self) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(error) => {
                let caller = std::panic::Location::caller();
                error!(
                    error = ?error,
                    file = caller.file(),
                    line = caller.line(),
                    "Operation failed"
                );
                None
            }
        }
    }

    #[track_caller]
    fn warn_on_err(self) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(error) => {
                let caller = std::panic::Location::caller();
                warn!(
                    error = ?error,
                    file = caller.file(),
                    line = caller.line(),
                    "Operation had warning"
                );
                None
            }
        }
    }
}

/// Log an error from an async operation. Use for fire-and-forget patterns.
///
/// This is a simpler alternative to a full TryFutureExt trait that works
/// well with GPUI's async model. Use this for background tasks where you
/// want to log failures without propagating them.
///
#[allow(dead_code)]
pub fn log_async_err<T, E: std::fmt::Debug>(
    result: std::result::Result<T, E>,
    operation: &str,
) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(err) => {
            error!(
                error = ?err,
                operation = operation,
                "Async operation failed"
            );
            None
        }
    }
}

/// Panic in debug mode, log error in release mode.
///
/// Use for "impossible" states that should crash during development
/// but gracefully degrade in production. This follows the Zed pattern
/// for handling invariant violations.
///
#[macro_export]
macro_rules! debug_panic {
    ( $($fmt_arg:tt)* ) => {
        if cfg!(debug_assertions) {
            panic!( $($fmt_arg)* );
        } else {
            tracing::error!("IMPOSSIBLE STATE: {}", format_args!($($fmt_arg)*));
        }
    };
}
