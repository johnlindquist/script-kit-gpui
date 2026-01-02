//! SVG Icon Library
//!
//! This module provides access to SVG icons from various icon sets.
//! Icons are loaded from assets/icons/ and rendered using GPUI's svg() element.
//!
//! # Icon Sources
//! - Zed's built-in icons (Lucide-based)
//! - Custom Script Kit icons
//!
//! # Usage
//! ```ignore
//! use designs::icon_variations::{IconName, IconStyle};
//!
//! // Get the SVG path for an icon
//! let path = IconName::File.path();
//!
//! // Render with GPUI
//! svg().path(path).size(px(16.)).color(rgb(0xffffff))
//! ```

use std::sync::Arc;

/// Categories of icons based on their use case
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconCategory {
    /// File type indicators (file, folder, code)
    Files,
    /// Navigation and UI actions (plus, trash, copy)
    Actions,
    /// Indicators and status (check, star, bolt)
    Status,
    /// Directional arrows and chevrons
    Arrows,
    /// Media controls (play, stop)
    Media,
}

impl IconCategory {
    /// Get the display name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Actions => "Actions",
            Self::Status => "Status",
            Self::Arrows => "Arrows",
            Self::Media => "Media",
        }
    }

    /// Get all categories
    pub fn all() -> &'static [IconCategory] {
        &[
            Self::Files,
            Self::Actions,
            Self::Status,
            Self::Arrows,
            Self::Media,
        ]
    }

    /// Get icons belonging to this category
    pub fn icons(&self) -> Vec<IconName> {
        IconName::all()
            .iter()
            .filter(|icon| icon.category() == *self)
            .copied()
            .collect()
    }
}

/// Available SVG icons from assets/icons/
///
/// These map to actual .svg files that can be rendered with GPUI's svg() element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconName {
    // Files
    File,
    FileCode,
    Folder,
    FolderOpen,

    // Actions
    Plus,
    Trash,
    Copy,
    Settings,
    MagnifyingGlass,
    Terminal,
    Code,

    // Status
    Check,
    Star,
    StarFilled,
    BoltFilled,
    BoltOutlined,

    // Arrows
    ArrowRight,
    ArrowDown,
    ArrowUp,
    ChevronRight,
    ChevronDown,

    // UI
    Close,

    // Media
    PlayFilled,
    PlayOutlined,

    // UI/Layout
    Sidebar,
}

