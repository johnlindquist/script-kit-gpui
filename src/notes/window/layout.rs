//! Single Notes window layout resolver (GEO-004).
//!
//! One typed owner for the two results every Notes height consumer must share:
//!
//! - [`ResolvedNotesFooterActionRow`]: the in-window footer action-button row.
//!   Its height routes through the SAME shared `footer_chrome` formula owner
//!   (`footer_button_height_in`) the renderer would consume, and its role is
//!   the landed protocol [`crate::protocol::GeometryRole::FooterActionRow`].
//! - [`ResolvedNotesAutosize`]: the complete window-height composition —
//!   editor content height, editor insets, heading/body line metrics, footer
//!   action-row reservation, window non-content insets, and the structural /
//!   effective minimum and maximum heights.
//!
//! Runtime truth (verified 2026-08-07 against the live render tree): the
//! Notes window paints NO footer action row. The footer rail was removed in
//! commit `4e1a71a84` ("Notes mode owns the full window"); `render_editor`
//! renders titlebar → optional search → optional toolbar → flexible editor
//! body and nothing after it. [`production_notes_footer_action_row`] therefore
//! resolves `visible = false` and reserves ZERO height. The resolver still
//! carries the full role/height derivation so the model, the autosize
//! composition, the protocol projection, and any future re-introduced row all
//! consume ONE source instead of drifting scalars (the retired state was a
//! 28-point model reservation vs a 32-point "painted band" fiction, neither of
//! which matched the footer-less runtime).
//!
//! `FooterNativeHost` stays a separate diagnostic concept: a native host-band
//! measurement is never substituted for the action row and never added to the
//! autosize total ([`ResolvedNotesAutosize::native_footer_host_included`] is
//! always `false`).

use super::style::NotesLayoutMetrics;

/// Whether the current Notes runtime paints an in-window footer action row.
///
/// `false` since commit `4e1a71a84` removed the Notes footer rail. Flipping
/// this back to `true` is a product decision (it re-reserves the resolved row
/// height in autosize, minimum height, and the layout projection at once).
pub(crate) const NOTES_FOOTER_ACTION_ROW_PRESENT: bool = false;

/// Vertical insets (used for both editor padding and window non-content
/// frame-vs-content differences).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct NotesInsets {
    pub top: f32,
    pub bottom: f32,
}

impl NotesInsets {
    pub(crate) const fn vertical(self) -> f32 {
        self.top + self.bottom
    }

    pub(crate) const fn symmetric(value: f32) -> Self {
        Self {
            top: value,
            bottom: value,
        }
    }
}

/// The resolved Notes footer action row — the ONLY footer quantity allowed to
/// take editor/window space. Distinct from the native footer host band, which
/// is diagnostic geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedNotesFooterActionRow {
    /// The landed protocol geometry role for in-window footer action rows.
    pub role: crate::protocol::GeometryRole,
    /// Whether the runtime paints the row at all.
    pub visible: bool,
    /// Host band base height the row derives from (`HINT_STRIP_HEIGHT`).
    pub base_height: f32,
    /// Vertical button padding consumed by the shared footer chrome formula.
    pub button_padding_y: f32,
    /// Reserved height: the shared-formula row height when visible, else 0.
    pub height: f32,
}

/// Resolve the Notes footer action row through the shared footer chrome
/// formula owner. `button_padding_y` is explicit so deterministic resolvers
/// (the design-contract exporter passes base, non-runtime-override metrics)
/// and runtime callers share one derivation.
pub(crate) fn resolve_notes_footer_action_row(
    visible: bool,
    button_padding_y: f32,
) -> ResolvedNotesFooterActionRow {
    let base_height = crate::window_resize::main_layout::HINT_STRIP_HEIGHT;
    let height =
        crate::components::footer_chrome::footer_button_height_in(base_height, button_padding_y);
    ResolvedNotesFooterActionRow {
        role: crate::protocol::GeometryRole::FooterActionRow,
        visible,
        base_height,
        button_padding_y,
        height: if visible { height } else { 0.0 },
    }
}

