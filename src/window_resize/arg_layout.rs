//! GEO-002 — Arg/Mini prompt geometry resolved from the active renderer.
//!
//! The historical model priced Arg/Mini windows from `LIST_ITEM_HEIGHT`
//! (40px) plus modeled list padding (`ARG_LIST_PADDING_Y`) and a divider
//! (`ARG_DIVIDER_HEIGHT`) that the minimal prompt shell never paints, and a
//! mixed `ARG_HEADER_HEIGHT` that baked a stale 30px GPUI footer into the
//! "header". The renderer truth is:
//!
//! - complete canonical context/input header, measured by `main_view_header_metrics`;
//! - rows: the legacy `ListItem` component rendered with the CURRENT main-menu
//!   theme metrics (`ListItemMetricsOverride::from_main_menu_theme`), i.e. the
//!   canonical 44px general row — not the stale 40px constant;
//! - no painted list padding and no painted divider inside the choice list;
//! - footer: the native main-window footer and its detached gutter (when active),
//!   carried as the derived `RenderedFooterReservation` role — not a header
//!   component;
//! - window border: `layout::WINDOW_BORDER_Y` painted by `render_impl`.
//! - additional in-flow chrome, including the real missing-Bun warning, is
//!   reconciled from the rendered prompt shell's available height.
//!
//! [`resolved_arg_layout`] is pure and testable; the runtime constructors
//! ([`current_arg_layout_inputs`]) obtain every input from the current
//! design/theme/footer owners so the model can never freeze a stale copy.

use crate::list_item::geometry_roles::GeometryRole;

/// Presentation mode for an Arg-family choice prompt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgPresentationMode {
    /// Compact prompt with an intended visible-row cap (currently five).
    Mini,
    /// Full Arg prompt; intends to show every choice up to the window clamp.
    Full,
}

impl ArgPresentationMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mini => "mini",
            Self::Full => "full",
        }
    }
}

/// The rendered footer reservation the content layer must keep clear.
/// `owner_role` names the painted owner the reservation is derived from; the
/// reservation itself carries the distinct `RenderedFooterReservation` role.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderedFooterReservation {
    pub owner_role: GeometryRole,
    pub reservation_height: f32,
}

/// Every input the pure resolver needs, sourced from the active renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArgLayoutInputs {
    pub mode: ArgPresentationMode,
    pub choice_count: usize,
    pub window_height: f32,
    pub window_non_content_height: f32,
    pub header_chrome_height: f32,
    pub section_slot_height: f32,
    pub row_slot_height: f32,
    pub list_padding_top: f32,
    pub list_padding_bottom: f32,
    pub footer: RenderedFooterReservation,
    pub mini_visible_row_limit: usize,
}

/// Resolved Arg geometry: one source for window sizing, layout models, and
/// runtime receipts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedArgLayout {
    pub mode: ArgPresentationMode,
    pub header_chrome_height: f32,
    pub section_slot_height: f32,
    pub row_slot_height: f32,
    pub list_padding_top: f32,
    pub list_padding_bottom: f32,
    /// Painted owner of the footer reservation (metadata only; the
    /// reservation is never numerically joined to the owner's own bounds).
    pub footer_owner_role: GeometryRole,
    pub footer_reservation_height: f32,
    pub viewport_height: f32,
    pub visible_row_capacity: usize,
    pub intended_visible_rows: usize,
}

