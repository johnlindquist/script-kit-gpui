//! UI Foundation - Shared UI patterns for consistent vibrancy and layout
//!
//! This module extracts common UI patterns from the main menu (render_script_list.rs)
//! into reusable helpers. The main menu is the "gold standard" for vibrancy support.
//!
//! NOTE: Many items are currently unused as this is a foundation module.
//! They will be used as other modules are refactored to use the shared patterns.
#![allow(dead_code)]
//!
//! # Key Vibrancy Pattern (from render_script_list.rs:699-707)
//!
//! ```ignore
//! // VIBRANCY: Remove background from content div - let gpui-component Root's
//! // semi-transparent background handle vibrancy effect. Content areas should NOT
//! // have their own backgrounds to allow blur to show through.
//! let _bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);
//!
//! let mut main_div = div()
//!     .flex()
//!     .flex_col()
//!     // Removed: .bg(rgba(bg_with_alpha)) - let vibrancy show through from Root
//!     .shadow(box_shadows)
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use crate::ui_foundation::{get_vibrancy_background, container_div, content_div};
//!
//! // In your render function:
//! let bg = get_vibrancy_background(&theme);
//! let container = container_div()
//!     .when_some(bg, |d, bg| d.bg(bg))
//!     .child(content_div().child(...));
//! ```

// --- merged from part_000.rs ---
use crate::designs::{get_tokens, DesignColors, DesignSpacing, DesignVariant};
use crate::theme::{ColorScheme, Theme};
use gpui::{
    div, linear_color_stop, linear_gradient, px, rgba, Background, Div, ElementId, Hsla,
    InteractiveElement, Rgba, Stateful, Styled,
};
/// Convert a hex color (u32) to RGBA with specified opacity.
///
/// This is the standard way to create semi-transparent colors for vibrancy support.
/// The hex color provides RGB values, and opacity controls the alpha channel.
///
/// # Arguments
/// * `hex` - A u32 hex color (e.g., 0x1E1E1E for dark gray)
/// * `opacity` - Alpha value from 0.0 (transparent) to 1.0 (opaque)
///
/// # Returns
/// A u32 suitable for use with `gpui::rgba()` - format is 0xRRGGBBAA
///
/// # Example (from main menu)
/// ```ignore
/// let bg_hex = theme.colors.background.main; // 0x1E1E1E
/// let opacity = theme.get_opacity().main;     // 0.30
/// let bg_with_alpha = hex_to_rgba_with_opacity(bg_hex, opacity);
/// // Result: 0x1E1E1E4D (30% opacity)
/// ```
#[inline]
pub fn hex_to_rgba_with_opacity(hex: u32, opacity: f32) -> u32 {
    // Convert opacity (0.0-1.0) to alpha byte (0x00-0xFF)
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0) as u32;
    // Shift hex left 8 bits and add alpha
    (hex << 8) | alpha
}
/// Convert a hex color to HSLA with specified alpha.
///
/// Used when GPUI components expect Hsla instead of Rgba.
///
/// # Arguments
/// * `hex` - A u32 hex color
/// * `alpha` - Alpha value from 0.0 to 1.0
///
/// # Returns
/// An Hsla color with the specified alpha
#[inline]
pub fn hex_to_hsla_with_alpha(hex: u32, alpha: f32) -> Hsla {
    let rgba = gpui::rgb(hex);
    let hsla: Hsla = rgba.into();
    Hsla {
        h: hsla.h,
        s: hsla.s,
        l: hsla.l,
        a: alpha.clamp(0.0, 1.0),
    }
}
/// Opacity for vibrancy window backgrounds in dark mode.
/// Sourced from the canonical token so legibility stays backdrop-independent.
pub const VIBRANCY_DARK_OPACITY: f32 = crate::theme::opacity::OPACITY_VIBRANCY_BACKGROUND;
/// Opacity for vibrancy window backgrounds in light mode.
/// Sourced from the canonical token so legibility stays backdrop-independent.
pub const VIBRANCY_LIGHT_OPACITY: f32 = crate::theme::opacity::OPACITY_VIBRANCY_BACKGROUND;
/// 38% opacity - arg input selection highlight fill.
pub(crate) const ALPHA_SELECTION: u8 = 0x60;
/// 38% opacity - prompt dividers and subtle section borders.
pub(crate) const ALPHA_DIVIDER: u8 = 0x60;
/// 19% opacity - subtle borders and badge backgrounds.
pub(crate) const ALPHA_BORDER_SUBTLE: u8 = 0x30;
/// 12% opacity - faint tint for secondary badges.
pub(crate) const ALPHA_TINT_SUBTLE: u8 = 0x20;
/// Get the vibrancy background for window root containers.
///
/// Canonical helper used by AI/Notes window root containers to tint macOS blur.
///
/// **NOTE:** Root no longer provides this background (was removed for vibrancy support).
/// Windows must explicitly apply this to their outermost container div.
///
/// # Returns
/// An Rgba color with appropriate opacity for the current theme mode:
/// - Light mode: 50% opacity
/// - Dark mode: 50% opacity
///
/// # Example
/// ```ignore
/// let vibrancy_bg = get_window_vibrancy_background();
/// div()
///     .size_full()
///     .bg(vibrancy_bg)  // Tints the blur effect with theme color
///     .child(content)
/// ```
///
/// Uses cached theme to avoid file I/O on every render.
/// If `theme.opacity.vibrancy_background` is set, that value is used.
/// Otherwise, the canonical `OPACITY_VIBRANCY_BACKGROUND` token is used.
#[inline]
fn resolve_window_vibrancy_opacity(theme: &Theme) -> f32 {
    let opacity = theme.get_opacity();
    let alpha = opacity
        .vibrancy_background
        .unwrap_or(if theme.has_dark_colors() {
            VIBRANCY_DARK_OPACITY
        } else {
            VIBRANCY_LIGHT_OPACITY
        })
        .clamp(0.0, 1.0);
    // Glass mode: the NSGlassEffectView backdrop frosts on its own, so the
    // veil is the theme's glass_veil_opacity (a theme-designer slider),
    // defaulting to the bare-glass cap.
    if theme.is_vibrancy_enabled() && crate::platform::tahoe_liquid_glass_available() {
        opacity
            .glass_veil_opacity
            .unwrap_or(crate::theme::opacity::OPACITY_GLASS_MODE_VEIL_CAP)
            .clamp(0.0, 1.0)
    } else {
        alpha
    }
}