/// The production Notes footer action row: absent (see module docs), with the
/// would-be row height still derived from the same runtime footer metrics the
/// renderer's `footer_button_height` consumes.
pub(crate) fn production_notes_footer_action_row() -> ResolvedNotesFooterActionRow {
    let button_padding_y =
        crate::components::footer_chrome::current_main_menu_footer_metrics().button_padding_y;
    resolve_notes_footer_action_row(NOTES_FOOTER_ACTION_ROW_PRESENT, button_padding_y)
}

/// Structured input for the complete autosize composition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NotesAutosizeInput {
    /// Editor content height. Production source: soft-wrapped display-line
    /// count × the shared assumed line height (documented fallback — the
    /// shared editor exposes display-line counts, not intrinsic pixel
    /// bounds; see `editor_content_measurement_source`).
    pub editor_content_height: f32,
    pub editor_insets: NotesInsets,
    pub heading_line_height: f32,
    pub body_line_height: f32,
    /// Minimum editor content the structural minimum must keep visible.
    pub minimum_editor_content_height: f32,
    pub titlebar_height: f32,
    /// Transient chrome contributions to the autosize equation. The Notes
    /// autosize contract intentionally models only persistent chrome: opening
    /// search/toolbar flexes the editor body instead of resizing the window,
    /// so production passes 0 here even while those bars are visible.
    pub search_height: f32,
    pub formatting_toolbar_height: f32,
    pub footer_action_row: ResolvedNotesFooterActionRow,
    /// AppKit frame-vs-content difference. The Notes window is a `PopUp`
    /// NSPanel with `appears_transparent` titlebar: GPUI paints the full
    /// window content (the 36px titlebar is CONTENT chrome), so the frame
    /// equals the content rect and these resolve to zero. Recorded
    /// explicitly rather than omitted.
    pub window_non_content_insets: NotesInsets,
    /// Restored/initial window height — the historical autosize floor.
    pub restored_minimum_height: f32,
    pub authored_maximum_height: f32,
    /// Diagnostic only. NEVER added to any total.
    pub native_footer_host_height: Option<f32>,
}

/// Complete resolved autosize composition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedNotesAutosize {
    pub editor_content_height: f32,
    pub editor_insets: NotesInsets,
    pub editor_box_height: f32,
    pub heading_line_height: f32,
    pub body_line_height: f32,
    pub content_chrome_height: f32,
    pub footer_action_row: ResolvedNotesFooterActionRow,
    pub window_non_content_insets: NotesInsets,
    pub structural_minimum_height: f32,
    pub effective_minimum_height: f32,
    pub effective_maximum_height: f32,
    pub desired_height: f32,
    pub clamped_height: f32,
    pub native_footer_host_height: Option<f32>,
    /// Always `false`: host geometry is diagnostic, never additive.
    pub native_footer_host_included: bool,
}

