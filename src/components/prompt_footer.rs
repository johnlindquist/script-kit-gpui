//! PromptFooter - Reusable footer component for prompts
//!
//! This module provides a theme-aware footer component used across all prompt types.
//! It includes a Script Kit logo, optional helper text, primary action button, divider,
//! and secondary action button.
//!
//! # Structure
//! ```text
//! | [Logo] [Helper Text] |              | [Info] | [Primary ↵] | [Secondary ⌘K] |
//! ```
//!
//! # Example
//! ```rust,ignore
//! let footer = PromptFooter::new(
//!     PromptFooterConfig::new()
//!         .primary_label("Run Script")
//!         .primary_shortcut("↵")
//!         .secondary_label("Actions")
//!         .secondary_shortcut("⌘K"),
//!     PromptFooterColors::from_theme(&theme),
//! )
//! .on_primary_click(Box::new(|_, _, _| { /* handle run */ }))
//! .on_secondary_click(Box::new(|_, _, _| { /* handle actions */ }));
//! ```

#![allow(dead_code)]

use gpui::*;
use gpui_component::tooltip::Tooltip;
use std::rc::Rc;

use crate::components::footer_chrome::{
    footer_key_glyph_nudge_y, is_footer_return_key_glyph, split_footer_shortcut,
    FOOTER_KEYCAP_HEIGHT_PX, FOOTER_KEYCAP_PADDING_X_PX, FOOTER_KEYCAP_RADIUS_PX,
    FOOTER_KEY_GLYPH_NUDGE_Y_PX, FOOTER_RETURN_GLYPH_NUDGE_Y_PX,
};
use crate::designs::DesignColors;
use crate::theme::Theme;
use crate::ui::chrome::alpha_from_opacity;
use crate::ui_foundation::{hstack, HexColorExt};
use crate::window_resize::layout::FOOTER_HEIGHT;

/// Helper text width cap to preserve room for footer actions.
pub const PROMPT_FOOTER_HELPER_TEXT_MAX_WIDTH_PX: f32 = 420.0;
/// Info label width cap so long labels do not crowd footer actions.
pub const PROMPT_FOOTER_INFO_TEXT_MAX_WIDTH_PX: f32 = 220.0;
/// Shared horizontal spacing between footer sections.
const PROMPT_FOOTER_SECTION_GAP_PX: f32 = 8.0;
/// Shared horizontal spacing between footer buttons/divider.
const PROMPT_FOOTER_BUTTON_GAP_PX: f32 = 4.0;
/// Active/pressed opacity for footer action buttons.
pub(crate) const PROMPT_FOOTER_BUTTON_ACTIVE_OPACITY: u8 = 0x3a;
/// Footer button font size delta from base UI font size.
const PROMPT_FOOTER_BUTTON_FONT_DELTA_PX: f32 = 2.0;
/// Minimum footer button font size.
const PROMPT_FOOTER_BUTTON_FONT_MIN_PX: f32 = 10.0;
/// Footer horizontal padding.
const PROMPT_FOOTER_PADDING_X_PX: f32 = 12.0;
/// Optical bottom padding to align footer content vertically.
const PROMPT_FOOTER_PADDING_BOTTOM_PX: f32 = 2.0;
/// Footer logo icon size.
const PROMPT_FOOTER_LOGO_SIZE_PX: f32 = 16.0;
/// Small optical nudge so the logo appears centered with adjacent text.
const PROMPT_FOOTER_LOGO_NUDGE_X_PX: f32 = 2.0;
/// Divider width between footer buttons.
const PROMPT_FOOTER_DIVIDER_WIDTH_PX: f32 = 1.0;
/// Divider height between footer buttons.
const PROMPT_FOOTER_DIVIDER_HEIGHT_PX: f32 = 16.0;
/// Footer top-border opacity for contrast on light/dark surfaces.
const PROMPT_FOOTER_BORDER_OPACITY: u8 = 0x50;
/// Footer shadow Y-offset.
const PROMPT_FOOTER_SHADOW_OFFSET_Y_PX: f32 = -1.0;
/// Footer shadow blur radius.
const PROMPT_FOOTER_SHADOW_BLUR_PX: f32 = 8.0;
/// Info label font size delta from base UI font size.
const PROMPT_FOOTER_INFO_FONT_DELTA_PX: f32 = 4.0;
/// Minimum info label font size.
const PROMPT_FOOTER_INFO_FONT_MIN_PX: f32 = 9.0;
/// Helper label font size delta from base UI font size.
const PROMPT_FOOTER_HELPER_FONT_DELTA_PX: f32 = 2.0;
/// Minimum helper label font size.
const PROMPT_FOOTER_HELPER_FONT_MIN_PX: f32 = 10.0;
/// Pre-computed colors for PromptFooter rendering
///
/// This struct holds the primitive color values needed for footer rendering,
/// allowing efficient use in closures without cloning the full theme.
#[derive(Clone, Copy, Debug)]
pub struct PromptFooterColors {
    /// Accent color for logo background and primary text
    pub accent: u32,
    /// Muted text color for shortcut hints
    pub text_muted: u32,
    /// Border color for top border and divider
    pub border: u32,
    /// Surface color for the footer container background
    pub surface: u32,
    /// Background color for footer surface
    pub background: u32,
    /// Hover overlay alpha derived from the active theme.
    pub hover_alpha: u8,
    /// Primary text color for focused/active overlays (matches selection style).
    pub text_primary: u32,
    /// Selected opacity alpha (0–255) for active/toggle states.
    pub selected_alpha: u8,
    /// Whether we're in light mode (affects opacity)
    pub is_light_mode: bool,
}

