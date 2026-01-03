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
//! | Script Kit                    | Terminal Use              |
//! |-------------------------------|---------------------------|
//! | `background.main`             | Terminal background       |
//! | `text.primary`                | Default foreground        |
//! | `accent.selected`             | Cursor                    |
//! | `accent.selected_subtle`      | Selection background      |
//! | `text.secondary`              | Selection foreground      |
//!
//! # Focus-Aware Colors
//!
//! When the window is unfocused, colors are dimmed by blending toward gray
//! to provide visual feedback that the terminal is not active.
//!

use vte::ansi::Rgb;

use crate::theme::Theme;

/// Standard ANSI colors - used as fallback/base for the 16-color palette.
///
/// These colors follow the standard ANSI color naming convention:
/// - Colors 0-7: Normal (black, red, green, yellow, blue, magenta, cyan, white)
/// - Colors 8-15: Bright variants of the above
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnsiColors {
    /// ANSI 0: Black
    pub black: Rgb,
    /// ANSI 1: Red
    pub red: Rgb,
    /// ANSI 2: Green
    pub green: Rgb,
    /// ANSI 3: Yellow
    pub yellow: Rgb,
    /// ANSI 4: Blue
    pub blue: Rgb,
    /// ANSI 5: Magenta
    pub magenta: Rgb,
    /// ANSI 6: Cyan
    pub cyan: Rgb,
    /// ANSI 7: White
    pub white: Rgb,
    /// ANSI 8: Bright Black (Gray)
    pub bright_black: Rgb,
    /// ANSI 9: Bright Red
    pub bright_red: Rgb,
    /// ANSI 10: Bright Green
    pub bright_green: Rgb,
    /// ANSI 11: Bright Yellow
    pub bright_yellow: Rgb,
    /// ANSI 12: Bright Blue
    pub bright_blue: Rgb,
    /// ANSI 13: Bright Magenta
    pub bright_magenta: Rgb,
    /// ANSI 14: Bright Cyan
    pub bright_cyan: Rgb,
    /// ANSI 15: Bright White
    pub bright_white: Rgb,
}

impl Default for AnsiColors {
    /// Default ANSI colors matching common dark terminal themes.
    fn default() -> Self {
        Self {
            // Normal colors (0-7)
            black: hex_to_rgb(0x000000),
            red: hex_to_rgb(0xcd3131),
            green: hex_to_rgb(0x0dbc79),
            yellow: hex_to_rgb(0xe5e510),
            blue: hex_to_rgb(0x2472c8),
            magenta: hex_to_rgb(0xbc3fbc),
            cyan: hex_to_rgb(0x11a8cd),
            white: hex_to_rgb(0xe5e5e5),
            // Bright colors (8-15)
            bright_black: hex_to_rgb(0x666666),
            bright_red: hex_to_rgb(0xf14c4c),
            bright_green: hex_to_rgb(0x23d18b),
            bright_yellow: hex_to_rgb(0xf5f543),
            bright_blue: hex_to_rgb(0x3b8eea),
            bright_magenta: hex_to_rgb(0xd670d6),
            bright_cyan: hex_to_rgb(0x29b8db),
            bright_white: hex_to_rgb(0xffffff),
        }
    }
}

