use gpui::{rgb, rgba, Hsla, Rgba};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use tracing::{info, warn, error, debug};

/// Hex color representation (u32)
pub type HexColor = u32;

/// Background opacity settings for window transparency
/// Values range from 0.0 (fully transparent) to 1.0 (fully opaque)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundOpacity {
    /// Main background opacity (default: 0.85)
    pub main: f32,
    /// Title bar opacity (default: 0.9)
    pub title_bar: f32,
    /// Search box opacity (default: 0.92)
    pub search_box: f32,
    /// Log panel opacity (default: 0.8)
    pub log_panel: f32,
}

impl Default for BackgroundOpacity {
    fn default() -> Self {
        BackgroundOpacity {
            // Lower opacity values to allow vibrancy blur to show through
            // Values below ~0.7 will show more blur effect
            main: 0.60,          // Was 0.85 - lower for more vibrancy
            title_bar: 0.65,     // Was 0.9
            search_box: 0.70,    // Was 0.92
            log_panel: 0.55,     // Was 0.8
        }
    }
}

/// Vibrancy/blur effect settings for the window background
/// This creates the native macOS translucent effect like Spotlight/Raycast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibrancySettings {
    /// Whether vibrancy is enabled (default: true)
    pub enabled: bool,
    /// Vibrancy material type: "hud", "popover", "menu", "sidebar", "content"
    /// - "hud": Dark, high contrast (like HUD windows)
    /// - "popover": Light blur, used in popovers
    /// - "menu": Similar to system menus
    /// - "sidebar": Sidebar-style blur
    /// - "content": Content background blur
    /// Default: "popover" for a subtle, native feel
    pub material: String,
}

impl Default for VibrancySettings {
    fn default() -> Self {
        VibrancySettings {
            enabled: true,
            material: "popover".to_string(),
        }
    }
}

/// Drop shadow configuration for the window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropShadow {
    /// Whether drop shadow is enabled (default: true)
    pub enabled: bool,
    /// Blur radius for the shadow (default: 20.0)
    pub blur_radius: f32,
    /// Spread radius for the shadow (default: 0.0)
    pub spread_radius: f32,
    /// Horizontal offset (default: 0.0)
    pub offset_x: f32,
    /// Vertical offset (default: 8.0)
    pub offset_y: f32,
    /// Shadow color as hex (default: 0x000000 - black)
    pub color: HexColor,
    /// Shadow opacity (default: 0.25)
    pub opacity: f32,
}

impl Default for DropShadow {
    fn default() -> Self {
        DropShadow {
            enabled: true,
            blur_radius: 20.0,
            spread_radius: 0.0,
            offset_x: 0.0,
            offset_y: 8.0,
            color: 0x000000,
            opacity: 0.25,
        }
    }
}

/// Background color definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundColors {
    /// Main background (0x1e1e1e)
    pub main: HexColor,
    /// Title bar background (0x2d2d30)
    pub title_bar: HexColor,
    /// Search box background (0x3c3c3c)
    pub search_box: HexColor,
    /// Log panel background (0x0d0d0d)
    pub log_panel: HexColor,
}

/// Text color definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextColors {
    /// Primary text color (0xffffff - white)
    pub primary: HexColor,
    /// Secondary text color (0xe0e0e0)
    pub secondary: HexColor,
    /// Tertiary text color (0x999999)
    pub tertiary: HexColor,
    /// Muted text color (0x808080)
    pub muted: HexColor,
    /// Dimmed text color (0x666666)
    pub dimmed: HexColor,
}

/// Accent and highlight colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccentColors {
    /// Selected item highlight (0x007acc - blue)
    pub selected: HexColor,
    /// Subtle selection for list items - barely visible highlight (0x2a2a2a - dark gray)
    /// Used for polished, Raycast-like selection backgrounds
    #[serde(default = "default_selected_subtle")]
    pub selected_subtle: HexColor,
    /// Button text color for action buttons like Run, Actions, Edit, New (0x5eead4 - teal/cyan)
    /// Used for interactive button text that should stand out from regular text
    #[serde(default = "default_button_text")]
    pub button_text: HexColor,
}

