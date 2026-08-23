//! Theme type definitions
//!
//! Contains all the struct definitions for theme configuration:
//! - BackgroundOpacity, VibrancySettings, DropShadow
//! - BackgroundColors, TextColors, AccentColors, UIColors
//! - TerminalColors (ANSI 16-color palette)
//! - ColorScheme, FocusColorScheme, FocusAwareColorScheme
//! - FontConfig, Theme

// --- merged from part_01.rs ---
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::fmt;
use std::process::Command;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Cache for system appearance detection to avoid spawning subprocesses on every render
static APPEARANCE_CACHE: LazyLock<Mutex<AppearanceCache>> =
    LazyLock::new(|| Mutex::new(AppearanceCache::default()));

/// Cache for loaded theme to avoid file I/O on every render
static THEME_CACHE: LazyLock<Mutex<ThemeCache>> =
    LazyLock::new(|| Mutex::new(ThemeCache::default()));

/// How long to cache the system appearance before re-detecting
const APPEARANCE_CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalDefaultPalette {
    Light,
    Dark,
}

thread_local! {
    static TERMINAL_DEFAULT_PALETTE_HINT: Cell<Option<TerminalDefaultPalette>> = const { Cell::new(None) };
}

#[derive(Debug)]
struct AppearanceCache {
    is_dark: bool,
    last_check: Instant,
}

impl Default for AppearanceCache {
    fn default() -> Self {
        Self {
            is_dark: true,                                     // Default to dark mode
            last_check: Instant::now() - APPEARANCE_CACHE_TTL, // Force immediate check
        }
    }
}

/// Cache for loaded theme to avoid repeated file I/O
#[derive(Debug, Clone)]
struct ThemeCache {
    theme: Theme,
}

impl Default for ThemeCache {
    fn default() -> Self {
        // Create with a dark default theme - will be replaced on first load
        Self {
            theme: Theme::dark_default(),
        }
    }
}

use super::hex_color::{hex_color_option_serde, hex_color_serde, HexColor};

/// Theme appearance mode for determining light/dark rendering
///
/// This controls how the theme system renders colors and vibrancy effects.
/// - `Auto`: Detect from system preferences (macOS AppleInterfaceStyle)
/// - `Light`: Force light mode appearance
/// - `Dark`: Force dark mode appearance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceMode {
    /// Automatically detect from system preferences
    #[default]
    Auto,
    /// Force light mode appearance
    Light,
    /// Force dark mode appearance
    Dark,
}

/// Background opacity settings for window transparency
/// Values range from 0.0 (fully transparent) to 1.0 (fully opaque)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundOpacity {
    /// Main background opacity (default: 0.50)
    pub main: f32,
    /// Title bar opacity (default: 0.50)
    pub title_bar: f32,
    /// Search box/input opacity (default: 0.50)
    pub search_box: f32,
    /// Log panel opacity (default: 0.50)
    pub log_panel: f32,
    /// Selected list item background opacity.
    #[serde(default = "default_selected_opacity")]
    pub selected: f32,
    /// Hovered list item background opacity.
    #[serde(default = "default_hover_opacity")]
    pub hover: f32,
    /// Preview panel background opacity (default: 0.50)
    #[serde(default = "default_preview_opacity")]
    pub preview: f32,
    /// Dialog/popup background opacity (default: 0.50)
    #[serde(default = "default_dialog_opacity")]
    pub dialog: f32,
    /// Input field background opacity (default: 0.50)
    #[serde(default = "default_input_opacity")]
    pub input: f32,
    /// Panel/container background opacity (default: 0.50)
    #[serde(default = "default_panel_opacity")]
    pub panel: f32,
    /// Input field inactive/empty state background opacity (default: 0.50)
    #[serde(default = "default_input_inactive_opacity")]
    pub input_inactive: f32,
    /// Input field active/filled state background opacity (default: 0.50)
    #[serde(default = "default_input_active_opacity")]
    pub input_active: f32,
    /// Border inactive/empty state opacity (default: 0.125)
    #[serde(default = "default_border_inactive_opacity")]
    pub border_inactive: f32,
    /// Border active/filled state opacity (default: 0.25)
    #[serde(default = "default_border_active_opacity")]
    pub border_active: f32,
    /// Opacity for main window vibrancy background (0.0-1.0)
    /// Lower = more blur visible, Higher = more solid color
    /// Default: 0.50
    #[serde(default)]
    pub vibrancy_background: Option<f32>,
    /// Glass mode (macOS 26 Tahoe): themed veil painted over the native
    /// NSGlassEffectView backdrop. 0.0 = bare glass (demo look).
    /// None -> OPACITY_GLASS_MODE_VEIL_CAP.
    #[serde(default)]
    pub glass_veil_opacity: Option<f32>,
    /// Glass mode: NSGlassEffectView tintColor alpha (theme background hue).
    /// 0.0/None = untinted glass.
    #[serde(default)]
    pub glass_tint_opacity: Option<f32>,
    /// Glass appear morph duration in seconds (0 disables the morph).
    /// None -> GLASS_MORPH_DEFAULT_DURATION.
    #[serde(default)]
    pub glass_morph_duration: Option<f32>,
    /// Glass appear morph start inset as a fraction of each dimension.
    /// None -> GLASS_MORPH_DEFAULT_INSET.
    #[serde(default)]
    pub glass_morph_inset: Option<f32>,

    // ── Text grading tiers (Raycast-style) ──────────────────────────────
    // All text elements use text_primary (white on dark) as the base color.
    // Brightness is controlled purely via these opacity tiers (0.0–1.0).
    // Use `opacity_to_alpha(value)` to convert to u32 for rgba bit-packing.
    /// Names / primary labels — always full brightness.
    #[serde(default = "default_text_name")]
    pub text_name: f32,
    /// Badges, shortcuts, section headers — clearly readable chrome.
    #[serde(default = "default_text_strong")]
    pub text_strong: f32,
    /// Focused descriptions, source hints — readable but secondary.
    #[serde(default = "default_text_muted")]
    pub text_muted_alpha: f32,
    /// Hovered descriptions, type labels — visible hint tier.
    #[serde(default = "default_text_hint")]
    pub text_hint: f32,
    /// Placeholder text, idle captions — quiet but not invisible.
    #[serde(default = "default_text_placeholder")]
    pub text_placeholder: f32,
    /// Idle icons — recedes behind name text.
    #[serde(default = "default_text_icon")]
    pub text_icon: f32,
}

pub const DARK_ROW_SELECTED_OPACITY: f32 = 0.20;
pub const DARK_ROW_HOVER_OPACITY: f32 = 0.06;
pub const LIGHT_ROW_SELECTED_OPACITY: f32 = 0.07;
pub const LIGHT_ROW_HOVER_OPACITY: f32 = 0.04;

fn default_selected_opacity() -> f32 {
    DARK_ROW_SELECTED_OPACITY
}

fn default_hover_opacity() -> f32 {
    DARK_ROW_HOVER_OPACITY
}

fn default_preview_opacity() -> f32 {
    0.50
}

fn default_dialog_opacity() -> f32 {
    0.50
}

fn default_input_opacity() -> f32 {
    0.50
}

fn default_panel_opacity() -> f32 {
    0.50
}

fn default_input_inactive_opacity() -> f32 {
    0.50
}

fn default_input_active_opacity() -> f32 {
    0.50
}

fn default_border_inactive_opacity() -> f32 {
    0.125 // 0x20 / 255 ≈ 0.125
}

fn default_border_active_opacity() -> f32 {
    0.25 // 0x40 / 255 ≈ 0.25
}

