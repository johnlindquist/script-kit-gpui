//! App-authored Notes window design contract.
//!
//! Single typed source for the Notes window's app-owned chrome values —
//! default window geometry, titlebar reserves and traffic-light origin,
//! rest opacities, footer presentation facts, and the layout *model*
//! (autosize reservation) — consumed by BOTH the renderer/window-ops paths
//! and the `design_contract` exporter so the two can never drift.
//!
//! Contract rules:
//! - `production_*` resolvers read `NotesWindowStyle::current()` directly —
//!   NEVER the feature-sensitive `adopted_style()` — so a storybook-enabled
//!   exporter build produces the same bundle as a production renderer.
//! - Model values (the 28px footer reservation, autosize constants) are
//!   exported under honest model names; the *painted* footer band is a
//!   separate resolved value (see [`resolved_notes_footer_intrinsic_height`]).
//!   The disagreement is recorded as the
//!   `notesFooter.layoutReservationVsIntrinsicPaint` conflict, not collapsed.

use super::style::{NotesLayoutMetrics, NotesWindowStyle};

// ── Default window geometry (shared by first-open placement and the
//    "Reset Window Position" action; window_ops consumes these) ────────────

/// Default Notes window width in px.
pub(crate) const NOTES_DEFAULT_WIDTH: f32 = 350.0;
/// Default Notes window height in px.
pub(crate) const NOTES_DEFAULT_HEIGHT: f32 = 280.0;
/// Edge padding from the display's top-right corner for default placement.
pub(crate) const NOTES_DEFAULT_EDGE_PADDING: f32 = 20.0;

/// Physical bottom-edge band that routes a GPUI pointer-down into AppKit's
/// native resize tracker. This is interaction geometry only; it paints no
/// strip and does not change the footer layout.
pub(crate) const NOTES_BOTTOM_RESIZE_HIT_HEIGHT_PX: f32 = 6.0;
/// Avoid ambiguous diagonal/corner ownership; this feature is bottom-only.
pub(crate) const NOTES_BOTTOM_RESIZE_CORNER_GUARD_PX: f32 = 6.0;
/// Small rounding guard around every measured floating-button rectangle.
pub(crate) const NOTES_BOTTOM_RESIZE_BUTTON_GUARD_PX: f32 = 1.0;

// ── Titlebar chrome ────────────────────────────────────────────────────────

/// Horizontal titlebar padding (was an inline `.px_3()` in
/// `render_editor_titlebar`; the renderer now consumes this const).
pub(crate) const NOTES_TITLEBAR_PADDING_X: f32 = 12.0;

/// App-authored traffic-light group origin passed to
/// `TitlebarOptions::traffic_light_position` (window_ops).
pub(crate) const NOTES_TRAFFIC_LIGHT_ORIGIN_X: f32 = 8.0;
pub(crate) const NOTES_TRAFFIC_LIGHT_ORIGIN_Y: f32 = 7.0;

// ── Footer presentation facts (contract, not visual numbers) ──────────────

/// The Notes footer is a GPUI strip rendered INSIDE the window — not the
/// main window's native AppKit overlay.
pub(crate) const NOTES_FOOTER_PRESENTATION: &str = "inWindowGpui";
pub(crate) const NOTES_FOOTER_NATIVE_OVERLAY: bool = false;
/// The footer renders only while a note is selected
/// (`render_editor.rs` gates on selection).
pub(crate) const NOTES_FOOTER_VISIBILITY: &str = "selectedNoteOnly";

/// App-authored Notes window/titlebar/footer chrome values, resolved from
/// the production style profile. Both the renderer paths and the design
/// contract exporter consume this struct.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NotesWindowChromeContract {
    pub default_width: f32,
    pub default_height: f32,
    pub default_edge_padding: f32,
    pub titlebar_height: f32,
    pub titlebar_padding_x: f32,
    /// Width reserved for macOS traffic lights (leading reserve).
    pub titlebar_leading_reserve_width: f32,
    /// Width reserved for the hover-reveal icon cluster (trailing reserve).
    pub titlebar_trailing_reserve_width: f32,
    pub traffic_light_origin_x: f32,
    pub traffic_light_origin_y: f32,
    /// Rest-state (window not hovered) title opacity (`OPACITY_MUTED`).
    pub title_rest_opacity: f32,
    /// Rest-state footer strip opacity (`OPACITY_SUBTLE`).
    pub footer_rest_opacity: f32,
    /// Leading save-status slot minimum width (`MIN_TARGET_SIZE`).
    pub footer_status_min_width: f32,
    /// Footer strip horizontal content inset (`HINT_STRIP_PADDING_X`).
    pub footer_content_inset_x: f32,
    pub editor_padding_x: f32,
    pub editor_padding_y: f32,
}