/// Default subtle selection color (dark gray, barely visible)
fn default_selected_subtle() -> HexColor {
    0x2a2a2a
}

/// Default button text color (teal/cyan - matches common accent colors)
/// 0x5eead4 is a pleasant teal that works well on dark backgrounds
fn default_button_text() -> HexColor {
    0x5eead4
}

/// Border and UI element colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIColors {
    /// Border color (0x464647)
    pub border: HexColor,
    /// Success color for logs (0x00ff00 - green)
    pub success: HexColor,
    /// Error color for error messages (0xef4444 - red-500)
    #[serde(default = "default_error_color")]
    pub error: HexColor,
    /// Warning color for warning messages (0xf59e0b - amber-500)
    #[serde(default = "default_warning_color")]
    pub warning: HexColor,
    /// Info color for informational messages (0x3b82f6 - blue-500)
    #[serde(default = "default_info_color")]
    pub info: HexColor,
}

/// Default error color (red-500)
fn default_error_color() -> HexColor {
    0xef4444
}

/// Default warning color (amber-500)
fn default_warning_color() -> HexColor {
    0xf59e0b
}

/// Default info color (blue-500)
fn default_info_color() -> HexColor {
    0x3b82f6
}

/// Cursor styling for text input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorStyle {
    /// Cursor color when focused (0x00ffff - cyan)
    pub color: HexColor,
    /// Cursor blink interval in milliseconds
    pub blink_interval_ms: u64,
}

/// Color scheme for a specific window focus state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusColorScheme {
    pub background: BackgroundColors,
    pub text: TextColors,
    pub accent: AccentColors,
    pub ui: UIColors,
    /// Optional cursor styling
    #[serde(default)]
    pub cursor: Option<CursorStyle>,
}

/// Complete color scheme definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorScheme {
    pub background: BackgroundColors,
    pub text: TextColors,
    pub accent: AccentColors,
    pub ui: UIColors,
}

/// Window focus-aware theme with separate styles for focused and unfocused states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusAwareColorScheme {
    /// Colors when window is focused (default to standard colors if not specified)
    #[serde(default)]
    pub focused: Option<FocusColorScheme>,
    /// Colors when window is unfocused (dimmed/desaturated)
    #[serde(default)]
    pub unfocused: Option<FocusColorScheme>,
}

/// Complete theme definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub colors: ColorScheme,
    /// Optional focus-aware colors (new feature)
    #[serde(default)]
    pub focus_aware: Option<FocusAwareColorScheme>,
    /// Background opacity settings for window transparency
    #[serde(default)]
    pub opacity: Option<BackgroundOpacity>,
    /// Drop shadow configuration
    #[serde(default)]
    pub drop_shadow: Option<DropShadow>,
    /// Vibrancy/blur effect settings
    #[serde(default)]
    pub vibrancy: Option<VibrancySettings>,
}

#[allow(dead_code)]
impl CursorStyle {
    /// Create a default blinking cursor style
    pub fn default_focused() -> Self {
        CursorStyle {
            color: 0x00ffff, // Cyan cursor when focused
            blink_interval_ms: 500,
        }
    }
}

#[allow(dead_code)]
impl FocusColorScheme {
    /// Convert to a standard ColorScheme
    pub fn to_color_scheme(&self) -> ColorScheme {
        ColorScheme {
            background: self.background.clone(),
            text: self.text.clone(),
            accent: self.accent.clone(),
            ui: self.ui.clone(),
        }
    }
}

impl ColorScheme {
    /// Create a dark mode color scheme (default dark colors)
    pub fn dark_default() -> Self {
        ColorScheme {
            background: BackgroundColors {
                main: 0x1e1e1e,
                title_bar: 0x2d2d30,
                search_box: 0x3c3c3c,
                log_panel: 0x0d0d0d,
            },
            text: TextColors {
                primary: 0xffffff,
                secondary: 0xcccccc,
                tertiary: 0x999999,
                muted: 0x808080,
                dimmed: 0x666666,
            },
            accent: AccentColors {
                selected: 0xfbbf24,    // Script Kit primary: #fbbf24 (yellow/gold) - for text highlights
                selected_subtle: 0x2a2a2a, // Subtle dark gray for list selection backgrounds
                button_text: 0x5eead4, // Teal/cyan for button text (Run, Actions, Edit, New)
            },
            ui: UIColors {
                border: 0x464647,
                success: 0x00ff00,
                error: 0xef4444,   // red-500
                warning: 0xf59e0b, // amber-500
                info: 0x3b82f6,    // blue-500
            },
        }
    }

