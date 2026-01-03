use gpui::{rgb, rgba, App, Hsla, Rgba};
use gpui_component::theme::{Theme as GpuiTheme, ThemeColor, ThemeMode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use tracing::info as tracing_info;
use tracing::{debug, error, info, warn};

/// Transparent color constant (fully transparent black)
pub const TRANSPARENT: u32 = 0x00000000;

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
            main: 0.60,       // Was 0.85 - lower for more vibrancy
            title_bar: 0.65,  // Was 0.9
            search_box: 0.70, // Was 0.92
            log_panel: 0.55,  // Was 0.8
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
    ///
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
    /// Primary accent color (0xfbbf24 - yellow/gold for Script Kit)
    /// Used for: selected items, button text, logo, highlights
    pub selected: HexColor,
    /// Subtle selection for list items - barely visible highlight (0x2a2a2a - dark gray)
    /// Used for polished, Raycast-like selection backgrounds
    #[serde(default = "default_selected_subtle")]
    pub selected_subtle: HexColor,
}

/// Default subtle selection color (dark gray, barely visible)
fn default_selected_subtle() -> HexColor {
    0x2a2a2a
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

/// Terminal ANSI color palette (16 colors)
///
/// These colors are used by the embedded terminal emulator for ANSI escape sequences.
/// Colors 0-7 are the normal palette, colors 8-15 are the bright variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalColors {
    /// ANSI 0: Black
    #[serde(default = "default_terminal_black")]
    pub black: HexColor,
    /// ANSI 1: Red
    #[serde(default = "default_terminal_red")]
    pub red: HexColor,
    /// ANSI 2: Green
    #[serde(default = "default_terminal_green")]
    pub green: HexColor,
    /// ANSI 3: Yellow
    #[serde(default = "default_terminal_yellow")]
    pub yellow: HexColor,
    /// ANSI 4: Blue
    #[serde(default = "default_terminal_blue")]
    pub blue: HexColor,
    /// ANSI 5: Magenta
    #[serde(default = "default_terminal_magenta")]
    pub magenta: HexColor,
    /// ANSI 6: Cyan
    #[serde(default = "default_terminal_cyan")]
    pub cyan: HexColor,
    /// ANSI 7: White
    #[serde(default = "default_terminal_white")]
    pub white: HexColor,
    /// ANSI 8: Bright Black (Gray)
    #[serde(default = "default_terminal_bright_black")]
    pub bright_black: HexColor,
    /// ANSI 9: Bright Red
    #[serde(default = "default_terminal_bright_red")]
    pub bright_red: HexColor,
    /// ANSI 10: Bright Green
    #[serde(default = "default_terminal_bright_green")]
    pub bright_green: HexColor,
    /// ANSI 11: Bright Yellow
    #[serde(default = "default_terminal_bright_yellow")]
    pub bright_yellow: HexColor,
    /// ANSI 12: Bright Blue
    #[serde(default = "default_terminal_bright_blue")]
    pub bright_blue: HexColor,
    /// ANSI 13: Bright Magenta
    #[serde(default = "default_terminal_bright_magenta")]
    pub bright_magenta: HexColor,
    /// ANSI 14: Bright Cyan
    #[serde(default = "default_terminal_bright_cyan")]
    pub bright_cyan: HexColor,
    /// ANSI 15: Bright White
    #[serde(default = "default_terminal_bright_white")]
    pub bright_white: HexColor,
}

// Terminal color defaults (VS Code dark theme inspired)
fn default_terminal_black() -> HexColor {
    0x000000
}
fn default_terminal_red() -> HexColor {
    0xcd3131
}
fn default_terminal_green() -> HexColor {
    0x0dbc79
}
fn default_terminal_yellow() -> HexColor {
    0xe5e510
}
fn default_terminal_blue() -> HexColor {
    0x2472c8
}
fn default_terminal_magenta() -> HexColor {
    0xbc3fbc
}
fn default_terminal_cyan() -> HexColor {
    0x11a8cd
}
fn default_terminal_white() -> HexColor {
    0xe5e5e5
}
fn default_terminal_bright_black() -> HexColor {
    0x666666
}
fn default_terminal_bright_red() -> HexColor {
    0xf14c4c
}
fn default_terminal_bright_green() -> HexColor {
    0x23d18b
}
fn default_terminal_bright_yellow() -> HexColor {
    0xf5f543
}
fn default_terminal_bright_blue() -> HexColor {
    0x3b8eea
}
fn default_terminal_bright_magenta() -> HexColor {
    0xd670d6
}
fn default_terminal_bright_cyan() -> HexColor {
    0x29b8db
}
fn default_terminal_bright_white() -> HexColor {
    0xffffff
}