impl AnsiColors {
    /// Get an ANSI color by index (0-15).
    ///
    /// # Arguments
    ///
    /// * `index` - ANSI color index (0-15)
    ///
    /// # Returns
    ///
    /// The corresponding RGB color, or black if index is out of range.
    pub fn get(&self, index: u8) -> Rgb {
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
            _ => self.black, // Fallback for out-of-range indices
        }
    }

    /// Apply dimming factor to all colors for unfocused state.
    fn dimmed(&self, factor: f32) -> Self {
        Self {
            black: dim_color(self.black, factor),
            red: dim_color(self.red, factor),
            green: dim_color(self.green, factor),
            yellow: dim_color(self.yellow, factor),
            blue: dim_color(self.blue, factor),
            magenta: dim_color(self.magenta, factor),
            cyan: dim_color(self.cyan, factor),
            white: dim_color(self.white, factor),
            bright_black: dim_color(self.bright_black, factor),
            bright_red: dim_color(self.bright_red, factor),
            bright_green: dim_color(self.bright_green, factor),
            bright_yellow: dim_color(self.bright_yellow, factor),
            bright_blue: dim_color(self.bright_blue, factor),
            bright_magenta: dim_color(self.bright_magenta, factor),
            bright_cyan: dim_color(self.bright_cyan, factor),
            bright_white: dim_color(self.bright_white, factor),
        }
    }
}

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
///
/// # Focus-Aware Colors
///
/// When the window loses focus, call [`update_for_focus`](ThemeAdapter::update_for_focus)
/// to dim the colors and provide visual feedback.
///
/// # Dynamic Updates
///
/// When the Script Kit theme changes, create a new `ThemeAdapter` and
/// apply it to the terminal for seamless theme switching.
#[derive(Debug, Clone)]
pub struct ThemeAdapter {
    /// Foreground text color
    foreground: Rgb,
    /// Background color
    background: Rgb,
    /// Cursor color
    cursor: Rgb,
    /// Selection background color
    selection_background: Rgb,
    /// Selection foreground color
    selection_foreground: Rgb,
    /// The 16 ANSI colors
    ansi: AnsiColors,
    /// Whether the window is currently focused
    is_focused: bool,
    /// Original colors before focus dimming (for restoration)
    original_foreground: Rgb,
    original_background: Rgb,
    original_cursor: Rgb,
    original_selection_background: Rgb,
    original_selection_foreground: Rgb,
    original_ansi: AnsiColors,
}

impl ThemeAdapter {
    /// Creates a theme adapter from a Script Kit theme.
    ///
    /// Maps theme colors to terminal colors:
    /// - `theme.colors.text.primary` → foreground
    /// - `theme.colors.background.main` → background
    /// - `theme.colors.accent.selected` → cursor
    /// - `theme.colors.accent.selected_subtle` → selection background
    /// - `theme.colors.text.secondary` → selection foreground
    /// - `theme.colors.terminal.*` → ANSI color palette (all 16 colors)
    ///
    /// All 16 ANSI colors are now fully themeable via `theme.colors.terminal`.
    pub fn from_theme(theme: &Theme) -> Self {
        let colors = &theme.colors;

        let foreground = hex_to_rgb(colors.text.primary);
        let background = hex_to_rgb(colors.background.main);
        let cursor = hex_to_rgb(colors.accent.selected);
        let selection_background = hex_to_rgb(colors.accent.selected_subtle);
        let selection_foreground = hex_to_rgb(colors.text.secondary);

        // Use all 16 ANSI colors from theme's terminal palette
        let terminal = &colors.terminal;
        let ansi = AnsiColors {
            black: hex_to_rgb(terminal.black),
            red: hex_to_rgb(terminal.red),
            green: hex_to_rgb(terminal.green),
            yellow: hex_to_rgb(terminal.yellow),
            blue: hex_to_rgb(terminal.blue),
            magenta: hex_to_rgb(terminal.magenta),
            cyan: hex_to_rgb(terminal.cyan),
            white: hex_to_rgb(terminal.white),
            bright_black: hex_to_rgb(terminal.bright_black),
            bright_red: hex_to_rgb(terminal.bright_red),
            bright_green: hex_to_rgb(terminal.bright_green),
            bright_yellow: hex_to_rgb(terminal.bright_yellow),
            bright_blue: hex_to_rgb(terminal.bright_blue),
            bright_magenta: hex_to_rgb(terminal.bright_magenta),
            bright_cyan: hex_to_rgb(terminal.bright_cyan),
            bright_white: hex_to_rgb(terminal.bright_white),
        };

        Self {
            foreground,
            background,
            cursor,
            selection_background,
            selection_foreground,
            ansi,
            is_focused: true,
            original_foreground: foreground,
            original_background: background,
            original_cursor: cursor,
            original_selection_background: selection_background,
            original_selection_foreground: selection_foreground,
            original_ansi: ansi,
        }
    }

