pub use crate::window_resize::main_layout::{
    DIVIDER_HEIGHT, HEADER_PADDING_X, HEADER_PADDING_Y, HINT_STRIP_HEIGHT, HINT_STRIP_PADDING_X,
    HINT_STRIP_PADDING_Y, HINT_TEXT_OPACITY,
};

pub const DIVIDER_OPACITY: f32 = crate::theme::opacity::OPACITY_HOVER;
/// Theme-derived scrim opacity for dimming GPUI content behind modal windows.
pub const MAIN_WINDOW_MODAL_DIM_OPACITY: f32 = crate::theme::opacity::OPACITY_MODAL_DIM;

pub const LIQUID_GLASS_MIN_HIT_PX: f32 = 28.0;
pub const LIQUID_GLASS_MIN_VISUAL_PX: f32 = 20.0;
pub const LIQUID_GLASS_PREFERRED_HIT_PX: f32 = 44.0;
pub const LIQUID_GLASS_PREFERRED_CENTER_GAP_PX: f32 = 60.0;

pub const LIQUID_GLASS_WINDOW_RADIUS_PX: f32 = 22.0;
/// Radius for the Tahoe-style floating popup shell (e.g. the actions dialog).
pub const LIQUID_GLASS_POPUP_RADIUS_PX: f32 = 18.0;
pub const LIQUID_GLASS_PANEL_RADIUS_PX: f32 = 16.0;
pub const LIQUID_GLASS_CONTROL_RADIUS_PX: f32 = 14.0;
pub const LIQUID_GLASS_COMPACT_RADIUS_PX: f32 = 10.0;

/// Canonical interior padding for a Liquid Glass panel surface.
pub const LIQUID_GLASS_PANEL_PADDING_PX: f32 = 16.0;
/// Canonical dense gap between stacked controls/labels inside chrome.
pub const LIQUID_GLASS_DENSE_GAP_PX: f32 = 8.0;

/// Canonical hover-button corner radius. The footer trailing action buttons
/// own this value; every other hover-pill button (header context chips,
/// footer tip, hint-strip buttons, prompt-footer buttons) must share it so
/// all buttons read as one system.
pub const ACTION_BUTTON_RADIUS_PX: f32 = 6.0;
/// Canonical keycap-border alpha while a hover-pill button is hovered.
pub const ACTION_BUTTON_HOVER_BORDER_ALPHA: u32 = 0x57;

/// Danger (destructive/deny) action fill alphas layered over the theme's
/// `ui.error` color. One owner for the red family so no surface hardcodes
/// a danger hue again.
pub const DANGER_ACTION_REST_ALPHA: u32 = 0x08;
pub const DANGER_ACTION_HOVER_ALPHA: u32 = 0x14;
pub const DANGER_ACTION_BORDER_ALPHA: u32 = 0xAA;

/// Minimum cycle for ambient/pulsing motion so status indicators stay calm.
pub const AMBIENT_PULSE_CYCLE_MS: u64 = 2_000;

/// Internal horizontal text inset for the main search input. This is shared by
/// the theme and dev style tool so saved design exports can tune the real
/// main-window input padding without reintroducing local renderer constants.
pub const SEARCH_INPUT_TEXT_INSET_X_PX: f32 = 16.0;

pub const CHROME_LAYER_CONTENT: &str = "content";
pub const CHROME_LAYER_FUNCTIONAL: &str = "functionalChrome";
pub const CHROME_LAYER_FLOATING: &str = "floatingTransient";
pub const CHROME_LAYER_WINDOW_BACKDROP: &str = "windowBackdrop";

pub const MATERIAL_SOLID_THEME_TOKEN: &str = "solidThemeToken";
pub const MATERIAL_NS_VISUAL_EFFECT: &str = "NSVisualEffectView";
pub const MATERIAL_NATIVE_WINDOW_BACKDROP: &str = "nativeWindowBackdrop";

pub fn alpha_from_opacity(opacity: f32) -> u32 {
    (opacity.clamp(0.0, 1.0) * 255.0).round() as u32
}