impl Default for TerminalColors {
    fn default() -> Self {
        Self::dark_default()
    }
}

impl TerminalColors {
    /// Dark mode terminal colors (VS Code dark inspired)
    pub fn dark_default() -> Self {
        TerminalColors {
            black: 0x000000,
            red: 0xcd3131,
            green: 0x0dbc79,
            yellow: 0xe5e510,
            blue: 0x2472c8,
            magenta: 0xbc3fbc,
            cyan: 0x11a8cd,
            white: 0xe5e5e5,
            bright_black: 0x666666,
            bright_red: 0xf14c4c,
            bright_green: 0x23d18b,
            bright_yellow: 0xf5f543,
            bright_blue: 0x3b8eea,
            bright_magenta: 0xd670d6,
            bright_cyan: 0x29b8db,
            bright_white: 0xffffff,
        }
    }

    /// Light mode terminal colors
    pub fn light_default() -> Self {
        TerminalColors {
            black: 0x000000,
            red: 0xcd3131,
            green: 0x00bc00,
            yellow: 0x949800,
            blue: 0x0451a5,
            magenta: 0xbc05bc,
            cyan: 0x0598bc,
            white: 0x555555,
            bright_black: 0x666666,
            bright_red: 0xcd3131,
            bright_green: 0x14ce14,
            bright_yellow: 0xb5ba00,
            bright_blue: 0x0451a5,
            bright_magenta: 0xbc05bc,
            bright_cyan: 0x0598bc,
            bright_white: 0xa5a5a5,
        }
    }

    /// Get color by ANSI index (0-15)
    #[allow(dead_code)]
    pub fn get(&self, index: u8) -> HexColor {
        match index {
            0 => self.black,
            1 => self.red,
            2 => self.green,
            3 => self.yellow,
            4 => self.blue,
            5 => self.magenta,
            6 => self.cyan,
            7 => self.white,
            8 => self.bright_black,
            9 => self.bright_red,
            10 => self.bright_green,
            11 => self.bright_yellow,
            12 => self.bright_blue,
            13 => self.bright_magenta,
            14 => self.bright_cyan,
            15 => self.bright_white,
            _ => self.black, // Fallback
        }
    }
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
    /// Terminal ANSI colors (optional, defaults provided)
    #[serde(default)]
    pub terminal: TerminalColors,
}

/// Complete color scheme definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorScheme {
    pub background: BackgroundColors,
    pub text: TextColors,
    pub accent: AccentColors,
    pub ui: UIColors,
    /// Terminal ANSI colors (optional, defaults provided)
    #[serde(default)]
    pub terminal: TerminalColors,
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

/// Font configuration for the editor and terminal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontConfig {
    /// Monospace font family for editor/terminal (default: "Menlo" on macOS)
    #[serde(default = "default_mono_font_family")]
    pub mono_family: String,
    /// Monospace font size in pixels (default: 14.0)
    #[serde(default = "default_mono_font_size")]
    pub mono_size: f32,
    /// UI font family (default: system font)
    #[serde(default = "default_ui_font_family")]
    pub ui_family: String,
    /// UI font size in pixels (default: 14.0)
    #[serde(default = "default_ui_font_size")]
    pub ui_size: f32,
}

fn default_mono_font_family() -> String {
    // JetBrains Mono is bundled with the app and registered at startup
    // It provides excellent code readability with ligatures support
    "JetBrains Mono".to_string()
}

fn default_mono_font_size() -> f32 {
    // 16px provides better readability, especially on high-DPI displays
    16.0
}

fn default_ui_font_family() -> String {
    ".SystemUIFont".to_string()
}

fn default_ui_font_size() -> f32 {
    // 16px provides better readability and matches rem_size for gpui-component
    16.0
}