// ── Text grading defaults (Liquid Glass) ─────────────────────────────
// Applied to text_primary. Keep labels bright while supporting copy recedes
// on translucent 50% surfaces; do not reintroduce secondary/muted hex dimming.
const TEXT_NAME_OPACITY: f32 = 1.00; // Names / primary labels (0xFF)
const TEXT_STRONG_OPACITY: f32 = 0.80; // Badges, shortcuts, section headers (0xCC)
const TEXT_MUTED_OPACITY: f32 = 0.65; // Focused descriptions, source hints (0xA5)
const TEXT_HINT_OPACITY: f32 = 0.45; // Hovered descriptions, type labels (0x72)
const TEXT_PLACEHOLDER_OPACITY: f32 = 0.40; // Placeholders, idle captions (0x66)
const TEXT_ICON_OPACITY: f32 = 0.50; // Idle icons (0x7F)

fn default_text_name() -> f32 {
    TEXT_NAME_OPACITY
}
fn default_text_strong() -> f32 {
    TEXT_STRONG_OPACITY
}
fn default_text_muted() -> f32 {
    TEXT_MUTED_OPACITY
}
fn default_text_hint() -> f32 {
    TEXT_HINT_OPACITY
}
fn default_text_placeholder() -> f32 {
    TEXT_PLACEHOLDER_OPACITY
}
fn default_text_icon() -> f32 {
    TEXT_ICON_OPACITY
}

/// Convert 0.0–1.0 opacity to u32 alpha (0x00–0xFF) for rgba bit-packing.
pub fn opacity_to_alpha(opacity: f32) -> u32 {
    (opacity.clamp(0.0, 1.0) * 255.0) as u32
}

impl Default for BackgroundOpacity {
    fn default() -> Self {
        Self::dark_default()
    }
}

impl BackgroundOpacity {
    /// Dark mode opacity defaults.
    ///
    /// Keep dark themes aligned with the theme designer's 50% surface-opacity
    /// preset so new/default themes land on a concrete preset instead of the
    /// old split shell + vibrancy background state.
    pub fn dark_default() -> Self {
        BackgroundOpacity {
            main: 0.50,                          // 50% base
            title_bar: 0.50,                     // Match main for consistency
            search_box: 0.50,                    // Match main
            log_panel: 0.50,                     // Match main
            selected: DARK_ROW_SELECTED_OPACITY, // Focused list item, strongest visible row highlight
            hover: DARK_ROW_HOVER_OPACITY, // Hovered list item, ghost-tier tint below focused state
            preview: 0.50,                 // Preview panel matches main
            dialog: 0.50,                  // Dialogs match main
            input: 0.50,                   // Match main
            panel: 0.50,                   // Panels/containers
            input_inactive: 0.50,          // Match main
            input_active: 0.50,            // Match main
            border_inactive: 0.125,        // Borders when inactive
            border_active: 0.25,           // Borders when active
            // Window-root tint over native blur. Thin theme veil only — the
            // vibrancy material (menu) carries legibility over bright
            // backdrops; a heavy tint here greys out backdrop color.
            vibrancy_background: Some(crate::theme::opacity::OPACITY_VIBRANCY_BACKGROUND),
            glass_veil_opacity: None,
            glass_tint_opacity: None,
            glass_morph_duration: None,
            glass_morph_inset: None,
            // Text grading (all applied to text_primary)
            text_name: default_text_name(),
            text_strong: default_text_strong(),
            text_muted_alpha: default_text_muted(),
            text_hint: default_text_hint(),
            text_placeholder: default_text_placeholder(),
            text_icon: default_text_icon(),
        }
    }

    /// Light mode opacity defaults - tuned for vibrancy blur visibility
    ///
    /// Lower opacity allows more blur to show through while keeping text readable.
    /// Use Cmd+Shift+[ and Cmd+Shift+] to adjust opacity in real-time.
    ///
    /// Base value is 50% and all background surfaces match the base value.
    pub fn light_default() -> Self {
        BackgroundOpacity {
            main: 0.50,                           // 50% base
            title_bar: 0.50,                      // Match main for consistency
            search_box: 0.50,                     // Match main
            log_panel: 0.50,                      // Match main
            selected: LIGHT_ROW_SELECTED_OPACITY, // Light mode focus, down to the old hover strength
            hover: LIGHT_ROW_HOVER_OPACITY,       // Light mode hover, about half the focus strength
            preview: 0.50,                        // Preview panel matches main
            dialog: 0.50,                         // Dialogs match main
            input: 0.50,                          // Match main
            panel: 0.50,                          // Panels match main
            input_inactive: 0.50,                 // Match main
            input_active: 0.50,                   // Match main
            border_inactive: 0.30,                // Borders when inactive
            border_active: 0.45,                  // Borders when active
            // Match dark mode: thin theme veil; the material carries
            // legibility (see dark_default).
            vibrancy_background: Some(crate::theme::opacity::OPACITY_VIBRANCY_BACKGROUND),
            glass_veil_opacity: None,
            glass_tint_opacity: None,
            glass_morph_duration: None,
            glass_morph_inset: None,
            // Text grading (same defaults for light mode)
            text_name: default_text_name(),
            text_strong: default_text_strong(),
            text_muted_alpha: default_text_muted(),
            text_hint: default_text_hint(),
            text_placeholder: default_text_placeholder(),
            text_icon: default_text_icon(),
        }
    }

    /// Clamp all opacity values to the valid 0.0..=1.0 range.
    pub fn clamped(mut self) -> Self {
        self.main = self.main.clamp(0.0, 1.0);
        self.title_bar = self.title_bar.clamp(0.0, 1.0);
        self.search_box = self.search_box.clamp(0.0, 1.0);
        self.log_panel = self.log_panel.clamp(0.0, 1.0);
        self.selected = self.selected.clamp(0.0, 1.0);
        self.hover = self.hover.clamp(0.0, 1.0);
        self.preview = self.preview.clamp(0.0, 1.0);
        self.dialog = self.dialog.clamp(0.0, 1.0);
        self.input = self.input.clamp(0.0, 1.0);
        self.panel = self.panel.clamp(0.0, 1.0);
        self.input_inactive = self.input_inactive.clamp(0.0, 1.0);
        self.input_active = self.input_active.clamp(0.0, 1.0);
        self.border_inactive = self.border_inactive.clamp(0.0, 1.0);
        self.border_active = self.border_active.clamp(0.0, 1.0);
        self.vibrancy_background = self.vibrancy_background.map(|value| value.clamp(0.0, 1.0));
        self.glass_veil_opacity = self.glass_veil_opacity.map(|value| value.clamp(0.0, 1.0));
        self.glass_tint_opacity = self.glass_tint_opacity.map(|value| value.clamp(0.0, 1.0));
        self.glass_morph_duration = self.glass_morph_duration.map(|value| value.clamp(0.0, 2.0));
        self.glass_morph_inset = self.glass_morph_inset.map(|value| value.clamp(0.0, 0.4));
        // Text grading tiers
        self.text_name = self.text_name.clamp(0.0, 1.0);
        self.text_strong = self.text_strong.clamp(0.0, 1.0);
        self.text_muted_alpha = self.text_muted_alpha.clamp(0.0, 1.0);
        self.text_hint = self.text_hint.clamp(0.0, 1.0);
        self.text_placeholder = self.text_placeholder.clamp(0.0, 1.0);
        self.text_icon = self.text_icon.clamp(0.0, 1.0);
        self
    }
}

/// Vibrancy material type for macOS window backgrounds
///
/// Different materials provide different levels of blur and background interaction.
/// Maps to NSVisualEffectMaterial values on macOS.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Default,
    strum::Display,
    strum::EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum VibrancyMaterial {
    /// Dark, high contrast material (like HUD windows). The most translucent
    /// material: its luminance tracks the backdrop (measured 63% body
    /// luminance over a white app at zero tint), so it needs a heavy tint to
    /// stay legible — which crushes backdrop color.
    Hud,
    /// Light blur, used in popovers
    Popover,
    /// Similar to system menus (default). The system's "stays dark over
    /// bright content" material: measured 39% body luminance over a white app
    /// at zero tint while retaining backdrop color, so a thin theme tint is
    /// enough for legibility.
    #[default]
    Menu,
    /// Sidebar-style blur
    Sidebar,
    /// Content background blur (near-opaque plate; backdrop barely shows)
    Content,
}