/// Production chrome contract: `NotesWindowStyle::current()` plus the
/// app-authored consts above. Deliberately NOT `adopted_style()` — the
/// checked-in design bundle must not be feature-sensitive.
pub(crate) fn production_notes_window_contract() -> NotesWindowChromeContract {
    let style = NotesWindowStyle::current();
    NotesWindowChromeContract {
        default_width: NOTES_DEFAULT_WIDTH,
        default_height: NOTES_DEFAULT_HEIGHT,
        default_edge_padding: NOTES_DEFAULT_EDGE_PADDING,
        titlebar_height: style.titlebar_height,
        titlebar_padding_x: NOTES_TITLEBAR_PADDING_X,
        titlebar_leading_reserve_width: super::TITLEBAR_TRAFFIC_LIGHT_W,
        titlebar_trailing_reserve_width: super::TITLEBAR_ICONS_W,
        traffic_light_origin_x: NOTES_TRAFFIC_LIGHT_ORIGIN_X,
        traffic_light_origin_y: NOTES_TRAFFIC_LIGHT_ORIGIN_Y,
        title_rest_opacity: super::OPACITY_MUTED,
        footer_rest_opacity: super::OPACITY_SUBTLE,
        footer_status_min_width: super::MIN_TARGET_SIZE,
        footer_content_inset_x: crate::window_resize::main_layout::HINT_STRIP_PADDING_X,
        editor_padding_x: style.editor_padding_x,
        editor_padding_y: style.editor_padding_y,
    }
}

/// The Notes layout *model* — the values autosize math and
/// `automation_layout_info` reserve, exported under honest model names.
/// `footer_reservation_height` is `NotesLayoutMetrics.footer_height` (28)
/// and intentionally disagrees with the painted 32px footer band; that
/// drift stays a recorded conflict, never a silent fix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NotesLayoutModelContract {
    pub footer_reservation_height: f32,
    pub auto_resize_max_height: f32,
    pub auto_resize_assumed_line_height: f32,
    pub auto_resize_threshold: f32,
    pub auto_resize_padding: f32,
}

/// Production layout model, via the SAME typed function
/// (`NotesLayoutMetrics::from_style`) the renderer/autosize path calls.
pub(crate) fn production_notes_layout_model() -> NotesLayoutModelContract {
    let metrics = NotesLayoutMetrics::from_style(NotesWindowStyle::current());
    NotesLayoutModelContract {
        footer_reservation_height: metrics.footer_height,
        auto_resize_max_height: metrics.auto_resize_max_height,
        auto_resize_assumed_line_height: metrics.auto_resize_line_height,
        auto_resize_threshold: metrics.auto_resize_threshold,
        auto_resize_padding: metrics.auto_resize_padding,
    }
}

/// The footer band the Notes window actually PAINTS: the shared universal
/// footer action-button row height. `render_universal_footer_action_buttons`
/// sizes buttons with `footer_button_height(HINT_STRIP_HEIGHT)`; this
/// resolver routes through the same shared `footer_chrome` formula owner
/// (`footer_button_height_in`) with an explicit `button_padding_y` so the
/// checked-in exporter can pass the base (non-runtime-override) footer
/// metrics.
pub(crate) fn resolved_notes_footer_intrinsic_height(button_padding_y: f32) -> f32 {
    crate::components::footer_chrome::footer_button_height_in(
        crate::window_resize::main_layout::HINT_STRIP_HEIGHT,
        button_padding_y,
    )
}

/// Glass capsule group id the Notes editor footer rail used to own. The rail
/// is gone (Notes mode renders no footer), but the constant remains the one
/// name mode-transition cleanup uses — never duplicate the string.
pub(crate) const NOTES_EDITOR_FOOTER_GROUP: &str = "notes-footer-action-rail";

/// Hit-region group name used to fail closed while Notes mode owns no footer.
pub(crate) const NOTES_EMPTY_FOOTER_HIT_GROUP: &str = "notes-footer-empty";

// ── Notes chrome policy (mode → footer/backdrop partition) ────────────────
//
// One pure decision point driving the GPUI stage height, gap reservation,
// footer rendering, native backdrop inset, and capsule-group cleanup for the
// Notes window shell. Notes mode owns the full window (no floating footer,
// no empty gap); Agent mode keeps the external cwd/agent/model/Send capsule
// row below the glass body. No independent booleans in five files.

/// Which external footer band the Notes window shell currently owns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NotesFooterMode {
    /// Notes editor: no floating capsules, no reserved gap, full backdrop.
    None,
    /// Embedded Agent Chat: external capsule row + gap below the glass body.
    AgentChatExternal,
}

/// The resolved chrome partition for the current surface mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NotesChromePolicy {
    pub footer_mode: NotesFooterMode,
    /// Height (px) the native backdrop and GPUI stage leave below the glass
    /// body: footer strip + the fixed container gap when Agent mode owns an
    /// external footer under active glass bands, otherwise `0`.
    pub backdrop_bottom_inset: f32,
}

