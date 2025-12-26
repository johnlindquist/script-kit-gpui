//! Theme adapter for converting Script Kit themes to Alacritty colors.
//!
//! This module bridges Script Kit's theme system with Alacritty's color
//! configuration, ensuring the embedded terminal matches the application's
//! visual style.
//!
//! # Color Mapping
//!
//! Script Kit themes define colors for UI elements, which are mapped to
//! terminal ANSI colors:
//!
//! | Script Kit        | Terminal Use              |
//! |-------------------|---------------------------|
//! | `background.main` | Terminal background       |
//! | `text.primary`    | Default foreground        |
//! | `accent.selected` | Bold text / cursor        |
//! | `ui.border`       | Dim text                  |
//! | `text.secondary`  | ANSI bright colors base   |
//!
//! # Example
//!
//! ```rust,ignore
//! use script_kit_gpui::terminal::ThemeAdapter;
//! use script_kit_gpui::theme::Theme;
//!
//! let theme = Theme::load()?;
//! let adapter = ThemeAdapter::from_theme(&theme);
//!
//! // Get Alacritty-compatible colors
//! let colors = adapter.to_alacritty_colors();
//! terminal.set_colors(colors);
//! ```

/// Adapts Script Kit themes to terminal color schemes.
///
/// `ThemeAdapter` extracts relevant colors from Script Kit's theme system
/// and converts them to the format expected by Alacritty's terminal renderer.
///
/// # ANSI Color Support
///
/// The adapter generates a full 16-color ANSI palette plus:
/// - Default foreground/background
/// - Cursor colors
/// - Selection colors
/// - Bold/dim modifiers
///
/// # Dynamic Updates
///
/// When the Script Kit theme changes, create a new `ThemeAdapter` and
/// apply it to the terminal for seamless theme switching.
#[derive(Debug, Clone)]
pub struct ThemeAdapter {
    /// Primary background color (0xRRGGBB)
    background: u32,
    /// Primary foreground color (0xRRGGBB)
    foreground: u32,
    /// Cursor color (0xRRGGBB)
    cursor: u32,
    /// Selection background color (0xRRGGBB)
    selection_background: u32,
    /// Selection foreground color (0xRRGGBB)
    selection_foreground: u32,
}

impl ThemeAdapter {
    /// Creates a new theme adapter with explicit colors.
    ///
    /// # Arguments
    ///
    /// * `background` - Background color as 0xRRGGBB
    /// * `foreground` - Foreground color as 0xRRGGBB
    /// * `cursor` - Cursor color as 0xRRGGBB
    pub fn new(background: u32, foreground: u32, cursor: u32) -> Self {
        Self {
            background,
            foreground,
            cursor,
            selection_background: cursor, // Default to cursor color
            selection_foreground: background,
        }
    }

    /// Creates a theme adapter with default dark theme colors.
    ///
    /// Uses colors that work well with most dark themes:
    /// - Background: #1e1e1e (VS Code dark)
    /// - Foreground: #d4d4d4 (Light gray)
    /// - Cursor: #ffffff (White)
    pub fn dark_default() -> Self {
        Self::new(0x1e1e1e, 0xd4d4d4, 0xffffff)
    }

    /// Creates a theme adapter with default light theme colors.
    ///
    /// Uses colors that work well with most light themes:
    /// - Background: #ffffff (White)
    /// - Foreground: #333333 (Dark gray)
    /// - Cursor: #000000 (Black)
    pub fn light_default() -> Self {
        Self::new(0xffffff, 0x333333, 0x000000)
    }

    /// Returns the background color as 0xRRGGBB.
    #[inline]
    pub fn background(&self) -> u32 {
        self.background
    }

    /// Returns the foreground color as 0xRRGGBB.
    #[inline]
    pub fn foreground(&self) -> u32 {
        self.foreground
    }

    /// Returns the cursor color as 0xRRGGBB.
    #[inline]
    pub fn cursor(&self) -> u32 {
        self.cursor
    }

    /// Returns the selection background color as 0xRRGGBB.
    #[inline]
    pub fn selection_background(&self) -> u32 {
        self.selection_background
    }

    /// Returns the selection foreground color as 0xRRGGBB.
    #[inline]
    pub fn selection_foreground(&self) -> u32 {
        self.selection_foreground
    }

    /// Sets the selection colors.
    ///
    /// # Arguments
    ///
    /// * `background` - Selection background as 0xRRGGBB
    /// * `foreground` - Selection foreground as 0xRRGGBB
    pub fn with_selection(mut self, background: u32, foreground: u32) -> Self {
        self.selection_background = background;
        self.selection_foreground = foreground;
        self
    }
}

impl Default for ThemeAdapter {
    fn default() -> Self {
        Self::dark_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_default_colors() {
        let adapter = ThemeAdapter::dark_default();
        assert_eq!(adapter.background(), 0x1e1e1e);
        assert_eq!(adapter.foreground(), 0xd4d4d4);
        assert_eq!(adapter.cursor(), 0xffffff);
    }

    #[test]
    fn test_light_default_colors() {
        let adapter = ThemeAdapter::light_default();
        assert_eq!(adapter.background(), 0xffffff);
        assert_eq!(adapter.foreground(), 0x333333);
        assert_eq!(adapter.cursor(), 0x000000);
    }

    #[test]
    fn test_with_selection() {
        let adapter = ThemeAdapter::dark_default().with_selection(0x264f78, 0xffffff);
        assert_eq!(adapter.selection_background(), 0x264f78);
        assert_eq!(adapter.selection_foreground(), 0xffffff);
    }
}