impl IconName {
    /// Get all available icons
    pub fn all() -> &'static [IconName] {
        &[
            // Files
            Self::File,
            Self::FileCode,
            Self::Folder,
            Self::FolderOpen,
            // Actions
            Self::Plus,
            Self::Trash,
            Self::Copy,
            Self::Settings,
            Self::MagnifyingGlass,
            Self::Terminal,
            Self::Code,
            // Status
            Self::Check,
            Self::Star,
            Self::StarFilled,
            Self::BoltFilled,
            Self::BoltOutlined,
            // Arrows
            Self::ArrowRight,
            Self::ArrowDown,
            Self::ArrowUp,
            Self::ChevronRight,
            Self::ChevronDown,
            // UI
            Self::Close,
            // Media
            Self::PlayFilled,
            Self::PlayOutlined,
            // UI/Layout
            Self::Sidebar,
        ]
    }

    /// Get total count of icons
    pub fn count() -> usize {
        Self::all().len()
    }

    /// Get the display name
    pub fn name(&self) -> &'static str {
        match self {
            Self::File => "File",
            Self::FileCode => "File Code",
            Self::Folder => "Folder",
            Self::FolderOpen => "Folder Open",
            Self::Plus => "Plus",
            Self::Trash => "Trash",
            Self::Copy => "Copy",
            Self::Settings => "Settings",
            Self::MagnifyingGlass => "Search",
            Self::Terminal => "Terminal",
            Self::Code => "Code",
            Self::Check => "Check",
            Self::Star => "Star",
            Self::StarFilled => "Star Filled",
            Self::BoltFilled => "Bolt Filled",
            Self::BoltOutlined => "Bolt Outlined",
            Self::ArrowRight => "Arrow Right",
            Self::ArrowDown => "Arrow Down",
            Self::ArrowUp => "Arrow Up",
            Self::ChevronRight => "Chevron Right",
            Self::ChevronDown => "Chevron Down",
            Self::Close => "Close",
            Self::PlayFilled => "Play Filled",
            Self::PlayOutlined => "Play Outlined",
            Self::Sidebar => "Sidebar",
        }
    }

    /// Get the description
    pub fn description(&self) -> &'static str {
        match self {
            Self::File => "Generic file indicator",
            Self::FileCode => "Source code file",
            Self::Folder => "Closed directory",
            Self::FolderOpen => "Open/expanded directory",
            Self::Plus => "Add/create action",
            Self::Trash => "Delete action",
            Self::Copy => "Copy to clipboard",
            Self::Settings => "Configuration/preferences",
            Self::MagnifyingGlass => "Search/find",
            Self::Terminal => "Terminal/command line",
            Self::Code => "Code/development",
            Self::Check => "Complete/success",
            Self::Star => "Favorite (outline)",
            Self::StarFilled => "Favorite (filled)",
            Self::BoltFilled => "Quick action (filled)",
            Self::BoltOutlined => "Quick action (outline)",
            Self::ArrowRight => "Navigate forward",
            Self::ArrowDown => "Navigate down/expand",
            Self::ArrowUp => "Navigate up/collapse",
            Self::ChevronRight => "Expand right",
            Self::ChevronDown => "Expand down",
            Self::Close => "Close/dismiss",
            Self::PlayFilled => "Run/execute (filled)",
            Self::PlayOutlined => "Run/execute (outline)",
            Self::Sidebar => "Toggle sidebar panel",
        }
    }

    /// Get the SVG file path (relative to assets/)
    #[allow(dead_code)]
    pub fn path(&self) -> Arc<str> {
        let file_name = match self {
            Self::File => "file",
            Self::FileCode => "file_code",
            Self::Folder => "folder",
            Self::FolderOpen => "folder_open",
            Self::Plus => "plus",
            Self::Trash => "trash",
            Self::Copy => "copy",
            Self::Settings => "settings",
            Self::MagnifyingGlass => "magnifying_glass",
            Self::Terminal => "terminal",
            Self::Code => "code",
            Self::Check => "check",
            Self::Star => "star",
            Self::StarFilled => "star_filled",
            Self::BoltFilled => "bolt_filled",
            Self::BoltOutlined => "bolt_outlined",
            Self::ArrowRight => "arrow_right",
            Self::ArrowDown => "arrow_down",
            Self::ArrowUp => "arrow_up",
            Self::ChevronRight => "chevron_right",
            Self::ChevronDown => "chevron_down",
            Self::Close => "close",
            Self::PlayFilled => "play_filled",
            Self::PlayOutlined => "play_outlined",
            Self::Sidebar => "sidebar",
        };
        format!("icons/{}.svg", file_name).into()
    }

    /// Get the full external path for GPUI svg().external_path()
    /// Returns a &'static str for GPUI compatibility
    pub fn external_path(&self) -> &'static str {
        match self {
            Self::File => concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/file.svg"),
            Self::FileCode => concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/file_code.svg"),
            Self::Folder => concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/folder.svg"),
            Self::FolderOpen => {
                concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/folder_open.svg")
            }
            Self::Plus => concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/plus.svg"),
            Self::Trash => concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/trash.svg"),
            Self::Copy => concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/copy.svg"),
            Self::Settings => concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/settings.svg"),
            Self::MagnifyingGlass => concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/icons/magnifying_glass.svg"
            ),
            Self::Terminal => concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/terminal.svg"),
            Self::Code => concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/code.svg"),
            Self::Check => concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/check.svg"),
            Self::Star => concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/star.svg"),
            Self::StarFilled => {
                concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/star_filled.svg")
            }
            Self::BoltFilled => {
                concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/bolt_filled.svg")
            }
            Self::BoltOutlined => concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/icons/bolt_outlined.svg"
            ),
            Self::ArrowRight => {
                concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/arrow_right.svg")
            }
            Self::ArrowDown => concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/arrow_down.svg"),
            Self::ArrowUp => concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/arrow_up.svg"),
            Self::ChevronRight => concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/icons/chevron_right.svg"
            ),
            Self::ChevronDown => {
                concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/chevron_down.svg")
            }
            Self::Close => concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/close.svg"),
            Self::PlayFilled => {
                concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/play_filled.svg")
            }
            Self::PlayOutlined => concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/icons/play_outlined.svg"
            ),
            Self::Sidebar => concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/sidebar.svg"),
        }
    }

    /// Get the category this icon belongs to
    pub fn category(&self) -> IconCategory {
        match self {
            Self::File | Self::FileCode | Self::Folder | Self::FolderOpen => IconCategory::Files,
            Self::Plus
            | Self::Trash
            | Self::Copy
            | Self::Settings
            | Self::MagnifyingGlass
            | Self::Terminal
            | Self::Code => IconCategory::Actions,
            Self::Check | Self::Star | Self::StarFilled | Self::BoltFilled | Self::BoltOutlined => {
                IconCategory::Status
            }
            Self::ArrowRight
            | Self::ArrowDown
            | Self::ArrowUp
            | Self::ChevronRight
            | Self::ChevronDown => IconCategory::Arrows,
            Self::Close => IconCategory::Actions,
            Self::PlayFilled | Self::PlayOutlined => IconCategory::Media,
            Self::Sidebar => IconCategory::Actions,
        }
    }
}