impl NotesChromePolicy {
    pub(crate) fn reserves_external_footer(&self) -> bool {
        self.footer_mode == NotesFooterMode::AgentChatExternal && self.backdrop_bottom_inset > 0.0
    }
}

/// Map the Notes surface mode (+ live glass-band availability) to its chrome
/// partition. Pure so both the renderer and the native-backdrop sync consume
/// the same decision.
pub(crate) fn notes_chrome_policy(
    mode: crate::notes::window::NotesSurfaceMode,
    glass_active: bool,
) -> NotesChromePolicy {
    match mode {
        crate::notes::window::NotesSurfaceMode::Notes => NotesChromePolicy {
            footer_mode: NotesFooterMode::None,
            backdrop_bottom_inset: 0.0,
        },
        crate::notes::window::NotesSurfaceMode::AgentChat => NotesChromePolicy {
            footer_mode: NotesFooterMode::AgentChatExternal,
            backdrop_bottom_inset: if glass_active {
                crate::components::footer_chrome::current_main_menu_footer_height()
                    + crate::footer_popup::FLOAT_FOOTER_CONTAINER_GAP_PX
            } else {
                0.0
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_chrome_contract_matches_authored_values() {
        let c = production_notes_window_contract();
        assert_eq!(c.default_width, 350.0);
        assert_eq!(c.default_height, 280.0);
        assert_eq!(c.default_edge_padding, 20.0);
        assert_eq!(c.titlebar_height, 36.0);
        assert_eq!(c.titlebar_padding_x, 12.0);
        assert_eq!(c.titlebar_leading_reserve_width, 60.0);
        assert_eq!(c.titlebar_trailing_reserve_width, 100.0);
        assert_eq!(c.traffic_light_origin_x, 8.0);
        assert_eq!(c.traffic_light_origin_y, 7.0);
        assert_eq!(c.title_rest_opacity, 0.7);
        assert_eq!(c.footer_rest_opacity, 0.5);
        assert_eq!(c.footer_status_min_width, 24.0);
        assert_eq!(c.footer_content_inset_x, 14.0);
        assert_eq!(c.editor_padding_x, 16.0);
        assert_eq!(c.editor_padding_y, 12.0);
    }

    #[test]
    fn production_layout_model_preserves_28pt_reservation() {
        let m = production_notes_layout_model();
        assert_eq!(m.footer_reservation_height, 28.0);
        assert_eq!(m.auto_resize_max_height, 600.0);
        assert_eq!(m.auto_resize_assumed_line_height, 20.0);
        assert_eq!(m.auto_resize_threshold, 5.0);
        assert_eq!(m.auto_resize_padding, 24.0);
    }

    #[test]
    fn notes_chrome_policy_matches_surface_mode() {
        use crate::notes::window::NotesSurfaceMode;

        // Notes mode: never a footer band, never an inset — regardless of
        // glass availability.
        for glass_active in [false, true] {
            let chrome = notes_chrome_policy(NotesSurfaceMode::Notes, glass_active);
            assert_eq!(chrome.footer_mode, NotesFooterMode::None);
            assert_eq!(chrome.backdrop_bottom_inset, 0.0);
            assert!(!chrome.reserves_external_footer());
        }

        // Agent mode with glass bands: external footer + footer height plus
        // exactly the fixed 8pt container gap.
        let chrome = notes_chrome_policy(NotesSurfaceMode::AgentChat, true);
        assert_eq!(chrome.footer_mode, NotesFooterMode::AgentChatExternal);
        assert_eq!(
            chrome.backdrop_bottom_inset,
            crate::components::footer_chrome::current_main_menu_footer_height()
                + crate::footer_popup::FLOAT_FOOTER_CONTAINER_GAP_PX
        );
        assert!(chrome.reserves_external_footer());

        // Agent mode without glass bands: footer identity is preserved but no
        // band is reserved (the in-window fallback footer owns its own height).
        let chrome = notes_chrome_policy(NotesSurfaceMode::AgentChat, false);
        assert_eq!(chrome.footer_mode, NotesFooterMode::AgentChatExternal);
        assert_eq!(chrome.backdrop_bottom_inset, 0.0);
        assert!(!chrome.reserves_external_footer());
    }

    #[test]
    fn painted_footer_band_is_32pt_with_base_button_padding() {
        // Base footer metrics author button_padding_y = 2 → 36 − 2×2 = 32.
        assert_eq!(resolved_notes_footer_intrinsic_height(2.0), 32.0);
        // The model reservation (28) intentionally under-reserves this band;
        // notesFooter.layoutReservationVsIntrinsicPaint records the drift.
        assert_ne!(
            resolved_notes_footer_intrinsic_height(2.0),
            production_notes_layout_model().footer_reservation_height
        );
    }
}
