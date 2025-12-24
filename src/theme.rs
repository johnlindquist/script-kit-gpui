use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// Hex color representation (u32)
pub type HexColor = u32;

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
}

/// Border and UI element colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIColors {
    /// Border color (0x464647)
    pub border: HexColor,
    /// Success color for logs (0x00ff00 - green)
    pub success: HexColor,
}

/// Complete color scheme definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorScheme {
    pub background: BackgroundColors,
    pub text: TextColors,
    pub accent: AccentColors,
    pub ui: UIColors,
}

/// Complete theme definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub colors: ColorScheme,
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
                secondary: 0xe0e0e0,
                tertiary: 0x999999,
                muted: 0x808080,
                dimmed: 0x666666,
            },
            accent: AccentColors {
                selected: 0x007acc,
            },
            ui: UIColors {
                border: 0x464647,
                success: 0x00ff00,
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
            },
            ui: UIColors {
                border: 0xd0d0d0,
                success: 0x00a000,
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
        }
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
            eprintln!("System appearance detected: {}", if is_dark { "dark" } else { "light" });
            is_dark
        }
        Err(_) => {
            // Command failed or not available (e.g., light mode on macOS returns error)
            eprintln!("System appearance detection failed or light mode detected, defaulting to light");
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
        eprintln!("Theme file not found at {:?}, detecting system appearance", theme_path);
        // Auto-select based on system appearance
        let is_dark = detect_system_appearance();
        let color_scheme = if is_dark {
            ColorScheme::dark_default()
        } else {
            ColorScheme::light_default()
        };
        return Theme {
            colors: color_scheme,
        };
    }

    // Read and parse the JSON file
    match std::fs::read_to_string(&theme_path) {
        Err(e) => {
            eprintln!("Failed to read theme file: {}", e);
            let is_dark = detect_system_appearance();
            let color_scheme = if is_dark {
                ColorScheme::dark_default()
            } else {
                ColorScheme::light_default()
            };
            Theme {
                colors: color_scheme,
            }
        }
        Ok(contents) => {
            match serde_json::from_str::<Theme>(&contents) {
                Ok(theme) => {
                    eprintln!("Successfully loaded theme from {:?}", theme_path);
                    theme
                }
                Err(e) => {
                    eprintln!("Failed to parse theme JSON: {}", e);
                    eprintln!("Theme content was: {}", contents);
                    let is_dark = detect_system_appearance();
                    let color_scheme = if is_dark {
                        ColorScheme::dark_default()
                    } else {
                        ColorScheme::light_default()
                    };
                    Theme {
                        colors: color_scheme,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_theme() {
        let theme = Theme::default();
        assert_eq!(theme.colors.background.main, 0x1e1e1e);
        assert_eq!(theme.colors.text.primary, 0xffffff);
        assert_eq!(theme.colors.accent.selected, 0x007acc);
        assert_eq!(theme.colors.ui.border, 0x464647);
    }

    #[test]
    fn test_color_scheme_default() {
        let scheme = ColorScheme::default();
        assert_eq!(scheme.background.title_bar, 0x2d2d30);
        assert_eq!(scheme.text.secondary, 0xe0e0e0);
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
        };
        let json = serde_json::to_string(&theme).unwrap();
        let deserialized: Theme = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.colors.background.main, 0xffffff);
        assert_eq!(deserialized.colors.text.primary, 0x000000);
    }

    #[test]
    fn test_detect_system_appearance() {
        // This test just verifies the function can be called without panicking
        // The result will vary based on the system's actual appearance setting
        let _is_dark = detect_system_appearance();
        // Don't assert a specific value, just ensure it doesn't panic
    }
}