    /// Creates a theme adapter with sensible dark defaults.
    ///
    /// Uses colors that work well with most dark themes:
    /// - Background: #1e1e1e (VS Code dark)
    /// - Foreground: #d4d4d4 (Light gray)
    /// - Cursor: #ffffff (White)
    pub fn dark_default() -> Self {
        let foreground = hex_to_rgb(0xd4d4d4);
        let background = hex_to_rgb(0x1e1e1e);
        let cursor = hex_to_rgb(0xffffff);
        let selection_background = hex_to_rgb(0x264f78);
        let selection_foreground = hex_to_rgb(0xffffff);
        let ansi = AnsiColors::default();

        Self {
            foreground,
            background,
            cursor,
            selection_background,
            selection_foreground,
            ansi,
            is_focused: true,
            original_foreground: foreground,
            original_background: background,
            original_cursor: cursor,
            original_selection_background: selection_background,
            original_selection_foreground: selection_foreground,
            original_ansi: ansi,
        }
    }

    /// Returns the foreground text color.
    #[inline]
    pub fn foreground(&self) -> Rgb {
        self.foreground
    }

    /// Returns the background color.
    #[inline]
    pub fn background(&self) -> Rgb {
        self.background
    }

    /// Returns the cursor color.
    #[inline]
    pub fn cursor(&self) -> Rgb {
        self.cursor
    }

    /// Returns the selection background color.
    #[inline]
    pub fn selection_background(&self) -> Rgb {
        self.selection_background
    }

    /// Returns the selection foreground color.
    #[inline]
    pub fn selection_foreground(&self) -> Rgb {
        self.selection_foreground
    }

    /// Returns an ANSI color by index (0-15).
    ///
    /// # Arguments
    ///
    /// * `index` - ANSI color index (0-15)
    ///
    /// # Returns
    ///
    /// The RGB color for the given index. Returns black for out-of-range indices.
    #[inline]
    pub fn ansi_color(&self, index: u8) -> Rgb {
        self.ansi.get(index)
    }

    /// Returns whether the adapter is in focused state.
    #[inline]
    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    /// Updates colors based on window focus state.
    ///
    /// When the window loses focus, colors are dimmed by blending toward
    /// gray to provide visual feedback that the terminal is not active.
    ///
    /// # Arguments
    ///
    /// * `is_focused` - Whether the window is currently focused
    ///
    pub fn update_for_focus(&mut self, is_focused: bool) {
        if self.is_focused == is_focused {
            return; // No change needed
        }

        self.is_focused = is_focused;

        if is_focused {
            // Restore original colors
            self.foreground = self.original_foreground;
            self.background = self.original_background;
            self.cursor = self.original_cursor;
            self.selection_background = self.original_selection_background;
            self.selection_foreground = self.original_selection_foreground;
            self.ansi = self.original_ansi;
        } else {
            // Dim colors by blending toward gray (30% blend factor)
            const DIM_FACTOR: f32 = 0.7;

            self.foreground = dim_color(self.original_foreground, DIM_FACTOR);
            self.background = dim_color(self.original_background, DIM_FACTOR);
            self.cursor = dim_color(self.original_cursor, DIM_FACTOR);
            self.selection_background = dim_color(self.original_selection_background, DIM_FACTOR);
            self.selection_foreground = dim_color(self.original_selection_foreground, DIM_FACTOR);
            self.ansi = self.original_ansi.dimmed(DIM_FACTOR);
        }
    }
}

impl Default for ThemeAdapter {
    fn default() -> Self {
        Self::dark_default()
    }
}