impl PromptFooterColors {
    /// Create PromptFooterColors from theme reference
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            accent: theme.colors.accent.selected,
            text_muted: theme.colors.text.muted,
            border: theme.colors.ui.border,
            surface: theme.colors.background.main,
            // Match selected item surface token for footer consistency.
            background: theme.colors.accent.selected_subtle,
            hover_alpha: alpha_from_opacity(theme.get_opacity().hover) as u8,
            text_primary: theme.colors.text.primary,
            selected_alpha: alpha_from_opacity(theme.get_opacity().selected) as u8,
            is_light_mode: !theme.is_dark_mode(),
        }
    }

    /// Create PromptFooterColors from design colors.
    ///
    /// Prompt footers should stay visually aligned with the app shell, so
    /// this path intentionally resolves to the cached theme tokens.
    pub fn from_design(_colors: &DesignColors) -> Self {
        Self::from_theme(&crate::theme::get_cached_theme())
    }
}

impl Default for PromptFooterColors {
    fn default() -> Self {
        Self::from_theme(&crate::theme::get_cached_theme())
    }
}

/// Resolve footer surface color with mode-specific opacity.
pub fn footer_surface_rgba(colors: PromptFooterColors) -> u32 {
    if colors.is_light_mode {
        // Light mode uses the theme's base surface token as an opaque footer background.
        (colors.surface << 8) | 0xff
    } else {
        // Dark mode: use selected_subtle as a subtle overlay (~12% opacity).
        (colors.background << 8) | 0x1f
    }
}

fn footer_shadow_alpha(colors: PromptFooterColors) -> u8 {
    if colors.is_light_mode {
        0x28
    } else {
        0x50
    }
}

fn is_footer_button_clickable(has_click_handler: bool, disabled: bool) -> bool {
    has_click_handler && !disabled
}

fn is_footer_button_activation_key(key: &str) -> bool {
    matches!(
        key,
        "enter" | "return" | "Enter" | "Return" | " " | "space" | "Space"
    )
}

fn return_key_glyph_nudge_y(configured: Option<f32>) -> f32 {
    configured.unwrap_or(FOOTER_KEY_GLYPH_NUDGE_Y_PX + FOOTER_RETURN_GLYPH_NUDGE_Y_PX)
}

fn shortcut_key_glyph_nudge_y(key: &str, configured_return: Option<f32>) -> f32 {
    if is_footer_return_key_glyph(key) {
        return_key_glyph_nudge_y(configured_return)
    } else {
        footer_key_glyph_nudge_y(key)
    }
}

fn footer_button_hover_rgba(colors: PromptFooterColors) -> u32 {
    // Canonical hover pill: primary-text overlay at theme hover opacity,
    // matching the native footer buttons (not a background-tinted pill).
    (colors.text_primary << 8) | (colors.hover_alpha as u32)
}

fn footer_button_active_rgba(colors: PromptFooterColors) -> u32 {
    (colors.text_primary << 8) | (colors.selected_alpha as u32)
}

/// Configuration for PromptFooter display
#[derive(Clone, Debug)]
pub struct PromptFooterConfig {
    /// Label for the primary button (e.g., "Run Script", "Submit", "Paste")
    pub primary_label: String,
    /// Shortcut hint for primary button (e.g., "↵", "⌘+Enter")
    pub primary_shortcut: String,
    /// Label for the secondary button (e.g., "Actions")
    pub secondary_label: String,
    /// Shortcut hint for secondary button (e.g., "⌘K")
    pub secondary_shortcut: String,
    /// Whether to show the Script Kit logo
    pub show_logo: bool,
    /// Whether to show the secondary button
    pub show_secondary: bool,
    /// Whether to show the primary button
    pub show_primary: bool,
    /// Whether to show the info label
    pub show_info_label: bool,
    /// Disable interactions on the primary button
    pub primary_disabled: bool,
    /// Disable interactions on the secondary button
    pub secondary_disabled: bool,
    /// Whether the secondary button is in a pressed/active toggle state
    pub secondary_active: bool,
    /// Optional helper text shown next to logo (e.g., "Tab 1 of 2 · Tab to continue")
    pub helper_text: Option<String>,
    /// Optional info label shown before buttons (e.g., "typescript", "5 items")
    pub info_label: Option<String>,
    /// Optional font family override for the keyboard shortcut glyphs
    pub shortcut_font_family: Option<SharedString>,
    /// Optional optical Y nudge for the return glyph inside shortcut keycaps.
    pub shortcut_return_glyph_nudge_y: Option<f32>,
}