/// Vibrancy/blur effect settings for the window background
/// This creates the native macOS translucent effect like Spotlight/Raycast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibrancySettings {
    /// Whether vibrancy is enabled (default: true)
    pub enabled: bool,
    /// Vibrancy material type
    /// - `hud`: Dark, high contrast (like HUD windows)
    /// - `popover`: Light blur, used in popovers
    /// - `menu`: Similar to system menus (default)
    /// - `sidebar`: Sidebar-style blur
    /// - `content`: Content background blur
    #[serde(default)]
    pub material: VibrancyMaterial,
    /// Saturation multiplier applied to the blurred backdrop before the
    /// material and window tints composite over it (the `colorSaturate`
    /// filter on the backdrop layer). Materials ship their own default
    /// (menu: 2.4); raising it makes backdrop color glow through the dark
    /// panel without changing its darkness — the Raycast look. Clamped to
    /// 0.0..=4.0 at the apply site.
    #[serde(default = "default_backdrop_saturation")]
    pub backdrop_saturation: f32,
}

/// Default backdrop saturation boost. Measured on the menu material over a
/// rainbow backdrop: the material's built-in 2.4 retains ~30% body
/// saturation; 2.6 retains ~36% at identical luminance.
pub const DEFAULT_BACKDROP_SATURATION: f32 = 2.6;

fn default_backdrop_saturation() -> f32 {
    DEFAULT_BACKDROP_SATURATION
}

impl Default for VibrancySettings {
    fn default() -> Self {
        VibrancySettings {
            enabled: true,
            material: VibrancyMaterial::default(),
            backdrop_saturation: DEFAULT_BACKDROP_SATURATION,
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
    /// Shadow color as hex (default: #000000 - black)
    #[serde(with = "hex_color_serde")]
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

impl DropShadow {
    /// Clamp drop shadow values to valid ranges.
    ///
    /// - `opacity` is clamped to 0.0..=1.0
    /// - `blur_radius` is clamped to >= 0.0
    /// - `spread_radius` is clamped to >= 0.0
    /// - Offsets are left unchanged to allow directional shadows
    pub fn clamped(mut self) -> Self {
        self.opacity = self.opacity.clamp(0.0, 1.0);
        self.blur_radius = self.blur_radius.max(0.0);
        self.spread_radius = self.spread_radius.max(0.0);
        self
    }
}

/// Optional two-stop background gradient for the main Script Kit shell.
///
/// This stays deliberately small so user-authored themes can add visual
/// texture without pulling CSS-style background complexity into theme.json.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BackgroundGradient {
    /// Whether the gradient should be rendered.
    pub enabled: bool,
    /// Starting color for the base gradient.
    #[serde(with = "hex_color_serde")]
    pub from: HexColor,
    /// Ending color for the base gradient.
    #[serde(with = "hex_color_serde")]
    pub to: HexColor,
    /// Angle in degrees for the base gradient.
    pub angle: f32,
    /// Alpha applied to base gradient stops.
    pub opacity: f32,
    /// Optional additional overlapping layers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<GradientLayer>,
}

/// A single overlapping gradient layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GradientLayer {
    /// Whether this layer is enabled.
    pub enabled: bool,
    /// Starting color for this layer.
    #[serde(with = "hex_color_serde")]
    pub from: HexColor,
    /// Ending color for this layer.
    #[serde(with = "hex_color_serde")]
    pub to: HexColor,
    /// Angle in degrees for this layer.
    pub angle: f32,
    /// Opacity for this layer (0.0-1.0).
    pub opacity: f32,
}

impl Default for GradientLayer {
    fn default() -> Self {
        Self {
            enabled: true,
            from: 0xFB22FF, // Fun pink default
            to: 0x22E0FF,   // Fun cyan default
            angle: 45.0,
            opacity: 0.25,
        }
    }
}

impl Default for BackgroundGradient {
    fn default() -> Self {
        Self {
            enabled: false,
            from: 0x1e1e1e,
            to: 0x2d2d30,
            angle: 135.0,
            opacity: 0.35,
            layers: Vec::new(),
        }
    }
}

impl BackgroundGradient {
    pub fn clamped(mut self) -> Self {
        self.angle = self.angle.rem_euclid(360.0);
        self.opacity = self.opacity.clamp(0.0, 1.0);
        for layer in self.layers.iter_mut() {
            layer.angle = layer.angle.rem_euclid(360.0);
            layer.opacity = layer.opacity.clamp(0.0, 1.0);
        }
        self
    }
}

/// Background color definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundColors {
    /// Main background (#1E1E1E)
    #[serde(with = "hex_color_serde")]
    pub main: HexColor,
    /// Title bar background (#2D2D30)
    #[serde(with = "hex_color_serde")]
    pub title_bar: HexColor,
    /// Search box background (#3C3C3C)
    #[serde(with = "hex_color_serde")]
    pub search_box: HexColor,
    /// Log panel background (#0D0D0D)
    #[serde(with = "hex_color_serde")]
    pub log_panel: HexColor,
}

// --- merged from part_02.rs ---
/// Text color definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextColors {
    /// Primary text color (#FFFFFF - white)
    #[serde(with = "hex_color_serde")]
    pub primary: HexColor,
    /// Secondary text color (#CCCCCC - light gray)
    #[serde(with = "hex_color_serde")]
    pub secondary: HexColor,
    /// Tertiary text color (#999999)
    #[serde(with = "hex_color_serde")]
    pub tertiary: HexColor,
    /// Muted text color (#808080)
    #[serde(with = "hex_color_serde")]
    pub muted: HexColor,
    /// Dimmed text color (#666666)
    #[serde(with = "hex_color_serde")]
    pub dimmed: HexColor,
    /// Text color for content on accent backgrounds (#FFFFFF - white for dark themes)
    /// Used for text on selected items, warning banners, etc.
    #[serde(with = "hex_color_serde", default = "default_text_on_accent")]
    pub on_accent: HexColor,
}

fn default_text_on_accent() -> HexColor {
    0xffffff // White provides good contrast on most accent colors
}

/// Accent and highlight colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccentColors {
    /// Primary accent color (#FBBF24 - yellow/gold for Script Kit)
    /// Used for: selected items, button text, logo, highlights
    #[serde(with = "hex_color_serde")]
    pub selected: HexColor,
    /// Subtle selection for list items - barely visible highlight (#2A2A2A - dark gray)
    /// Used for polished, Raycast-like selection backgrounds
    #[serde(default = "default_selected_subtle", with = "hex_color_serde")]
    pub selected_subtle: HexColor,
}

/// Default subtle selection color
/// Uses white for near-invisible Raycast-like highlighting
fn default_selected_subtle() -> HexColor {
    0x5a5a5a // Optimal: closest to dark bg that passes 4.5:1 contrast at selected opacity
}

/// Border and UI element colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIColors {
    /// Border color (#464647)
    #[serde(with = "hex_color_serde")]
    pub border: HexColor,
    /// Success color for logs (#00FF00 - green)
    #[serde(with = "hex_color_serde")]
    pub success: HexColor,
    /// Error color for error messages (#EF4444 - red-500)
    #[serde(default = "default_error_color", with = "hex_color_serde")]
    pub error: HexColor,
    /// Warning color for warning messages (#F59E0B - amber-500)
    #[serde(default = "default_warning_color", with = "hex_color_serde")]
    pub warning: HexColor,
    /// Info color for informational messages (#3B82F6 - blue-500)
    #[serde(default = "default_info_color", with = "hex_color_serde")]
    pub info: HexColor,
}