/// Pure Arg geometry resolution. `viewport_height` is derived from the given
/// window height; `visible_row_capacity` is the floor of viewport over the
/// active row slot.
pub fn resolved_arg_layout(inputs: ArgLayoutInputs) -> ResolvedArgLayout {
    assert!(
        inputs.row_slot_height > 0.0,
        "Arg row-slot height must come from an active renderer"
    );
    let intended_visible_rows = if inputs.choice_count == 0 {
        0
    } else {
        match inputs.mode {
            ArgPresentationMode::Mini => inputs.choice_count.min(inputs.mini_visible_row_limit),
            ArgPresentationMode::Full => inputs.choice_count,
        }
    };
    let viewport_height = (inputs.window_height
        - inputs.window_non_content_height
        - inputs.header_chrome_height
        - inputs.section_slot_height
        - inputs.list_padding_top
        - inputs.list_padding_bottom
        - inputs.footer.reservation_height)
        .max(0.0);
    let visible_row_capacity = (viewport_height / inputs.row_slot_height).floor().max(0.0) as usize;
    ResolvedArgLayout {
        mode: inputs.mode,
        header_chrome_height: inputs.header_chrome_height,
        section_slot_height: inputs.section_slot_height,
        row_slot_height: inputs.row_slot_height,
        list_padding_top: inputs.list_padding_top,
        list_padding_bottom: inputs.list_padding_bottom,
        footer_owner_role: inputs.footer.owner_role,
        footer_reservation_height: inputs.footer.reservation_height,
        viewport_height,
        visible_row_capacity,
        intended_visible_rows,
    }
}

/// Target window height for a resolved layout, clamped to the standard
/// min/max policy.
pub fn target_arg_window_height(
    layout: ResolvedArgLayout,
    window_non_content_height: f32,
    min_height: f32,
    max_height: f32,
) -> f32 {
    let requested = window_non_content_height
        + layout.header_chrome_height
        + layout.section_slot_height
        + layout.list_padding_top
        + layout.list_padding_bottom
        + layout.footer_reservation_height
        + layout.intended_visible_rows as f32 * layout.row_slot_height;
    requested.clamp(min_height, max_height)
}

/// Resolve the chrome excluded by the production root's actual shell allocation.
/// Both target sizing and semantic geometry consume these same inputs. A
/// detached footer is already outside the shell; an in-flow footer is inside.
pub fn arg_layout_inputs_from_rendered_content(
    mut inputs: ArgLayoutInputs,
    rendered_content_height: f32,
    footer_outside_content: bool,
) -> ArgLayoutInputs {
    let outside_footer_height = if footer_outside_content {
        inputs.footer.reservation_height
    } else {
        0.0
    };
    inputs.window_non_content_height =
        (inputs.window_height - rendered_content_height - outside_footer_height).max(0.0);
    inputs
}

/// Mini's intended visible-row policy (product policy, not a measured value).
pub const MINI_VISIBLE_ROW_LIMIT: usize = 5;

/// Stable measurement IDs shared by the renderer wrappers, the layout model,
/// and paint joins. Rows append `:{index}`.
pub const ARG_LIST_VIEWPORT_MEASUREMENT_ID: &str = "arg-list-viewport";
pub const ARG_ROW_MEASUREMENT_ID_PREFIX: &str = "arg-row";
pub const MINI_LIST_VIEWPORT_MEASUREMENT_ID: &str = "mini-list-viewport";
pub const MINI_ROW_MEASUREMENT_ID_PREFIX: &str = "mini-row";

/// Complete context/input header owned by the main-view chrome renderer.
pub fn arg_header_chrome_height() -> f32 {
    let def = crate::designs::current_main_menu_theme().def();
    crate::components::main_view_chrome::main_view_header_metrics(def, Some(def.search.height))
        .header_height
}

/// The full footer exclusion for Arg/Mini prompts. The detached footer's real
/// gutter belongs to the reservation too; it is not usable choice-list space.
pub fn arg_rendered_footer_reservation() -> RenderedFooterReservation {
    RenderedFooterReservation {
        owner_role: GeometryRole::FooterNativeHost,
        reservation_height: crate::components::footer_chrome::current_main_menu_footer_height()
            + if crate::footer_popup::glass_scroll_bands_active() {
                crate::footer_popup::FLOAT_FOOTER_CONTAINER_GAP_PX
            } else {
                0.0
            },
    }
}

/// The active Arg/Mini row slot: the legacy `ListItem` renderer resolves the
/// CURRENT main-menu theme metrics at paint time, so prediction must use the
/// same source (GEO-009 ledger: `ArgPromptChoices` / `MiniPromptChoices`).
pub fn arg_row_slot_height() -> f32 {
    crate::list_item::effective_list_item_height_for_theme(crate::designs::current_main_menu_theme())
}

