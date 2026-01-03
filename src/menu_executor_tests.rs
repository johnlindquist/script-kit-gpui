//! Tests for menu_executor module
//!
//! Following TDD: tests written first before implementation

use super::*;

// ============================================================================
// Unit Tests - Pure Logic (no system calls)
// ============================================================================

#[test]
fn test_menu_executor_error_display() {
    // Test that our error types have useful display messages
    let err = MenuExecutorError::MenuItemDisabled {
        path: vec!["File".to_string(), "Save".to_string()],
    };
    let msg = err.to_string();
    assert!(msg.contains("File"));
    assert!(msg.contains("Save"));
    assert!(msg.contains("disabled"));

    let err = MenuExecutorError::MenuItemNotFound {
        path: vec!["Edit".to_string(), "Copy".to_string()],
        searched_in: "Nonexistent Menu".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("Edit"));
    assert!(msg.contains("Copy"));
    assert!(msg.contains("not found"));

    let err = MenuExecutorError::AppNotFrontmost {
        bundle_id: "com.apple.Safari".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("com.apple.Safari"));
    assert!(msg.contains("frontmost"));

    let err = MenuExecutorError::MenuStructureChanged {
        expected_path: vec!["File".to_string()],
        reason: "Menu bar not accessible".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("structure changed"));
}

#[test]
fn test_validate_menu_path_empty() {
    // Empty path should fail validation
    let result = validate_menu_path(&[]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("empty"));
}

#[test]
fn test_validate_menu_path_valid() {
    // Valid path should pass
    let path = vec!["File".to_string(), "New Window".to_string()];
    let result = validate_menu_path(&path);
    assert!(result.is_ok());
}

#[test]
fn test_validate_menu_path_single_item() {
    // Single item is valid (top-level menu click)
    let path = vec!["File".to_string()];
    let result = validate_menu_path(&path);
    assert!(result.is_ok());
}

// ============================================================================
// Integration Tests - Require Accessibility Permission
// ============================================================================

#[cfg(target_os = "macos")]
mod integration_tests {
    use super::*;

    #[test]
    #[ignore = "Requires accessibility permissions and specific app state"]
    fn test_execute_menu_action_valid_path() {
        // This test requires:
        // 1. Accessibility permission
        // 2. A known app (like Finder) to be frontmost
        // 3. The app to have a predictable menu structure

        // Assuming Finder is frontmost, "File" -> "New Finder Window" should work
        // Note: This is an interactive test that modifies system state
        let result = execute_menu_action(
            "com.apple.finder",
            &["File".to_string(), "New Finder Window".to_string()],
        );

        // We don't assert success because it depends on system state,
        // but we verify it returns a meaningful result
        match result {
            Ok(()) => println!("Menu action executed successfully"),
            Err(e) => {
                // Should be one of our typed errors, not a generic error
                let msg = e.to_string();
                assert!(
                    msg.contains("not frontmost")
                        || msg.contains("not found")
                        || msg.contains("disabled")
                        || msg.contains("structure changed")
                        || msg.contains("permission"),
                    "Error should be a typed MenuExecutorError: {}",
                    msg
                );
            }
        }
    }

    #[test]
    #[ignore = "Requires accessibility permissions"]
    fn test_execute_disabled_menu_item_fails() {
        // Most apps have disabled menu items (e.g., "Undo" when nothing to undo)
        // This test verifies we get a MenuItemDisabled error

        // Note: The specific disabled item depends on app state
        // Using a commonly disabled item like "Redo" in most apps
        let result = execute_menu_action(
            "com.apple.finder",
            &["Edit".to_string(), "Redo".to_string()],
        );

        match result {
            Err(e) => {
                let msg = e.to_string();
                // Could be disabled OR not found depending on app state
                assert!(
                    msg.contains("disabled")
                        || msg.contains("not found")
                        || msg.contains("not frontmost"),
                    "Expected disabled/not found error, got: {}",
                    msg
                );
            }
            Ok(()) => {
                // If it succeeded, the menu item wasn't disabled
                println!("Note: Menu item was not disabled in current state");
            }
        }
    }

    #[test]
    #[ignore = "Requires accessibility permissions"]
    fn test_ax_element_path_navigation() {
        // Test that we can navigate the AX hierarchy correctly
        // This verifies the path navigation logic works

        // First, get the menu bar to verify navigation works
        let result = crate::menu_bar::get_frontmost_menu_bar();

        match result {
            Ok(items) => {
                // Verify we got menu items
                assert!(!items.is_empty(), "Should have menu bar items");

                // Verify first item (usually Apple menu or app menu)
                let first = &items[0];
                println!("First menu item: {}", first.title);

                // Verify ax_element_path is populated
                assert!(
                    !first.ax_element_path.is_empty(),
                    "ax_element_path should be populated"
                );
            }
            Err(e) => {
                // May fail without permissions
                let msg = e.to_string();
                assert!(
                    msg.contains("permission") || msg.contains("Accessibility"),
                    "Expected permission error, got: {}",
                    msg
                );
            }
        }
    }

    #[test]
    #[ignore = "Requires accessibility permissions"]
    fn test_execute_with_wrong_bundle_id() {
        // Using a bundle ID that's not frontmost should fail
        let result = execute_menu_action(
            "com.nonexistent.app",
            &["File".to_string(), "New".to_string()],
        );

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not frontmost") || msg.contains("not found"),
            "Expected not frontmost error, got: {}",
            msg
        );
    }

    #[test]
    #[ignore = "Requires accessibility permissions"]
    fn test_navigate_to_ax_element_invalid_path() {
        // Test with a path that doesn't exist
        let result = execute_menu_action(
            "com.apple.finder",
            &["NonexistentMenu".to_string(), "NonexistentItem".to_string()],
        );

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not found") || msg.contains("not frontmost"),
            "Expected not found error, got: {}",
            msg
        );
    }
}
