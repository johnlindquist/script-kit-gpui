//! Reusable PromptHeader component for GPUI Script Kit
//!
//! This module provides a theme-aware header component used across all prompt types.
//! It includes a search input with blinking cursor, action buttons, and logo.
//!
//! # Example
//! ```ignore
//! let colors = PromptHeaderColors::from_theme(&theme);
//! let config = PromptHeaderConfig::new()
//!     .placeholder("Search scripts...")
//!     .primary_button_label("Run")
//!     .primary_button_shortcut("↵");
//!     
//! PromptHeader::new(config, colors)
//!     .on_primary_click(Box::new(|_, _, _| println!("Run clicked!")))
//!     .on_actions_click(Box::new(|_, _, _| println!("Actions clicked!")))
//! ```

#![allow(dead_code)]

use gpui::*;
use std::rc::Rc;

use crate::components::{Button, ButtonColors, ButtonVariant};
use crate::designs::DesignColors;
use crate::theme::Theme;

/// Height of the cursor in the main filter input
const CURSOR_HEIGHT_LG: f32 = 20.0;
/// Vertical margin for cursor
const CURSOR_MARGIN_Y: f32 = 2.0;
/// Cursor width
const CURSOR_WIDTH: f32 = 2.0;

/// Pre-computed colors for PromptHeader rendering
///
/// This struct holds the primitive color values needed for header rendering,
/// allowing efficient use in closures without cloning the full theme.
#[derive(Clone, Copy, Debug)]
pub struct PromptHeaderColors {
    /// Main text color (for typed input)
    pub text_primary: u32,
    /// Placeholder/muted text color
    pub text_muted: u32,
    /// Separator and dimmed text color
    pub text_dimmed: u32,
    /// Accent color for logo and buttons
    pub accent: u32,
    /// Background color (usually transparent for header)
    pub background: u32,
    /// Search box background color
    pub search_box_bg: u32,
    /// Border color
    pub border: u32,
}

impl PromptHeaderColors {
    /// Create PromptHeaderColors from theme reference
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            text_primary: theme.colors.text.primary,
            text_muted: theme.colors.text.muted,
            text_dimmed: theme.colors.text.dimmed,
            accent: theme.colors.accent.selected,
            background: theme.colors.background.main,
            search_box_bg: theme.colors.background.search_box,
            border: theme.colors.ui.border,
        }
    }

    /// Create PromptHeaderColors from design colors for design system support
    pub fn from_design(colors: &DesignColors) -> Self {
        Self {
            text_primary: colors.text_primary,
            text_muted: colors.text_muted,
            text_dimmed: colors.text_dimmed,
            accent: colors.accent,
            background: colors.background,
            search_box_bg: colors.background_secondary,
            border: colors.border,
        }
    }
}

impl Default for PromptHeaderColors {
    fn default() -> Self {
        Self {
            text_primary: 0xffffff,
            text_muted: 0x808080,
            text_dimmed: 0x666666,
            accent: 0xfbbf24, // Script Kit yellow/gold
            background: 0x1e1e1e,
            search_box_bg: 0x2d2d30,
            border: 0x464647,
        }
    }
}

/// Configuration for PromptHeader display
#[derive(Clone, Debug)]
pub struct PromptHeaderConfig {
    /// Current input text
    pub filter_text: String,
    /// Placeholder shown when input is empty
    pub placeholder: String,
    /// Optional path prefix displayed before filter (e.g., "/Users/john/")
    pub path_prefix: Option<String>,
    /// Label for the primary button (e.g., "Run", "Select")
    pub primary_button_label: String,
    /// Shortcut hint for primary button (e.g., "↵")
    pub primary_button_shortcut: String,
    /// Whether to show the Actions button
    pub show_actions_button: bool,
    /// Whether the cursor is currently visible (for blinking)
    pub cursor_visible: bool,
    /// When true, show actions search input instead of buttons
    pub actions_mode: bool,
    /// Actions search text (when in actions_mode)
    pub actions_search_text: String,
    /// Whether the input is focused
    pub is_focused: bool,
}