pub fn get_window_vibrancy_background() -> Rgba {
    let theme = crate::theme::get_cached_theme();
    let opacity = resolve_window_vibrancy_opacity(&theme);
    let bg_hex = theme.colors.background.main;
    gpui::rgba(hex_to_rgba_with_opacity(bg_hex, opacity))
}

/// Background tint used by floating/native windows that must visually match
/// the main window's themed surface.
///
/// Resolves through the same path as `get_window_vibrancy_background` so every
/// window (main, notes, dictation, HUD, actions popup) shares one tint.
#[inline]
pub fn main_window_matched_background_rgba(theme: &Theme) -> u32 {
    let background_alpha = resolve_window_vibrancy_opacity(theme);

    hex_to_rgba_with_opacity(theme.colors.background.main, background_alpha)
}

#[inline]
pub fn main_window_matched_background(theme: &Theme) -> Rgba {
    gpui::rgba(main_window_matched_background_rgba(theme))
}

/// Lighter vibrancy background for floating surfaces (dialogs, notifications).
/// Uses a lower alpha than the window root so native macOS blur shows through.
pub fn get_vibrancy_surface_background(alpha: f32) -> Rgba {
    let theme = crate::theme::get_cached_theme();
    let bg_hex = theme.colors.background.main;
    if theme.is_vibrancy_enabled() {
        gpui::rgba(hex_to_rgba_with_opacity(bg_hex, alpha))
    } else {
        gpui::rgba(hex_to_rgba_with_opacity(bg_hex, 1.0))
    }
}

/// Get the background color for vibrancy-aware containers.
///
/// **CRITICAL VIBRANCY PATTERN:** When vibrancy is enabled, content divs should NOT
/// have their own backgrounds. Instead, they rely on the gpui-component Root wrapper
/// to provide a semi-transparent background that allows blur to show through.
///
/// # Arguments
/// * `theme` - The current theme
///
/// # Returns
/// * `None` when vibrancy is enabled (let Root handle the background)
/// * `Some(Rgba)` when vibrancy is disabled (use solid background)
///
/// # Example (from main menu render_script_list.rs)
/// ```ignore
/// let bg = get_vibrancy_background(&self.theme);
/// let main_div = div()
///     .flex()
///     .flex_col()
///     .when_some(bg, |d, bg| d.bg(bg)) // Only apply bg when vibrancy disabled
///     .shadow(box_shadows);
/// ```
pub fn get_vibrancy_background(theme: &Theme) -> Option<Rgba> {
    if theme.is_vibrancy_enabled() {
        // VIBRANCY: Let Root's semi-transparent background handle blur
        None
    } else {
        // No vibrancy: use solid background
        Some(gpui::rgb(theme.colors.background.main))
    }
}

/// Resolve the optional theme background gradient layers as GPUI fills.
pub fn get_theme_background_gradients(theme: &Theme) -> Vec<Background> {
    let mut backgrounds = Vec::new();
    if let Some(gradient) = theme.active_background_gradient() {
        // Base layer
        let alpha = crate::theme::types::opacity_to_alpha(gradient.opacity);
        let from = rgba((gradient.from << 8) | alpha);
        let to = rgba((gradient.to << 8) | alpha);
        backgrounds.push(linear_gradient(
            gradient.angle,
            linear_color_stop(from, 0.0),
            linear_color_stop(to, 1.0),
        ));

        // Overlapping layers
        for layer in gradient
            .layers
            .iter()
            .filter(|l| l.enabled && l.opacity > 0.0)
        {
            let alpha = crate::theme::types::opacity_to_alpha(layer.opacity);
            let from = rgba((layer.from << 8) | alpha);
            let to = rgba((layer.to << 8) | alpha);
            backgrounds.push(linear_gradient(
                layer.angle,
                linear_color_stop(from, 0.0),
                linear_color_stop(to, 1.0),
            ));
        }
    }
    backgrounds
}

