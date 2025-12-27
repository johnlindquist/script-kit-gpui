//! Built-in Features Registry
//!
//! Provides a registry of built-in features that appear in the main search
//! alongside scripts. Features like Clipboard History and App Launcher are
//! configurable and can be enabled/disabled via config.
//!
//! ## Usage
//! ```ignore
//! use crate::builtins::get_builtin_entries;
//! use crate::config::BuiltInConfig;
//!
//! let config = BuiltInConfig::default();
//! let entries = get_builtin_entries(&config);
//! for entry in entries {
//!     println!("{}: {}", entry.name, entry.description);
//! }
//! ```

use crate::config::BuiltInConfig;
use tracing::debug;

/// Types of built-in features
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltInFeature {
    /// Clipboard history viewer/manager
    ClipboardHistory,
    /// Application launcher for opening installed apps
    AppLauncher,
    /// Individual application entry (for future use when apps appear in search)
    App(String),
}

/// A built-in feature entry that appears in the main search
#[derive(Debug, Clone)]
pub struct BuiltInEntry {
    /// Unique identifier for the entry
    pub id: String,
    /// Display name shown in search results
    pub name: String,
    /// Description shown below the name
    pub description: String,
    /// Keywords for fuzzy matching in search
    pub keywords: Vec<String>,
    /// The actual feature this entry represents
    pub feature: BuiltInFeature,
}

impl BuiltInEntry {
    /// Create a new built-in entry
    fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        keywords: Vec<&str>,
        feature: BuiltInFeature,
    ) -> Self {
        BuiltInEntry {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            keywords: keywords.into_iter().map(String::from).collect(),
            feature,
        }
    }
}