impl Default for PromptHeaderConfig {
    fn default() -> Self {
        Self {
            filter_text: String::new(),
            placeholder: "Type to search...".to_string(),
            path_prefix: None,
            primary_button_label: "Run".to_string(),
            primary_button_shortcut: "↵".to_string(),
            show_actions_button: true,
            cursor_visible: true,
            actions_mode: false,
            actions_search_text: String::new(),
            is_focused: true,
        }
    }
}

impl PromptHeaderConfig {
    /// Create a new default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the filter text
    pub fn filter_text(mut self, text: impl Into<String>) -> Self {
        self.filter_text = text.into();
        self
    }

    /// Set the placeholder text
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Set the path prefix
    pub fn path_prefix(mut self, prefix: Option<String>) -> Self {
        self.path_prefix = prefix;
        self
    }

    /// Set the primary button label
    pub fn primary_button_label(mut self, label: impl Into<String>) -> Self {
        self.primary_button_label = label.into();
        self
    }

    /// Set the primary button shortcut
    pub fn primary_button_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.primary_button_shortcut = shortcut.into();
        self
    }

    /// Set whether to show the actions button
    pub fn show_actions_button(mut self, show: bool) -> Self {
        self.show_actions_button = show;
        self
    }

    /// Set cursor visibility
    pub fn cursor_visible(mut self, visible: bool) -> Self {
        self.cursor_visible = visible;
        self
    }

    /// Set actions mode
    pub fn actions_mode(mut self, mode: bool) -> Self {
        self.actions_mode = mode;
        self
    }

    /// Set actions search text
    pub fn actions_search_text(mut self, text: impl Into<String>) -> Self {
        self.actions_search_text = text.into();
        self
    }

    /// Set whether the input is focused
    pub fn focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }
}

/// Callback type for button click events
pub type HeaderClickCallback = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A reusable header component for prompts
///
/// Displays:
/// - Search input with blinking cursor
/// - Path prefix (optional)
/// - Primary action button (Run/Select)
/// - Actions button (optional)
/// - Script Kit logo
///
/// # Example
/// ```ignore
/// let colors = PromptHeaderColors::from_theme(&theme);
/// let config = PromptHeaderConfig::new()
///     .placeholder("Search...")
///     .primary_button_label("Run");
///
/// PromptHeader::new(config, colors)
///     .on_primary_click(Box::new(|_, _, _| { /* handle run */ }))
/// ```
#[derive(IntoElement)]
pub struct PromptHeader {
    config: PromptHeaderConfig,
    colors: PromptHeaderColors,
    on_primary_click: Option<Rc<HeaderClickCallback>>,
    on_actions_click: Option<Rc<HeaderClickCallback>>,
}

impl PromptHeader {
    /// Create a new PromptHeader with the given configuration and colors
    pub fn new(config: PromptHeaderConfig, colors: PromptHeaderColors) -> Self {
        Self {
            config,
            colors,
            on_primary_click: None,
            on_actions_click: None,
        }
    }

    /// Set the primary button click callback
    pub fn on_primary_click(mut self, callback: HeaderClickCallback) -> Self {
        self.on_primary_click = Some(Rc::new(callback));
        self
    }

    /// Set the actions button click callback
    pub fn on_actions_click(mut self, callback: HeaderClickCallback) -> Self {
        self.on_actions_click = Some(Rc::new(callback));
        self
    }