/// Visual rendering styles for icons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum IconStyle {
    /// Standard 16px size, default color
    Default,
    /// Compact 12px size
    Small,
    /// Large 24px size
    Large,
    /// Reduced opacity (muted)
    Muted,
    /// Accent/highlight color
    Accent,
    /// With circular background
    CircleBackground,
    /// With rounded square background
    SquareBackground,
}

#[allow(dead_code)]
impl IconStyle {
    /// Get all styles
    pub fn all() -> &'static [IconStyle] {
        &[
            Self::Default,
            Self::Small,
            Self::Large,
            Self::Muted,
            Self::Accent,
            Self::CircleBackground,
            Self::SquareBackground,
        ]
    }

    /// Get style count
    pub fn count() -> usize {
        Self::all().len()
    }

    /// Get the display name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Small => "Small",
            Self::Large => "Large",
            Self::Muted => "Muted",
            Self::Accent => "Accent",
            Self::CircleBackground => "Circle BG",
            Self::SquareBackground => "Square BG",
        }
    }

    /// Get the description
    pub fn description(&self) -> &'static str {
        match self {
            Self::Default => "Standard 16px icon",
            Self::Small => "Compact 12px icon",
            Self::Large => "Prominent 24px icon",
            Self::Muted => "Reduced opacity",
            Self::Accent => "Highlighted color",
            Self::CircleBackground => "Circular background",
            Self::SquareBackground => "Rounded square background",
        }
    }

    /// Get the icon size in pixels
    pub fn size(&self) -> f32 {
        match self {
            Self::Small => 12.0,
            Self::Large => 24.0,
            _ => 16.0,
        }
    }

    /// Get the opacity (0.0-1.0)
    pub fn opacity(&self) -> f32 {
        match self {
            Self::Muted => 0.5,
            _ => 1.0,
        }
    }
}

/// Get total count of icon gallery items
/// (categories + icons * styles)
#[allow(dead_code)]
pub fn total_gallery_items() -> usize {
    IconCategory::all().len() + (IconName::count() * IconStyle::count())
}

/// Configuration for rendering an icon
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct IconConfig {
    pub size: f32,
    pub color: u32,
    pub opacity: f32,
    pub background: Option<IconBackground>,
}

/// Background style for icons
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum IconBackground {
    Circle { color: u32, radius: f32 },
    RoundedSquare { color: u32, radius: f32 },
}

impl Default for IconConfig {
    fn default() -> Self {
        Self {
            size: 16.0,
            color: 0xcccccc,
            opacity: 1.0,
            background: None,
        }
    }
}

impl IconConfig {
    /// Create config from style
    #[allow(dead_code)]
    pub fn from_style(style: IconStyle, base_color: u32, accent_color: u32) -> Self {
        let size = style.size();
        let opacity = style.opacity();

        let (color, background) = match style {
            IconStyle::Accent => (accent_color, None),
            IconStyle::CircleBackground => (
                base_color,
                Some(IconBackground::Circle {
                    color: 0x333333,
                    radius: size + 8.0,
                }),
            ),
            IconStyle::SquareBackground => (
                base_color,
                Some(IconBackground::RoundedSquare {
                    color: 0x333333,
                    radius: 4.0,
                }),
            ),
            _ => (base_color, None),
        };

        Self {
            size,
            color,
            opacity,
            background,
        }
    }
}

// Legacy compatibility - keep old types for existing code
pub use IconName as ScriptIcon;
pub use IconName as ScriptletIcon;
pub use IconName as BuiltInIcon;
pub use IconName as AppIcon;
pub use IconName as WindowIcon;
pub use IconName as FolderIcon;

/// Legacy function for compatibility
pub fn total_icon_count() -> usize {
    IconName::count()
}