/// Build the current runtime inputs for a mode and choice count. Every value
/// comes from the live design/theme/footer owners; nothing is frozen.
pub fn current_arg_layout_inputs(
    mode: ArgPresentationMode,
    choice_count: usize,
    window_height: f32,
) -> ArgLayoutInputs {
    ArgLayoutInputs {
        mode,
        choice_count,
        window_height,
        window_non_content_height: super::layout::WINDOW_BORDER_Y,
        header_chrome_height: arg_header_chrome_height(),
        // Arg/Mini choice lists are unsectioned.
        section_slot_height: 0.0,
        row_slot_height: arg_row_slot_height(),
        // Canonical chrome has no list padding or interior list divider.
        list_padding_top: 0.0,
        list_padding_bottom: 0.0,
        footer: arg_rendered_footer_reservation(),
        mini_visible_row_limit: MINI_VISIBLE_ROW_LIMIT,
    }
}

/// Resolve the current Arg layout for a target-height calculation: the window
/// height is derived from the intended rows, then re-resolved so capacity
/// reflects the final clamped height.
pub fn current_resolved_arg_layout_for_target(
    mode: ArgPresentationMode,
    choice_count: usize,
    min_height: f32,
    max_height: f32,
) -> (ResolvedArgLayout, f32) {
    // First pass with an unbounded window to price the intent.
    let intent_inputs = current_arg_layout_inputs(mode, choice_count, f32::MAX);
    let intent = resolved_arg_layout(intent_inputs);
    let target = target_arg_window_height(
        intent,
        intent_inputs.window_non_content_height,
        min_height,
        max_height,
    );
    // Second pass against the clamped window so capacity is honest.
    let final_inputs = current_arg_layout_inputs(mode, choice_count, target);
    (resolved_arg_layout(final_inputs), target)
}

#[cfg(test)]
mod arg_layout_contract_tests {
    use super::*;

    fn inputs(mode: ArgPresentationMode, choice_count: usize) -> ArgLayoutInputs {
        ArgLayoutInputs {
            mode,
            choice_count,
            window_height: 500.0,
            window_non_content_height: 2.0,
            header_chrome_height: 45.0,
            section_slot_height: 0.0,
            row_slot_height: 44.0,
            list_padding_top: 0.0,
            list_padding_bottom: 0.0,
            footer: RenderedFooterReservation {
                owner_role: GeometryRole::FooterNativeHost,
                reservation_height: 36.0,
            },
            mini_visible_row_limit: MINI_VISIBLE_ROW_LIMIT,
        }
    }

    /// Full mode prices rows at the canonical general row (44px under current
    /// defs), sourced from the active renderer's themed metrics.
    #[test]
    fn full_preserves_canonical_general_row() {
        let themed_row = crate::list_item::effective_list_item_height_for_theme(
            crate::designs::MainMenuThemeVariant::default(),
        );
        assert_eq!(themed_row, 44.0, "canonical general row is 44px");
        let layout = resolved_arg_layout(ArgLayoutInputs {
            row_slot_height: themed_row,
            ..inputs(ArgPresentationMode::Full, 6)
        });
        assert_eq!(layout.row_slot_height, 44.0);
        assert_eq!(layout.intended_visible_rows, 6);
    }

    /// Mini mode uses the same canonical row; only the intended visible count
    /// differs, never the density.
    #[test]
    fn mini_preserves_canonical_general_row() {
        let layout = resolved_arg_layout(inputs(ArgPresentationMode::Mini, 6));
        assert_eq!(layout.row_slot_height, 44.0);
    }

    /// The renderer paints no list padding; the runtime inputs must not
    /// resurrect the modeled `ARG_LIST_PADDING_Y`/`ARG_DIVIDER_HEIGHT`.
    #[test]
    fn unrendered_list_padding_is_zero() {
        let runtime = current_arg_layout_inputs(ArgPresentationMode::Full, 6, 500.0);
        assert_eq!(runtime.list_padding_top, 0.0);
        assert_eq!(runtime.list_padding_bottom, 0.0);
    }

