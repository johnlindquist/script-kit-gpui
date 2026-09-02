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
//! - Footer geometry is owned by the single resolver in [`super::layout`]:
//!   the runtime paints NO footer row (removed in 4e1a71a84), autosize and
//!   the projection reserve the resolved row height (0), and the would-be
//!   row height has exactly one derivation
//!   (see [`resolved_notes_footer_intrinsic_height`]).

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

// Footer facts project the actual footerless Notes root, not a hypothetical strip.
pub(crate) const NOTES_FOOTER_PRESENTATION: &str = "absent";
pub(crate) const NOTES_FOOTER_NATIVE_OVERLAY: bool = false;
pub(crate) const NOTES_FOOTER_VISIBILITY: &str = "never";

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

/// The actual production Notes autosize model exported to the design bundle.
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
        footer_reservation_height: super::layout::production_notes_footer_action_row().height,
        auto_resize_max_height: metrics.auto_resize_max_height,
        auto_resize_assumed_line_height: metrics.auto_resize_line_height,
        auto_resize_threshold: metrics.auto_resize_threshold,
        auto_resize_padding: metrics.auto_resize_padding,
    }
}

/// Hypothetical visible-row height for read-only compatibility diagnostics.
/// Actual Notes reservation is `production_notes_footer_action_row().height`.
pub(crate) fn resolved_notes_footer_intrinsic_height(button_padding_y: f32) -> f32 {
    super::layout::resolve_notes_footer_action_row(true, button_padding_y).height
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
    fn production_layout_model_matches_footerless_runtime() {
        let m = production_notes_layout_model();
        assert_eq!(m.footer_reservation_height, 0.0);
        assert_eq!(m.auto_resize_max_height, 600.0);
        assert_eq!(m.auto_resize_assumed_line_height, 20.0);
        assert_eq!(m.auto_resize_threshold, 5.0);
        assert_eq!(m.auto_resize_padding, 24.0);
    }

    #[test]
    fn model_and_renderer_consume_the_same_notes_footer_action_row() {
        use crate::notes::window::layout;

        // ONE resolver owns the row: the exported intrinsic height is the
        // resolver's visible-row height (shared footer_chrome formula), and
        // the production model/projection/autosize footer term is the
        // resolver's production row — absent, reserving zero.
        assert_eq!(
            resolved_notes_footer_intrinsic_height(2.0),
            layout::resolve_notes_footer_action_row(true, 2.0).height,
        );
        assert_eq!(resolved_notes_footer_intrinsic_height(2.0), 32.0);

        let production = layout::production_notes_footer_action_row();
        assert!(!production.visible, "Notes paints no footer action row");
        assert_eq!(production.height, 0.0);
        assert_eq!(
            production.role,
            crate::protocol::GeometryRole::FooterActionRow
        );

        // And the autosize composition consumes exactly that resolved row.
        let metrics = NotesLayoutMetrics::from_style(NotesWindowStyle::current());
        let resolved =
            layout::resolve_notes_autosize(layout::notes_autosize_input(&metrics, 3, 280.0));
        assert_eq!(resolved.footer_action_row.height, production.height,);
    }
}
