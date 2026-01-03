//! Permissions Wizard Module
//!
//! This module provides functionality for checking and guiding users through
//! required macOS permissions. It centralizes permission checking and provides
//! UI-ready data structures for displaying permission status.
//!
//! ## Permission Types
//!
//! Script Kit requires the following macOS permissions:
//!
//! - **Accessibility**: Required for keyboard monitoring (text expansion),
//!   window control, and getting selected text. This is the primary permission
//!   that most features depend on.
//!
//! ## Usage
//!
//! ```no_run
//! use script_kit_gpui::permissions_wizard::{check_all_permissions, PermissionType};
//!
//! let status = check_all_permissions();
//!
//! if !status.accessibility.granted {
//!     println!("Accessibility permission needed: {}", status.accessibility.description);
//!     // Show UI to guide user to System Settings
//! }
//!
//! if status.all_granted() {
//!     println!("All permissions granted!");
//! }
//! ```
//!
//! ## Architecture
//!
//! The wizard provides:
//! - `PermissionStatus`: Overall status of all required permissions
//! - `PermissionInfo`: Details about each individual permission
//! - `PermissionType`: Enum of all permission types
//! - Functions to check and request each permission type
//!
//! The structures are designed to be UI-ready, containing all information
//! needed to render a permissions wizard dialog.

use macos_accessibility_client::accessibility;
use tracing::{debug, info, instrument};

// ============================================================================
// Permission Types
// ============================================================================

/// Types of permissions that Script Kit may require
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionType {
    /// Accessibility permission for keyboard monitoring, window control, selected text
    Accessibility,
}

impl PermissionType {
    /// Get the human-readable name of this permission type
    pub fn name(&self) -> &'static str {
        match self {
            PermissionType::Accessibility => "Accessibility",
        }
    }

    /// Get the system settings path hint for this permission
    #[allow(dead_code)]
    pub fn settings_path(&self) -> &'static str {
        match self {
            PermissionType::Accessibility => "System Settings > Privacy & Security > Accessibility",
        }
    }

    /// Get the features that depend on this permission
    pub fn dependent_features(&self) -> &'static [&'static str] {
        match self {
            PermissionType::Accessibility => &[
                "Text expansion / snippets",
                "Window control (move, resize, tile)",
                "Get selected text from other apps",
                "Global keyboard shortcuts",
            ],
        }
    }
}

impl std::fmt::Display for PermissionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Permission Info
// ============================================================================

/// Information about a single permission's status
///
/// This struct contains all the information needed to display a permission
/// status in a wizard UI, including the current state, description, and
/// instructions for granting the permission.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PermissionInfo {
    /// The type of permission
    pub permission_type: PermissionType,

    /// Whether the permission is currently granted
    pub granted: bool,

    /// Human-readable description of why this permission is needed
    pub description: String,

    /// Instructions for how to grant this permission
    pub instructions: String,

    /// List of features that require this permission
    pub features: Vec<String>,
}

