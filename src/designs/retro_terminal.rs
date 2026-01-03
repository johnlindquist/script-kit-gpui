#![allow(dead_code)]
//! Retro Terminal Design
//!
//! A classic green-on-black CRT terminal aesthetic with:
//! - Phosphor green text (#00ff00) on black background
//! - Monospace font (Menlo)
//! - Scanline effect via alternating subtle darker rows
//! - ASCII box characters for borders
//! - Blinking block cursor
//! - `>_` prompt prefix
//! - UPPERCASE text for names
//! - Text glow effect using box-shadow
//! - Inverted colors for selected items

use gpui::*;

use super::{DesignRenderer, DesignVariant};
use crate::scripts::SearchResult;

/// Fixed height for terminal list items (dense terminal feel)
pub const TERMINAL_ITEM_HEIGHT: f32 = 28.0;

/// Phosphor green color (classic CRT green)
const PHOSPHOR_GREEN: u32 = 0x00ff00;

/// CRT black background
const CRT_BLACK: u32 = 0x000000;

/// Dimmed green for less prominent elements
const DIM_GREEN: u32 = 0x00aa00;

/// Very dim green for scanlines/borders
const SCANLINE_GREEN: u32 = 0x003300;

/// Pre-computed colors for terminal rendering
#[derive(Clone, Copy)]
pub struct TerminalColors {
    pub phosphor: u32,
    pub background: u32,
    pub dim: u32,
    pub scanline: u32,
}

impl Default for TerminalColors {
    fn default() -> Self {
        Self {
            phosphor: PHOSPHOR_GREEN,
            background: CRT_BLACK,
            dim: DIM_GREEN,
            scanline: SCANLINE_GREEN,
        }
    }
}

/// Retro Terminal design renderer
///
/// Implements a classic CRT terminal aesthetic with green phosphor text,
/// scanline effects, and ASCII box drawing characters.
pub struct RetroTerminalRenderer {
    colors: TerminalColors,
}

impl RetroTerminalRenderer {
    /// Create a new retro terminal renderer with default colors
    pub fn new() -> Self {
        Self {
            colors: TerminalColors::default(),
        }
    }

    /// Render a single terminal list item
    pub fn render_item(
        &self,
        result: &SearchResult,
        index: usize,
        is_selected: bool,
    ) -> impl IntoElement {
        let colors = self.colors;

        // Get name and convert to UPPERCASE for terminal aesthetic
        let name = result.name().to_uppercase();

        // Terminal-style item prefix
        let prefix = if is_selected { "> " } else { "  " };

        // Build the display text
        let display_text = format!("{}{}", prefix, name);

        // Determine colors based on selection (inverted when selected)
        let (text_color, bg_color) = if is_selected {
            (rgb(colors.background), rgb(colors.phosphor)) // Inverted: black on green
        } else {
            (rgb(colors.phosphor), rgb(colors.background)) // Normal: green on black
        };

        // Scanline effect: slightly darker background on odd rows
        let row_bg = if !is_selected && index % 2 == 1 {
            rgba((colors.scanline << 8) | 0x40) // Very subtle darker stripe
        } else {
            bg_color
        };

        // Create glow shadow for selected items
        let shadows = if is_selected {
            vec![BoxShadow {
                color: hsla(120.0 / 360.0, 1.0, 0.5, 0.6), // Green glow
                offset: point(px(0.), px(0.)),
                blur_radius: px(8.),
                spread_radius: px(0.),
            }]
        } else {
            vec![]
        };

        div()
            .id(ElementId::NamedInteger(
                "terminal-item".into(),
                index as u64,
            ))
            .w_full()
            .h(px(TERMINAL_ITEM_HEIGHT))
            .px(px(8.))
            .flex()
            .items_center()
            .bg(row_bg)
            .font_family("Menlo")
            .text_sm()
            .text_color(text_color)
            .shadow(shadows)
            .child(display_text)
    }

    /// Render the search input with terminal prompt style
    pub fn render_search_input(&self, filter_text: &str, cursor_visible: bool) -> impl IntoElement {
        let colors = self.colors;

        // Terminal prompt: >_
        let prompt = ">_ ";

        // Build input display with blinking cursor
        let cursor = if cursor_visible { "█" } else { " " };
        let display_text = format!("{}{}{}", prompt, filter_text.to_uppercase(), cursor);

        div()
            .w_full()
            .px(px(8.))
            .py(px(8.))
            .bg(rgb(colors.background))
            .border_b_1()
            .border_color(rgb(colors.dim))
            .font_family("Menlo")
            .text_sm()
            .text_color(rgb(colors.phosphor))
            .shadow(vec![BoxShadow {
                color: hsla(120.0 / 360.0, 1.0, 0.5, 0.3), // Subtle green glow
                offset: point(px(0.), px(0.)),
                blur_radius: px(4.),
                spread_radius: px(0.),
            }])
            .child(display_text)
    }

