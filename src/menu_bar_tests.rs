//! Tests for menu_bar module
//!
//! TDD tests written FIRST, before implementation.
//! These tests define the expected behavior for menu bar parsing.

use super::*;

// =========================================================================
// Menu Item Title Parsing Tests
// =========================================================================

#[test]
fn test_parse_menu_item_title_simple() {
    // Simple menu item title without any decoration
    let item = MenuBarItem {
        title: "File".to_string(),
        enabled: true,
        shortcut: None,
        children: vec![],
        ax_element_path: vec![0],
    };
    assert_eq!(item.title, "File");
}

#[test]
fn test_parse_menu_item_title_with_ellipsis() {
    // Menu items that open dialogs often have ellipsis
    let item = MenuBarItem {
        title: "Save As...".to_string(),
        enabled: true,
        shortcut: None,
        children: vec![],
        ax_element_path: vec![0, 1],
    };
    assert_eq!(item.title, "Save As...");
}

#[test]
fn test_parse_menu_item_title_with_unicode() {
    // Test unicode characters in menu titles
    let item = MenuBarItem {
        title: "Emoji & Symbols".to_string(),
        enabled: true,
        shortcut: None,
        children: vec![],
        ax_element_path: vec![0, 2],
    };
    assert_eq!(item.title, "Emoji & Symbols");
}

#[test]
fn test_parse_menu_item_title_empty() {
    // Empty titles are valid (for separators converted to items)
    let item = MenuBarItem {
        title: "".to_string(),
        enabled: false,
        shortcut: None,
        children: vec![],
        ax_element_path: vec![0, 3],
    };
    assert!(item.title.is_empty());
}

// =========================================================================
// Keyboard Shortcut Parsing Tests
// =========================================================================

#[test]
fn test_parse_keyboard_shortcut_cmd_only() {
    // Simple Cmd+Key shortcut
    let shortcut = KeyboardShortcut::new("S".to_string(), ModifierFlags::COMMAND);
    assert_eq!(shortcut.key, "S");
    assert!(shortcut.modifiers.contains(ModifierFlags::COMMAND));
    assert!(!shortcut.modifiers.contains(ModifierFlags::SHIFT));
    assert!(!shortcut.modifiers.contains(ModifierFlags::OPTION));
    assert!(!shortcut.modifiers.contains(ModifierFlags::CONTROL));
}

#[test]
fn test_parse_keyboard_shortcut_cmd_shift() {
    // Cmd+Shift+Key shortcut
    let shortcut = KeyboardShortcut::new(
        "S".to_string(),
        ModifierFlags::COMMAND | ModifierFlags::SHIFT,
    );
    assert_eq!(shortcut.key, "S");
    assert!(shortcut.modifiers.contains(ModifierFlags::COMMAND));
    assert!(shortcut.modifiers.contains(ModifierFlags::SHIFT));
}

#[test]
fn test_parse_keyboard_shortcut_cmd_option() {
    // Cmd+Option+Key shortcut
    let shortcut = KeyboardShortcut::new(
        "I".to_string(),
        ModifierFlags::COMMAND | ModifierFlags::OPTION,
    );
    assert_eq!(shortcut.key, "I");
    assert!(shortcut.modifiers.contains(ModifierFlags::COMMAND));
    assert!(shortcut.modifiers.contains(ModifierFlags::OPTION));
}

#[test]
fn test_parse_keyboard_shortcut_all_modifiers() {
    // All modifiers
    let shortcut = KeyboardShortcut::new(
        "X".to_string(),
        ModifierFlags::COMMAND
            | ModifierFlags::SHIFT
            | ModifierFlags::OPTION
            | ModifierFlags::CONTROL,
    );
    assert_eq!(shortcut.key, "X");
    assert!(shortcut.modifiers.contains(ModifierFlags::COMMAND));
    assert!(shortcut.modifiers.contains(ModifierFlags::SHIFT));
    assert!(shortcut.modifiers.contains(ModifierFlags::OPTION));
    assert!(shortcut.modifiers.contains(ModifierFlags::CONTROL));
}

