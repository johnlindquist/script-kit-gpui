use thiserror::Error;

/// Error severity for UI display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Info,     // Blue - informational
    Warning,  // Yellow - recoverable  
    Error,    // Red - operation failed
    Critical, // Red + modal - requires user action
}

/// Domain-specific errors for Script Kit
#[derive(Error, Debug)]
pub enum ScriptKitError {
    #[error("Script execution failed: {message}")]
    ScriptExecution { message: String, script_path: Option<String> },
    
    #[error("Failed to parse protocol message: {0}")]
    ProtocolParse(#[from] serde_json::Error),
    
    #[error("Theme loading failed for '{path}': {source}")]
    ThemeLoad { path: String, #[source] source: std::io::Error },
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Process spawn failed: {0}")]
    ProcessSpawn(String),
    
    #[error("File watch error: {0}")]
    FileWatch(String),
    
    #[error("Window operation failed: {0}")]
    Window(String),
}

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

pub type Result<T> = std::result::Result<T, ScriptKitError>;

/// Extension trait for ergonomic error logging
pub trait NotifyResultExt<T> {
    fn log_err(self) -> Option<T>;
    fn warn_on_err(self) -> Option<T>;
}

impl<T, E: std::fmt::Debug> NotifyResultExt<T> for std::result::Result<T, E> {
    fn log_err(self) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("[ERROR] {:?}", e);
                None
            }
        }
    }
    
    fn warn_on_err(self) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("[WARN] {:?}", e);
                None
            }
        }
    }
}