    /// The footer reservation is its own component with its own owner role —
    /// never folded into the header chrome value.
    #[test]
    fn footer_reservation_is_not_header_height() {
        let runtime = current_arg_layout_inputs(ArgPresentationMode::Full, 6, 500.0);
        let layout = resolved_arg_layout(runtime);
        assert_eq!(layout.footer_owner_role, GeometryRole::FooterNativeHost);
        assert!(
            !GeometryRole::RenderedFooterReservation.comparable_to(GeometryRole::MainHeaderChrome)
        );
        let def = crate::designs::current_main_menu_theme().def();
        assert_eq!(
            layout.header_chrome_height,
            crate::components::main_view_chrome::main_view_header_metrics(
                def,
                Some(def.search.height)
            )
            .header_height,
        );
        assert_ne!(layout.footer_reservation_height, 0.0);
    }

    /// Mini declares a five-row intent for six choices; the sixth row is
    /// reached by scrolling, not by growing the window.
    #[test]
    fn mini_declares_five_row_intent_for_six_choices() {
        let layout = resolved_arg_layout(inputs(ArgPresentationMode::Mini, 6));
        assert_eq!(layout.intended_visible_rows, 5);
        let full = resolved_arg_layout(inputs(ArgPresentationMode::Full, 6));
        assert_eq!(full.intended_visible_rows, 6);
    }

    /// The Full six-row branch is derived from resolved capacity, never
    /// hardcoded: capacity >= 6 means all six visible; below six means the
    /// selection path must scroll the sixth row into the safe viewport.
    #[test]
    fn full_six_row_branch_is_derived_from_capacity() {
        // Tall window: capacity covers all six rows.
        let (layout, target) = {
            let intent_inputs = ArgLayoutInputs {
                window_height: f32::MAX,
                ..inputs(ArgPresentationMode::Full, 6)
            };
            let intent = resolved_arg_layout(intent_inputs);
            let target = target_arg_window_height(intent, 2.0, 68.0, 500.0);
            (
                resolved_arg_layout(ArgLayoutInputs {
                    window_height: target,
                    ..inputs(ArgPresentationMode::Full, 6)
                }),
                target,
            )
        };
        assert_eq!(target, 2.0 + 45.0 + 36.0 + 6.0 * 44.0);
        assert!(layout.visible_row_capacity >= 6, "six rows fit unclamped");

        // Short clamp: capacity drops below six -> scroll branch.
        let clamped = resolved_arg_layout(ArgLayoutInputs {
            window_height: 260.0,
            ..inputs(ArgPresentationMode::Full, 6)
        });
        assert!(clamped.visible_row_capacity < 6);
    }

    /// The stale 40px model is rejected: pricing six rows with the legacy
    /// constant disagrees with the renderer-derived layout, so a 40px model
    /// can never reproduce the resolved target height.
    #[test]
    fn forty_pixel_model_is_rejected() {
        let renderer_layout = resolved_arg_layout(inputs(ArgPresentationMode::Full, 6));
        let stale_model = resolved_arg_layout(ArgLayoutInputs {
            row_slot_height: crate::list_item::LIST_ITEM_HEIGHT,
            ..inputs(ArgPresentationMode::Full, 6)
        });
        assert_eq!(crate::list_item::LIST_ITEM_HEIGHT, 40.0);
        let renderer_target = target_arg_window_height(renderer_layout, 2.0, 68.0, 800.0);
        let stale_target = target_arg_window_height(stale_model, 2.0, 68.0, 800.0);
        assert_ne!(
            renderer_target, stale_target,
            "a 40px row model must not be able to masquerade as the rendered 44px truth"
        );
    }

    /// A sixth row whose bottom crosses the footer-exclusion top by one pixel
    /// is NOT fully visible: capacity math must exclude it.
    #[test]
    fn one_pixel_footer_overlap_fails() {
        // Exactly six rows fit (viewport == 6 * 44).
        let exact = resolved_arg_layout(ArgLayoutInputs {
            window_height: 2.0 + 45.0 + 36.0 + 6.0 * 44.0,
            ..inputs(ArgPresentationMode::Full, 6)
        });
        assert_eq!(exact.visible_row_capacity, 6);
        // One pixel less viewport: the sixth row would overlap the rendered
        // footer reservation, so it no longer counts as fully visible.
        let one_pixel_short = resolved_arg_layout(ArgLayoutInputs {
            window_height: 2.0 + 45.0 + 36.0 + 6.0 * 44.0 - 1.0,
            ..inputs(ArgPresentationMode::Full, 6)
        });
        assert_eq!(one_pixel_short.visible_row_capacity, 5);
    }

