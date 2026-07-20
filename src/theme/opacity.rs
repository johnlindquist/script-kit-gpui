//! Canonical semantic opacity tokens used across the app.
//!
//! These constants intentionally define opacity in the standard direction:
//! `0.0` is fully transparent and `1.0` is fully opaque.
//! Keep all new UI opacity values sourced from this module.

/// Fully transparent — used for hover-reveal patterns where an element
/// starts invisible and fades in on group hover.
pub const OPACITY_HIDDEN: f32 = 0.0;
/// Window-root tint over the native vibrancy blur, shared by ALL windows
/// (main, notes, dictation, HUD, actions popup, AI/Agent Chat).
///
/// Deliberately THIN: legibility over bright backdrops is carried by the
/// vibrancy material (menu material measured 39% body luminance over a white
/// app at zero tint; 30% at this 0.30 tint), not by this tint. A plain-alpha
/// tint scales backdrop chroma by (1-alpha), so a heavy value here greys the
/// panel out over colorful backdrops — 0.85 crushed body saturation to ~15%
/// vs Raycast's ~54%. This veil only biases the material toward the theme
/// color. Pairs with `VibrancySettings::backdrop_saturation`.
pub const OPACITY_VIBRANCY_BACKGROUND: f32 = 0.30;
/// Cap applied on top of the vibrancy veil when the native Tahoe
/// `NSGlassEffectView` backdrop is the window material. The glass supplies
/// its own frost AND receives a theme tint derived from this same resolved
/// value, so the blur-era veil double-stacks: 0.30 veil + 0.30 glass tint
/// reads nearly opaque. In glass mode both layers resolve through this cap.
/// 0.0 = bare glass, exactly like the liquid-glass-demo bin: the material
/// alone carries the frost and content sits directly on it. Raise this to
/// restore a themed tint over the glass (0.15 was the previous value).
pub const OPACITY_GLASS_MODE_VEIL_CAP: f32 = 0.0;
/// Default duration (seconds) of the glass appear morph — one continuous
/// spring-bezier glide; Spotlight's enter reads at roughly a quarter second.
pub const GLASS_MORPH_DEFAULT_DURATION: f32 = 0.26;
/// Default start outset (fraction of each dimension) of the glass appear
/// morph: how much larger than final the window starts. Subtle by default.
pub const GLASS_MORPH_DEFAULT_INSET: f32 = 0.04;
/// Softer ghost-tier background opacity for barely-there fills (0.03–0.06 range).
#[allow(dead_code)]
pub const OPACITY_GHOST_SOFT: f32 = 0.03;
/// Ghost-tier divider/surface opacity for whisper chrome (0.03–0.06 range).
pub const OPACITY_GHOST: f32 = 0.06;
/// Assistant bubble tint background.
pub const OPACITY_MESSAGE_ASSISTANT_BACKGROUND: f32 = 0.10;
/// User bubble tint background (kept for reference, now using inline 0.18 with muted tone).
pub const OPACITY_MESSAGE_USER_BACKGROUND: f32 = 0.18;
/// Very subtle tint/background accents.
pub const OPACITY_SUBTLE: f32 = 0.15;
/// Background opacity for suggestion/prompt cards (subtle neutral tint).
pub const OPACITY_CARD_BG: f32 = 0.12;
/// Danger-tinted background surfaces (error banners, delete confirmation).
pub const OPACITY_DANGER_BG: f32 = 0.20;
/// Danger hover state (slightly stronger than OPACITY_DANGER_BG).
pub const OPACITY_DANGER_HOVER: f32 = 0.25;
/// Generic hover surfaces.
pub const OPACITY_HOVER: f32 = 0.30;
/// Suggestion card hover background.
pub const OPACITY_SUGGESTION_HOVER: f32 = 0.35;
/// Muted UI text/surfaces.
pub const OPACITY_MUTED: f32 = OPACITY_HOVER;
/// Disabled state content.
pub const OPACITY_DISABLED: f32 = 0.40;
/// Medium border emphasis.
pub const OPACITY_BORDER: f32 = 0.45;
/// Selected/highlighted state.
pub const OPACITY_SELECTED: f32 = 0.50;
/// Main-window content dim layer while a modal surface is open above it.
pub const OPACITY_MODAL_DIM: f32 = 0.52;
/// Secondary icon emphasis.
pub const OPACITY_ICON_MUTED: f32 = 0.55;
/// Medium accent emphasis.
pub const OPACITY_ACCENT_MEDIUM: f32 = 0.60;
/// Secondary label/body text emphasis.
pub const OPACITY_TEXT_MUTED: f32 = 0.80;
/// Strong emphasis.
pub const OPACITY_STRONG: f32 = 0.70;
/// Preview/secondary text in sidebars.
pub const OPACITY_PREVIEW_TEXT: f32 = 0.75;
/// Prominent emphasis (button borders, key UI chrome).
pub const OPACITY_PROMINENT: f32 = 0.80;
/// Near-full emphasis.
pub const OPACITY_NEAR_FULL: f32 = 0.85;
/// Warning banner hover overlay.
pub const OPACITY_WARNING_BANNER_HOVER: f32 = 0.88;
/// Active/pressed state (button hover, focused chrome).
pub const OPACITY_ACTIVE: f32 = 0.90;

/// Idle prompt-field whisper surface.
pub const OPACITY_WHISPER_SURFACE_IDLE: f32 = OPACITY_GHOST;
/// Focused prompt-field whisper surface.
pub const OPACITY_WHISPER_SURFACE_FOCUSED: f32 = 0.10;
/// Idle prompt-field whisper border.
pub const OPACITY_WHISPER_BORDER_IDLE: f32 = 0.13;
/// Accent fill for checked controls that still sit on whisper surfaces.
pub const OPACITY_WHISPER_ACCENT_FILL: f32 = OPACITY_ACCENT_MEDIUM;

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_opacity_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.000_001,
            "expected {}, got {}",
            expected,
            actual
        );
    }

    #[test]
    fn test_ai_legacy_opacity_values_remain_available_via_canonical_tokens() {
        assert_opacity_eq(OPACITY_HIDDEN, 0.0);
        assert_opacity_eq(OPACITY_GHOST_SOFT, 0.03);
        assert_opacity_eq(OPACITY_GHOST, 0.06);
        assert_opacity_eq(OPACITY_MESSAGE_ASSISTANT_BACKGROUND, 0.10);
        assert_opacity_eq(OPACITY_MESSAGE_USER_BACKGROUND, 0.18);
        assert_opacity_eq(OPACITY_MUTED, 0.30);
        assert_opacity_eq(OPACITY_TEXT_MUTED, 0.80);
        assert_opacity_eq(OPACITY_STRONG, 0.70);
        assert_opacity_eq(OPACITY_NEAR_FULL, 0.85);
        assert_opacity_eq(OPACITY_CARD_BG, 0.12);
        assert_opacity_eq(OPACITY_DANGER_BG, 0.20);
        assert_opacity_eq(OPACITY_DANGER_HOVER, 0.25);
        assert_opacity_eq(OPACITY_SUGGESTION_HOVER, 0.35);
        assert_opacity_eq(OPACITY_MODAL_DIM, 0.52);
        assert_opacity_eq(OPACITY_PREVIEW_TEXT, 0.75);
        assert_opacity_eq(OPACITY_PROMINENT, 0.80);
        assert_opacity_eq(OPACITY_ACTIVE, 0.90);
        assert_opacity_eq(OPACITY_WARNING_BANNER_HOVER, 0.88);
    }
}