/// Convert a string icon name to IconName enum
///
/// Supports various formats:
/// - Exact match: "File", "FileCode", "Terminal"
/// - Lowercase: "file", "terminal", "code"  
/// - With spaces: "file code", "folder open"
/// - Kebab case: "file-code", "folder-open"
/// - Snake case: "file_code", "folder_open"
///
/// Returns None if the name doesn't match any known icon.
pub fn icon_name_from_str(name: &str) -> Option<IconName> {
    // Normalize: lowercase, replace separators with nothing
    let normalized = name.to_lowercase().replace(['-', '_', ' '], "");

    match normalized.as_str() {
        // Files
        "file" => Some(IconName::File),
        "filecode" => Some(IconName::FileCode),
        "folder" => Some(IconName::Folder),
        "folderopen" => Some(IconName::FolderOpen),

        // Actions
        "plus" | "add" => Some(IconName::Plus),
        "trash" | "delete" | "remove" => Some(IconName::Trash),
        "copy" | "clipboard" => Some(IconName::Copy),
        "settings" | "gear" | "cog" | "config" => Some(IconName::Settings),
        "magnifyingglass" | "search" | "find" => Some(IconName::MagnifyingGlass),
        "terminal" | "console" | "shell" | "cli" => Some(IconName::Terminal),
        "code" | "script" | "dev" => Some(IconName::Code),

        // Status
        "check" | "checkmark" | "done" | "complete" => Some(IconName::Check),
        "star" | "favorite" => Some(IconName::Star),
        "starfilled" => Some(IconName::StarFilled),
        "boltfilled" | "bolt" | "lightning" | "flash" => Some(IconName::BoltFilled),
        "boltoutlined" => Some(IconName::BoltOutlined),

        // Arrows
        "arrowright" | "right" => Some(IconName::ArrowRight),
        "arrowdown" | "down" => Some(IconName::ArrowDown),
        "arrowup" | "up" => Some(IconName::ArrowUp),
        "chevronright" => Some(IconName::ChevronRight),
        "chevrondown" => Some(IconName::ChevronDown),

        // UI
        "close" | "x" | "dismiss" => Some(IconName::Close),

        // Media
        "playfilled" | "play" | "run" | "execute" => Some(IconName::PlayFilled),
        "playoutlined" => Some(IconName::PlayOutlined),

        // UI/Layout
        "sidebar" | "panel" | "layout" => Some(IconName::Sidebar),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_count() {
        assert_eq!(IconName::count(), 25); // 22 + Sidebar + ArrowUp + Close
    }

    #[test]
    fn test_style_count() {
        assert_eq!(IconStyle::count(), 7);
    }

    #[test]
    fn test_all_icons_have_paths() {
        for icon in IconName::all() {
            let path = icon.path();
            assert!(
                path.ends_with(".svg"),
                "Icon {:?} path doesn't end with .svg",
                icon
            );
            assert!(
                path.starts_with("icons/"),
                "Icon {:?} path doesn't start with icons/",
                icon
            );
        }
    }

    #[test]
    fn test_category_coverage() {
        let mut covered = 0;
        for cat in IconCategory::all() {
            covered += cat.icons().len();
        }
        assert_eq!(
            covered,
            IconName::count(),
            "Categories don't cover all icons"
        );
    }

    #[test]
    fn test_icon_name_from_str() {
        // Exact match
        assert_eq!(icon_name_from_str("File"), Some(IconName::File));
        assert_eq!(icon_name_from_str("Terminal"), Some(IconName::Terminal));

        // Lowercase
        assert_eq!(icon_name_from_str("file"), Some(IconName::File));
        assert_eq!(icon_name_from_str("code"), Some(IconName::Code));

        // With spaces
        assert_eq!(icon_name_from_str("file code"), Some(IconName::FileCode));
        assert_eq!(
            icon_name_from_str("folder open"),
            Some(IconName::FolderOpen)
        );

        // Kebab case
        assert_eq!(icon_name_from_str("file-code"), Some(IconName::FileCode));
        assert_eq!(
            icon_name_from_str("bolt-filled"),
            Some(IconName::BoltFilled)
        );

        // Snake case
        assert_eq!(icon_name_from_str("file_code"), Some(IconName::FileCode));
        assert_eq!(
            icon_name_from_str("magnifying_glass"),
            Some(IconName::MagnifyingGlass)
        );

        // Aliases
        assert_eq!(
            icon_name_from_str("search"),
            Some(IconName::MagnifyingGlass)
        );
        assert_eq!(icon_name_from_str("add"), Some(IconName::Plus));
        assert_eq!(icon_name_from_str("delete"), Some(IconName::Trash));
        assert_eq!(icon_name_from_str("gear"), Some(IconName::Settings));
        assert_eq!(icon_name_from_str("lightning"), Some(IconName::BoltFilled));
        assert_eq!(icon_name_from_str("run"), Some(IconName::PlayFilled));

        // Unknown
        assert_eq!(icon_name_from_str("unknown"), None);
        assert_eq!(icon_name_from_str(""), None);
    }

    #[test]
    fn test_style_sizes() {
        assert_eq!(IconStyle::Small.size(), 12.0);
        assert_eq!(IconStyle::Default.size(), 16.0);
        assert_eq!(IconStyle::Large.size(), 24.0);
    }
}