/// Build absolute background layers for an active theme gradient.
pub fn theme_background_gradient_layers(
    id_prefix: &'static str,
    theme: &Theme,
) -> Vec<Stateful<Div>> {
    get_theme_background_gradients(theme)
        .into_iter()
        .enumerate()
        .map(|(i, bg)| {
            div()
                .id(ElementId::NamedInteger(id_prefix.into(), i as u64))
                .absolute()
                .inset_0()
                .bg(bg)
        })
        .collect()
}
/// Get container background with optional opacity for semi-transparent areas.
///
/// Use this for inner containers that need subtle backgrounds even with vibrancy.
/// For example, log panels or input fields that need slight visual separation.
///
/// # Arguments
/// * `theme` - The current theme
/// * `opacity` - Opacity to apply (0.0-1.0)
///
/// # Returns
/// An Rgba color with the specified opacity applied
///
/// # Example
/// ```ignore
/// let log_bg = get_container_background(&theme, theme.get_opacity().log_panel);
/// div().bg(log_bg).child(logs)
/// ```
pub fn get_container_background(theme: &Theme, opacity: f32) -> Rgba {
    let hex = theme.colors.background.main;
    let rgba_u32 = hex_to_rgba_with_opacity(hex, opacity);
    gpui::rgba(rgba_u32)
}
/// Design colors extracted from tokens, ready for use in UI rendering.
///
/// This provides a consistent interface whether using the Default design
/// (which uses theme.colors) or other design variants (which use design tokens).
#[derive(Clone, Copy)]
pub struct UIDesignColors {
    /// Background color (hex)
    pub background: u32,
    /// Primary text color (hex)
    pub text_primary: u32,
    /// Secondary/muted text color (hex)
    pub text_muted: u32,
    /// Dimmed text color (hex)
    pub text_dimmed: u32,
    /// Accent/highlight color (hex)
    pub accent: u32,
    /// Border color (hex)
    pub border: u32,
}
impl UIDesignColors {
    /// Create design colors from theme (for Default design variant)
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            background: theme.colors.background.main,
            text_primary: theme.colors.text.primary,
            text_muted: theme.colors.text.muted,
            text_dimmed: theme.colors.text.dimmed,
            accent: theme.colors.accent.selected,
            border: theme.colors.ui.border,
        }
    }

    /// Create design colors from design tokens
    pub fn from_design(colors: &DesignColors) -> Self {
        Self {
            background: colors.background,
            text_primary: colors.text_primary,
            text_muted: colors.text_muted,
            text_dimmed: colors.text_dimmed,
            accent: colors.accent,
            border: colors.border,
        }
    }

    /// Get design colors based on variant - always uses theme for consistent styling
    pub fn for_variant(_variant: DesignVariant, theme: &Theme) -> Self {
        // Always use theme for consistent styling across all prompts
        Self::from_theme(theme)
    }
}
/// Get design colors for the current design variant.
///
/// This abstracts the pattern of choosing between theme.colors (Default design)
/// and design tokens (other designs).
///
/// # Arguments
/// * `variant` - The current design variant
/// * `theme` - The current theme
///
/// # Returns
/// Design colors appropriate for the variant
///
/// # Example (from main menu)
/// ```ignore
/// let design_colors = get_design_colors(self.current_design, &self.theme);
/// let text_color = rgb(design_colors.text_primary);
/// ```
pub fn get_design_colors(variant: DesignVariant, theme: &Theme) -> UIDesignColors {
    UIDesignColors::for_variant(variant, theme)
}
/// Get design spacing values for the current design variant.
///
/// # Arguments
/// * `variant` - The current design variant
///
/// # Returns
/// Design spacing tokens
pub fn get_design_spacing(variant: DesignVariant) -> DesignSpacing {
    let tokens = get_tokens(variant);
    tokens.spacing()
}
/// Opacity configuration extracted from theme, with helper methods.
///
/// This wraps the theme's BackgroundOpacity with convenient accessors.
#[derive(Clone, Copy)]
pub struct OpacityConfig {
    /// Main background opacity
    pub main: f32,
    /// Title bar opacity
    pub title_bar: f32,
    /// Search box/input opacity
    pub search_box: f32,
    /// Log panel opacity
    pub log_panel: f32,
    /// Selected item opacity
    pub selected: f32,
    /// Hovered item opacity
    pub hover: f32,
    /// Preview panel opacity
    pub preview: f32,
    /// Dialog/popup opacity
    pub dialog: f32,
    /// Input field opacity
    pub input: f32,
    /// Panel/container opacity
    pub panel: f32,
    /// Input inactive state opacity
    pub input_inactive: f32,
    /// Input active state opacity
    pub input_active: f32,
    /// Border inactive state opacity
    pub border_inactive: f32,
    /// Border active state opacity
    pub border_active: f32,
}
impl OpacityConfig {
    /// Create from theme
    pub fn from_theme(theme: &Theme) -> Self {
        let o = theme.get_opacity();
        Self {
            main: o.main,
            title_bar: o.title_bar,
            search_box: o.search_box,
            log_panel: o.log_panel,
            selected: o.selected,
            hover: o.hover,
            preview: o.preview,
            dialog: o.dialog,
            input: o.input,
            panel: o.panel,
            input_inactive: o.input_inactive,
            input_active: o.input_active,
            border_inactive: o.border_inactive,
            border_active: o.border_active,
        }
    }
}
/// Get opacity configuration from theme.
///
/// # Example
/// ```ignore
/// let opacity = get_opacity_config(&theme);
/// let bg = hex_to_rgba_with_opacity(bg_hex, opacity.main);
/// ```
pub fn get_opacity_config(theme: &Theme) -> OpacityConfig {
    OpacityConfig::from_theme(theme)
}
// ============================================================================
// Layout Primitives
// ============================================================================