impl Default for PromptFooterConfig {
    fn default() -> Self {
        Self {
            primary_label: "Run Script".to_string(),
            primary_shortcut: "↵".to_string(),
            secondary_label: "Actions".to_string(),
            secondary_shortcut: "⌘K".to_string(),
            show_logo: true,
            show_secondary: true,
            show_primary: true,
            show_info_label: true,
            primary_disabled: false,
            secondary_disabled: false,
            secondary_active: false,
            helper_text: None,
            info_label: None,
            shortcut_font_family: None,
            shortcut_return_glyph_nudge_y: None,
        }
    }
}

impl PromptFooterConfig {
    /// Create a new default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the primary button label
    pub fn primary_label(mut self, label: impl Into<String>) -> Self {
        self.primary_label = label.into();
        self
    }

    /// Set the primary button shortcut hint
    pub fn primary_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.primary_shortcut = shortcut.into();
        self
    }

    /// Set the secondary button label
    pub fn secondary_label(mut self, label: impl Into<String>) -> Self {
        self.secondary_label = label.into();
        self
    }

    /// Set the secondary button shortcut hint
    pub fn secondary_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.secondary_shortcut = shortcut.into();
        self
    }

    /// Set whether to show the logo
    pub fn show_logo(mut self, show: bool) -> Self {
        self.show_logo = show;
        self
    }

    /// Set whether to show the secondary button
    pub fn show_secondary(mut self, show: bool) -> Self {
        self.show_secondary = show;
        self
    }

    /// Set whether to show the primary button
    pub fn show_primary(mut self, show: bool) -> Self {
        self.show_primary = show;
        self
    }

    /// Set whether to show the info label
    pub fn show_info_label(mut self, show: bool) -> Self {
        self.show_info_label = show;
        self
    }

    /// Set whether the primary button is disabled
    pub fn primary_disabled(mut self, disabled: bool) -> Self {
        self.primary_disabled = disabled;
        self
    }

    /// Set whether the secondary button is disabled
    pub fn secondary_disabled(mut self, disabled: bool) -> Self {
        self.secondary_disabled = disabled;
        self
    }

    /// Set whether the secondary button is in a pressed/active toggle state
    pub fn secondary_active(mut self, active: bool) -> Self {
        self.secondary_active = active;
        self
    }

    /// Set optional helper text shown next to the logo
    pub fn helper_text(mut self, text: impl Into<String>) -> Self {
        self.helper_text = Some(text.into());
        self
    }

    /// Set optional info label shown before buttons (e.g., language indicator)
    pub fn info_label(mut self, label: impl Into<String>) -> Self {
        self.info_label = Some(label.into());
        self
    }

    /// Set optional font family override for the keyboard shortcut glyphs
    pub fn shortcut_font_family(mut self, font_family: impl Into<SharedString>) -> Self {
        self.shortcut_font_family = Some(font_family.into());
        self
    }

    /// Set optional return glyph Y nudge for visual comparison/experiments.
    pub fn shortcut_return_glyph_nudge_y(mut self, nudge_y: f32) -> Self {
        self.shortcut_return_glyph_nudge_y = Some(nudge_y);
        self
    }
}

/// Callback type for button click events
pub type FooterClickCallback = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A reusable footer component for prompts
///
/// Displays:
/// - Script Kit logo (left, optional)
/// - Primary action button with shortcut
/// - Divider (optional, when secondary visible)
/// - Secondary action button with shortcut (optional)
///
/// Height: 40px fixed
#[derive(IntoElement)]
pub struct PromptFooter {
    config: PromptFooterConfig,
    colors: PromptFooterColors,
    on_primary_click: Option<Rc<FooterClickCallback>>,
    on_secondary_click: Option<Rc<FooterClickCallback>>,
    left_slot: Option<AnyElement>,
    right_slot: Option<AnyElement>,
}

impl PromptFooter {
    /// Create a new PromptFooter with the given configuration and colors
    pub fn new(config: PromptFooterConfig, colors: PromptFooterColors) -> Self {
        Self {
            config,
            colors,
            on_primary_click: None,
            on_secondary_click: None,
            left_slot: None,
            right_slot: None,
        }
    }

    /// Set the primary button click callback
    pub fn on_primary_click(mut self, callback: FooterClickCallback) -> Self {
        self.on_primary_click = Some(Rc::new(callback));
        self
    }

    /// Set the secondary button click callback
    pub fn on_secondary_click(mut self, callback: FooterClickCallback) -> Self {
        self.on_secondary_click = Some(Rc::new(callback));
        self
    }