/// Terminal ANSI color palette (16 colors)
///
/// These colors are used by the embedded terminal emulator for ANSI escape sequences.
/// Colors 0-7 are the normal palette, colors 8-15 are the bright variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalColors {
    /// Optional default terminal foreground color.
    ///
    /// When unset, terminal rendering falls back to the theme text foreground.
    #[serde(
        default,
        with = "hex_color_option_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub foreground: Option<HexColor>,
    /// Optional default terminal background color.
    ///
    /// When unset, terminal rendering falls back to the theme main background.
    #[serde(
        default,
        with = "hex_color_option_serde",
        skip_serializing_if = "Option::is_none"
    )]
    pub background: Option<HexColor>,
    /// ANSI 0: Black
    #[serde(default = "default_terminal_black", with = "hex_color_serde")]
    pub black: HexColor,
    /// ANSI 1: Red
    #[serde(default = "default_terminal_red", with = "hex_color_serde")]
    pub red: HexColor,
    /// ANSI 2: Green
    #[serde(default = "default_terminal_green", with = "hex_color_serde")]
    pub green: HexColor,
    /// ANSI 3: Yellow
    #[serde(default = "default_terminal_yellow", with = "hex_color_serde")]
    pub yellow: HexColor,
    /// ANSI 4: Blue
    #[serde(default = "default_terminal_blue", with = "hex_color_serde")]
    pub blue: HexColor,
    /// ANSI 5: Magenta
    #[serde(default = "default_terminal_magenta", with = "hex_color_serde")]
    pub magenta: HexColor,
    /// ANSI 6: Cyan
    #[serde(default = "default_terminal_cyan", with = "hex_color_serde")]
    pub cyan: HexColor,
    /// ANSI 7: White
    #[serde(default = "default_terminal_white", with = "hex_color_serde")]
    pub white: HexColor,
    /// ANSI 8: Bright Black (Gray)
    #[serde(default = "default_terminal_bright_black", with = "hex_color_serde")]
    pub bright_black: HexColor,
    /// ANSI 9: Bright Red
    #[serde(default = "default_terminal_bright_red", with = "hex_color_serde")]
    pub bright_red: HexColor,
    /// ANSI 10: Bright Green
    #[serde(default = "default_terminal_bright_green", with = "hex_color_serde")]
    pub bright_green: HexColor,
    /// ANSI 11: Bright Yellow
    #[serde(default = "default_terminal_bright_yellow", with = "hex_color_serde")]
    pub bright_yellow: HexColor,
    /// ANSI 12: Bright Blue
    #[serde(default = "default_terminal_bright_blue", with = "hex_color_serde")]
    pub bright_blue: HexColor,
    /// ANSI 13: Bright Magenta
    #[serde(default = "default_terminal_bright_magenta", with = "hex_color_serde")]
    pub bright_magenta: HexColor,
    /// ANSI 14: Bright Cyan
    #[serde(default = "default_terminal_bright_cyan", with = "hex_color_serde")]
    pub bright_cyan: HexColor,
    /// ANSI 15: Bright White
    #[serde(default = "default_terminal_bright_white", with = "hex_color_serde")]
    pub bright_white: HexColor,
}

impl Default for TerminalColors {
    fn default() -> Self {
        Self::dark_default()
    }
}

impl TerminalColors {
    /// Dark mode terminal colors (Dracula/One Dark inspired for better visibility)
    pub fn dark_default() -> Self {
        TerminalColors {
            foreground: None,
            background: None,
            black: 0x000000,
            red: 0xcd3131,
            green: 0x50fa7b, // Dracula green - vibrant for executables
            yellow: 0xe5e510,
            blue: 0x5c9ceb, // Brighter blue for directories
            magenta: 0xbc3fbc,
            cyan: 0x56d4e2, // Brighter cyan for symlinks
            white: 0xe5e5e5,
            bright_black: 0x666666,
            bright_red: 0xf14c4c,
            bright_green: 0x69ff94, // Very bright green
            bright_yellow: 0xf5f543,
            bright_blue: 0x6eb4ff, // Vibrant blue for directories
            bright_magenta: 0xd670d6,
            bright_cyan: 0x8be9fd, // Dracula cyan - very visible
            bright_white: 0xffffff,
        }
    }