    /// Render the terminal header with ASCII box characters
    pub fn render_header(&self) -> impl IntoElement {
        let colors = self.colors;

        // ASCII box top border: ┌────────────────────────────┐
        let border_line = "┌────────────────────────────────────────┐";
        let title_line = "│           SCRIPT-KIT TERMINAL          │";

        div()
            .w_full()
            .flex()
            .flex_col()
            .bg(rgb(colors.background))
            .font_family("Menlo")
            .text_xs()
            .text_color(rgb(colors.dim))
            .child(div().px(px(8.)).child(border_line))
            .child(
                div()
                    .px(px(8.))
                    .text_color(rgb(colors.phosphor))
                    .child(title_line),
            )
            .child(
                div()
                    .px(px(8.))
                    .child("├────────────────────────────────────────┤"),
            )
    }

    /// Render the terminal footer with ASCII box characters
    pub fn render_footer(&self, item_count: usize) -> impl IntoElement {
        let colors = self.colors;

        let status = format!("│ {} ITEMS LOADED                         │", item_count);
        let border_bottom = "└────────────────────────────────────────┘";

        div()
            .w_full()
            .flex()
            .flex_col()
            .bg(rgb(colors.background))
            .font_family("Menlo")
            .text_xs()
            .text_color(rgb(colors.dim))
            .child(
                div()
                    .px(px(8.))
                    .child("├────────────────────────────────────────┤"),
            )
            .child(div().px(px(8.)).child(status))
            .child(div().px(px(8.)).child(border_bottom))
    }

    /// Render empty state message
    pub fn render_empty_state(&self, filter_text: &str) -> impl IntoElement {
        let colors = self.colors;

        let message = if filter_text.is_empty() {
            "NO SCRIPTS FOUND".to_string()
        } else {
            format!("NO MATCH FOR '{}'", filter_text.to_uppercase())
        };

        div()
            .w_full()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(colors.background))
            .font_family("Menlo")
            .text_sm()
            .text_color(rgb(colors.dim))
            .child(message)
    }
}

impl Default for RetroTerminalRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl<App> DesignRenderer<App> for RetroTerminalRenderer
where
    App: 'static,
{
    fn render_script_list(&self, _app: &App, _cx: &mut Context<App>) -> AnyElement {
        // Note: This is a placeholder implementation.
        // The actual integration requires access to app state (scripts, filter, selected_index).
        // For now, we return an empty terminal container.
        // The real implementation will be wired up when the main app integrates custom renderers.

        let colors = self.colors;

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(colors.background))
            .font_family("Menlo")
            .child(self.render_header())
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(colors.dim))
                    .child("INITIALIZING..."),
            )
            .child(self.render_footer(0))
            .into_any_element()
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::RetroTerminal
    }
}

/// Create a retro terminal renderer instance
pub fn create_renderer() -> RetroTerminalRenderer {
    RetroTerminalRenderer::new()
}

// ============================================================================
// Standalone Render Helper Functions
// ============================================================================

/// Terminal window container configuration
///
/// Returns styling properties for the terminal window wrapper.
/// Use this to apply consistent terminal aesthetic to the main container.
#[derive(Debug, Clone, Copy)]
pub struct TerminalWindowConfig {
    /// Background color (CRT black)
    pub background: u32,
    /// Border color (dim green)
    pub border: u32,
    /// Border width in pixels
    pub border_width: f32,
    /// Font family for all terminal text
    pub font_family: &'static str,
    /// Whether to show the CRT glow effect
    pub glow_enabled: bool,
    /// Glow color (phosphor green with alpha)
    pub glow_color: Hsla,
    /// Glow blur radius
    pub glow_blur: f32,
}

impl Default for TerminalWindowConfig {
    fn default() -> Self {
        Self {
            background: 0x0a0a0a, // Slightly off-black for CRT feel
            border: DIM_GREEN,
            border_width: 1.0,
            font_family: "Menlo",
            glow_enabled: true,
            glow_color: hsla(120.0 / 360.0, 1.0, 0.5, 0.15), // Subtle green glow
            glow_blur: 20.0,
        }
    }
}

/// Returns terminal window container configuration with CRT styling
///
/// Use this to wrap your main terminal UI with consistent styling:
/// - Black background (0x0a0a0a)
/// - Dim green border
/// - Monospace font (Menlo/SF Mono)
/// - Optional CRT glow effect
///
pub fn render_terminal_window_container() -> TerminalWindowConfig {
    TerminalWindowConfig::default()
}