    /// Insert custom content on the left side after logo/helper text.
    pub fn left_slot(mut self, slot: impl IntoElement) -> Self {
        self.left_slot = Some(slot.into_any_element());
        self
    }

    /// Insert optional custom content on the left side after logo/helper text.
    pub fn left_slot_opt(mut self, slot: Option<AnyElement>) -> Self {
        self.left_slot = slot;
        self
    }

    /// Insert custom content on the right side between info label and buttons.
    pub fn right_slot(mut self, slot: impl IntoElement) -> Self {
        self.right_slot = Some(slot.into_any_element());
        self
    }

    /// Insert optional custom content on the right side between info label and buttons.
    pub fn right_slot_opt(mut self, slot: Option<AnyElement>) -> Self {
        self.right_slot = slot;
        self
    }

    /// Render the Script Kit logo (accent-colored icon, no background)
    fn render_logo(&self) -> impl IntoElement {
        svg()
            .path("logo.svg")
            .size(px(PROMPT_FOOTER_LOGO_SIZE_PX))
            .ml(px(PROMPT_FOOTER_LOGO_NUDGE_X_PX))
            .text_color(rgb(self.colors.accent)) // Accent color (yellow/gold)
    }

    /// Render a footer button with label and shortcut
    fn render_button(
        &self,
        id: &'static str,
        label: String,
        shortcut: String,
        disabled: bool,
        active: bool,
        on_click: Option<Rc<FooterClickCallback>>,
    ) -> impl IntoElement {
        let theme = crate::theme::get_cached_theme();
        let button_font_size = (theme.get_fonts().ui_size - PROMPT_FOOTER_BUTTON_FONT_DELTA_PX)
            .max(PROMPT_FOOTER_BUTTON_FONT_MIN_PX);
        let has_click_handler = on_click.is_some();
        let is_clickable = is_footer_button_clickable(has_click_handler, disabled);
        let on_click_for_key = on_click.clone();
        let hover_bg = rgba(footer_button_hover_rgba(self.colors));
        let active_bg = rgba(footer_button_active_rgba(self.colors));

        // Children must stay non-interactive: any hit-testable hover/cursor
        // on a nested element re-computes the parent's hover state when the
        // pointer crosses child boundaries, causing flicker. Cursor and hover
        // live on the parent only (matches Zed's ButtonLike pattern).
        let label_element = div()
            .text_size(px(button_font_size))
            .text_color(self.colors.accent.to_rgb())
            .child(label);

        let shortcut_keys = split_footer_shortcut(&shortcut);
        let shortcut_element =
            hstack()
                .gap(px(3.0))
                .items_center()
                .children(shortcut_keys.into_iter().map(|key| {
                    let glyph_nudge_y =
                        shortcut_key_glyph_nudge_y(&key, self.config.shortcut_return_glyph_nudge_y);
                    let key_content: AnyElement = div()
                        .h(px(FOOTER_KEYCAP_HEIGHT_PX))
                        .line_height(px(FOOTER_KEYCAP_HEIGHT_PX))
                        .mt(px(glyph_nudge_y))
                        .child(key)
                        .into_any_element();
                    let mut key_el = div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .px(px(FOOTER_KEYCAP_PADDING_X_PX))
                        .h(px(FOOTER_KEYCAP_HEIGHT_PX))
                        .min_h(px(FOOTER_KEYCAP_HEIGHT_PX))
                        .min_w(px(FOOTER_KEYCAP_HEIGHT_PX))
                        .line_height(px(FOOTER_KEYCAP_HEIGHT_PX))
                        .rounded(px(FOOTER_KEYCAP_RADIUS_PX))
                        .border_1()
                        .border_color(self.colors.border.rgba8(0x50))
                        .bg(self.colors.border.rgba8(0x15))
                        .text_size(px(button_font_size))
                        .text_color(self.colors.text_muted.to_rgb())
                        .child(key_content);
                    if let Some(ref font_family) = self.config.shortcut_font_family {
                        key_el = key_el.font_family(font_family.clone());
                    }
                    key_el
                }));

        let mut button = div()
            .id(ElementId::Name(id.into()))
            .flex()
            .flex_row()
            .items_center()
            .px(px(8.))
            .py(px(6.))
            .rounded(px(crate::ui::chrome::ACTION_BUTTON_RADIUS_PX))
            .child(label_element);

        if !shortcut.is_empty() {
            button = button.child(div().ml(px(6.0)).child(shortcut_element));
        }

        if active {
            button = button
                .bg(active_bg)
                .hover(move |s| s.bg(active_bg))
                .cursor_pointer();
        } else if is_clickable {
            button = button
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_bg));
        } else if disabled {
            button = button.opacity(0.5).cursor_default();
        } else {
            button = button.cursor_default();
        }

        if is_clickable {
            if let Some(callback) = on_click {
                button = button.on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    callback(event, window, cx);
                });
            }

            if let Some(callback) = on_click_for_key {
                button = button.on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let key = event.keystroke.key.as_str();
                    if is_footer_button_activation_key(key) {
                        let click_event = ClickEvent::default();
                        callback(&click_event, window, cx);
                    }
                });
            }
        }

        button
    }

    /// Render the vertical divider between buttons
    fn render_divider(&self) -> impl IntoElement {
        div()
            .w(px(PROMPT_FOOTER_DIVIDER_WIDTH_PX))
            .h(px(PROMPT_FOOTER_DIVIDER_HEIGHT_PX))
            .bg(self.colors.border.rgba8(0x40)) // 25% opacity
    }
}