    /// Light mode terminal colors
    pub fn light_default() -> Self {
        TerminalColors {
            foreground: None,
            background: None,
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
    /// Cursor color when focused (#00FFFF - cyan)
    #[serde(with = "hex_color_serde")]
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
    /// Optional background gradient rendered behind the app shell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_gradient: Option<BackgroundGradient>,
    /// Font configuration for editor and terminal
    #[serde(default)]
    pub fonts: Option<FontConfig>,
    /// Appearance mode: Auto (detect from system), Light, or Dark
    ///
    /// Controls how the theme renders colors and vibrancy effects.
    /// When set to Auto (default), the system appearance is detected.
    #[serde(default)]
    pub appearance: AppearanceMode,
}

impl CursorStyle {
    /// Create a default blinking cursor style
    pub fn default_focused() -> Self {
        CursorStyle {
            color: 0x00ffff, // Cyan cursor when focused
            blink_interval_ms: 500,
        }
    }
}

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

// --- merged from part_03.rs ---
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
                on_accent: 0x1e1e1e, // Dark text for focused items on bright accent backgrounds
            },
            accent: AccentColors {
                selected: 0xfbbf24,        // Script Kit primary: #fbbf24 (yellow/gold)
                selected_subtle: 0x5a5a5a, // Optimal: closest to bg that passes 4.5:1 at selected opacity
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
    ///
    /// Colors derived from POC testing with Raycast-like light theme.
    /// Key differences from dark mode:
    /// - `selected_subtle` is BLACK (0x000000) instead of white for visible selection
    /// - Higher contrast text colors
    /// - Darker UI colors for visibility on light backgrounds
    pub fn light_default() -> Self {
        ColorScheme {
            background: BackgroundColors {
                main: 0xfafafa,       // Light gray from POC (0xFAFAFA)
                title_bar: 0xffffff,  // Pure white for input area
                search_box: 0xffffff, // Pure white for search
                log_panel: 0xf5f5f5,  // Slightly darker for terminal
            },
            text: TextColors {
                primary: 0x000000,   // Pure black for maximum contrast
                secondary: 0x4a4a4a, // Darker gray for better readability
                tertiary: 0x6b6b6b,  // Medium gray for hints
                muted: 0x808080,     // Mid gray for placeholders
                dimmed: 0x999999,    // Subtle but readable on light backgrounds
                on_accent: 0xffffff, // White text on accent backgrounds
            },
            accent: AccentColors {
                selected: 0x0078d4, // Blue accent for light mode
                // CRITICAL: Black for light mode selection visibility
                // White at low opacity is INVISIBLE on white backgrounds
                // Black at low opacity creates visible darkening effect
                selected_subtle: 0x000000,
            },
            ui: UIColors {
                border: 0xe0e0e0,  // Light border from POC (0xE0E0E0)
                success: 0x22c55e, // green-500
                error: 0xdc2626,   // red-600 (darker for light mode)
                warning: 0xd97706, // amber-600 (darker for light mode)
                info: 0x2563eb,    // blue-600 (darker for light mode)
            },
            terminal: TerminalColors::light_default(),
        }
    }

    /// Create an unfocused (dimmed) version of this color scheme
    pub fn to_unfocused(&self) -> Self {
        fn blend_toward(color: HexColor, target: HexColor, pct: f32) -> HexColor {
            let mix_channel = |source: u32, destination: u32| -> u32 {
                ((source as f32 * (1.0 - pct)) + (destination as f32 * pct)).round() as u32
            };

            let color_r = (color >> 16) & 0xFF;
            let color_g = (color >> 8) & 0xFF;
            let color_b = color & 0xFF;
            let target_r = (target >> 16) & 0xFF;
            let target_g = (target >> 8) & 0xFF;
            let target_b = target & 0xFF;

            let new_r = mix_channel(color_r, target_r);
            let new_g = mix_channel(color_g, target_g);
            let new_b = mix_channel(color_b, target_b);

            (new_r << 16) | (new_g << 8) | new_b
        }

        let is_dark = relative_luminance_srgb(self.background.main) < 0.5;
        let background_candidates = [
            self.background.main,
            self.background.title_bar,
            self.background.search_box,
            self.background.log_panel,
        ];

        let mut darkest_background = background_candidates[0];
        let mut lightest_background = background_candidates[0];
        let mut darkest_luminance = relative_luminance_srgb(darkest_background);
        let mut lightest_luminance = darkest_luminance;

        for candidate in &background_candidates[1..] {
            let candidate_luminance = relative_luminance_srgb(*candidate);
            if candidate_luminance < darkest_luminance {
                darkest_luminance = candidate_luminance;
                darkest_background = *candidate;
            }
            if candidate_luminance > lightest_luminance {
                lightest_luminance = candidate_luminance;
                lightest_background = *candidate;
            }
        }

        let blend_target = if is_dark {
            darkest_background
        } else {
            lightest_background
        };
        let blend_pct = 0.18;

        ColorScheme {
            background: BackgroundColors {
                main: blend_toward(self.background.main, blend_target, blend_pct),
                title_bar: blend_toward(self.background.title_bar, blend_target, blend_pct),
                search_box: blend_toward(self.background.search_box, blend_target, blend_pct),
                log_panel: blend_toward(self.background.log_panel, blend_target, blend_pct),
            },
            text: TextColors {
                primary: blend_toward(self.text.primary, blend_target, blend_pct),
                secondary: blend_toward(self.text.secondary, blend_target, blend_pct),
                tertiary: blend_toward(self.text.tertiary, blend_target, blend_pct),
                muted: blend_toward(self.text.muted, blend_target, blend_pct),
                dimmed: blend_toward(self.text.dimmed, blend_target, blend_pct),
                on_accent: blend_toward(self.text.on_accent, blend_target, blend_pct),
            },
            accent: AccentColors {
                selected: blend_toward(self.accent.selected, blend_target, blend_pct),
                selected_subtle: blend_toward(self.accent.selected_subtle, blend_target, blend_pct),
            },
            ui: UIColors {
                border: blend_toward(self.ui.border, blend_target, blend_pct),
                success: blend_toward(self.ui.success, blend_target, blend_pct),
                error: blend_toward(self.ui.error, blend_target, blend_pct),
                warning: blend_toward(self.ui.warning, blend_target, blend_pct),
                info: blend_toward(self.ui.info, blend_target, blend_pct),
            },
            terminal: TerminalColors {
                foreground: self
                    .terminal
                    .foreground
                    .map(|color| blend_toward(color, blend_target, blend_pct)),
                background: self
                    .terminal
                    .background
                    .map(|color| blend_toward(color, blend_target, blend_pct)),
                black: blend_toward(self.terminal.black, blend_target, blend_pct),
                red: blend_toward(self.terminal.red, blend_target, blend_pct),
                green: blend_toward(self.terminal.green, blend_target, blend_pct),
                yellow: blend_toward(self.terminal.yellow, blend_target, blend_pct),
                blue: blend_toward(self.terminal.blue, blend_target, blend_pct),
                magenta: blend_toward(self.terminal.magenta, blend_target, blend_pct),
                cyan: blend_toward(self.terminal.cyan, blend_target, blend_pct),
                white: blend_toward(self.terminal.white, blend_target, blend_pct),
                bright_black: blend_toward(self.terminal.bright_black, blend_target, blend_pct),
                bright_red: blend_toward(self.terminal.bright_red, blend_target, blend_pct),
                bright_green: blend_toward(self.terminal.bright_green, blend_target, blend_pct),
                bright_yellow: blend_toward(self.terminal.bright_yellow, blend_target, blend_pct),
                bright_blue: blend_toward(self.terminal.bright_blue, blend_target, blend_pct),
                bright_magenta: blend_toward(self.terminal.bright_magenta, blend_target, blend_pct),
                bright_cyan: blend_toward(self.terminal.bright_cyan, blend_target, blend_pct),
                bright_white: blend_toward(self.terminal.bright_white, blend_target, blend_pct),
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
            background_gradient: None,
            fonts: Some(FontConfig::default()),
            appearance: AppearanceMode::default(),
        }
    }
}

/// Calculate relative luminance for an sRGB hex color using gamma-corrected channels.
///
/// Uses WCAG sRGB linearization per channel before weighting:
/// `0.2126 * r + 0.7152 * g + 0.0722 * b`.
pub(crate) fn relative_luminance_srgb(hex: u32) -> f32 {
    let linearize = |offset: u32| {
        let channel = ((hex >> offset) & 0xFF) as f32 / 255.0;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };

    let r = linearize(16);
    let g = linearize(8);
    let b = linearize(0);
    (0.2126 * r) + (0.7152 * g) + (0.0722 * b)
}

impl Theme {
    /// Determine if the theme should render in dark mode
    ///
    /// This is the canonical method for checking theme appearance throughout the app.
    /// It handles the AppearanceMode enum and system detection.
    ///
    /// Returns:
    /// - `true` for dark mode rendering
    /// - `false` for light mode rendering
    pub fn is_dark_mode(&self) -> bool {
        match self.appearance {
            AppearanceMode::Dark => true,
            AppearanceMode::Light => false,
            AppearanceMode::Auto => detect_system_appearance(),
        }
    }

    /// Check if the theme's actual colors are dark (based on background luminance)
    ///
    /// This is used to determine vibrancy appearance (VibrantDark vs VibrantLight)
    /// regardless of system appearance setting. This ensures the blur effect matches
    /// the actual color scheme being used.
    ///
    /// Returns:
    /// - `true` if background color luminance < 0.5 (dark colors)
    /// - `false` if background color luminance >= 0.5 (light colors)
    pub fn has_dark_colors(&self) -> bool {
        relative_luminance_srgb(self.colors.background.main) < 0.5
    }

    /// Determine if vibrancy should use dark appearance
    ///
    /// This checks the actual theme colors to determine whether to use
    /// VibrantDark or VibrantLight NSAppearance. This is separate from
    /// is_dark_mode() because we want vibrancy to match the colors being
    /// displayed, not the system preference.
    pub fn should_use_dark_vibrancy(&self) -> bool {
        self.has_dark_colors()
    }

    /// Create a light theme with appropriate defaults
    pub fn light_default() -> Self {
        Theme {
            colors: ColorScheme::light_default(),
            focus_aware: None,
            opacity: Some(BackgroundOpacity::light_default()),
            drop_shadow: Some(DropShadow {
                // Lighter shadow for light theme
                opacity: 0.12,
                ..DropShadow::default()
            }),
            vibrancy: Some(VibrancySettings::default()),
            background_gradient: None,
            fonts: Some(FontConfig::default()),
            appearance: AppearanceMode::Light,
        }
    }

    /// Create a dark theme with appropriate defaults
    pub fn dark_default() -> Self {
        Theme {
            colors: ColorScheme::dark_default(),
            focus_aware: None,
            opacity: Some(BackgroundOpacity::dark_default()),
            drop_shadow: Some(DropShadow::default()),
            vibrancy: Some(VibrancySettings::default()),
            background_gradient: None,
            fonts: Some(FontConfig::default()),
            appearance: AppearanceMode::Dark,
        }
    }
}

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
                return focused
                    .cursor
                    .clone()
                    .or_else(|| Some(CursorStyle::default_focused()));
            }
        }

        // Return default blinking cursor if focused
        Some(CursorStyle::default_focused())
    }

    /// Get background opacity settings
    /// Returns the configured opacity or sensible defaults
    pub fn get_opacity(&self) -> BackgroundOpacity {
        self.opacity
            .clone()
            .unwrap_or_else(|| {
                if self.is_dark_mode() {
                    BackgroundOpacity::dark_default()
                } else {
                    BackgroundOpacity::light_default()
                }
            })
            .clamped()
    }

    /// Create a new theme with opacity adjusted by an offset
    ///
    /// Use Cmd+Shift+[ to decrease and Cmd+Shift+] to increase opacity.
    /// The offset is added to all opacity values (clamped to 0.0-1.0).
    ///
    /// # Arguments
    /// * `offset` - The amount to add to opacity values (can be negative)
    ///
    /// # Returns
    /// A new Theme with adjusted opacity values
    pub fn with_opacity_offset(&self, offset: f32) -> Theme {
        let mut theme = self.clone();
        let base = theme.get_opacity();
        theme.opacity = Some(BackgroundOpacity {
            main: (base.main + offset).clamp(0.0, 1.0),
            title_bar: (base.title_bar + offset).clamp(0.0, 1.0),
            search_box: (base.search_box + offset).clamp(0.0, 1.0),
            log_panel: (base.log_panel + offset).clamp(0.0, 1.0),
            selected: base.selected, // Keep selection/hover unchanged
            hover: base.hover,
            preview: base.preview,
            dialog: (base.dialog + offset).clamp(0.0, 1.0),
            input: (base.input + offset).clamp(0.0, 1.0),
            panel: (base.panel + offset).clamp(0.0, 1.0),
            input_inactive: (base.input_inactive + offset).clamp(0.0, 1.0),
            input_active: (base.input_active + offset).clamp(0.0, 1.0),
            border_inactive: base.border_inactive,
            border_active: base.border_active,
            vibrancy_background: base.vibrancy_background,
            glass_veil_opacity: base.glass_veil_opacity,
            glass_tint_opacity: base.glass_tint_opacity,
            glass_morph_duration: base.glass_morph_duration,
            glass_morph_inset: base.glass_morph_inset,
            // Text grading: keep unchanged on opacity offset
            text_name: base.text_name,
            text_strong: base.text_strong,
            text_muted_alpha: base.text_muted_alpha,
            text_hint: base.text_hint,
            text_placeholder: base.text_placeholder,
            text_icon: base.text_icon,
        });
        theme
    }

    /// Get drop shadow configuration
    /// Returns the configured shadow or sensible defaults
    pub fn get_drop_shadow(&self) -> DropShadow {
        self.drop_shadow.clone().unwrap_or_default().clamped()
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

    /// Get a clamped background gradient when enabled.
    pub fn active_background_gradient(&self) -> Option<BackgroundGradient> {
        self.background_gradient
            .clone()
            .map(BackgroundGradient::clamped)
            .filter(|gradient| gradient.enabled && gradient.opacity > 0.0)
    }

    /// Get font configuration
    /// Returns the configured fonts or sensible defaults
    pub fn get_fonts(&self) -> FontConfig {
        self.fonts.clone().unwrap_or_default()
    }
}