/// Render the terminal header/search bar with command prompt style
///
/// Displays a classic terminal prompt with `>_` prefix.
/// Shows filter text in UPPERCASE with optional blinking block cursor.
///
/// # Arguments
///
/// * `filter_text` - Current search/filter text
/// * `cursor_visible` - Whether the blinking cursor should be visible
/// * `colors` - Terminal color scheme
///
/// # Returns
///
/// A styled div element representing the terminal command prompt
pub fn render_terminal_header(
    filter_text: &str,
    cursor_visible: bool,
    colors: TerminalColors,
) -> impl IntoElement {
    // Terminal prompt: >_
    let prompt = ">_ ";

    // Build input display with blinking block cursor
    let cursor = if cursor_visible { "█" } else { " " };
    let display_text = format!("{}{}{}", prompt, filter_text.to_uppercase(), cursor);

    // Create green glow shadow for the header
    let glow_shadows = vec![BoxShadow {
        color: hsla(120.0 / 360.0, 1.0, 0.5, 0.3), // Subtle green glow
        offset: point(px(0.), px(0.)),
        blur_radius: px(4.),
        spread_radius: px(0.),
    }];

    div()
        .w_full()
        .px(px(8.))
        .py(px(8.))
        .bg(rgb(colors.background))
        .border_b_1()
        .border_color(rgb(colors.dim))
        .font_family("Menlo")
        .text_sm()
        .text_color(rgb(colors.phosphor))
        .shadow(glow_shadows)
        .child(display_text)
}

/// Render the terminal preview panel for code/content display
///
/// Displays content with classic terminal aesthetics:
/// - Green phosphor text on black background
/// - Monospace font
/// - Optional line numbers
/// - CRT-style glow effect
///
/// # Arguments
///
/// * `content` - The text content to display (can be code, text, etc.)
/// * `colors` - Terminal color scheme
///
/// # Returns
///
/// A styled div element representing the preview panel
pub fn render_terminal_preview_panel(content: &str, colors: TerminalColors) -> impl IntoElement {
    // Split content into lines for rendering
    let lines: Vec<&str> = content.lines().collect();

    // Create glow effect for the panel
    let panel_glow = vec![BoxShadow {
        color: hsla(120.0 / 360.0, 1.0, 0.5, 0.1), // Very subtle green glow
        offset: point(px(0.), px(0.)),
        blur_radius: px(12.),
        spread_radius: px(0.),
    }];

    div()
        .w_full()
        .h_full()
        .flex()
        .flex_col()
        .bg(rgb(colors.background))
        .border_l_1()
        .border_color(rgb(colors.scanline))
        .font_family("Menlo")
        .text_xs()
        .shadow(panel_glow)
        .child(
            // Header bar
            div()
                .w_full()
                .px(px(8.))
                .py(px(4.))
                .border_b_1()
                .border_color(rgb(colors.scanline))
                .text_color(rgb(colors.dim))
                .child("┌─ PREVIEW ─────────────────────────────┐"),
        )
        .child(
            // Content area with line numbers
            div()
                .flex_1()
                .w_full()
                .overflow_hidden()
                .px(px(8.))
                .py(px(4.))
                .children(lines.into_iter().enumerate().map(|(line_num, line)| {
                    // Line number + content
                    let line_prefix = format!("{:4} │ ", line_num + 1);
                    div()
                        .w_full()
                        .flex()
                        .flex_row()
                        .child(
                            // Line number (dim)
                            div().text_color(rgb(colors.scanline)).child(line_prefix),
                        )
                        .child(
                            // Line content (bright green)
                            div()
                                .text_color(rgb(colors.phosphor))
                                .child(line.to_string()),
                        )
                })),
        )
        .child(
            // Footer bar
            div()
                .w_full()
                .px(px(8.))
                .py(px(4.))
                .border_t_1()
                .border_color(rgb(colors.scanline))
                .text_color(rgb(colors.dim))
                .child("└────────────────────────────────────────┘"),
        )
}