    /// Render the search input area with cursor
    fn render_input_area(&self) -> impl IntoElement {
        let colors = self.colors;
        let filter_is_empty = self.config.filter_text.is_empty();
        let cursor_visible = self.config.cursor_visible && self.config.is_focused;

        // Display text: filter text or placeholder
        let display_text: SharedString = if filter_is_empty {
            self.config.placeholder.clone().into()
        } else {
            self.config.filter_text.clone().into()
        };

        // Text color: muted for placeholder, primary for input
        let text_color = if filter_is_empty {
            rgb(colors.text_muted)
        } else {
            rgb(colors.text_primary)
        };

        // Build input container
        let mut input = div()
            .flex_1()
            .flex()
            .flex_row()
            .items_center()
            .text_lg()
            .text_color(text_color);

        // Path prefix (if present)
        if let Some(ref prefix) = self.config.path_prefix {
            input = input.child(
                div()
                    .text_color(rgb(colors.text_muted))
                    .child(prefix.clone()),
            );
        }

        // Cursor position:
        // - When empty: cursor LEFT (before placeholder)
        // - When typing: cursor RIGHT (after text)

        // Left cursor (when empty)
        // Use conditional background instead of .when() to avoid type inference issues
        if filter_is_empty {
            let cursor_bg = if cursor_visible {
                rgb(colors.text_primary)
            } else {
                rgba(0x00000000)
            };
            input = input.child(
                div()
                    .w(px(CURSOR_WIDTH))
                    .h(px(CURSOR_HEIGHT_LG))
                    .my(px(CURSOR_MARGIN_Y))
                    .mr(px(4.))
                    .bg(cursor_bg),
            );
        }

        // Display text
        input = input.child(display_text);

        // Right cursor (when not empty)
        if !filter_is_empty {
            let cursor_bg = if cursor_visible {
                rgb(colors.text_primary)
            } else {
                rgba(0x00000000)
            };
            input = input.child(
                div()
                    .w(px(CURSOR_WIDTH))
                    .h(px(CURSOR_HEIGHT_LG))
                    .my(px(CURSOR_MARGIN_Y))
                    .ml(px(2.))
                    .bg(cursor_bg),
            );
        }

        input
    }

    /// Render the action buttons area (Run + Actions)
    fn render_buttons_area(&self) -> impl IntoElement {
        let colors = self.colors;
        let button_colors = ButtonColors {
            text_color: colors.accent,
            text_hover: colors.text_primary,
            background: colors.background,
            background_hover: colors.background,
            accent: colors.accent,
            border: colors.border,
        };

        let on_primary = self.on_primary_click.clone();
        let on_actions = self.on_actions_click.clone();

        let mut container = div().flex().flex_row().items_center().justify_end();

        // Primary button
        let mut primary_btn = Button::new(self.config.primary_button_label.clone(), button_colors)
            .variant(ButtonVariant::Ghost)
            .shortcut(self.config.primary_button_shortcut.clone());

        if let Some(callback) = on_primary {
            primary_btn = primary_btn.on_click(Box::new(move |event, window, cx| {
                tracing::debug!("Primary button callback invoked");
                callback(event, window, cx);
            }));
        }
        container = container.child(primary_btn);
        container = container.child(
            div()
                .mx(px(4.))
                .text_color(rgba((colors.text_dimmed << 8) | 0x60))
                .text_sm()
                .child("|"),
        );

        // Actions button (if enabled)
        if self.config.show_actions_button {
            let mut actions_btn = Button::new("Actions", button_colors)
                .variant(ButtonVariant::Ghost)
                .shortcut("⌘ K");

            if let Some(callback) = on_actions {
                actions_btn = actions_btn.on_click(Box::new(move |event, window, cx| {
                    tracing::debug!("Actions button callback invoked");
                    callback(event, window, cx);
                }));
            }

            container = container.child(actions_btn);
            container = container.child(
                div()
                    .mx(px(4.))
                    .text_color(rgba((colors.text_dimmed << 8) | 0x60))
                    .text_sm()
                    .child("|"),
            );
        }

        container
    }