/// Detect system appearance preference on macOS (cached)
///
/// Returns true if dark mode is enabled, false if light mode is enabled.
/// On non-macOS systems or if detection fails, defaults to true (dark mode).
///
/// This function caches the result for 5 seconds to avoid spawning subprocesses
/// on every render call. The system appearance doesn't change frequently, so
/// a small TTL is acceptable.
///
/// Uses the `defaults read -g AppleInterfaceStyle` command to detect the system appearance.
/// Note: On macOS in light mode, the command exits with non-zero status because the
/// AppleInterfaceStyle key doesn't exist, so we check exit status explicitly.
pub fn detect_system_appearance() -> bool {
    let cache = &*APPEARANCE_CACHE;

    let mut cache_guard = match cache.lock() {
        Ok(guard) => guard,
        Err(_) => {
            // Mutex poisoned, return default
            return true;
        }
    };

    // Check if cache is still valid
    if cache_guard.last_check.elapsed() < APPEARANCE_CACHE_TTL {
        return cache_guard.is_dark;
    }

    // Cache expired, re-detect
    let is_dark = detect_system_appearance_uncached();
    cache_guard.is_dark = is_dark;
    cache_guard.last_check = Instant::now();
    is_dark
}

/// Invalidate the appearance cache
///
/// Call this when the system appearance changes (e.g., from observe_window_appearance)
/// to force immediate re-detection on the next call to `detect_system_appearance()`.
pub fn invalidate_appearance_cache() {
    if let Ok(mut guard) = APPEARANCE_CACHE.lock() {
        // Set last_check to past the TTL so next call will re-detect
        guard.last_check = Instant::now() - APPEARANCE_CACHE_TTL - Duration::from_secs(1);
        debug!("Appearance cache invalidated");
    }
}

/// Uncached system appearance detection (internal use)
fn detect_system_appearance_uncached() -> bool {
    // Default to dark mode if detection fails or we're not on macOS
    const DEFAULT_DARK: bool = true;

    // Try to detect macOS dark mode using system defaults
    match Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
    {
        Ok(output) => {
            // In light mode, the AppleInterfaceStyle key typically doesn't exist,
            // causing the command to exit with non-zero status
            if !output.status.success() {
                debug!(
                    appearance = "light",
                    "System appearance detected (key not present)"
                );
                return false; // light mode
            }

            // If the command succeeds and returns "Dark", we're in dark mode
            let stdout = String::from_utf8_lossy(&output.stdout);
            let is_dark = stdout.to_lowercase().contains("dark");
            debug!(
                appearance = if is_dark { "dark" } else { "light" },
                "System appearance detected"
            );
            is_dark
        }
        Err(e) => {
            // Command failed to execute (e.g., not on macOS, or `defaults` not found)
            debug!(
                error = %e,
                default = DEFAULT_DARK,
                "System appearance detection failed, using default"
            );
            DEFAULT_DARK
        }
    }
}

// --- merged from part_05.rs ---
fn default_theme_from_system_appearance() -> Theme {
    let theme = if detect_system_appearance() {
        Theme::dark_default()
    } else {
        Theme::light_default()
    };
    normalize_theme_primary_text(theme)
}

fn normalize_focus_scheme_primary_text(colors: &mut FocusColorScheme) {
    colors.text.primary = super::helpers::hard_readable_text_hex(colors.background.main);
}

fn normalize_theme_primary_text(mut theme: Theme) -> Theme {
    theme.colors.text.primary =
        super::helpers::hard_readable_text_hex(theme.colors.background.main);

    if let Some(focus_aware) = theme.focus_aware.as_mut() {
        if let Some(focused) = focus_aware.focused.as_mut() {
            normalize_focus_scheme_primary_text(focused);
        }

        if let Some(unfocused) = focus_aware.unfocused.as_mut() {
            normalize_focus_scheme_primary_text(unfocused);
        }
    }

    theme
}

fn merge_json(base: &mut serde_json::Value, overlay: serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) => {
            for (key, overlay_value) in overlay_map {
                if let Some(base_value) = base_map.get_mut(&key) {
                    merge_json(base_value, overlay_value);
                } else {
                    base_map.insert(key, overlay_value);
                }
            }
        }
        (base_value, overlay_value) => {
            *base_value = overlay_value;
        }
    }
}

fn should_use_light_palette(appearance: AppearanceMode, is_system_dark: bool) -> bool {
    match appearance {
        AppearanceMode::Light => true,
        AppearanceMode::Dark => false,
        AppearanceMode::Auto => !is_system_dark,
    }
}