    /// Create a light mode color scheme
    pub fn light_default() -> Self {
        ColorScheme {
            background: BackgroundColors {
                main: 0xffffff,
                title_bar: 0xf3f3f3,
                search_box: 0xececec,
                log_panel: 0xfafafa,
            },
            text: TextColors {
                primary: 0x000000,
                secondary: 0x333333,
                tertiary: 0x666666,
                muted: 0x999999,
                dimmed: 0xcccccc,
            },
            accent: AccentColors {
                selected: 0x0078d4,
                selected_subtle: 0xe8e8e8, // Subtle light gray for list selections
                button_text: 0x0d9488, // Darker teal for light mode button text
            },
            ui: UIColors {
                border: 0xd0d0d0,
                success: 0x00a000,
                error: 0xdc2626,   // red-600 (darker for light mode)
                warning: 0xd97706, // amber-600 (darker for light mode)
                info: 0x2563eb,    // blue-600 (darker for light mode)
            },
        }
    }

    /// Create an unfocused (dimmed) version of this color scheme
    #[allow(dead_code)]
    pub fn to_unfocused(&self) -> Self {
        fn darken_hex(color: HexColor) -> HexColor {
            // Reduce brightness by blending towards mid-gray
            let r = (color >> 16) & 0xFF;
            let g = (color >> 8) & 0xFF;
            let b = color & 0xFF;
            
            // Reduce saturation and brightness: blend 30% toward gray
            let gray = 0x80u32;
            let new_r = ((r as u32 * 70 + gray * 30) / 100) as u8;
            let new_g = ((g as u32 * 70 + gray * 30) / 100) as u8;
            let new_b = ((b as u32 * 70 + gray * 30) / 100) as u8;
            
            ((new_r as u32) << 16) | ((new_g as u32) << 8) | (new_b as u32)
        }
        
        ColorScheme {
            background: BackgroundColors {
                main: darken_hex(self.background.main),
                title_bar: darken_hex(self.background.title_bar),
                search_box: darken_hex(self.background.search_box),
                log_panel: darken_hex(self.background.log_panel),
            },
            text: TextColors {
                primary: darken_hex(self.text.primary),
                secondary: darken_hex(self.text.secondary),
                tertiary: darken_hex(self.text.tertiary),
                muted: darken_hex(self.text.muted),
                dimmed: darken_hex(self.text.dimmed),
            },
            accent: AccentColors {
                selected: darken_hex(self.accent.selected),
                selected_subtle: darken_hex(self.accent.selected_subtle),
                button_text: darken_hex(self.accent.button_text),
            },
            ui: UIColors {
                border: darken_hex(self.ui.border),
                success: darken_hex(self.ui.success),
                error: darken_hex(self.ui.error),
                warning: darken_hex(self.ui.warning),
                info: darken_hex(self.ui.info),
            },
        }
    }
}

impl Default for ColorScheme {
    fn default() -> Self {
        ColorScheme::dark_default()
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            colors: ColorScheme::default(),
            focus_aware: None,
            opacity: Some(BackgroundOpacity::default()),
            drop_shadow: Some(DropShadow::default()),
            vibrancy: Some(VibrancySettings::default()),
        }
    }
}