/// Create a standard container div with flex column layout.
///
/// This is the base pattern for main content containers, matching the
/// main menu's structure.
///
/// # Returns
/// A `Div` configured with:
/// - `flex()` - Enable flexbox
/// - `flex_col()` - Column direction
/// - `w_full()` - Full width
/// - `h_full()` - Full height
///
/// # Example (from main menu)
/// ```ignore
/// let main_div = container_div()
///     .shadow(box_shadows)
///     .rounded(px(border_radius))
///     .child(...);
/// ```
pub fn container_div() -> Div {
    gpui::div().flex().flex_col().w_full().h_full()
}
/// Create a content area div with proper overflow handling.
///
/// Use this for content areas that may need scrolling or contain lists.
/// The `min_h(px(0.))` is critical for proper flex shrinking.
///
/// # Returns
/// A `Div` configured with:
/// - `flex()` - Enable flexbox
/// - `flex_col()` - Column direction
/// - `flex_1()` - Grow to fill available space
/// - `w_full()` - Full width
/// - `min_h(px(0.))` - Critical: allows flex container to shrink properly
/// - `overflow_hidden()` - Clip overflow content
///
/// # Example (from main menu)
/// ```ignore
/// main_div = main_div.child(
///     content_div()
///         .flex_row() // Override to row for split layout
///         .child(list_panel)
///         .child(preview_panel)
/// );
/// ```
pub fn content_div() -> Div {
    gpui::div()
        .flex()
        .flex_col()
        .flex_1()
        .w_full()
        .min_h(px(0.)) // Critical: allows flex container to shrink properly
        .overflow_hidden()
}
/// Create a panel div for split-view layouts (like list/preview).
///
/// # Arguments
/// * `width_fraction` - The width as a fraction (e.g., 0.5 for half width)
///
/// # Returns
/// A `Div` configured for panel layout with proper shrinking
pub fn panel_div() -> Div {
    gpui::div()
        .h_full()
        .min_h(px(0.)) // Allow shrinking
        .overflow_hidden()
}
// ============================================================================
// Color Scheme Helpers
// ============================================================================

/// Extension trait for ColorScheme to provide convenient color access.
pub trait ColorSchemeExt {
    /// Get text color for selection state
    fn text_for_selection(&self, is_selected: bool) -> u32;

    /// Get description color for selection state
    fn description_for_selection(&self, is_selected: bool) -> u32;
}
impl ColorSchemeExt for ColorScheme {
    fn text_for_selection(&self, is_selected: bool) -> u32 {
        if is_selected {
            self.text.primary
        } else {
            self.text.secondary
        }
    }

    fn description_for_selection(&self, is_selected: bool) -> u32 {
        if is_selected {
            self.accent.selected // Use accent color for selected item description
        } else {
            self.text.secondary
        }
    }
}
// ============================================================================
// HexColorExt - Extension trait for u32 hex colors
// ============================================================================

/// Extension trait for u32 hex colors to provide convenient color conversion.
///
/// This eliminates the need for manual `rgb(colors.*)` calls and `<< 8 | alpha`
/// packing throughout the codebase.
///
/// # Example
/// ```ignore
/// use crate::ui_foundation::HexColorExt;
///
/// let colors = theme.colors;
/// // Instead of: rgb(colors.text.primary)
/// // Use: colors.text.primary.to_rgb()
///
/// // Instead of: rgba((colors.border << 8) | 0x80)
/// // Use: colors.border.rgba8(0x80)
///
/// // Instead of manual opacity calculation:
/// // Use: colors.background.with_opacity(0.5)
/// ```
pub trait HexColorExt {
    /// Convert hex color to GPUI Hsla (fully opaque).
    ///
    /// Replaces `rgb(color)` calls.
    fn to_rgb(self) -> Hsla;