fn set_requested_appearance_on_theme_json(
    merged_theme_json: &mut serde_json::Value,
    requested_appearance: AppearanceMode,
) {
    if let Some(map) = merged_theme_json.as_object_mut() {
        let appearance_str = match requested_appearance {
            AppearanceMode::Auto => "auto",
            AppearanceMode::Light => "light",
            AppearanceMode::Dark => "dark",
        };
        map.insert(
            "appearance".to_string(),
            serde_json::Value::String(appearance_str.to_string()),
        );
    }
}

fn terminal_palette_for_appearance(
    appearance: AppearanceMode,
    is_system_dark: bool,
) -> TerminalDefaultPalette {
    if should_use_light_palette(appearance, is_system_dark) {
        TerminalDefaultPalette::Light
    } else {
        TerminalDefaultPalette::Dark
    }
}

fn terminal_defaults_for_palette(palette: TerminalDefaultPalette) -> TerminalColors {
    match palette {
        TerminalDefaultPalette::Light => TerminalColors::light_default(),
        TerminalDefaultPalette::Dark => TerminalColors::dark_default(),
    }
}

fn with_terminal_default_palette_hint<T>(
    palette: TerminalDefaultPalette,
    f: impl FnOnce() -> T,
) -> T {
    TERMINAL_DEFAULT_PALETTE_HINT.with(|hint| {
        let previous = hint.replace(Some(palette));
        let result = f();
        hint.set(previous);
        result
    })
}

fn terminal_default_palette_for_serde() -> TerminalDefaultPalette {
    TERMINAL_DEFAULT_PALETTE_HINT
        .with(Cell::get)
        .unwrap_or_else(|| {
            terminal_palette_for_appearance(AppearanceMode::Auto, detect_system_appearance())
        })
}

fn terminal_defaults_for_serde_fallback() -> TerminalColors {
    terminal_defaults_for_palette(terminal_default_palette_for_serde())
}

macro_rules! define_terminal_component_default {
    ($fn_name:ident, $field:ident) => {
        fn $fn_name() -> HexColor {
            terminal_defaults_for_serde_fallback().$field
        }
    };
}

define_terminal_component_default!(default_terminal_black, black);
define_terminal_component_default!(default_terminal_red, red);
define_terminal_component_default!(default_terminal_green, green);
define_terminal_component_default!(default_terminal_yellow, yellow);
define_terminal_component_default!(default_terminal_blue, blue);
define_terminal_component_default!(default_terminal_magenta, magenta);
define_terminal_component_default!(default_terminal_cyan, cyan);
define_terminal_component_default!(default_terminal_white, white);
define_terminal_component_default!(default_terminal_bright_black, bright_black);
define_terminal_component_default!(default_terminal_bright_red, bright_red);
define_terminal_component_default!(default_terminal_bright_green, bright_green);
define_terminal_component_default!(default_terminal_bright_yellow, bright_yellow);
define_terminal_component_default!(default_terminal_bright_blue, bright_blue);
define_terminal_component_default!(default_terminal_bright_magenta, bright_magenta);
define_terminal_component_default!(default_terminal_bright_cyan, bright_cyan);
define_terminal_component_default!(default_terminal_bright_white, bright_white);

fn terminal_defaults_for_appearance(
    appearance: AppearanceMode,
    is_system_dark: bool,
) -> TerminalColors {
    terminal_defaults_for_palette(terminal_palette_for_appearance(appearance, is_system_dark))
}

fn hydrate_terminal_colors_for_color_scheme_json(
    color_scheme_json: &mut serde_json::Value,
    terminal_defaults_json: &serde_json::Value,
) {
    let Some(color_scheme_map) = color_scheme_json.as_object_mut() else {
        return;
    };

    let existing_terminal_json = color_scheme_map.remove("terminal");
    let mut hydrated_terminal_json = terminal_defaults_json.clone();

    if let Some(existing_terminal_json) = existing_terminal_json {
        if existing_terminal_json.is_object() {
            merge_json(&mut hydrated_terminal_json, existing_terminal_json);
        } else {
            warn!(
                terminal_json = ?existing_terminal_json,
                "Theme terminal colors must be an object; using appearance-aware defaults"
            );
        }
    }

    color_scheme_map.insert("terminal".to_string(), hydrated_terminal_json);
}

fn hydrate_terminal_colors_for_deserialize(
    merged_theme_json: &mut serde_json::Value,
    is_system_dark: bool,
) {
    let Some(theme_map) = merged_theme_json.as_object_mut() else {
        warn!("Theme JSON root is not an object; skipping terminal color hydration");
        return;
    };

    let appearance = theme_map
        .get("appearance")
        .cloned()
        .and_then(|value| serde_json::from_value::<AppearanceMode>(value).ok())
        .unwrap_or_default();

    let terminal_defaults_json =
        match serde_json::to_value(terminal_defaults_for_appearance(appearance, is_system_dark)) {
            Ok(value) => value,
            Err(e) => {
                error!(
                    error = ?e,
                    appearance = ?appearance,
                    "Failed to serialize terminal defaults for hydration"
                );
                return;
            }
        };

    if let Some(colors_json) = theme_map.get_mut("colors") {
        hydrate_terminal_colors_for_color_scheme_json(colors_json, &terminal_defaults_json);
    }

    if let Some(focus_aware_json) = theme_map.get_mut("focus_aware") {
        if let Some(focus_aware_map) = focus_aware_json.as_object_mut() {
            if let Some(focused_json) = focus_aware_map.get_mut("focused") {
                hydrate_terminal_colors_for_color_scheme_json(
                    focused_json,
                    &terminal_defaults_json,
                );
            }

            if let Some(unfocused_json) = focus_aware_map.get_mut("unfocused") {
                hydrate_terminal_colors_for_color_scheme_json(
                    unfocused_json,
                    &terminal_defaults_json,
                );
            }
        }
    }
}

fn theme_from_user_preferences(
    preferences: &crate::config::ScriptKitUserPreferences,
    correlation_id: &str,
) -> Option<Theme> {
    let preset_id = preferences.theme.preset_id.as_ref()?.trim();
    if preset_id.is_empty() {
        warn!(
            correlation_id = %correlation_id,
            "Theme preset id in config is empty; ignoring"
        );
        return None;
    }

    let preset = super::presets::all_presets()
        .into_iter()
        .find(|candidate| candidate.id == preset_id);

    match preset {
        Some(selected) => {
            debug!(
                correlation_id = %correlation_id,
                preset_id = selected.id,
                preset_name = selected.name,
                "Using theme preset from config"
            );
            Some(normalize_theme_primary_text(selected.create_theme()))
        }
        None => {
            warn!(
                correlation_id = %correlation_id,
                preset_id,
                "Unknown theme preset id in config; falling back to theme file/default"
            );
            None
        }
    }
}

fn load_theme_from_user_preferences(correlation_id: &str) -> Option<Theme> {
    let preferences = crate::config::load_user_preferences();
    theme_from_user_preferences(&preferences, correlation_id)
}

fn log_theme_load_result(correlation_id: &str, source: &str, theme: &Theme) {
    info!(
        correlation_id = %correlation_id,
        source,
        appearance = ?theme.appearance,
        has_dark_colors = theme.has_dark_colors(),
        vibrancy_enabled = theme.is_vibrancy_enabled(),
        focus_aware_present = theme.focus_aware.is_some(),
        "Theme load completed"
    );
}