#[allow(dead_code)]
impl Theme {
    /// Get the appropriate color scheme based on window focus state
    /// 
    /// If focus-aware colors are configured:
    /// - Returns focused colors when focused=true
    /// - Returns unfocused colors when focused=false
    /// 
    /// If focus-aware colors are not configured:
    /// - Always returns the standard colors (automatic dimmed version for unfocused)
    pub fn get_colors(&self, is_focused: bool) -> ColorScheme {
        if let Some(ref focus_aware) = self.focus_aware {
            if is_focused {
                if let Some(ref focused) = focus_aware.focused {
                    return focused.to_color_scheme();
                }
            } else {
                if let Some(ref unfocused) = focus_aware.unfocused {
                    return unfocused.to_color_scheme();
                }
            }
        }
        
        // Fallback: use standard colors, with automatic dimming for unfocused
        if is_focused {
            self.colors.clone()
        } else {
            self.colors.to_unfocused()
        }
    }
    
    /// Get cursor style if window is focused
    pub fn get_cursor_style(&self, is_focused: bool) -> Option<CursorStyle> {
        if !is_focused {
            return None;
        }
        
        if let Some(ref focus_aware) = self.focus_aware {
            if let Some(ref focused) = focus_aware.focused {
                return focused.cursor.clone();
            }
        }
        
        // Return default blinking cursor if focused
        Some(CursorStyle::default_focused())
    }
    
    /// Get background opacity settings
    /// Returns the configured opacity or sensible defaults
    pub fn get_opacity(&self) -> BackgroundOpacity {
        self.opacity.clone().unwrap_or_default()
    }
    
    /// Get opacity adjusted for focus state
    /// Unfocused windows are slightly more transparent
    pub fn get_opacity_for_focus(&self, is_focused: bool) -> BackgroundOpacity {
        let base = self.get_opacity();
        if is_focused {
            base
        } else {
            // Reduce opacity by 10% when unfocused
            BackgroundOpacity {
                main: (base.main * 0.9).clamp(0.0, 1.0),
                title_bar: (base.title_bar * 0.9).clamp(0.0, 1.0),
                search_box: (base.search_box * 0.9).clamp(0.0, 1.0),
                log_panel: (base.log_panel * 0.9).clamp(0.0, 1.0),
            }
        }
    }
    
    /// Get drop shadow configuration
    /// Returns the configured shadow or sensible defaults
    pub fn get_drop_shadow(&self) -> DropShadow {
        self.drop_shadow.clone().unwrap_or_default()
    }
    
    /// Get vibrancy/blur effect settings
    /// Returns the configured vibrancy or sensible defaults
    pub fn get_vibrancy(&self) -> VibrancySettings {
        self.vibrancy.clone().unwrap_or_default()
    }
    
    /// Check if vibrancy effect should be enabled
    pub fn is_vibrancy_enabled(&self) -> bool {
        self.get_vibrancy().enabled
    }
}

/// Detect system appearance preference on macOS
///
/// Returns true if dark mode is enabled, false if light mode is enabled.
/// On non-macOS systems or if detection fails, defaults to true (dark mode).
///
/// Uses the `defaults read -g AppleInterfaceStyle` command to detect the system appearance.
pub fn detect_system_appearance() -> bool {
    // Try to detect macOS dark mode using system defaults
    match Command::new("defaults")
        .args(&["read", "-g", "AppleInterfaceStyle"])
        .output()
    {
        Ok(output) => {
            // If the command succeeds and returns "Dark", we're in dark mode
            let stdout = String::from_utf8_lossy(&output.stdout);
            let is_dark = stdout.to_lowercase().contains("dark");
            info!(appearance = if is_dark { "dark" } else { "light" }, "System appearance detected");
            is_dark
        }
        Err(_) => {
            // Command failed or not available (e.g., light mode on macOS returns error)
            debug!("System appearance detection failed or light mode detected, defaulting to light");
            false
        }
    }
}