/// Get the list of enabled built-in entries based on configuration
///
/// # Arguments
/// * `config` - The built-in features configuration
///
/// # Returns
/// A vector of enabled built-in entries that should appear in the main search
pub fn get_builtin_entries(config: &BuiltInConfig) -> Vec<BuiltInEntry> {
    let mut entries = Vec::new();

    if config.clipboard_history {
        entries.push(BuiltInEntry::new(
            "builtin-clipboard-history",
            "Clipboard History",
            "View and manage your clipboard history",
            vec!["clipboard", "history", "paste", "copy"],
            BuiltInFeature::ClipboardHistory,
        ));
        debug!("Added Clipboard History built-in entry");
    }

    if config.app_launcher {
        entries.push(BuiltInEntry::new(
            "builtin-app-launcher",
            "App Launcher",
            "Search and launch installed applications",
            vec!["app", "launch", "open", "application"],
            BuiltInFeature::AppLauncher,
        ));
        debug!("Added App Launcher built-in entry");
    }

    debug!(count = entries.len(), "Built-in entries loaded");
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BuiltInConfig;

    #[test]
    fn test_builtin_config_default() {
        let config = BuiltInConfig::default();
        assert!(config.clipboard_history);
        assert!(config.app_launcher);
    }

    #[test]
    fn test_builtin_config_custom() {
        let config = BuiltInConfig {
            clipboard_history: false,
            app_launcher: true,
        };
        assert!(!config.clipboard_history);
        assert!(config.app_launcher);
    }

    #[test]
    fn test_get_builtin_entries_all_enabled() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        assert_eq!(entries.len(), 2);

        // Check clipboard history entry
        let clipboard = entries.iter().find(|e| e.id == "builtin-clipboard-history");
        assert!(clipboard.is_some());
        let clipboard = clipboard.unwrap();
        assert_eq!(clipboard.name, "Clipboard History");
        assert_eq!(clipboard.feature, BuiltInFeature::ClipboardHistory);
        assert!(clipboard.keywords.contains(&"clipboard".to_string()));
        assert!(clipboard.keywords.contains(&"history".to_string()));
        assert!(clipboard.keywords.contains(&"paste".to_string()));
        assert!(clipboard.keywords.contains(&"copy".to_string()));

        // Check app launcher entry
        let launcher = entries.iter().find(|e| e.id == "builtin-app-launcher");
        assert!(launcher.is_some());
        let launcher = launcher.unwrap();
        assert_eq!(launcher.name, "App Launcher");
        assert_eq!(launcher.feature, BuiltInFeature::AppLauncher);
        assert!(launcher.keywords.contains(&"app".to_string()));
        assert!(launcher.keywords.contains(&"launch".to_string()));
        assert!(launcher.keywords.contains(&"open".to_string()));
        assert!(launcher.keywords.contains(&"application".to_string()));
    }

    #[test]
    fn test_get_builtin_entries_clipboard_only() {
        let config = BuiltInConfig {
            clipboard_history: true,
            app_launcher: false,
        };
        let entries = get_builtin_entries(&config);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "builtin-clipboard-history");
        assert_eq!(entries[0].feature, BuiltInFeature::ClipboardHistory);
    }

    #[test]
    fn test_get_builtin_entries_app_launcher_only() {
        let config = BuiltInConfig {
            clipboard_history: false,
            app_launcher: true,
        };
        let entries = get_builtin_entries(&config);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "builtin-app-launcher");
        assert_eq!(entries[0].feature, BuiltInFeature::AppLauncher);
    }

    #[test]
    fn test_get_builtin_entries_none_enabled() {
        let config = BuiltInConfig {
            clipboard_history: false,
            app_launcher: false,
        };
        let entries = get_builtin_entries(&config);

        assert!(entries.is_empty());
    }

    #[test]
    fn test_builtin_feature_equality() {
        assert_eq!(
            BuiltInFeature::ClipboardHistory,
            BuiltInFeature::ClipboardHistory
        );
        assert_eq!(BuiltInFeature::AppLauncher, BuiltInFeature::AppLauncher);
        assert_ne!(
            BuiltInFeature::ClipboardHistory,
            BuiltInFeature::AppLauncher
        );

        // Test App variant
        assert_eq!(
            BuiltInFeature::App("Safari".to_string()),
            BuiltInFeature::App("Safari".to_string())
        );
        assert_ne!(
            BuiltInFeature::App("Safari".to_string()),
            BuiltInFeature::App("Chrome".to_string())
        );
        assert_ne!(
            BuiltInFeature::App("Safari".to_string()),
            BuiltInFeature::AppLauncher
        );
    }

    #[test]
    fn test_builtin_entry_new() {
        let entry = BuiltInEntry::new(
            "test-id",
            "Test Entry",
            "Test description",
            vec!["test", "keyword"],
            BuiltInFeature::ClipboardHistory,
        );

        assert_eq!(entry.id, "test-id");
        assert_eq!(entry.name, "Test Entry");
        assert_eq!(entry.description, "Test description");
        assert_eq!(
            entry.keywords,
            vec!["test".to_string(), "keyword".to_string()]
        );
        assert_eq!(entry.feature, BuiltInFeature::ClipboardHistory);
    }

    #[test]
    fn test_builtin_entry_clone() {
        let entry = BuiltInEntry::new(
            "test-id",
            "Test Entry",
            "Test description",
            vec!["test"],
            BuiltInFeature::AppLauncher,
        );

        let cloned = entry.clone();
        assert_eq!(entry.id, cloned.id);
        assert_eq!(entry.name, cloned.name);
        assert_eq!(entry.description, cloned.description);
        assert_eq!(entry.keywords, cloned.keywords);
        assert_eq!(entry.feature, cloned.feature);
    }

    #[test]
    fn test_builtin_config_clone() {
        let config = BuiltInConfig {
            clipboard_history: true,
            app_launcher: false,
        };

        let cloned = config.clone();
        assert_eq!(config.clipboard_history, cloned.clipboard_history);
        assert_eq!(config.app_launcher, cloned.app_launcher);
    }
}