    /// Convert hex color to GPUI Hsla with alpha byte (0-255).
    ///
    /// Replaces `rgba((color << 8) | alpha)` patterns.
    fn rgba8(self, alpha: u8) -> Hsla;

    /// Convert hex color to GPUI Hsla with opacity float (0.0-1.0).
    ///
    /// More readable than manual alpha calculation.
    fn with_opacity(self, opacity: f32) -> Hsla;
}
impl HexColorExt for u32 {
    #[inline]
    fn to_rgb(self) -> Hsla {
        gpui::rgb(self).into()
    }

    #[inline]
    fn rgba8(self, alpha: u8) -> Hsla {
        gpui::rgba((self << 8) | alpha as u32).into()
    }

    #[inline]
    fn with_opacity(self, opacity: f32) -> Hsla {
        let alpha = (opacity.clamp(0.0, 1.0) * 255.0) as u8;
        gpui::rgba((self << 8) | alpha as u32).into()
    }
}
// ============================================================================
// Layout Primitives - Free functions for common patterns
// ============================================================================

/// Create a vertical stack (flex column).
///
/// Replaces `div().flex().flex_col()` pattern.
#[inline]
pub fn vstack() -> Div {
    gpui::div().flex().flex_col()
}
/// Create a horizontal stack (flex row with centered items).
///
/// Replaces `div().flex().flex_row().items_center()` pattern.
#[inline]
pub fn hstack() -> Div {
    gpui::div().flex().flex_row().items_center()
}
/// Create a centered container (items centered both axes).
///
/// Replaces `div().flex().items_center().justify_center()` pattern.
#[inline]
pub fn centered() -> Div {
    gpui::div().flex().items_center().justify_center()
}
/// Create a flexible spacer that fills available space.
///
/// Replaces `div().flex_1()` pattern.
#[inline]
pub fn spacer() -> Div {
    gpui::div().flex_1()
}

// ============================================================================
// Action / Overlay Primitives
// ============================================================================

/// Identifies the UI surface where an action originates.
/// Names are stable wire values — do not rename variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiSurface {
    NotesToolbar,
    NotesExportMenu,
    NotesOverlay,
    ThemeChooserPreview,
}

impl UiSurface {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotesToolbar => "notes_toolbar",
            Self::NotesExportMenu => "notes_export_menu",
            Self::NotesOverlay => "notes_overlay",
            Self::ThemeChooserPreview => "theme_chooser_preview",
        }
    }
}

/// Describes a single UI action for logging and introspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UiActionSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub shortcut: Option<&'static str>,
}

/// Emit a structured log line for a UI action.
#[inline]
pub(crate) fn log_ui_action(surface: UiSurface, spec: UiActionSpec, phase: &'static str) {
    tracing::info!(
        surface = surface.as_str(),
        control_id = spec.id,
        control_label = spec.label,
        shortcut = spec.shortcut.unwrap_or(""),
        phase = phase,
        "ui_action"
    );
}

/// A compact horizontal action row with 4px gap and wrapping.
#[inline]
pub(crate) fn compact_action_row() -> Div {
    hstack().gap(px(4.0)).flex_wrap()
}

/// Root div for a modal overlay: absolute, fills parent, flex.
#[inline]
pub(crate) fn modal_overlay_root(id: &'static str) -> Stateful<Div> {
    gpui::div().id(id).absolute().inset_0().flex()
}

// --- merged from part_001.rs ---
// ============================================================================
// Key Normalization - Allocation-free key matching
// ============================================================================
//
// IMPORTANT: These helpers use eq_ignore_ascii_case() instead of to_lowercase()
// to avoid allocations on every keystroke. This is a hot path optimization.