/// Load theme from ~/.kit/theme.json
/// 
/// Colors should be specified as decimal integers in the JSON file.
/// For example, 0x1e1e1e (hex) = 1980410 (decimal).
/// 
/// Example theme.json structure:
/// ```json
/// {
///   "colors": {
///     "background": {
///       "main": 1980410,
///       "title_bar": 2961712,
///       "search_box": 3947580,
///       "log_panel": 851213
///     },
///     "text": {
///       "primary": 16777215,
///       "secondary": 14737920,
///       "tertiary": 10066329,
///       "muted": 8421504,
///       "dimmed": 6710886
///     },
///     "accent": {
///       "selected": 31948
///     },
///     "ui": {
///       "border": 4609607,
///       "success": 65280
///     }
///   }
/// }
/// ```
/// 
/// If the file doesn't exist or fails to parse, returns a theme based on system appearance detection.
/// If system appearance detection is not available, defaults to dark mode.
/// Logs errors to stderr but doesn't fail the application.
pub fn load_theme() -> Theme {
    let theme_path = PathBuf::from(shellexpand::tilde("~/.kit/theme.json").as_ref());

    // Check if theme file exists
    if !theme_path.exists() {
        warn!(path = %theme_path.display(), "Theme file not found, using defaults based on system appearance");
        // Auto-select based on system appearance
        let is_dark = detect_system_appearance();
        let color_scheme = if is_dark {
            ColorScheme::dark_default()
        } else {
            ColorScheme::light_default()
        };
        let theme = Theme {
            focus_aware: None,
            colors: color_scheme,
            opacity: Some(BackgroundOpacity::default()),
            drop_shadow: Some(DropShadow::default()),
            vibrancy: Some(VibrancySettings::default()),
        };
        log_theme_config(&theme);
        return theme;
    }

    // Read and parse the JSON file
    match std::fs::read_to_string(&theme_path) {
        Err(e) => {
            error!(path = %theme_path.display(), error = %e, "Failed to read theme file, using defaults");
            let is_dark = detect_system_appearance();
            let color_scheme = if is_dark {
                ColorScheme::dark_default()
            } else {
                ColorScheme::light_default()
            };
            let theme = Theme {
                colors: color_scheme,
                focus_aware: None,
                opacity: Some(BackgroundOpacity::default()),
                drop_shadow: Some(DropShadow::default()),
                vibrancy: Some(VibrancySettings::default()),
            };
            log_theme_config(&theme);
            theme
        }
        Ok(contents) => {
            match serde_json::from_str::<Theme>(&contents) {
                Ok(theme) => {
                    info!(path = %theme_path.display(), "Successfully loaded theme");
                    log_theme_config(&theme);
                    theme
                }
                Err(e) => {
                    error!(
                        path = %theme_path.display(),
                        error = %e,
                        "Failed to parse theme JSON, using defaults"
                    );
                    debug!(content = %contents, "Malformed theme file content");
                    let is_dark = detect_system_appearance();
                    let color_scheme = if is_dark {
                        ColorScheme::dark_default()
                    } else {
                        ColorScheme::light_default()
                    };
                    let theme = Theme {
                        colors: color_scheme,
                        focus_aware: None,
                        opacity: Some(BackgroundOpacity::default()),
                        drop_shadow: Some(DropShadow::default()),
                        vibrancy: Some(VibrancySettings::default()),
                    };
                    log_theme_config(&theme);
                    theme
                }
            }
        }
    }
}

// ============================================================================
// Lightweight Theme Extraction Helpers
// ============================================================================
// These structs pre-compute theme values for efficient use in render closures.
// They implement Copy to avoid heap allocations when captured by closures.

/// Lightweight struct for list item rendering - Copy to avoid clone in closures
/// 
/// This struct pre-computes the exact colors needed for rendering list items,
/// avoiding the need to clone the full ThemeColors struct into render closures.
/// 
/// # Example
/// ```ignore
/// let list_colors = theme.colors.list_item_colors();
/// // Pass list_colors into closure - it's Copy, so no heap allocation
/// uniform_list(cx, |_this, visible_range, _window, _cx| {
///     for ix in visible_range {
///         let bg = if is_selected { list_colors.background_selected } else { list_colors.background };
///         // ... render item
///     }
/// })
/// ```
#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub struct ListItemColors {
    /// Normal item background (usually transparent)
    pub background: Rgba,
    /// Background when hovering over an item
    pub background_hover: Rgba,
    /// Background when item is selected
    pub background_selected: Rgba,
    /// Primary text color (for item names)
    pub text: Rgba,
    /// Secondary/muted text color (for descriptions when not selected)
    pub text_secondary: Rgba,
    /// Dimmed text color (for shortcuts, metadata)
    pub text_dimmed: Rgba,
    /// Accent text color (for descriptions when selected)
    pub text_accent: Rgba,
    /// Border color for separators
    pub border: Rgba,
}