/// Converts a u32 hex color (0xRRGGBB) to an Rgb struct.
///
/// # Arguments
///
/// * `hex` - Color as 0xRRGGBB
///
/// # Returns
///
/// An `Rgb` struct with the extracted red, green, and blue components.
///
#[inline]
pub fn hex_to_rgb(hex: u32) -> Rgb {
    Rgb {
        r: ((hex >> 16) & 0xFF) as u8,
        g: ((hex >> 8) & 0xFF) as u8,
        b: (hex & 0xFF) as u8,
    }
}

/// Dims a color by blending it toward mid-gray.
///
/// # Arguments
///
/// * `color` - The original color
/// * `factor` - Blend factor (0.0 = full gray, 1.0 = original color)
///
/// # Returns
///
/// The dimmed color blended toward gray.
fn dim_color(color: Rgb, factor: f32) -> Rgb {
    const GRAY: u8 = 0x80;

    let blend = |c: u8| -> u8 {
        let c = c as f32;
        let gray = GRAY as f32;
        ((c * factor + gray * (1.0 - factor)).clamp(0.0, 255.0)) as u8
    };

    Rgb {
        r: blend(color.r),
        g: blend(color.g),
        b: blend(color.b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{ColorScheme, Theme};

    // ========================================================================
    // hex_to_rgb Tests
    // ========================================================================

    #[test]
    fn test_hex_to_rgb_white() {
        let rgb = hex_to_rgb(0xffffff);
        assert_eq!(rgb.r, 255);
        assert_eq!(rgb.g, 255);
        assert_eq!(rgb.b, 255);
    }

    #[test]
    fn test_hex_to_rgb_black() {
        let rgb = hex_to_rgb(0x000000);
        assert_eq!(rgb.r, 0);
        assert_eq!(rgb.g, 0);
        assert_eq!(rgb.b, 0);
    }

    #[test]
    fn test_hex_to_rgb_red() {
        let rgb = hex_to_rgb(0xff0000);
        assert_eq!(rgb.r, 255);
        assert_eq!(rgb.g, 0);
        assert_eq!(rgb.b, 0);
    }

    #[test]
    fn test_hex_to_rgb_green() {
        let rgb = hex_to_rgb(0x00ff00);
        assert_eq!(rgb.r, 0);
        assert_eq!(rgb.g, 255);
        assert_eq!(rgb.b, 0);
    }

    #[test]
    fn test_hex_to_rgb_blue() {
        let rgb = hex_to_rgb(0x0000ff);
        assert_eq!(rgb.r, 0);
        assert_eq!(rgb.g, 0);
        assert_eq!(rgb.b, 255);
    }

    #[test]
    fn test_hex_to_rgb_vscode_dark_bg() {
        let rgb = hex_to_rgb(0x1e1e1e);
        assert_eq!(rgb.r, 0x1e);
        assert_eq!(rgb.g, 0x1e);
        assert_eq!(rgb.b, 0x1e);
    }

    // ========================================================================
    // AnsiColors Tests
    // ========================================================================

    #[test]
    fn test_ansi_colors_default() {
        let ansi = AnsiColors::default();
        // Black should be black
        assert_eq!(ansi.black, hex_to_rgb(0x000000));
        // Bright white should be white
        assert_eq!(ansi.bright_white, hex_to_rgb(0xffffff));
    }

    #[test]
    fn test_ansi_colors_get_normal_range() {
        let ansi = AnsiColors::default();
        assert_eq!(ansi.get(0), ansi.black);
        assert_eq!(ansi.get(1), ansi.red);
        assert_eq!(ansi.get(2), ansi.green);
        assert_eq!(ansi.get(3), ansi.yellow);
        assert_eq!(ansi.get(4), ansi.blue);
        assert_eq!(ansi.get(5), ansi.magenta);
        assert_eq!(ansi.get(6), ansi.cyan);
        assert_eq!(ansi.get(7), ansi.white);
    }

    #[test]
    fn test_ansi_colors_get_bright_range() {
        let ansi = AnsiColors::default();
        assert_eq!(ansi.get(8), ansi.bright_black);
        assert_eq!(ansi.get(9), ansi.bright_red);
        assert_eq!(ansi.get(10), ansi.bright_green);
        assert_eq!(ansi.get(11), ansi.bright_yellow);
        assert_eq!(ansi.get(12), ansi.bright_blue);
        assert_eq!(ansi.get(13), ansi.bright_magenta);
        assert_eq!(ansi.get(14), ansi.bright_cyan);
        assert_eq!(ansi.get(15), ansi.bright_white);
    }

    #[test]
    fn test_ansi_colors_get_out_of_range() {
        let ansi = AnsiColors::default();
        // Out of range should return black
        assert_eq!(ansi.get(16), ansi.black);
        assert_eq!(ansi.get(255), ansi.black);
    }

    #[test]
    fn test_ansi_colors_dimmed() {
        let ansi = AnsiColors::default();
        let dimmed = ansi.dimmed(0.5);

        // White (255, 255, 255) dimmed toward gray (128) at 0.5 should be ~191
        // (255 * 0.5 + 128 * 0.5 = 191.5)
        assert!(dimmed.bright_white.r < 255);
        assert!(dimmed.bright_white.r > 128);
    }

    // ========================================================================
    // ThemeAdapter Tests
    // ========================================================================

    #[test]
    fn test_dark_default_colors() {
        let adapter = ThemeAdapter::dark_default();
        assert_eq!(adapter.background(), hex_to_rgb(0x1e1e1e));
        assert_eq!(adapter.foreground(), hex_to_rgb(0xd4d4d4));
        assert_eq!(adapter.cursor(), hex_to_rgb(0xffffff));
    }

    #[test]
    fn test_dark_default_is_focused() {
        let adapter = ThemeAdapter::dark_default();
        assert!(adapter.is_focused());
    }

    #[test]
    fn test_from_theme_maps_colors() {
        let theme = Theme::default();
        let adapter = ThemeAdapter::from_theme(&theme);

        // Check that colors are mapped from theme
        assert_eq!(adapter.foreground(), hex_to_rgb(theme.colors.text.primary));
        assert_eq!(
            adapter.background(),
            hex_to_rgb(theme.colors.background.main)
        );
        assert_eq!(adapter.cursor(), hex_to_rgb(theme.colors.accent.selected));
        assert_eq!(
            adapter.selection_background(),
            hex_to_rgb(theme.colors.accent.selected_subtle)
        );
        assert_eq!(
            adapter.selection_foreground(),
            hex_to_rgb(theme.colors.text.secondary)
        );
    }

    #[test]
    fn test_from_theme_uses_terminal_colors_for_ansi() {
        let theme = Theme::default();
        let adapter = ThemeAdapter::from_theme(&theme);

        // ANSI colors are now mapped from theme.colors.terminal (not ui colors)
        // This provides full terminal color customization via theme.json
        assert_eq!(adapter.ansi_color(1), hex_to_rgb(theme.colors.terminal.red));
        assert_eq!(
            adapter.ansi_color(2),
            hex_to_rgb(theme.colors.terminal.green)
        );
        assert_eq!(
            adapter.ansi_color(3),
            hex_to_rgb(theme.colors.terminal.yellow)
        );
        assert_eq!(
            adapter.ansi_color(4),
            hex_to_rgb(theme.colors.terminal.blue)
        );
    }

    #[test]
    fn test_ansi_color_returns_correct_colors() {
        let adapter = ThemeAdapter::dark_default();

        // Verify we can access all 16 ANSI colors
        for i in 0..16 {
            let _color = adapter.ansi_color(i);
            // Just verify no panic
        }
    }

    #[test]
    fn test_update_for_focus_dims_colors() {
        let mut adapter = ThemeAdapter::dark_default();
        let original_fg = adapter.foreground();

        // Lose focus
        adapter.update_for_focus(false);

        // Colors should be dimmed (moved toward gray)
        let dimmed_fg = adapter.foreground();
        assert_ne!(original_fg, dimmed_fg);
        assert!(!adapter.is_focused());
    }

    #[test]
    fn test_update_for_focus_restores_colors() {
        let mut adapter = ThemeAdapter::dark_default();
        let original_fg = adapter.foreground();
        let original_bg = adapter.background();

        // Lose focus
        adapter.update_for_focus(false);

        // Regain focus
        adapter.update_for_focus(true);

        // Colors should be restored
        assert_eq!(adapter.foreground(), original_fg);
        assert_eq!(adapter.background(), original_bg);
        assert!(adapter.is_focused());
    }

    #[test]
    fn test_update_for_focus_noop_when_unchanged() {
        let mut adapter = ThemeAdapter::dark_default();
        let original_fg = adapter.foreground();

        // Call with same state (focused)
        adapter.update_for_focus(true);

        // Should be unchanged
        assert_eq!(adapter.foreground(), original_fg);
    }

    #[test]
    fn test_update_for_focus_dims_ansi_colors() {
        let mut adapter = ThemeAdapter::dark_default();
        let original_red = adapter.ansi_color(1);

        // Lose focus
        adapter.update_for_focus(false);

        // ANSI colors should also be dimmed
        let dimmed_red = adapter.ansi_color(1);
        assert_ne!(original_red, dimmed_red);
    }

    #[test]
    fn test_default_is_dark_default() {
        let default_adapter = ThemeAdapter::default();
        let dark_adapter = ThemeAdapter::dark_default();

        assert_eq!(default_adapter.foreground(), dark_adapter.foreground());
        assert_eq!(default_adapter.background(), dark_adapter.background());
        assert_eq!(default_adapter.cursor(), dark_adapter.cursor());
    }

    // ========================================================================
    // dim_color Tests
    // ========================================================================

    #[test]
    fn test_dim_color_full_gray() {
        let white = Rgb {
            r: 255,
            g: 255,
            b: 255,
        };
        let dimmed = dim_color(white, 0.0);
        // Should be mid-gray
        assert_eq!(dimmed.r, 0x80);
        assert_eq!(dimmed.g, 0x80);
        assert_eq!(dimmed.b, 0x80);
    }

    #[test]
    fn test_dim_color_no_change() {
        let color = Rgb {
            r: 100,
            g: 150,
            b: 200,
        };
        let dimmed = dim_color(color, 1.0);
        // Should be unchanged
        assert_eq!(dimmed.r, 100);
        assert_eq!(dimmed.g, 150);
        assert_eq!(dimmed.b, 200);
    }

    #[test]
    fn test_dim_color_half_blend() {
        let white = Rgb {
            r: 255,
            g: 255,
            b: 255,
        };
        let dimmed = dim_color(white, 0.5);
        // Should be between white (255) and gray (128)
        // (255 * 0.5 + 128 * 0.5) = 191.5 ≈ 191
        assert!((dimmed.r as i32 - 191).abs() <= 1);
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    #[test]
    fn test_light_theme_adapter() {
        let theme = Theme {
            colors: ColorScheme::light_default(),
            ..Default::default()
        };
        let adapter = ThemeAdapter::from_theme(&theme);

        // Light theme should have light background
        assert_eq!(
            adapter.background(),
            hex_to_rgb(theme.colors.background.main)
        );
        // And dark foreground
        assert_eq!(adapter.foreground(), hex_to_rgb(theme.colors.text.primary));
    }

    #[test]
    fn test_focus_cycle() {
        let mut adapter = ThemeAdapter::dark_default();

        // Start focused
        assert!(adapter.is_focused());

        // Cycle through focus states
        adapter.update_for_focus(false);
        assert!(!adapter.is_focused());

        adapter.update_for_focus(true);
        assert!(adapter.is_focused());

        adapter.update_for_focus(false);
        assert!(!adapter.is_focused());

        adapter.update_for_focus(true);
        assert!(adapter.is_focused());
    }
}