    /// Render the actions search input (when in actions_mode)
    fn render_actions_search(&self) -> impl IntoElement {
        let colors = self.colors;
        let search_is_empty = self.config.actions_search_text.is_empty();
        let cursor_visible = self.config.cursor_visible && self.config.is_focused;

        let search_display: SharedString = if search_is_empty {
            "Search actions...".into()
        } else {
            self.config.actions_search_text.clone().into()
        };

        // Compute cursor background color
        let cursor_bg = if cursor_visible {
            rgb(colors.accent)
        } else {
            rgba(0x00000000)
        };

        // Build the search input element
        let mut search_input = div()
            .flex_shrink_0()
            .w(px(130.))
            .min_w(px(130.))
            .max_w(px(130.))
            .h(px(24.))
            .min_h(px(24.))
            .max_h(px(24.))
            .overflow_hidden()
            .flex()
            .flex_row()
            .items_center()
            .px(px(8.))
            .rounded(px(4.))
            .bg(rgba(
                (colors.search_box_bg << 8) | if search_is_empty { 0x40 } else { 0x80 },
            ))
            .border_1()
            .border_color(rgba(
                (colors.accent << 8) | if search_is_empty { 0x20 } else { 0x40 },
            ))
            .text_sm()
            .text_color(if search_is_empty {
                rgb(colors.text_muted)
            } else {
                rgb(colors.text_primary)
            });

        // Cursor before placeholder when empty
        if search_is_empty {
            search_input = search_input.child(
                div()
                    .w(px(2.))
                    .h(px(14.))
                    .mr(px(2.))
                    .rounded(px(1.))
                    .bg(cursor_bg),
            );
        }

        search_input = search_input.child(search_display);

        // Cursor after text when not empty
        if !search_is_empty {
            search_input = search_input.child(
                div()
                    .w(px(2.))
                    .h(px(14.))
                    .ml(px(2.))
                    .rounded(px(1.))
                    .bg(cursor_bg),
            );
        }

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .gap(px(8.))
            // ⌘K indicator
            .child(
                div()
                    .text_color(rgb(colors.text_dimmed))
                    .text_xs()
                    .child("⌘K"),
            )
            // Search input display
            .child(search_input)
            .child(
                div()
                    .mx(px(4.))
                    .text_color(rgba((colors.text_dimmed << 8) | 0x60))
                    .text_sm()
                    .child("|"),
            )
    }

    /// Render the Script Kit logo
    fn render_logo(&self) -> impl IntoElement {
        svg()
            .external_path(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/logo.svg"))
            .size(px(16.))
            .text_color(rgb(self.colors.accent))
    }
}

impl RenderOnce for PromptHeader {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let actions_mode = self.config.actions_mode;

        // Pre-compute visibility styles for buttons and search layers
        // Use opacity and visibility for CLS-free toggling
        let (buttons_opacity, buttons_visible) = if actions_mode {
            (0., false)
        } else {
            (1., true)
        };
        let (search_opacity, search_visible) = if actions_mode {
            (1., true)
        } else {
            (0., false)
        };

        // Build buttons layer
        let mut buttons_layer = div()
            .absolute()
            .inset_0()
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .opacity(buttons_opacity);

        if !buttons_visible {
            buttons_layer = buttons_layer.invisible();
        }
        buttons_layer = buttons_layer.child(self.render_buttons_area());

        // Build search layer
        let mut search_layer = div()
            .absolute()
            .inset_0()
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .opacity(search_opacity);

        if !search_visible {
            search_layer = search_layer.invisible();
        }
        search_layer = search_layer.child(self.render_actions_search());

        div()
            .w_full()
            .px(px(16.))
            .py(px(8.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            // Search input area
            .child(self.render_input_area())
            // CLS-free actions area with stacked layers
            .child(
                div()
                    .relative()
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .child(buttons_layer)
                    .child(search_layer),
            )
            // Script Kit logo
            .child(self.render_logo())
    }
}

// Note: Tests omitted for this module due to GPUI macro recursion limit issues.
// The PromptHeader component is integration-tested via the main application's
// prompt rendering in main.rs.
//
// Verified traits:
// - PromptHeaderColors: Copy, Clone, Debug, Default
// - PromptHeaderConfig: Clone, Debug, Default + builder pattern
// - PromptHeader: builder pattern with .on_primary_click(), .on_actions_click()