#[allow(dead_code)]
impl ListItemColors {
    /// Create ListItemColors from a ColorScheme
    /// 
    /// This extracts only the colors needed for list item rendering.
    pub fn from_color_scheme(colors: &ColorScheme) -> Self {
        // Pre-compute rgba colors with appropriate alpha values
        // Background is transparent, hover/selected use subtle colors
        let selected_subtle = colors.accent.selected_subtle;
        
        #[cfg(debug_assertions)]
        debug!(selected_subtle = format!("#{:06x}", selected_subtle), "Extracting list item colors");
        
        ListItemColors {
            background: rgba(0x00000000),  // Fully transparent
            background_hover: rgba((selected_subtle << 8) | 0x40),  // 25% opacity
            background_selected: rgba((selected_subtle << 8) | 0x80),  // 50% opacity
            text: rgb(colors.text.primary),
            text_secondary: rgb(colors.text.secondary),
            text_dimmed: rgb(colors.text.dimmed),
            text_accent: rgb(colors.accent.selected),
            border: rgb(colors.ui.border),
        }
    }
    
    /// Convert a specific color to Hsla for advanced styling
    pub fn text_as_hsla(&self) -> Hsla {
        self.text.into()
    }
    
    /// Get description color based on selection state
    pub fn description_color(&self, is_selected: bool) -> Rgba {
        if is_selected {
            self.text_accent
        } else {
            self.text_secondary
        }
    }
    
    /// Get item text color based on selection state
    pub fn item_text_color(&self, is_selected: bool) -> Rgba {
        if is_selected {
            self.text
        } else {
            self.text_secondary
        }
    }
}

#[allow(dead_code)]
impl ColorScheme {
    /// Extract only the colors needed for list item rendering
    /// 
    /// This creates a lightweight, Copy struct that can be efficiently
    /// passed into closures without cloning the full ColorScheme.
    pub fn list_item_colors(&self) -> ListItemColors {
        ListItemColors::from_color_scheme(self)
    }
}

/// Lightweight struct for input field rendering
/// 
/// Pre-computes colors for search boxes, text inputs, etc.
#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub struct InputFieldColors {
    /// Background color of the input
    pub background: Rgba,
    /// Text color when typing
    pub text: Rgba,
    /// Placeholder text color
    pub placeholder: Rgba,
    /// Border color
    pub border: Rgba,
    /// Cursor color
    pub cursor: Rgba,
}

#[allow(dead_code)]
impl InputFieldColors {
    /// Create InputFieldColors from a ColorScheme
    pub fn from_color_scheme(colors: &ColorScheme) -> Self {
        #[cfg(debug_assertions)]
        debug!("Extracting input field colors");
        
        InputFieldColors {
            background: rgba((colors.background.search_box << 8) | 0x80),
            text: rgb(colors.text.primary),
            placeholder: rgb(colors.text.muted),
            border: rgba((colors.ui.border << 8) | 0x60),
            cursor: rgb(0x00ffff),  // Cyan cursor
        }
    }
}

#[allow(dead_code)]
impl ColorScheme {
    /// Extract colors for input field rendering
    pub fn input_field_colors(&self) -> InputFieldColors {
        InputFieldColors::from_color_scheme(self)
    }
}

// ============================================================================
// End Lightweight Theme Extraction Helpers
// ============================================================================