impl RenderOnce for PromptFooter {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut this = self;
        let colors = this.colors;
        let theme = crate::theme::get_cached_theme();
        let ui_font_size = theme.get_fonts().ui_size;
        let info_font_size =
            (ui_font_size - PROMPT_FOOTER_INFO_FONT_DELTA_PX).max(PROMPT_FOOTER_INFO_FONT_MIN_PX);
        let helper_font_size = (ui_font_size - PROMPT_FOOTER_HELPER_FONT_DELTA_PX)
            .max(PROMPT_FOOTER_HELPER_FONT_MIN_PX);

        // Build the right-side container (info label + buttons)
        let mut right_side = hstack()
            .gap(px(PROMPT_FOOTER_SECTION_GAP_PX))
            .items_center()
            .min_w(px(0.));

        // Info label (e.g., "typescript", "5 items") - shown before buttons
        if this.config.show_info_label {
            if let Some(ref info) = this.config.info_label {
                let info_text = info.clone();
                right_side = right_side.child(
                    div()
                        .id(ElementId::Name(SharedString::from(
                            "prompt-footer-info-ellipsis",
                        )))
                        .max_w(px(PROMPT_FOOTER_INFO_TEXT_MAX_WIDTH_PX))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_size(px(info_font_size))
                        .text_color(colors.text_muted.to_rgb())
                        .tooltip(move |window, cx| {
                            Tooltip::new(info_text.clone()).build(window, cx)
                        })
                        .child(info.clone()),
                );
            }
        }

        if let Some(right_slot) = this.right_slot.take() {
            right_side = right_side.child(div().min_w(px(0.)).overflow_hidden().child(right_slot));
        }

        // Build the buttons container
        let mut buttons = hstack().gap(px(PROMPT_FOOTER_BUTTON_GAP_PX)).items_center();
        let show_primary_button = this.config.show_primary;

        // Primary button
        if show_primary_button {
            buttons = buttons.child(this.render_button(
                "footer-primary-button",
                this.config.primary_label.clone(),
                this.config.primary_shortcut.clone(),
                this.config.primary_disabled,
                false,
                this.on_primary_click.clone(),
            ));
        }

        // Divider + Secondary button (if enabled)
        if this.config.show_secondary {
            if show_primary_button {
                buttons = buttons.child(this.render_divider());
            }
            buttons = buttons.child(this.render_button(
                "footer-secondary-button",
                this.config.secondary_label.clone(),
                this.config.secondary_shortcut.clone(),
                this.config.secondary_disabled,
                this.config.secondary_active,
                this.on_secondary_click.clone(),
            ));
        }

        right_side = right_side.child(buttons);

        // Main footer container (uses FOOTER_HEIGHT constant for single source of truth)
        // Resolve from PromptFooterColors.background so color ownership stays within footer tokens.
        let footer_bg = rgba(footer_surface_rgba(colors));

        let mut footer = div()
            .w_full()
            .h(px(FOOTER_HEIGHT))
            .min_h(px(FOOTER_HEIGHT))
            .max_h(px(FOOTER_HEIGHT))
            .flex_shrink_0()
            .overflow_hidden()
            .px(px(PROMPT_FOOTER_PADDING_X_PX))
            .pt(px(0.))
            .pb(px(PROMPT_FOOTER_PADDING_BOTTOM_PX))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .border_t_1()
            .border_color(colors.border.rgba8(PROMPT_FOOTER_BORDER_OPACITY))
            .bg(footer_bg)
            // Inner shadow above the footer for visual separation from content
            // Footers are the ONE scenario where blocking vibrancy is OK
            .shadow(vec![BoxShadow {
                color: colors.border.rgba8(footer_shadow_alpha(colors)),
                offset: point(px(0.), px(PROMPT_FOOTER_SHADOW_OFFSET_Y_PX)),
                blur_radius: px(PROMPT_FOOTER_SHADOW_BLUR_PX),
                spread_radius: px(0.),
            }]);

        // Left side: Logo + helper text
        let mut left_side = hstack()
            .flex_1()
            .min_w(px(0.))
            .overflow_hidden()
            .gap(px(PROMPT_FOOTER_SECTION_GAP_PX))
            .items_center();