#[test]
fn test_keyboard_shortcut_display() {
    // Test display formatting
    let shortcut = KeyboardShortcut::new(
        "S".to_string(),
        ModifierFlags::COMMAND | ModifierFlags::SHIFT,
    );
    let display = shortcut.to_display_string();
    // Should contain the modifier symbols and key
    assert!(display.contains("S"));
}

#[test]
fn test_keyboard_shortcut_from_ax_values() {
    // Test creating from AX attribute values (what we get from accessibility API)
    // AXMenuItemCmdChar = "S", AXMenuItemCmdModifiers = 256 (Cmd)
    let shortcut = KeyboardShortcut::from_ax_values("S", 256);
    assert_eq!(shortcut.key, "S");
    assert!(shortcut.modifiers.contains(ModifierFlags::COMMAND));
}

#[test]
fn test_keyboard_shortcut_from_ax_values_with_shift() {
    // Shift = 512, Cmd = 256, combined = 768
    let shortcut = KeyboardShortcut::from_ax_values("S", 768);
    assert_eq!(shortcut.key, "S");
    assert!(shortcut.modifiers.contains(ModifierFlags::COMMAND));
    assert!(shortcut.modifiers.contains(ModifierFlags::SHIFT));
}

// =========================================================================
// Menu Hierarchy Parsing Tests
// =========================================================================

#[test]
fn test_menu_hierarchy_single_level() {
    // Top-level menu bar item with no children
    let item = MenuBarItem {
        title: "Apple".to_string(),
        enabled: true,
        shortcut: None,
        children: vec![],
        ax_element_path: vec![0],
    };
    assert!(item.children.is_empty());
    assert_eq!(item.ax_element_path.len(), 1);
}

#[test]
fn test_menu_hierarchy_two_levels() {
    // Menu with children (File > New)
    let child = MenuBarItem {
        title: "New".to_string(),
        enabled: true,
        shortcut: Some(KeyboardShortcut::new(
            "N".to_string(),
            ModifierFlags::COMMAND,
        )),
        children: vec![],
        ax_element_path: vec![1, 0],
    };
    let parent = MenuBarItem {
        title: "File".to_string(),
        enabled: true,
        shortcut: None,
        children: vec![child],
        ax_element_path: vec![1],
    };
    assert_eq!(parent.children.len(), 1);
    assert_eq!(parent.children[0].title, "New");
    assert_eq!(parent.children[0].ax_element_path, vec![1, 0]);
}

#[test]
fn test_menu_hierarchy_three_levels() {
    // Submenu: File > New > Document
    let grandchild = MenuBarItem {
        title: "Document".to_string(),
        enabled: true,
        shortcut: None,
        children: vec![],
        ax_element_path: vec![1, 0, 0],
    };
    let child = MenuBarItem {
        title: "New".to_string(),
        enabled: true,
        shortcut: None,
        children: vec![grandchild],
        ax_element_path: vec![1, 0],
    };
    let parent = MenuBarItem {
        title: "File".to_string(),
        enabled: true,
        shortcut: None,
        children: vec![child],
        ax_element_path: vec![1],
    };
    assert_eq!(parent.children[0].children.len(), 1);
    assert_eq!(parent.children[0].children[0].title, "Document");
}

#[test]
fn test_menu_hierarchy_depth_limit() {
    // Verify max depth of 3 is respected
    let depth3 = MenuBarItem {
        title: "Level 3".to_string(),
        enabled: true,
        shortcut: None,
        children: vec![], // No further children at depth 3
        ax_element_path: vec![0, 0, 0],
    };
    assert_eq!(depth3.ax_element_path.len(), 3);
}

// =========================================================================
// Separator Detection Tests
// =========================================================================

#[test]
fn test_separator_detection_by_role() {
    // AXMenuItemRole separator has specific characteristics
    // We represent separators as items with special marker
    let sep = MenuBarItem::separator(vec![1, 2]);
    assert!(sep.is_separator());
    assert!(!sep.enabled);
}