    /// Empty choice lists intend zero rows in both modes.
    #[test]
    fn empty_choices_intend_zero_rows() {
        for mode in [ArgPresentationMode::Mini, ArgPresentationMode::Full] {
            let layout = resolved_arg_layout(inputs(mode, 0));
            assert_eq!(layout.intended_visible_rows, 0);
        }
    }

    #[test]
    fn canonical_mini_target_preserves_exact_five_row_viewport() {
        let (mini, height) =
            current_resolved_arg_layout_for_target(ArgPresentationMode::Mini, 6, 0.0, 2000.0);
        assert_eq!(mini.visible_row_capacity, MINI_VISIBLE_ROW_LIMIT);
        assert!((mini.viewport_height - 5.0 * mini.row_slot_height).abs() < 0.001);
        let (full, full_height) =
            current_resolved_arg_layout_for_target(ArgPresentationMode::Full, 6, 0.0, 2000.0);
        assert_eq!(full.visible_row_capacity, 6);
        assert!((full_height - height - full.row_slot_height).abs() < 0.001);
    }

    #[test]
    fn rendered_warning_and_footer_chrome_preserve_five_whole_mini_rows() {
        for warning_height in [0.0, 60.5] {
            for detached_footer in [false, true] {
                let footer_height = 32.0;
                let gutter_height = if detached_footer {
                    crate::footer_popup::FLOAT_FOOTER_CONTAINER_GAP_PX
                } else {
                    0.0
                };
                let reservation_height = footer_height + gutter_height;
                let header_height = 58.0;
                let root_chrome = 2.0
                    + warning_height
                    + if detached_footer {
                        reservation_height
                    } else {
                        0.0
                    };
                let initial = ArgLayoutInputs {
                    window_height: 312.0,
                    header_chrome_height: header_height,
                    footer: RenderedFooterReservation {
                        owner_role: GeometryRole::FooterNativeHost,
                        reservation_height,
                    },
                    ..inputs(ArgPresentationMode::Mini, 6)
                };
                let rendered_inputs = arg_layout_inputs_from_rendered_content(
                    initial,
                    initial.window_height - root_chrome,
                    detached_footer,
                );
                let target = target_arg_window_height(
                    resolved_arg_layout(rendered_inputs),
                    rendered_inputs.window_non_content_height,
                    68.0,
                    500.0,
                );
                assert_eq!(
                    target,
                    2.0 + warning_height + header_height + reservation_height + 5.0 * 44.0
                );
                let settled_inputs = arg_layout_inputs_from_rendered_content(
                    ArgLayoutInputs {
                        window_height: target,
                        ..initial
                    },
                    target - root_chrome,
                    detached_footer,
                );
                let settled = resolved_arg_layout(settled_inputs);
                assert_eq!(settled.viewport_height, 220.0);
                assert_eq!(settled.visible_row_capacity, 5);
                assert_eq!(settled.intended_visible_rows, 5);
                assert_eq!(
                    target_arg_window_height(
                        settled,
                        settled_inputs.window_non_content_height,
                        68.0,
                        500.0,
                    ),
                    target,
                    "settled geometry must not schedule another resize"
                );
            }
        }
    }

    #[test]
    fn rendered_chrome_reconciliation_preserves_the_window_clamp() {
        let rendered_inputs = arg_layout_inputs_from_rendered_content(
            inputs(ArgPresentationMode::Mini, 6),
            100.0,
            true,
        );
        assert_eq!(
            target_arg_window_height(
                resolved_arg_layout(rendered_inputs),
                rendered_inputs.window_non_content_height,
                68.0,
                300.0,
            ),
            300.0,
        );
    }
}