        // Logo (if enabled)
        if this.config.show_logo {
            left_side = left_side.child(this.render_logo());
        }

        // Helper text (e.g., "Tab 1 of 2 · Tab to continue, Esc to exit")
        if let Some(ref helper) = this.config.helper_text {
            let helper_text = helper.clone();
            left_side = left_side.child(
                div()
                    .id(ElementId::Name(SharedString::from(
                        "prompt-footer-helper-ellipsis",
                    )))
                    .max_w(px(PROMPT_FOOTER_HELPER_TEXT_MAX_WIDTH_PX))
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .text_size(px(helper_font_size))
                    .text_color(colors.accent.to_rgb())
                    .tooltip(move |window, cx| Tooltip::new(helper_text.clone()).build(window, cx))
                    .child(helper.clone()),
            );
        }

        if let Some(left_slot) = this.left_slot.take() {
            left_side = left_side.child(div().min_w(px(0.)).overflow_hidden().child(left_slot));
        }

        footer = footer.child(left_side);

        // Right: Info label + Buttons
        footer.child(right_side)
    }
}

// Note: Tests omitted for this module due to GPUI macro recursion limit issues.
// The PromptFooter component is integration-tested via the main application's
// prompt rendering in main.rs.
//
// Verified traits:
// - PromptFooterColors: Copy, Clone, Debug, Default
// - PromptFooterConfig: Clone, Debug, Default + builder pattern
// - PromptFooter: builder pattern with .on_primary_click(), .on_secondary_click()

#[cfg(test)]
mod tests {
    use super::{
        footer_surface_rgba, PromptFooterColors, PromptFooterConfig, PROMPT_FOOTER_BORDER_OPACITY,
        PROMPT_FOOTER_BUTTON_ACTIVE_OPACITY, PROMPT_FOOTER_BUTTON_FONT_DELTA_PX,
        PROMPT_FOOTER_BUTTON_FONT_MIN_PX, PROMPT_FOOTER_BUTTON_GAP_PX,
        PROMPT_FOOTER_DIVIDER_HEIGHT_PX, PROMPT_FOOTER_DIVIDER_WIDTH_PX,
        PROMPT_FOOTER_HELPER_FONT_DELTA_PX, PROMPT_FOOTER_HELPER_FONT_MIN_PX,
        PROMPT_FOOTER_INFO_FONT_DELTA_PX, PROMPT_FOOTER_INFO_FONT_MIN_PX,
        PROMPT_FOOTER_LOGO_NUDGE_X_PX, PROMPT_FOOTER_LOGO_SIZE_PX, PROMPT_FOOTER_PADDING_BOTTOM_PX,
        PROMPT_FOOTER_PADDING_X_PX, PROMPT_FOOTER_SECTION_GAP_PX, PROMPT_FOOTER_SHADOW_BLUR_PX,
        PROMPT_FOOTER_SHADOW_OFFSET_Y_PX,
    };
    use crate::components::footer_chrome::{
        is_footer_return_key_glyph, split_footer_shortcut, FOOTER_KEY_GLYPH_NUDGE_Y_PX,
        FOOTER_RETURN_GLYPH_NUDGE_Y_PX,
    };

    #[test]
    fn test_footer_surface_rgba_uses_surface_token_with_full_opacity_in_light_mode() {
        let colors = PromptFooterColors {
            accent: 0,
            text_muted: 0,
            border: 0,
            surface: 0x112233,
            background: 0x2255aa,
            hover_alpha: 0,
            text_primary: 0xffffff,
            selected_alpha: 0x4d,
            is_light_mode: true,
        };

        // Light mode uses the theme-derived surface token, not a hard-coded gray.
        assert_eq!(footer_surface_rgba(colors), 0x112233ff);
        assert_ne!(footer_surface_rgba(colors), 0xf0eeefff);
    }

    #[test]
    fn test_footer_surface_rgba_uses_background_overlay_in_dark_mode() {
        let colors = PromptFooterColors {
            accent: 0,
            text_muted: 0,
            border: 0,
            surface: 0x112233,
            background: 0x2255aa,
            hover_alpha: 0,
            text_primary: 0xffffff,
            selected_alpha: 0x4d,
            is_light_mode: false,
        };

        // Dark mode uses background token at ~12% opacity.
        assert_eq!(footer_surface_rgba(colors), 0x2255aa1f);
    }

    #[test]
    fn test_prompt_footer_colors_from_theme_sets_surface_from_background_main() {
        let mut theme = crate::theme::Theme::default();
        theme.colors.background.main = 0x445566;

        let resolved = PromptFooterColors::from_theme(&theme);

        assert_eq!(resolved.surface, 0x445566);
    }