/// Check if key is an up arrow (handles both "up" and "arrowup" formats).
///
/// Uses allocation-free ASCII case-insensitive comparison.
#[inline]
pub fn is_key_up(key: &str) -> bool {
    key.eq_ignore_ascii_case("up") || key.eq_ignore_ascii_case("arrowup")
}
/// Check if key is a down arrow (handles both "down" and "arrowdown" formats).
///
/// Uses allocation-free ASCII case-insensitive comparison.
#[inline]
pub fn is_key_down(key: &str) -> bool {
    key.eq_ignore_ascii_case("down") || key.eq_ignore_ascii_case("arrowdown")
}
/// Check if key is a left arrow (handles both "left" and "arrowleft" formats).
///
/// Uses allocation-free ASCII case-insensitive comparison.
#[inline]
pub fn is_key_left(key: &str) -> bool {
    key.eq_ignore_ascii_case("left") || key.eq_ignore_ascii_case("arrowleft")
}
/// Check if key is a right arrow (handles both "right" and "arrowright" formats).
///
/// Uses allocation-free ASCII case-insensitive comparison.
#[inline]
pub fn is_key_right(key: &str) -> bool {
    key.eq_ignore_ascii_case("right") || key.eq_ignore_ascii_case("arrowright")
}
/// Check if key is Enter/Return.
///
/// Uses allocation-free ASCII case-insensitive comparison.
#[inline]
pub fn is_key_enter(key: &str) -> bool {
    key.eq_ignore_ascii_case("enter") || key.eq_ignore_ascii_case("return")
}
/// Check if key is Escape.
///
/// Uses allocation-free ASCII case-insensitive comparison.
#[inline]
pub fn is_key_escape(key: &str) -> bool {
    key.eq_ignore_ascii_case("escape") || key.eq_ignore_ascii_case("esc")
}
/// Check if key is Backspace.
///
/// Uses allocation-free ASCII case-insensitive comparison.
#[inline]
pub fn is_key_backspace(key: &str) -> bool {
    key.eq_ignore_ascii_case("backspace")
}
/// Check if key is Delete.
///
/// Uses allocation-free ASCII case-insensitive comparison.
#[inline]
pub fn is_key_delete(key: &str) -> bool {
    key.eq_ignore_ascii_case("delete")
}
/// Check if key is Tab.
///
/// Uses allocation-free ASCII case-insensitive comparison.
#[inline]
pub fn is_key_tab(key: &str) -> bool {
    key == "\t" || key.eq_ignore_ascii_case("tab")
}
/// Check if key is Space.
///
/// Uses allocation-free ASCII case-insensitive comparison.
#[inline]
pub fn is_key_space(key: &str) -> bool {
    key == " " || key.eq_ignore_ascii_case("space")
}
/// Check if key is the "k" key (for Cmd+K shortcut).
///
/// Uses allocation-free ASCII case-insensitive comparison.
#[inline]
pub fn is_key_k(key: &str) -> bool {
    key.eq_ignore_ascii_case("k")
}
/// Extract printable character from a KeyDownEvent's key_char field.
///
/// Returns Some(char) if the key_char contains a non-control character,
/// None otherwise (for special keys like arrows, escape, etc.).
#[inline]
pub fn printable_char(key_char: Option<&str>) -> Option<char> {
    key_char
        .and_then(|s| s.chars().next())
        .filter(|ch| !ch.is_control())
}