impl Default for FontConfig {
    fn default() -> Self {
        FontConfig {
            mono_family: default_mono_font_family(),
            mono_size: default_mono_font_size(),
            ui_family: default_ui_font_family(),
            ui_size: default_ui_font_size(),
        }
    }
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
    /// Font configuration for editor and terminal
    #[serde(default)]
    pub fonts: Option<FontConfig>,
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
            terminal: self.terminal.clone(),
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
                selected: 0xfbbf24,        // Script Kit primary: #fbbf24 (yellow/gold)
                selected_subtle: 0x2a2a2a, // Subtle dark gray for list selection backgrounds
            },
            ui: UIColors {
                border: 0x464647,
                success: 0x00ff00,
                error: 0xef4444,   // red-500
                warning: 0xf59e0b, // amber-500
                info: 0x3b82f6,    // blue-500
            },
            terminal: TerminalColors::dark_default(),
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
            },
            ui: UIColors {
                border: 0xd0d0d0,
                success: 0x00a000,
                error: 0xdc2626,   // red-600 (darker for light mode)
                warning: 0xd97706, // amber-600 (darker for light mode)
                info: 0x2563eb,    // blue-600 (darker for light mode)
            },
            terminal: TerminalColors::light_default(),
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
            let new_r = ((r * 70 + gray * 30) / 100) as u8;
            let new_g = ((g * 70 + gray * 30) / 100) as u8;
            let new_b = ((b * 70 + gray * 30) / 100) as u8;

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
            },
            ui: UIColors {
                border: darken_hex(self.ui.border),
                success: darken_hex(self.ui.success),
                error: darken_hex(self.ui.error),
                warning: darken_hex(self.ui.warning),
                info: darken_hex(self.ui.info),
            },
            terminal: TerminalColors {
                black: darken_hex(self.terminal.black),
                red: darken_hex(self.terminal.red),
                green: darken_hex(self.terminal.green),
                yellow: darken_hex(self.terminal.yellow),
                blue: darken_hex(self.terminal.blue),
                magenta: darken_hex(self.terminal.magenta),
                cyan: darken_hex(self.terminal.cyan),
                white: darken_hex(self.terminal.white),
                bright_black: darken_hex(self.terminal.bright_black),
                bright_red: darken_hex(self.terminal.bright_red),
                bright_green: darken_hex(self.terminal.bright_green),
                bright_yellow: darken_hex(self.terminal.bright_yellow),
                bright_blue: darken_hex(self.terminal.bright_blue),
                bright_magenta: darken_hex(self.terminal.bright_magenta),
                bright_cyan: darken_hex(self.terminal.bright_cyan),
                bright_white: darken_hex(self.terminal.bright_white),
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
            fonts: Some(FontConfig::default()),
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
            } else if let Some(ref unfocused) = focus_aware.unfocused {
                return unfocused.to_color_scheme();
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

    /// Get font configuration
    /// Returns the configured fonts or sensible defaults
    pub fn get_fonts(&self) -> FontConfig {
        self.fonts.clone().unwrap_or_default()
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
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
    {
        Ok(output) => {
            // If the command succeeds and returns "Dark", we're in dark mode
            let stdout = String::from_utf8_lossy(&output.stdout);
            let is_dark = stdout.to_lowercase().contains("dark");
            info!(
                appearance = if is_dark { "dark" } else { "light" },
                "System appearance detected"
            );
            is_dark
        }
        Err(_) => {
            // Command failed or not available (e.g., light mode on macOS returns error)
            debug!(
                "System appearance detection failed or light mode detected, defaulting to light"
            );
            false
        }
    }
}

/// Load theme from ~/.kenv/theme.json
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
    let theme_path = PathBuf::from(shellexpand::tilde("~/.kenv/theme.json").as_ref());

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
            fonts: Some(FontConfig::default()),
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
                fonts: Some(FontConfig::default()),
            };
            log_theme_config(&theme);
            theme
        }
        Ok(contents) => match serde_json::from_str::<Theme>(&contents) {
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
                    fonts: Some(FontConfig::default()),
                };
                log_theme_config(&theme);
                theme
            }
        },
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
        debug!(
            selected_subtle = format!("#{:06x}", selected_subtle),
            "Extracting list item colors"
        );

        ListItemColors {
            background: rgba(TRANSPARENT), // Fully transparent
            background_hover: rgba((selected_subtle << 8) | 0x40), // 25% opacity
            background_selected: rgba((selected_subtle << 8) | 0x80), // 50% opacity
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
            cursor: rgb(0x00ffff), // Cyan cursor
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
        "Theme accent colors"
    );
    debug!(
        error = format!("#{:06x}", theme.colors.ui.error),
        warning = format!("#{:06x}", theme.colors.ui.warning),
        info = format!("#{:06x}", theme.colors.ui.info),
        "Theme status colors"
    );
}