    #[test]
    fn test_prompt_footer_colors_from_design_uses_cached_theme_tokens() {
        let design = crate::designs::DesignColors {
            accent: 0x010203,
            text_muted: 0x040506,
            border: 0x070809,
            background_selected: 0x0a0b0c,
            ..crate::designs::DesignColors::default()
        };

        let resolved = PromptFooterColors::from_design(&design);
        let expected = PromptFooterColors::from_theme(&crate::theme::get_cached_theme());

        assert_eq!(resolved.accent, expected.accent);
        assert_eq!(resolved.text_muted, expected.text_muted);
        assert_eq!(resolved.border, expected.border);
        assert_eq!(resolved.surface, expected.surface);
        assert_eq!(resolved.background, expected.background);
        assert_eq!(resolved.hover_alpha, expected.hover_alpha);
        assert_eq!(resolved.is_light_mode, expected.is_light_mode);
    }

    #[test]
    fn test_prompt_footer_colors_default_uses_cached_theme_tokens() {
        let resolved = PromptFooterColors::default();
        let expected = PromptFooterColors::from_theme(&crate::theme::get_cached_theme());

        assert_eq!(resolved.accent, expected.accent);
        assert_eq!(resolved.text_muted, expected.text_muted);
        assert_eq!(resolved.border, expected.border);
        assert_eq!(resolved.surface, expected.surface);
        assert_eq!(resolved.background, expected.background);
        assert_eq!(resolved.hover_alpha, expected.hover_alpha);
        assert_eq!(resolved.is_light_mode, expected.is_light_mode);
    }

    #[test]
    fn test_footer_shadow_alpha_uses_higher_alpha_in_dark_mode() {
        let light = PromptFooterColors {
            accent: 0,
            text_muted: 0,
            border: 0,
            surface: 0,
            background: 0,
            hover_alpha: 0,
            text_primary: 0xffffff,
            selected_alpha: 0x4d,
            is_light_mode: true,
        };
        let dark = PromptFooterColors {
            is_light_mode: false,
            ..light
        };

        assert_eq!(super::footer_shadow_alpha(light), 0x28);
        assert_eq!(super::footer_shadow_alpha(dark), 0x50);
    }

    #[test]
    fn test_footer_button_hover_rgba_uses_text_primary_with_dark_opacity() {
        let colors = PromptFooterColors {
            accent: 0,
            text_muted: 0,
            border: 0,
            surface: 0,
            background: 0x2255aa,
            hover_alpha: 0x38,
            text_primary: 0xffffff,
            selected_alpha: 0x4d,
            is_light_mode: false,
        };

        // Hover pill is a primary-text overlay (canonical footer style), not
        // a background tint.
        assert_eq!(super::footer_button_hover_rgba(colors), 0xffffff38);
    }

    #[test]
    fn test_prompt_footer_colors_from_theme_uses_hover_opacity() {
        let mut theme = crate::theme::Theme::light_default();
        let mut opacity = theme.get_opacity();
        opacity.hover = 0.18;
        theme.opacity = Some(opacity);

        let colors = PromptFooterColors::from_theme(&theme);
        assert_eq!(colors.hover_alpha, 0x2e);
    }

    #[test]
    fn test_footer_button_hover_rgba_uses_text_primary_with_light_opacity() {
        let colors = PromptFooterColors {
            accent: 0,
            text_muted: 0,
            border: 0,
            surface: 0,
            background: 0x2255aa,
            hover_alpha: 0x2e,
            text_primary: 0xffffff,
            selected_alpha: 0x4d,
            is_light_mode: true,
        };

        assert_eq!(super::footer_button_hover_rgba(colors), 0xffffff2e);
    }

    #[test]
    fn test_footer_button_active_rgba_uses_text_primary_with_selected_opacity() {
        let colors = PromptFooterColors {
            accent: 0,
            text_muted: 0,
            border: 0,
            surface: 0,
            background: 0x2255aa,
            hover_alpha: 0,
            text_primary: 0xf0f4f8,
            selected_alpha: 0x4d,
            is_light_mode: false,
        };

        assert_eq!(super::footer_button_active_rgba(colors), 0xf0f4f84d);
    }

    #[test]
    fn test_is_footer_button_clickable_requires_handler_and_enabled_state() {
        assert!(super::is_footer_button_clickable(true, false));
        assert!(!super::is_footer_button_clickable(false, false));
        assert!(!super::is_footer_button_clickable(true, true));
    }

    #[test]
    fn test_is_footer_button_activation_key_accepts_enter_and_space_variants() {
        assert!(super::is_footer_button_activation_key("enter"));
        assert!(super::is_footer_button_activation_key("Enter"));
        assert!(super::is_footer_button_activation_key("return"));
        assert!(super::is_footer_button_activation_key("Space"));
        assert!(super::is_footer_button_activation_key(" "));
        assert!(!super::is_footer_button_activation_key("tab"));
    }

    #[test]
    fn test_prompt_footer_config_shows_primary_and_info_by_default() {
        let config = PromptFooterConfig::new();

        assert!(config.show_primary);
        assert!(config.show_info_label);
    }