impl PermissionInfo {
    /// Create a new PermissionInfo for accessibility permission
    fn accessibility(granted: bool) -> Self {
        Self {
            permission_type: PermissionType::Accessibility,
            granted,
            description:
                "Accessibility permission allows Script Kit to monitor keyboard input \
                for text expansion, control windows, and get selected text from other applications."
                    .to_string(),
            instructions: "1. Open System Settings\n\
                 2. Go to Privacy & Security > Accessibility\n\
                 3. Click the + button\n\
                 4. Find and select Script Kit\n\
                 5. Enable the toggle next to Script Kit"
                .to_string(),
            features: PermissionType::Accessibility
                .dependent_features()
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

// ============================================================================
// Permission Status
// ============================================================================

/// Overall status of all required permissions
///
/// This struct provides a comprehensive view of all permissions Script Kit
/// needs, making it easy to check if the app is fully operational or if
/// some permissions need to be granted.
#[derive(Debug, Clone)]
pub struct PermissionStatus {
    /// Accessibility permission status
    pub accessibility: PermissionInfo,
}

impl PermissionStatus {
    /// Check if all required permissions are granted
    pub fn all_granted(&self) -> bool {
        self.accessibility.granted
    }

    /// Get a list of all permissions that are missing
    pub fn missing_permissions(&self) -> Vec<&PermissionInfo> {
        let mut missing = Vec::new();
        if !self.accessibility.granted {
            missing.push(&self.accessibility);
        }
        missing
    }

    /// Get the count of granted permissions
    #[allow(dead_code)]
    pub fn granted_count(&self) -> usize {
        if self.accessibility.granted {
            1
        } else {
            0
        }
    }

    /// Get the total count of required permissions
    #[allow(dead_code)]
    pub fn total_count(&self) -> usize {
        1 // Currently only accessibility
    }

    /// Get all permission infos as a vector
    #[allow(dead_code)]
    pub fn all_permissions(&self) -> Vec<&PermissionInfo> {
        vec![&self.accessibility]
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Check all required permissions and return their status
///
/// This is the main entry point for checking permissions. It queries the
/// system for each required permission and returns a comprehensive status
/// object that can be used to render a permissions wizard UI.
///
/// # Example
///
/// ```no_run
/// use script_kit_gpui::permissions_wizard::check_all_permissions;
///
/// let status = check_all_permissions();
/// println!("All granted: {}", status.all_granted());
/// println!("Missing: {:?}", status.missing_permissions().len());
/// ```
#[instrument]
pub fn check_all_permissions() -> PermissionStatus {
    let accessibility_granted = check_accessibility_permission();

    let status = PermissionStatus {
        accessibility: PermissionInfo::accessibility(accessibility_granted),
    };

    info!(
        all_granted = status.all_granted(),
        accessibility = accessibility_granted,
        "Checked all permissions"
    );

    status
}

/// Check if accessibility permission is granted
///
/// This checks whether the application has been granted accessibility
/// permission in System Settings. This permission is required for:
/// - Global keyboard monitoring (text expansion)
/// - Window control operations
/// - Getting selected text from other applications
///
/// # Returns
///
/// `true` if accessibility permission is granted, `false` otherwise.
#[instrument]
pub fn check_accessibility_permission() -> bool {
    let granted = accessibility::application_is_trusted();
    debug!(granted, "Checked accessibility permission");
    granted
}

/// Request accessibility permission from the user
///
/// This function triggers the macOS system prompt asking the user to grant
/// accessibility permission. If the permission is already granted, this
/// returns `true` immediately without showing a prompt.
///
/// # Returns
///
/// `true` if permission is granted (either already or after the prompt),
/// `false` if the user denies the permission or dismisses the dialog.
///
/// # Note
///
/// After granting permission in System Settings, the user may need to
/// restart Script Kit for the changes to take effect, depending on when
/// during the app lifecycle this was called.
#[instrument]
pub fn request_accessibility_permission() -> bool {
    info!("Requesting accessibility permission");
    let granted = accessibility::application_is_trusted_with_prompt();
    info!(granted, "Accessibility permission request completed");
    granted
}

/// Open System Settings to the accessibility privacy pane
///
/// This opens the Privacy & Security > Accessibility section of
/// System Settings where the user can grant permission to Script Kit.
///
/// # Errors
///
/// Returns an error if the system settings URL could not be opened.
pub fn open_accessibility_settings() -> std::io::Result<()> {
    info!("Opening accessibility settings");

    // Use the macOS URL scheme to open the specific settings pane
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn()?;

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_type_name() {
        assert_eq!(PermissionType::Accessibility.name(), "Accessibility");
    }

    #[test]
    fn test_permission_type_display() {
        assert_eq!(
            format!("{}", PermissionType::Accessibility),
            "Accessibility"
        );
    }

    #[test]
    fn test_permission_type_settings_path() {
        assert!(PermissionType::Accessibility
            .settings_path()
            .contains("Accessibility"));
    }

    #[test]
    fn test_permission_type_dependent_features() {
        let features = PermissionType::Accessibility.dependent_features();
        assert!(!features.is_empty());
        assert!(features.iter().any(|f| f.contains("expansion")));
    }

    #[test]
    fn test_permission_info_accessibility() {
        let info = PermissionInfo::accessibility(true);
        assert_eq!(info.permission_type, PermissionType::Accessibility);
        assert!(info.granted);
        assert!(!info.description.is_empty());
        assert!(!info.instructions.is_empty());
        assert!(!info.features.is_empty());
    }

    #[test]
    fn test_permission_status_all_granted_true() {
        let status = PermissionStatus {
            accessibility: PermissionInfo::accessibility(true),
        };
        assert!(status.all_granted());
        assert!(status.missing_permissions().is_empty());
        assert_eq!(status.granted_count(), 1);
        assert_eq!(status.total_count(), 1);
    }

    #[test]
    fn test_permission_status_all_granted_false() {
        let status = PermissionStatus {
            accessibility: PermissionInfo::accessibility(false),
        };
        assert!(!status.all_granted());
        assert_eq!(status.missing_permissions().len(), 1);
        assert_eq!(status.granted_count(), 0);
        assert_eq!(status.total_count(), 1);
    }

    #[test]
    fn test_permission_status_all_permissions() {
        let status = PermissionStatus {
            accessibility: PermissionInfo::accessibility(true),
        };
        let all = status.all_permissions();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].permission_type, PermissionType::Accessibility);
    }

    #[test]
    fn test_check_accessibility_permission_does_not_panic() {
        // This test just verifies the function doesn't panic
        // The actual result depends on system permissions
        let _ = check_accessibility_permission();
    }

    #[test]
    fn test_check_all_permissions_does_not_panic() {
        // This test just verifies the function doesn't panic
        let status = check_all_permissions();
        // Status should always be valid
        assert!(status.total_count() > 0);
    }
}