// ============================================================================
// gpui-component Theme Integration
// ============================================================================
// These functions sync Script Kit's theme with gpui-component's ThemeColor system.
// Used by both main.rs and notes/window.rs for consistent theming.

/// Convert a u32 hex color to Hsla
#[inline]
pub fn hex_to_hsla(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// Map Script Kit's ColorScheme to gpui-component's ThemeColor
///
/// This function takes our Script Kit theme and maps all colors to the
/// gpui-component ThemeColor system, enabling consistent styling across
/// all gpui-component widgets (buttons, inputs, lists, etc.)
///
/// NOTE: We intentionally do NOT apply opacity.* values to theme colors here.
/// The opacity values are for window-level transparency (vibrancy effect),
/// not for making UI elements semi-transparent. UI elements should remain solid
/// so that text and icons are readable regardless of the vibrancy setting.
pub fn map_scriptkit_to_gpui_theme(sk_theme: &Theme) -> ThemeColor {
    let colors = &sk_theme.colors;
    // NOTE: opacity is NOT used here - it's only for window background vibrancy

    // Get default dark theme as base and override with Script Kit colors
    let mut theme_color = *ThemeColor::dark();

    // Main background and foreground
    theme_color.background = hex_to_hsla(colors.background.main);
    theme_color.foreground = hex_to_hsla(colors.text.primary);

    // Accent colors (Script Kit yellow/gold)
    theme_color.accent = hex_to_hsla(colors.accent.selected);
    theme_color.accent_foreground = hex_to_hsla(colors.text.primary);

    // Border
    theme_color.border = hex_to_hsla(colors.ui.border);
    theme_color.input = hex_to_hsla(colors.ui.border);

    // List/sidebar colors
    theme_color.list = hex_to_hsla(colors.background.main);
    theme_color.list_active = hex_to_hsla(colors.accent.selected_subtle);
    theme_color.list_active_border = hex_to_hsla(colors.accent.selected);
    theme_color.list_hover = hex_to_hsla(colors.accent.selected_subtle);
    theme_color.list_even = hex_to_hsla(colors.background.main);
    theme_color.list_head = hex_to_hsla(colors.background.title_bar);

    // Sidebar (use slightly lighter background)
    theme_color.sidebar = hex_to_hsla(colors.background.title_bar);
    theme_color.sidebar_foreground = hex_to_hsla(colors.text.primary);
    theme_color.sidebar_border = hex_to_hsla(colors.ui.border);
    theme_color.sidebar_accent = hex_to_hsla(colors.accent.selected_subtle);
    theme_color.sidebar_accent_foreground = hex_to_hsla(colors.text.primary);
    theme_color.sidebar_primary = hex_to_hsla(colors.accent.selected);
    theme_color.sidebar_primary_foreground = hex_to_hsla(colors.text.primary);

    // Primary (accent-colored buttons)
    theme_color.primary = hex_to_hsla(colors.accent.selected);
    theme_color.primary_foreground = hex_to_hsla(colors.background.main);
    theme_color.primary_hover = hex_to_hsla(colors.accent.selected);
    theme_color.primary_active = hex_to_hsla(colors.accent.selected);

    // Secondary (muted buttons)
    theme_color.secondary = hex_to_hsla(colors.background.search_box);
    theme_color.secondary_foreground = hex_to_hsla(colors.text.primary);
    theme_color.secondary_hover = hex_to_hsla(colors.background.title_bar);
    theme_color.secondary_active = hex_to_hsla(colors.background.title_bar);

    // Muted (disabled states, subtle elements)
    theme_color.muted = hex_to_hsla(colors.background.search_box);
    theme_color.muted_foreground = hex_to_hsla(colors.text.muted);

    // Title bar
    theme_color.title_bar = hex_to_hsla(colors.background.title_bar);
    theme_color.title_bar_border = hex_to_hsla(colors.ui.border);

    // Popover
    theme_color.popover = hex_to_hsla(colors.background.main);
    theme_color.popover_foreground = hex_to_hsla(colors.text.primary);

    // Status colors
    theme_color.success = hex_to_hsla(colors.ui.success);
    theme_color.success_foreground = hex_to_hsla(colors.text.primary);
    theme_color.danger = hex_to_hsla(colors.ui.error);
    theme_color.danger_foreground = hex_to_hsla(colors.text.primary);
    theme_color.warning = hex_to_hsla(colors.ui.warning);
    theme_color.warning_foreground = hex_to_hsla(colors.text.primary);
    theme_color.info = hex_to_hsla(colors.ui.info);
    theme_color.info_foreground = hex_to_hsla(colors.text.primary);

    // Scrollbar
    theme_color.scrollbar = hex_to_hsla(colors.background.main);
    theme_color.scrollbar_thumb = hex_to_hsla(colors.text.dimmed);
    theme_color.scrollbar_thumb_hover = hex_to_hsla(colors.text.muted);

    // Caret (cursor) - match main input text color
    theme_color.caret = hex_to_hsla(colors.text.primary);

    // Selection - match main input selection alpha (0x60)
    let mut selection = hex_to_hsla(colors.accent.selected);
    selection.a = 96.0 / 255.0;
    theme_color.selection = selection;

    // Ring (focus ring)
    theme_color.ring = hex_to_hsla(colors.accent.selected);

    // Tab colors
    theme_color.tab = hex_to_hsla(colors.background.main);
    theme_color.tab_active = hex_to_hsla(colors.background.search_box);
    theme_color.tab_active_foreground = hex_to_hsla(colors.text.primary);
    theme_color.tab_foreground = hex_to_hsla(colors.text.secondary);
    theme_color.tab_bar = hex_to_hsla(colors.background.title_bar);

    debug!(
        background = format!("#{:06x}", colors.background.main),
        accent = format!("#{:06x}", colors.accent.selected),
        "Script Kit theme mapped to gpui-component"
    );

    theme_color
}

/// Sync Script Kit theme with gpui-component's global Theme
///
/// This function loads the Script Kit theme and applies it to gpui-component's
/// global Theme, ensuring all gpui-component widgets use our colors.
///
/// Call this:
/// 1. After `gpui_component::init(cx)` in main.rs
/// 2. When system appearance changes (light/dark mode)
/// 3. When theme.json is reloaded
pub fn sync_gpui_component_theme(cx: &mut App) {
    // Load Script Kit's theme
    let sk_theme = load_theme();

    // Map Script Kit colors to gpui-component ThemeColor
    let custom_colors = map_scriptkit_to_gpui_theme(&sk_theme);

    // Get font configuration
    let fonts = sk_theme.get_fonts();

    // Apply the custom colors and fonts to the global theme
    let theme = GpuiTheme::global_mut(cx);
    theme.colors = custom_colors;
    theme.mode = ThemeMode::Dark; // Script Kit uses dark mode by default

    // Set monospace font for code editor (used by InputState in code_editor mode)
    theme.mono_font_family = fonts.mono_family.clone().into();
    theme.mono_font_size = gpui::px(fonts.mono_size);

    // Set UI font
    theme.font_family = fonts.ui_family.clone().into();
    theme.font_size = gpui::px(fonts.ui_size);

    debug!(
        mono_font = fonts.mono_family,
        mono_size = fonts.mono_size,
        ui_font = fonts.ui_family,
        ui_size = fonts.ui_size,
        "Font configuration applied to gpui-component"
    );

    tracing_info!("gpui-component theme synchronized with Script Kit");
}

// ============================================================================
// End gpui-component Theme Integration
// ============================================================================

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

        assert_eq!(
            deserialized.colors.background.main,
            theme.colors.background.main
        );
        assert_eq!(deserialized.colors.text.primary, theme.colors.text.primary);
        assert_eq!(
            deserialized.colors.accent.selected,
            theme.colors.accent.selected
        );
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
            fonts: Some(FontConfig::default()),
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