/// Render the terminal log panel
///
/// Displays log entries with classic terminal aesthetics:
/// - Green text on black background
/// - Monospace font throughout
/// - Alternating row colors for scanline effect
/// - Log level indicators (INFO, WARN, ERR)
///
/// # Arguments
///
/// * `logs` - Vector of log entry strings
/// * `colors` - Terminal color scheme
///
/// # Returns
///
/// A styled div element representing the log panel
pub fn render_terminal_log_panel(logs: &[String], colors: TerminalColors) -> impl IntoElement {
    // Create glow effect for the panel
    let panel_glow = vec![BoxShadow {
        color: hsla(120.0 / 360.0, 1.0, 0.5, 0.08), // Very subtle green glow
        offset: point(px(0.), px(0.)),
        blur_radius: px(8.),
        spread_radius: px(0.),
    }];

    div()
        .w_full()
        .flex()
        .flex_col()
        .bg(rgb(colors.background))
        .border_t_1()
        .border_color(rgb(colors.dim))
        .font_family("Menlo")
        .text_xs()
        .shadow(panel_glow)
        .child(
            // Header bar
            div()
                .w_full()
                .px(px(8.))
                .py(px(2.))
                .border_b_1()
                .border_color(rgb(colors.scanline))
                .text_color(rgb(colors.dim))
                .child("─── LOG OUTPUT ───────────────────────────"),
        )
        .child(
            // Log entries
            div()
                .w_full()
                .overflow_hidden()
                .max_h(px(150.))
                .children(logs.iter().enumerate().map(|(index, log_entry)| {
                    // Determine log level and color from content
                    let (level_indicator, text_color) = if log_entry.contains("[ERR]")
                        || log_entry.contains("ERROR")
                        || log_entry.contains("error")
                    {
                        ("█", rgb(0xff4444)) // Red for errors
                    } else if log_entry.contains("[WARN]")
                        || log_entry.contains("WARNING")
                        || log_entry.contains("warn")
                    {
                        ("▒", rgb(0xffff00)) // Yellow for warnings
                    } else {
                        ("░", rgb(colors.phosphor)) // Green for info
                    };

                    // Scanline effect: slightly darker on odd rows
                    let row_bg = if index % 2 == 1 {
                        rgba((colors.scanline << 8) | 0x20)
                    } else {
                        rgb(colors.background)
                    };

                    div()
                        .w_full()
                        .px(px(8.))
                        .py(px(1.))
                        .bg(row_bg)
                        .flex()
                        .flex_row()
                        .gap(px(4.))
                        .child(
                            // Level indicator
                            div().text_color(text_color).child(level_indicator),
                        )
                        .child(
                            // Log content
                            div()
                                .flex_1()
                                .text_color(text_color)
                                .overflow_hidden()
                                .child(log_entry.clone()),
                        )
                })),
        )
}

/// Render an empty terminal state with retro messaging
pub fn render_terminal_empty_state(message: &str, colors: TerminalColors) -> impl IntoElement {
    let display_message = message.to_uppercase();

    div()
        .w_full()
        .h_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .bg(rgb(colors.background))
        .font_family("Menlo")
        .text_sm()
        .gap(px(8.))
        .child(
            div()
                .text_color(rgb(colors.dim))
                .child("┌────────────────────────────┐"),
        )
        .child(
            div()
                .text_color(rgb(colors.phosphor))
                .child(format!("│  {}  │", display_message)),
        )
        .child(
            div()
                .text_color(rgb(colors.dim))
                .child("└────────────────────────────┘"),
        )
}

/// Terminal list rendering helper
///
/// Renders a list of search results in full terminal style.
/// Use this with uniform_list for virtualized rendering.
pub fn render_terminal_list(
    results: &[SearchResult],
    selected_index: usize,
    colors: TerminalColors,
) -> impl IntoElement {
    let renderer = RetroTerminalRenderer::new();

    div()
        .w_full()
        .h_full()
        .bg(rgb(colors.background))
        .flex()
        .flex_col()
        .font_family("Menlo")
        .children(results.iter().enumerate().map(|(index, result)| {
            let is_selected = index == selected_index;
            renderer.render_item(result, index, is_selected)
        }))
}

/// Get terminal design constants for external use
pub struct TerminalConstants;

impl TerminalConstants {
    /// Item height for terminal list (dense: 28px)
    pub const fn item_height() -> f32 {
        TERMINAL_ITEM_HEIGHT
    }

    /// Phosphor green color constant
    pub const fn phosphor_green() -> u32 {
        PHOSPHOR_GREEN
    }

    /// CRT black background
    pub const fn crt_black() -> u32 {
        CRT_BLACK
    }

    /// Dim green for secondary elements
    pub const fn dim_green() -> u32 {
        DIM_GREEN
    }

    /// Glow green color (brighter than phosphor for glow effects)
    pub const fn glow_green() -> u32 {
        0x33ff33
    }
}

// Note: Tests omitted due to GPUI macro recursion limit issues.
// TerminalColors defaults:
// - phosphor: 0x00ff00 (bright green)
// - background: 0x000000 (black)
// - dim: 0x00aa00 (dim green)
// - scanline: 0x003300 (very dim green)
// TERMINAL_ITEM_HEIGHT = 28.0 (dense terminal feel)