    #[test]
    fn test_prompt_footer_config_can_hide_primary_and_info_labels_explicitly() {
        let config = PromptFooterConfig::new()
            .show_primary(false)
            .show_info_label(false);

        assert!(!config.show_primary);
        assert!(!config.show_info_label);
    }

    #[test]
    fn test_prompt_footer_layout_tokens_stay_consistent_when_spacing_is_adjusted() {
        assert_eq!(PROMPT_FOOTER_SECTION_GAP_PX, 8.0);
        assert_eq!(PROMPT_FOOTER_BUTTON_GAP_PX, 4.0);
        assert_eq!(PROMPT_FOOTER_BUTTON_ACTIVE_OPACITY, 0x3a);
        assert_eq!(PROMPT_FOOTER_BUTTON_FONT_DELTA_PX, 2.0);
        assert_eq!(PROMPT_FOOTER_BUTTON_FONT_MIN_PX, 10.0);
        assert_eq!(PROMPT_FOOTER_PADDING_X_PX, 12.0);
        assert_eq!(PROMPT_FOOTER_PADDING_BOTTOM_PX, 2.0);
        assert_eq!(PROMPT_FOOTER_LOGO_SIZE_PX, 16.0);
        assert_eq!(PROMPT_FOOTER_LOGO_NUDGE_X_PX, 2.0);
        assert_eq!(PROMPT_FOOTER_DIVIDER_WIDTH_PX, 1.0);
        assert_eq!(PROMPT_FOOTER_DIVIDER_HEIGHT_PX, 16.0);

        assert_eq!(PROMPT_FOOTER_BORDER_OPACITY, 0x50);
        assert_eq!(PROMPT_FOOTER_SHADOW_OFFSET_Y_PX, -1.0);
        assert_eq!(PROMPT_FOOTER_SHADOW_BLUR_PX, 8.0);
        assert_eq!(PROMPT_FOOTER_INFO_FONT_DELTA_PX, 4.0);
        assert_eq!(PROMPT_FOOTER_INFO_FONT_MIN_PX, 9.0);
        assert_eq!(PROMPT_FOOTER_HELPER_FONT_DELTA_PX, 2.0);
        assert_eq!(PROMPT_FOOTER_HELPER_FONT_MIN_PX, 10.0);
        assert_eq!(FOOTER_KEY_GLYPH_NUDGE_Y_PX, 1.0);
        assert_eq!(FOOTER_RETURN_GLYPH_NUDGE_Y_PX, 1.0);
    }

    #[test]
    fn test_split_shortcut_parses_simple_and_complex_keys() {
        assert_eq!(split_footer_shortcut(""), Vec::<String>::new());
        assert_eq!(split_footer_shortcut("↵"), vec!["↵"]);
        assert_eq!(split_footer_shortcut("⌘K"), vec!["⌘", "K"]);
        assert_eq!(split_footer_shortcut("⌥↵"), vec!["⌥", "↵"]);
        assert_eq!(split_footer_shortcut("Enter"), vec!["↵"]);
        assert_eq!(split_footer_shortcut("Cmd+K"), vec!["⌘", "K"]);
        assert_eq!(split_footer_shortcut("cmd++"), vec!["⌘", "+"]);
        assert_eq!(split_footer_shortcut("ctrl+\\"), vec!["⌃", "\\"]);
        assert_eq!(split_footer_shortcut("⌘F1"), vec!["⌘", "F1"]);
        assert_eq!(split_footer_shortcut("⌥⌘I"), vec!["⌥", "⌘", "I"]);
    }

    #[test]
    fn test_shortcut_key_glyph_nudges_follow_shared_footer_contract() {
        let metrics = crate::components::footer_chrome::current_main_menu_footer_metrics();
        assert!(is_footer_return_key_glyph("↵"));
        assert!(!is_footer_return_key_glyph("Enter"));
        assert!(!is_footer_return_key_glyph("Return"));
        assert!(!is_footer_return_key_glyph("⌘"));
        // Non-return glyphs delegate to the shared footer resolver so
        // PromptFooter cannot become a second owner of per-glyph token
        // routing (footer_chrome owns the resolver-to-token mapping).
        assert_eq!(
            super::shortcut_key_glyph_nudge_y("⌘", None),
            crate::components::footer_chrome::footer_key_glyph_nudge_y("⌘"),
        );
        assert_eq!(
            crate::components::footer_chrome::footer_key_glyph_nudge_y("⌘"),
            metrics.cmd_glyph_nudge_y,
        );
        // Return remains the one PromptFooter-specific override path.
        assert_eq!(
            super::shortcut_key_glyph_nudge_y("↵", None),
            metrics.key_glyph_nudge_y + metrics.return_glyph_nudge_y,
        );
        assert_eq!(super::return_key_glyph_nudge_y(Some(100.0)), 100.0);
        assert_eq!(super::shortcut_key_glyph_nudge_y("↵", Some(100.0)), 100.0);
    }
}