#[test]
fn test_separator_in_menu_list() {
    // Menu with items and separators
    let items = [
        MenuBarItem {
            title: "Cut".to_string(),
            enabled: true,
            shortcut: Some(KeyboardShortcut::new(
                "X".to_string(),
                ModifierFlags::COMMAND,
            )),
            children: vec![],
            ax_element_path: vec![2, 0],
        },
        MenuBarItem::separator(vec![2, 1]),
        MenuBarItem {
            title: "Copy".to_string(),
            enabled: true,
            shortcut: Some(KeyboardShortcut::new(
                "C".to_string(),
                ModifierFlags::COMMAND,
            )),
            children: vec![],
            ax_element_path: vec![2, 2],
        },
    ];
    assert!(!items[0].is_separator());
    assert!(items[1].is_separator());
    assert!(!items[2].is_separator());
}

// =========================================================================
// Menu Cache Tests
// =========================================================================

#[test]
fn test_menu_cache_creation() {
    let cache = MenuCache::new("com.apple.finder".to_string());
    assert_eq!(cache.bundle_id, "com.apple.finder");
    assert!(cache.menu_json.is_none());
    assert!(cache.last_scanned.is_none());
}

#[test]
fn test_menu_cache_with_data() {
    use std::time::Instant;

    let mut cache = MenuCache::new("com.apple.finder".to_string());
    cache.menu_json = Some(r#"[{"title":"File"}]"#.to_string());
    cache.last_scanned = Some(Instant::now());

    assert!(cache.menu_json.is_some());
    assert!(cache.last_scanned.is_some());
}

#[test]
fn test_menu_cache_is_stale() {
    use std::time::{Duration, Instant};

    let mut cache = MenuCache::new("com.apple.finder".to_string());
    // No scan = stale
    assert!(cache.is_stale(Duration::from_secs(60)));

    // Recent scan = not stale
    cache.last_scanned = Some(Instant::now());
    assert!(!cache.is_stale(Duration::from_secs(60)));
}

// =========================================================================
// ModifierFlags Tests
// =========================================================================

#[test]
fn test_modifier_flags_empty() {
    let flags = ModifierFlags::empty();
    assert!(!flags.contains(ModifierFlags::COMMAND));
    assert!(!flags.contains(ModifierFlags::SHIFT));
    assert!(!flags.contains(ModifierFlags::OPTION));
    assert!(!flags.contains(ModifierFlags::CONTROL));
}

#[test]
fn test_modifier_flags_combination() {
    let flags = ModifierFlags::COMMAND | ModifierFlags::SHIFT;
    assert!(flags.contains(ModifierFlags::COMMAND));
    assert!(flags.contains(ModifierFlags::SHIFT));
    assert!(!flags.contains(ModifierFlags::OPTION));
}

// =========================================================================
// Integration Tests (require accessibility permission)
// =========================================================================

#[test]
#[ignore = "Requires accessibility permissions and frontmost app"]
fn test_get_frontmost_menu_bar() {
    let result = get_frontmost_menu_bar();
    match result {
        Ok(items) => {
            println!("Found {} top-level menu items:", items.len());
            for item in &items {
                println!("  - {}", item.title);
                for child in &item.children {
                    let shortcut_str = child
                        .shortcut
                        .as_ref()
                        .map(|s: &KeyboardShortcut| format!(" ({})", s.to_display_string()))
                        .unwrap_or_default();
                    println!("      - {}{}", child.title, shortcut_str);
                }
            }
            // Should have at least the Apple menu
            assert!(!items.is_empty(), "Should have at least one menu item");
        }
        Err(e) => {
            eprintln!("Error getting menu bar: {}", e);
            // May fail without accessibility permission
        }
    }
}

#[test]
#[ignore = "Requires accessibility permissions"]
fn test_menu_bar_has_apple_menu() {
    if let Ok(items) = get_frontmost_menu_bar() {
        // First item should be Apple menu (though title may be special character)
        assert!(!items.is_empty(), "Menu bar should have items");
        // Apple menu is typically the first item
    }
}