/// Load theme from `<SK_PATH>/theme.json` (or `~/.scriptkit/theme.json`)
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
    let correlation_id = format!("theme_load:{}", uuid::Uuid::new_v4());

    if let Some(theme) = load_theme_from_user_preferences(&correlation_id) {
        log_theme_load_result(&correlation_id, "user_preferences", &theme);
        log_theme_config(&theme);
        return theme;
    }

    let theme_path = crate::setup::theme_json_path();

    // Check if theme file exists
    if !theme_path.exists() {
        warn!(
            correlation_id = %correlation_id,
            path = %theme_path.display(),
            "Theme file not found, using defaults based on system appearance"
        );
        let theme = default_theme_from_system_appearance();
        log_theme_load_result(&correlation_id, "default_missing_theme_file", &theme);
        log_theme_config(&theme);
        return theme;
    }

    // Read and parse the JSON file
    match std::fs::read_to_string(&theme_path) {
        Err(e) => {
            error!(
                correlation_id = %correlation_id,
                path = %theme_path.display(),
                io_error_kind = ?e.kind(),
                error = ?e,
                "Failed to read theme file, using defaults"
            );
            let theme = default_theme_from_system_appearance();
            log_theme_load_result(&correlation_id, "default_theme_file_read_error", &theme);
            log_theme_config(&theme);
            theme
        }
        Ok(contents) => match serde_json::from_str::<serde_json::Value>(&contents) {
            Ok(user_theme_json) => {
                // Key behavior: When appearance is Auto, use system appearance to
                // determine which color scheme to use (light or dark).
                // This allows the app to follow macOS light/dark mode automatically.
                let is_system_dark = detect_system_appearance();
                let requested_appearance = user_theme_json
                    .get("appearance")
                    .cloned()
                    .and_then(|appearance| {
                        serde_json::from_value::<AppearanceMode>(appearance).ok()
                    })
                    .unwrap_or_default();
                let should_use_light =
                    should_use_light_palette(requested_appearance, is_system_dark);

                let mut merged_theme_json = match serde_json::to_value(if should_use_light {
                    Theme::light_default()
                } else {
                    Theme::dark_default()
                }) {
                    Ok(default_theme_json) => default_theme_json,
                    Err(e) => {
                        error!(
                            correlation_id = %correlation_id,
                            serialize_error = ?e,
                            "Failed to serialize default theme, using defaults"
                        );
                        let theme = default_theme_from_system_appearance();
                        log_theme_load_result(
                            &correlation_id,
                            "default_theme_serialization_error",
                            &theme,
                        );
                        log_theme_config(&theme);
                        return theme;
                    }
                };

                merge_json(&mut merged_theme_json, user_theme_json);
                set_requested_appearance_on_theme_json(
                    &mut merged_theme_json,
                    requested_appearance,
                );
                hydrate_terminal_colors_for_deserialize(&mut merged_theme_json, is_system_dark);
                let terminal_palette =
                    terminal_palette_for_appearance(requested_appearance, is_system_dark);

                match with_terminal_default_palette_hint(terminal_palette, || {
                    serde_json::from_value::<Theme>(merged_theme_json)
                }) {
                    Ok(mut theme) => {
                        debug!(
                            correlation_id = %correlation_id,
                            path = %theme_path.display(),
                            "Successfully loaded theme"
                        );

                        if should_use_light {
                            // Use light opacity defaults
                            if theme.opacity.is_none() {
                                theme.opacity = Some(BackgroundOpacity::light_default());
                            }

                            debug!(
                                correlation_id = %correlation_id,
                                system_appearance = if is_system_dark { "dark" } else { "light" },
                                "Using light theme colors (system is in light mode)"
                            );
                        } else {
                            // System is in dark mode (or explicitly set to dark)
                            if theme.opacity.is_none() {
                                theme.opacity = Some(BackgroundOpacity::dark_default());
                            }
                        }

                        theme = normalize_theme_primary_text(theme);
                        log_theme_load_result(&correlation_id, "theme_json", &theme);
                        log_theme_config(&theme);
                        theme
                    }
                    Err(e) => {
                        error!(
                            correlation_id = %correlation_id,
                            path = %theme_path.display(),
                            parse_error = ?e,
                            content_len = contents.len(),
                            "Failed to parse theme JSON, using defaults"
                        );
                        debug!(
                            correlation_id = %correlation_id,
                            content_len = contents.len(),
                            "Malformed theme file content"
                        );
                        let theme = default_theme_from_system_appearance();
                        log_theme_load_result(
                            &correlation_id,
                            "default_theme_json_parse_error",
                            &theme,
                        );
                        log_theme_config(&theme);
                        theme
                    }
                }
            }
            Err(e) => {
                error!(
                    correlation_id = %correlation_id,
                    path = %theme_path.display(),
                    parse_error = ?e,
                    content_len = contents.len(),
                    "Failed to parse theme JSON, using defaults"
                );
                debug!(
                    correlation_id = %correlation_id,
                    content_len = contents.len(),
                    "Malformed theme file content"
                );
                let theme = default_theme_from_system_appearance();
                log_theme_load_result(&correlation_id, "default_theme_json_parse_error", &theme);
                log_theme_config(&theme);
                theme
            }
        },
    }
}

/// Get a cached version of the theme for use in render functions
///
/// This avoids file I/O on every render call by caching the loaded theme.
/// Use `reload_theme_cache()` when you need to refresh cached values.
///
/// # Performance
///
/// Use this function instead of `load_theme()` in render paths:
/// - Render methods
/// - Background color calculations
/// - Any code that runs frequently
///
/// Use `load_theme()` for:
/// - Initial setup
/// - When you need guaranteed fresh theme data
/// - After explicitly invalidating the cache
pub fn get_cached_theme() -> Theme {
    let cache = &*THEME_CACHE;
    let cache_guard = cache.lock().unwrap_or_else(|error| {
        warn!(
            operation = "get_cached_theme_lock",
            error = ?error,
            "Theme cache mutex poisoned; recovering cached theme state"
        );
        error.into_inner()
    });

    cache_guard.theme.clone()
}

/// Reload and cache the theme from disk
///
/// Call this when you need to refresh the cached theme (e.g., from the theme watcher).
/// This function loads the theme from disk and updates the cache.
pub fn reload_theme_cache() -> Theme {
    let theme = load_theme();

    let cache = &*THEME_CACHE;
    let mut guard = cache.lock().unwrap_or_else(|error| {
        warn!(
            operation = "reload_theme_cache_lock",
            error = ?error,
            "Theme cache mutex poisoned; recovering cache for theme reload"
        );
        error.into_inner()
    });
    guard.theme = theme.clone();
    debug!("Theme cache reloaded");

    theme
}

#[allow(dead_code)]
pub(crate) fn set_cached_theme_for_preview(theme: &Theme) {
    let cache = &*THEME_CACHE;
    let mut guard = cache.lock().unwrap_or_else(|error| {
        warn!(
            operation = "set_cached_theme_for_preview_lock",
            error = ?error,
            "Theme cache mutex poisoned; recovering cache for theme preview"
        );
        error.into_inner()
    });
    guard.theme = theme.clone();
    debug!("Theme cache updated from preview");
}

/// Initialize the theme cache on startup
///
/// Call this during app initialization to ensure the theme is loaded
/// before any render calls. This ensures `get_cached_theme()` returns
/// the correct theme from the first render.
pub fn init_theme_cache() {
    reload_theme_cache();
    debug!("Theme cache initialized");
}

// ============================================================================
// End Lightweight Theme Extraction Helpers
// ============================================================================

struct Hex(u32);

impl fmt::Display for Hex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:06X}", self.0 & 0x00FF_FFFF)
    }
}

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
        selected = theme.colors.accent.selected,
        selected_hex = %Hex(theme.colors.accent.selected),
        selected_subtle = theme.colors.accent.selected_subtle,
        selected_subtle_hex = %Hex(theme.colors.accent.selected_subtle),
        "Theme accent colors"
    );
    debug!(
        status_error = theme.colors.ui.error,
        status_error_hex = %Hex(theme.colors.ui.error),
        status_warning = theme.colors.ui.warning,
        status_warning_hex = %Hex(theme.colors.ui.warning),
        status_info = theme.colors.ui.info,
        status_info_hex = %Hex(theme.colors.ui.info),
        "Theme status colors"
    );
}

// --- merged from part_06.rs ---
#[cfg(test)]
include!("types_tests.rs");