/// Log theme configuration for debugging
fn log_theme_config(theme: &Theme) {
    let opacity = theme.get_opacity();
    let shadow = theme.get_drop_shadow();
    let vibrancy = theme.get_vibrancy();
    debug!(
        opacity_main = opacity.main,
        opacity_title_bar = opacity.title_bar,
        opacity_search_box = opacity.search_box,
        opacity_log_panel = opacity.log_panel,
        "Theme opacity configured"
    );
    debug!(
        shadow_enabled = shadow.enabled,
        blur_radius = shadow.blur_radius,
        spread_radius = shadow.spread_radius,
        offset_x = shadow.offset_x,
        offset_y = shadow.offset_y,
        shadow_opacity = shadow.opacity,
        "Theme shadow configured"
    );
    debug!(
        vibrancy_enabled = vibrancy.enabled,
        material = %vibrancy.material,
        "Theme vibrancy configured"
    );
    debug!(
        selected = format!("#{:06x}", theme.colors.accent.selected),
        selected_subtle = format!("#{:06x}", theme.colors.accent.selected_subtle),
        button_text = format!("#{:06x}", theme.colors.accent.button_text),
        "Theme accent colors"
    );
    debug!(
        error = format!("#{:06x}", theme.colors.ui.error),
        warning = format!("#{:06x}", theme.colors.ui.warning),
        info = format!("#{:06x}", theme.colors.ui.info),
        "Theme status colors"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_theme() {
        let theme = Theme::default();
        assert_eq!(theme.colors.background.main, 0x1e1e1e);
        assert_eq!(theme.colors.text.primary, 0xffffff);
        assert_eq!(theme.colors.accent.selected, 0xfbbf24);
        assert_eq!(theme.colors.ui.border, 0x464647);
    }

    #[test]
    fn test_color_scheme_default() {
        let scheme = ColorScheme::default();
        assert_eq!(scheme.background.title_bar, 0x2d2d30);
        assert_eq!(scheme.text.secondary, 0xcccccc);
        assert_eq!(scheme.ui.success, 0x00ff00);
    }

    #[test]
    fn test_dark_default() {
        let scheme = ColorScheme::dark_default();
        assert_eq!(scheme.background.main, 0x1e1e1e);
        assert_eq!(scheme.text.primary, 0xffffff);
        assert_eq!(scheme.background.title_bar, 0x2d2d30);
        assert_eq!(scheme.ui.success, 0x00ff00);
    }

    #[test]
    fn test_light_default() {
        let scheme = ColorScheme::light_default();
        assert_eq!(scheme.background.main, 0xffffff);
        assert_eq!(scheme.text.primary, 0x000000);
        assert_eq!(scheme.background.title_bar, 0xf3f3f3);
        assert_eq!(scheme.ui.border, 0xd0d0d0);
    }

    #[test]
    fn test_theme_serialization() {
        let theme = Theme::default();
        let json = serde_json::to_string(&theme).unwrap();
        let deserialized: Theme = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.colors.background.main, theme.colors.background.main);
        assert_eq!(deserialized.colors.text.primary, theme.colors.text.primary);
        assert_eq!(deserialized.colors.accent.selected, theme.colors.accent.selected);
        assert_eq!(deserialized.colors.ui.border, theme.colors.ui.border);
    }

    #[test]
    fn test_light_theme_serialization() {
        let theme = Theme {
            colors: ColorScheme::light_default(),
            focus_aware: None,
            opacity: Some(BackgroundOpacity::default()),
            drop_shadow: Some(DropShadow::default()),
            vibrancy: Some(VibrancySettings::default()),
        };
        let json = serde_json::to_string(&theme).unwrap();
        let deserialized: Theme = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.colors.background.main, 0xffffff);
        assert_eq!(deserialized.colors.text.primary, 0x000000);
    }
    
    #[test]
    fn test_opacity_defaults() {
        let opacity = BackgroundOpacity::default();
        assert_eq!(opacity.main, 0.60);
        assert_eq!(opacity.title_bar, 0.65);
        assert_eq!(opacity.search_box, 0.70);
        assert_eq!(opacity.log_panel, 0.55);
    }
    
    #[test]
    fn test_drop_shadow_defaults() {
        let shadow = DropShadow::default();
        assert!(shadow.enabled);
        assert_eq!(shadow.blur_radius, 20.0);
        assert_eq!(shadow.spread_radius, 0.0);
        assert_eq!(shadow.offset_x, 0.0);
        assert_eq!(shadow.offset_y, 8.0);
        assert_eq!(shadow.color, 0x000000);
        assert_eq!(shadow.opacity, 0.25);
    }
    
    #[test]
    fn test_vibrancy_defaults() {
        let vibrancy = VibrancySettings::default();
        assert!(vibrancy.enabled);
        assert_eq!(vibrancy.material, "popover");
    }

    #[test]
    fn test_detect_system_appearance() {
        // This test just verifies the function can be called without panicking
        // The result will vary based on the system's actual appearance setting
        let _is_dark = detect_system_appearance();
        // Don't assert a specific value, just ensure it doesn't panic
    }
    
    // ========================================================================
    // ListItemColors Tests
    // ========================================================================
    
    #[test]
    fn test_list_item_colors_is_copy() {
        // Compile-time verification that ListItemColors implements Copy
        fn assert_copy<T: Copy>() {}
        assert_copy::<ListItemColors>();
    }
    
    #[test]
    fn test_list_item_colors_from_dark_scheme() {
        let scheme = ColorScheme::dark_default();
        let colors = scheme.list_item_colors();
        
        // Verify background is transparent
        assert_eq!(colors.background.a, 0.0);
        
        // Verify hover and selected have some opacity (not transparent)
        assert!(colors.background_hover.a > 0.0);
        assert!(colors.background_selected.a > 0.0);
        
        // Verify selected has more opacity than hover
        assert!(colors.background_selected.a > colors.background_hover.a);
    }
    
    #[test]
    fn test_list_item_colors_from_light_scheme() {
        let scheme = ColorScheme::light_default();
        let colors = scheme.list_item_colors();
        
        // Verify we get colors from light scheme
        // Light scheme uses 0xe8e8e8 for selected_subtle
        assert!(colors.background_selected.a > 0.0);
    }
    
    #[test]
    fn test_list_item_colors_description_color() {
        let scheme = ColorScheme::dark_default();
        let colors = scheme.list_item_colors();
        
        let selected_desc = colors.description_color(true);
        let unselected_desc = colors.description_color(false);
        
        // Selected should use accent, unselected should use secondary
        // These should be different colors
        assert_ne!(selected_desc.r, unselected_desc.r);
    }
    
    #[test]
    fn test_list_item_colors_item_text_color() {
        let scheme = ColorScheme::dark_default();
        let colors = scheme.list_item_colors();
        
        let selected_text = colors.item_text_color(true);
        let unselected_text = colors.item_text_color(false);
        
        // For dark theme, selected should be primary (white), unselected secondary
        assert!(selected_text.r >= unselected_text.r);
    }
    
    #[test]
    fn test_list_item_colors_text_as_hsla() {
        let scheme = ColorScheme::dark_default();
        let colors = scheme.list_item_colors();
        
        let hsla = colors.text_as_hsla();
        
        // Dark theme primary text is white (0xffffff)
        // White should have high lightness
        assert!(hsla.l > 0.9);
    }
    
    // ========================================================================
    // InputFieldColors Tests
    // ========================================================================
    
    #[test]
    fn test_input_field_colors_is_copy() {
        // Compile-time verification that InputFieldColors implements Copy
        fn assert_copy<T: Copy>() {}
        assert_copy::<InputFieldColors>();
    }
    
    #[test]
    fn test_input_field_colors_from_scheme() {
        let scheme = ColorScheme::dark_default();
        let colors = scheme.input_field_colors();
        
        // Background should have some alpha (semi-transparent)
        assert!(colors.background.a > 0.0);
        assert!(colors.background.a < 1.0);
        
        // Border should have some alpha
        assert!(colors.border.a > 0.0);
        
        // Text should be fully opaque
        assert_eq!(colors.text.a, 1.0);
    }
    
    #[test]
    fn test_input_field_cursor_color() {
        let scheme = ColorScheme::dark_default();
        let colors = scheme.input_field_colors();
        
        // Cursor should be cyan (0x00ffff)
        // In rgba, cyan has g=1.0, b=1.0, r=0.0
        assert!(colors.cursor.g > 0.9);
        assert!(colors.cursor.b > 0.9);
    }
}