fn guarded_minimum(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

/// Pure autosize resolver. Preserves the historical clamping semantics of
/// `resolve_auto_resize_height`: a restored initial height above the authored
/// maximum remains a valid minimum (the maximum is raised to it), and
/// non-finite inputs degrade to the effective minimum instead of poisoning
/// the clamp.
pub(crate) fn resolve_notes_autosize(input: NotesAutosizeInput) -> ResolvedNotesAutosize {
    let editor_box_height = input.editor_content_height.max(0.0) + input.editor_insets.vertical();
    let content_chrome_height =
        input.titlebar_height + input.search_height + input.formatting_toolbar_height;
    let footer_height = input.footer_action_row.height;

    let desired_height = input.window_non_content_insets.vertical()
        + content_chrome_height
        + editor_box_height
        + footer_height;

    let structural_minimum_height = input.window_non_content_insets.vertical()
        + content_chrome_height
        + input.editor_insets.vertical()
        + input.minimum_editor_content_height.max(0.0)
        + footer_height;

    let effective_minimum_height =
        guarded_minimum(input.restored_minimum_height).max(structural_minimum_height);
    let effective_maximum_height =
        if input.authored_maximum_height.is_finite() && input.authored_maximum_height > 0.0 {
            input.authored_maximum_height.max(effective_minimum_height)
        } else {
            effective_minimum_height
        };
    let desired_for_clamp = if desired_height.is_finite() {
        desired_height
    } else {
        effective_minimum_height
    };
    let clamped_height =
        desired_for_clamp.clamp(effective_minimum_height, effective_maximum_height);

    ResolvedNotesAutosize {
        editor_content_height: input.editor_content_height,
        editor_insets: input.editor_insets,
        editor_box_height,
        heading_line_height: input.heading_line_height,
        body_line_height: input.body_line_height,
        content_chrome_height,
        footer_action_row: input.footer_action_row,
        window_non_content_insets: input.window_non_content_insets,
        structural_minimum_height,
        effective_minimum_height,
        effective_maximum_height,
        desired_height,
        clamped_height,
        native_footer_host_height: input.native_footer_host_height,
        native_footer_host_included: false,
    }
}

/// The measurement source label for the production editor content height.
/// Honest naming: this is NOT an intrinsic pixel measurement.
pub(crate) const EDITOR_CONTENT_MEASUREMENT_SOURCE: &str = "softWrappedDisplayLines";

/// Build the production autosize input from the live layout metrics and the
/// soft-wrapped display-line count — the same inputs `update_window_height`
/// and `automation_state` consume, so the two can never drift.
pub(crate) fn notes_autosize_input(
    metrics: &NotesLayoutMetrics,
    line_count: usize,
    restored_minimum_height: f32,
) -> NotesAutosizeInput {
    NotesAutosizeInput {
        editor_content_height: (line_count as f32) * metrics.auto_resize_line_height,
        // `auto_resize_padding` is editor_padding_y × 2 (from_style); express
        // it as the symmetric per-side inset it actually is.
        editor_insets: NotesInsets::symmetric(metrics.auto_resize_padding / 2.0),
        // The shared editor's markdown heading rows currently share the body
        // line box (assumed-line-height model); both metrics are carried
        // separately so a future shaped measurement can diverge without a
        // schema change.
        heading_line_height: metrics.auto_resize_line_height,
        body_line_height: metrics.auto_resize_line_height,
        minimum_editor_content_height: metrics.auto_resize_line_height,
        titlebar_height: metrics.titlebar_height,
        search_height: 0.0,
        formatting_toolbar_height: 0.0,
        footer_action_row: production_notes_footer_action_row(),
        window_non_content_insets: NotesInsets::default(),
        restored_minimum_height,
        authored_maximum_height: metrics.auto_resize_max_height,
        native_footer_host_height: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::window::style::{NotesLayoutMetrics, NotesWindowStyle};

    fn metrics() -> NotesLayoutMetrics {
        NotesLayoutMetrics::from_style(NotesWindowStyle::current())
    }

    #[test]
    fn production_footer_action_row_is_absent_and_reserves_zero() {
        // Runtime truth: render_editor paints no footer child (rail removed
        // in 4e1a71a84). The production resolver must agree.
        let row = production_notes_footer_action_row();
        assert!(!row.visible);
        assert_eq!(row.height, 0.0);
        assert_eq!(row.role, crate::protocol::GeometryRole::FooterActionRow);
    }

    #[test]
    fn visible_footer_action_row_routes_through_shared_footer_chrome_formula() {
        // Same derivation as the shared owner: base band minus 2×padding.
        let row = resolve_notes_footer_action_row(true, 2.0);
        assert_eq!(
            row.height,
            crate::components::footer_chrome::footer_button_height_in(row.base_height, 2.0)
        );
        assert_eq!(row.height, 32.0);
        assert_eq!(row.role, crate::protocol::GeometryRole::FooterActionRow);
    }

    #[test]
    fn production_autosize_includes_the_resolved_footer_action_row() {
        // The footer term in the equation IS the resolver height (0 while the
        // row is absent) — not an independent scalar.
        let resolved = resolve_notes_autosize(notes_autosize_input(&metrics(), 7, 280.0));
        let expected_desired = metrics().titlebar_height
            + 7.0 * metrics().auto_resize_line_height
            + metrics().auto_resize_padding
            + resolved.footer_action_row.height;
        assert_eq!(resolved.desired_height, expected_desired);
        assert_eq!(resolved.footer_action_row.height, 0.0);
    }

    #[test]
    fn hidden_footer_reserves_zero_height() {
        let hidden = resolve_notes_footer_action_row(false, 2.0);
        assert_eq!(hidden.height, 0.0);
        let mut input = notes_autosize_input(&metrics(), 3, 280.0);
        input.footer_action_row = hidden;
        let without = resolve_notes_autosize(input);
        input.footer_action_row = resolve_notes_footer_action_row(true, 2.0);
        let with = resolve_notes_autosize(input);
        assert_eq!(with.desired_height - without.desired_height, 32.0);
        assert_eq!(
            with.structural_minimum_height - without.structural_minimum_height,
            32.0
        );
    }

    #[test]
    fn native_footer_host_is_diagnostic_not_additive() {
        let mut input = notes_autosize_input(&metrics(), 5, 280.0);
        let baseline = resolve_notes_autosize(input);
        input.native_footer_host_height = Some(44.0);
        let with_host = resolve_notes_autosize(input);
        assert_eq!(with_host.desired_height, baseline.desired_height);
        assert_eq!(
            with_host.structural_minimum_height,
            baseline.structural_minimum_height
        );
        assert_eq!(with_host.native_footer_host_height, Some(44.0));
        assert!(!with_host.native_footer_host_included);
    }

    #[test]
    fn restored_height_above_authored_max_remains_the_minimum() {
        let resolved = resolve_notes_autosize(notes_autosize_input(&metrics(), 1, 900.0));
        assert_eq!(resolved.effective_minimum_height, 900.0);
        assert_eq!(resolved.effective_maximum_height, 900.0);
        assert_eq!(resolved.clamped_height, 900.0);
    }

    #[test]
    fn resolver_clamp_matches_the_historical_helper_for_finite_inputs() {
        // The resolver must not drift from resolve_auto_resize_height, the
        // historical clamp owner update_window_height still calls.
        for (lines, restored) in [(0usize, 280.0f32), (7, 280.0), (80, 280.0), (2, 900.0)] {
            let resolved =
                resolve_notes_autosize(notes_autosize_input(&metrics(), lines, restored));
            let historical = crate::notes::window::NotesApp::resolve_auto_resize_height(
                resolved.desired_height,
                restored,
                metrics().auto_resize_max_height,
            );
            assert_eq!(resolved.clamped_height, historical, "lines={lines}");
        }
    }

    #[test]
    fn stale_28_point_test_input_overlaps_the_painted_action_row() {
        // Negative fixture (historical red state, TEST DATA ONLY): a model
        // that reserves 28.0 under-reserves a painted 32.0 action row by 4pt;
        // the editor's bottom edge would overlap the row. The evaluator that
        // catches this lives in scripts/devtools/notes.ts
        // (evaluateEditorFooterExclusion); this locks the same arithmetic.
        let stale_reservation = 28.0_f32;
        let painted = resolve_notes_footer_action_row(true, 2.0);
        let window_height = 280.0_f32;
        let editor_bottom = window_height - stale_reservation;
        let footer_top = window_height - painted.height;
        let overlap = (editor_bottom - footer_top).max(0.0);
        assert_eq!(overlap, 4.0);
        // And the production resolver never reproduces the stale scalar.
        assert_ne!(painted.height, stale_reservation);
    }

    #[test]
    fn heading_and_body_metrics_are_preserved_in_the_resolved_breakdown() {
        let resolved = resolve_notes_autosize(notes_autosize_input(&metrics(), 4, 280.0));
        assert_eq!(
            resolved.heading_line_height,
            metrics().auto_resize_line_height
        );
        assert_eq!(resolved.body_line_height, metrics().auto_resize_line_height);
        assert_eq!(
            resolved.editor_insets.vertical(),
            metrics().auto_resize_padding
        );
        assert_eq!(resolved.window_non_content_insets.vertical(), 0.0);
    }

    #[test]
    fn structural_minimum_keeps_one_editor_line_visible() {
        let resolved = resolve_notes_autosize(notes_autosize_input(&metrics(), 0, 280.0));
        let expected_structural = metrics().titlebar_height
            + metrics().auto_resize_padding
            + metrics().auto_resize_line_height
            + resolved.footer_action_row.height;
        assert_eq!(resolved.structural_minimum_height, expected_structural);
        // The restored floor (280) dominates today; the structural term is
        // the safety net, not the usual boundary.
        assert_eq!(resolved.effective_minimum_height, 280.0);
    }
}