/// Shared launcher/list click behavior.
///
/// The first click on an unselected row only selects it. Once the row is
/// selected, the next click submits it. Native double-clicks also submit.
#[inline]
pub fn should_submit_selected_row_click(was_selected: bool, click_count: usize) -> bool {
    was_selected || click_count >= 2
}
#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // HexColorExt Tests
    // ========================================================================

    #[test]
    fn test_hex_color_to_rgb() {
        // White should convert correctly
        let white = 0xFFFFFFu32.to_rgb();
        assert!(
            (white.l - 1.0).abs() < 0.01,
            "White should have lightness ~1.0"
        );

        // Black should convert correctly
        let black = 0x000000u32.to_rgb();
        assert!(black.l < 0.01, "Black should have lightness ~0.0");

        // Alpha should be 1.0 (fully opaque)
        assert!(
            (white.a - 1.0).abs() < 0.001,
            "to_rgb should be fully opaque"
        );
        assert!(
            (black.a - 1.0).abs() < 0.001,
            "to_rgb should be fully opaque"
        );
    }

    #[test]
    fn test_hex_color_rgba8() {
        // Test with 50% alpha (0x80 = 128)
        let semi = 0xFFFFFFu32.rgba8(0x80);
        // Alpha should be approximately 128/255 = 0.502
        assert!(
            (semi.a - 0.502).abs() < 0.01,
            "rgba8(0x80) should have ~50% alpha, got {}",
            semi.a
        );

        // Test with 0 alpha
        let transparent = 0xFFFFFFu32.rgba8(0x00);
        assert!(
            transparent.a < 0.01,
            "rgba8(0x00) should be fully transparent"
        );

        // Test with full alpha
        let opaque = 0xFFFFFFu32.rgba8(0xFF);
        assert!(
            (opaque.a - 1.0).abs() < 0.01,
            "rgba8(0xFF) should be fully opaque"
        );
    }

    #[test]
    fn test_hex_color_with_opacity() {
        // 50% opacity
        let half = 0xFFFFFFu32.with_opacity(0.5);
        assert!(
            (half.a - 0.5).abs() < 0.02,
            "with_opacity(0.5) should have ~50% alpha, got {}",
            half.a
        );

        // 0% opacity
        let transparent = 0xFFFFFFu32.with_opacity(0.0);
        assert!(
            transparent.a < 0.01,
            "with_opacity(0.0) should be fully transparent"
        );

        // 100% opacity
        let opaque = 0xFFFFFFu32.with_opacity(1.0);
        assert!(
            (opaque.a - 1.0).abs() < 0.01,
            "with_opacity(1.0) should be fully opaque"
        );
    }

    #[test]
    fn test_hex_color_opacity_clamping() {
        // Opacity > 1.0 should clamp to 1.0
        let over = 0xFFFFFFu32.with_opacity(1.5);
        assert!(
            (over.a - 1.0).abs() < 0.01,
            "with_opacity(1.5) should clamp to 1.0"
        );

        // Opacity < 0.0 should clamp to 0.0
        let under = 0xFFFFFFu32.with_opacity(-0.5);
        assert!(under.a < 0.01, "with_opacity(-0.5) should clamp to 0.0");
    }

    #[test]
    fn test_should_submit_selected_row_click_matches_launcher_contract() {
        assert!(!should_submit_selected_row_click(false, 1));
        assert!(should_submit_selected_row_click(false, 2));
        assert!(should_submit_selected_row_click(true, 1));
        assert!(should_submit_selected_row_click(true, 2));
        assert!(should_submit_selected_row_click(true, 3));
    }

    // ========================================================================
    // Key Normalization Tests (Allocation-free helpers)
    // ========================================================================

    #[test]
    fn test_is_key_up() {
        // All valid forms
        assert!(is_key_up("up"));
        assert!(is_key_up("Up"));
        assert!(is_key_up("UP"));
        assert!(is_key_up("arrowup"));
        assert!(is_key_up("ArrowUp"));
        assert!(is_key_up("ARROWUP"));
        // Invalid
        assert!(!is_key_up("down"));
        assert!(!is_key_up("left"));
        assert!(!is_key_up("enter"));
    }

    #[test]
    fn test_is_key_down() {
        assert!(is_key_down("down"));
        assert!(is_key_down("Down"));
        assert!(is_key_down("DOWN"));
        assert!(is_key_down("arrowdown"));
        assert!(is_key_down("ArrowDown"));
        assert!(is_key_down("ARROWDOWN"));
        assert!(!is_key_down("up"));
        assert!(!is_key_down("right"));
    }

    #[test]
    fn test_is_key_left() {
        assert!(is_key_left("left"));
        assert!(is_key_left("Left"));
        assert!(is_key_left("arrowleft"));
        assert!(is_key_left("ArrowLeft"));
        assert!(!is_key_left("right"));
        assert!(!is_key_left("up"));
    }

    #[test]
    fn test_is_key_right() {
        assert!(is_key_right("right"));
        assert!(is_key_right("Right"));
        assert!(is_key_right("arrowright"));
        assert!(is_key_right("ArrowRight"));
        assert!(!is_key_right("left"));
        assert!(!is_key_right("down"));
    }

    #[test]
    fn test_is_key_enter() {
        assert!(is_key_enter("enter"));
        assert!(is_key_enter("Enter"));
        assert!(is_key_enter("ENTER"));
        assert!(is_key_enter("return"));
        assert!(is_key_enter("Return"));
        assert!(!is_key_enter("escape"));
        assert!(!is_key_enter("space"));
    }

    #[test]
    fn test_is_key_escape() {
        assert!(is_key_escape("escape"));
        assert!(is_key_escape("Escape"));
        assert!(is_key_escape("ESCAPE"));
        assert!(is_key_escape("esc"));
        assert!(is_key_escape("Esc"));
        assert!(!is_key_escape("enter"));
    }

    #[test]
    fn test_is_key_backspace() {
        assert!(is_key_backspace("backspace"));
        assert!(is_key_backspace("Backspace"));
        assert!(is_key_backspace("BACKSPACE"));
        assert!(!is_key_backspace("delete"));
        assert!(!is_key_backspace("enter"));
    }

    #[test]
    fn test_is_key_delete() {
        assert!(is_key_delete("delete"));
        assert!(is_key_delete("Delete"));
        assert!(is_key_delete("DELETE"));
        assert!(!is_key_delete("backspace"));
        assert!(!is_key_delete("enter"));
    }

    #[test]
    fn test_is_key_tab() {
        assert!(is_key_tab("tab"));
        assert!(is_key_tab("Tab"));
        assert!(is_key_tab("TAB"));
        assert!(is_key_tab("\t"));
        assert!(!is_key_tab("space"));
        assert!(!is_key_tab("enter"));
    }

    #[test]
    fn test_is_key_space() {
        assert!(is_key_space("space"));
        assert!(is_key_space("Space"));
        assert!(is_key_space("SPACE"));
        assert!(is_key_space(" "));
        assert!(!is_key_space("tab"));
        assert!(!is_key_space("enter"));
    }

    #[test]
    fn test_is_key_k() {
        assert!(is_key_k("k"));
        assert!(is_key_k("K"));
        assert!(!is_key_k("j"));
        assert!(!is_key_k("enter"));
    }

    #[test]
    fn test_printable_char() {
        // Normal printable chars
        assert_eq!(printable_char(Some("a")), Some('a'));
        assert_eq!(printable_char(Some("A")), Some('A'));
        assert_eq!(printable_char(Some("1")), Some('1'));
        assert_eq!(printable_char(Some("!")), Some('!'));
        assert_eq!(printable_char(Some(" ")), Some(' '));

        // Control characters should return None
        assert_eq!(printable_char(Some("\n")), None);
        assert_eq!(printable_char(Some("\t")), None);
        assert_eq!(printable_char(Some("\x1b")), None); // ESC

        // Empty/None cases
        assert_eq!(printable_char(None), None);
        assert_eq!(printable_char(Some("")), None);
    }

    // ========================================================================
    // Original Tests
    // ========================================================================

    #[test]
    fn test_hex_to_rgba_with_opacity() {
        // Test 30% opacity (0.30 * 255 = 76.5 -> truncates to 76 = 0x4C)
        let result = hex_to_rgba_with_opacity(0x1E1E1E, 0.30);
        assert_eq!(result, 0x1E1E1E4C);

        // Test full opacity
        let result = hex_to_rgba_with_opacity(0xFFFFFF, 1.0);
        assert_eq!(result, 0xFFFFFFFF);

        // Test zero opacity
        let result = hex_to_rgba_with_opacity(0x000000, 0.0);
        assert_eq!(result, 0x00000000);

        // Test 50% opacity (0.5 * 255 = 127.5 -> truncates to 127 = 0x7F)
        let result = hex_to_rgba_with_opacity(0xABCDEF, 0.5);
        assert_eq!(result, 0xABCDEF7F);
    }

    #[test]
    fn test_opacity_clamping() {
        // Test opacity > 1.0 gets clamped
        let result = hex_to_rgba_with_opacity(0x123456, 1.5);
        assert_eq!(result, 0x123456FF);

        // Test opacity < 0.0 gets clamped
        let result = hex_to_rgba_with_opacity(0x123456, -0.5);
        assert_eq!(result, 0x12345600);
    }

    #[test]
    fn test_resolve_window_vibrancy_opacity_uses_mode_defaults_when_unset() {
        // With no overrides set, resolution falls back to the per-mode
        // vibrancy default — except in glass mode (Tahoe hardware, vibrancy
        // enabled), where the bare-glass veil cap owns the default.
        let expected_dark = if crate::platform::tahoe_liquid_glass_available() {
            crate::theme::opacity::OPACITY_GLASS_MODE_VEIL_CAP
        } else {
            VIBRANCY_DARK_OPACITY
        };
        let expected_light = if crate::platform::tahoe_liquid_glass_available() {
            crate::theme::opacity::OPACITY_GLASS_MODE_VEIL_CAP
        } else {
            VIBRANCY_LIGHT_OPACITY
        };

        let mut dark_theme = Theme::dark_default();
        if let Some(ref mut opacity) = dark_theme.opacity {
            opacity.vibrancy_background = None;
        }
        assert!((resolve_window_vibrancy_opacity(&dark_theme) - expected_dark).abs() < 0.0001);

        let mut light_theme = Theme::light_default();
        if let Some(ref mut opacity) = light_theme.opacity {
            opacity.vibrancy_background = None;
        }
        assert!((resolve_window_vibrancy_opacity(&light_theme) - expected_light).abs() < 0.0001);
    }

    #[test]
    fn test_resolve_window_vibrancy_opacity_prefers_theme_override_when_present() {
        let mut theme = Theme::dark_default();
        let mut opacity = theme.get_opacity();
        opacity.vibrancy_background = Some(0.62);
        // Glass mode (Tahoe hardware) resolves through the glass_veil_opacity
        // slider instead of vibrancy_background; set both so the "theme
        // override wins" invariant holds in either environment.
        opacity.glass_veil_opacity = Some(0.62);
        theme.opacity = Some(opacity);

        assert!((resolve_window_vibrancy_opacity(&theme) - 0.62).abs() < 0.0001);
    }

    #[test]
    fn test_resolve_window_vibrancy_opacity_clamps_theme_override() {
        // Set both sliders (vibrancy + glass veil) so the clamp invariant
        // holds in glass and non-glass environments alike.
        let mut theme = Theme::dark_default();
        let mut opacity = theme.get_opacity();
        opacity.vibrancy_background = Some(1.4);
        opacity.glass_veil_opacity = Some(1.4);
        theme.opacity = Some(opacity);
        assert!((resolve_window_vibrancy_opacity(&theme) - 1.0).abs() < 0.0001);

        let mut opacity = theme.get_opacity();
        opacity.vibrancy_background = Some(-0.2);
        opacity.glass_veil_opacity = Some(-0.2);
        theme.opacity = Some(opacity);
        assert!((resolve_window_vibrancy_opacity(&theme) - 0.0).abs() < 0.0001);
    }

    // ========================================================================
    // UiSurface / Action Primitive Tests
    // ========================================================================

    #[test]
    fn ui_surface_names_are_stable() {
        assert_eq!(UiSurface::NotesToolbar.as_str(), "notes_toolbar");
        assert_eq!(UiSurface::NotesExportMenu.as_str(), "notes_export_menu");
        assert_eq!(UiSurface::NotesOverlay.as_str(), "notes_overlay");
        assert_eq!(
            UiSurface::ThemeChooserPreview.as_str(),
            "theme_chooser_preview"
        );
    }

    #[test]
    fn test_vibrancy_background_with_default_theme() {
        let theme = Theme::default();
        // Default theme has vibrancy enabled
        let bg = get_vibrancy_background(&theme);
        // Should return None when vibrancy is enabled
        assert!(bg.is_none());
    }
}
