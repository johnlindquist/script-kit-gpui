/// Single source of truth for Actions shell height. Detached windows evaluate
/// it once from the opening root/unfiltered snapshot; inline dialogs use it for
/// their local content-derived shell.
#[inline]
pub(super) fn actions_window_dynamic_height(
    num_actions: usize,
    section_header_count: usize,
    hide_search: bool,
    has_header: bool,
    show_footer: bool,
    max_height: f32,
    row_height: f32,
) -> f32 {
    let tokens = crate::designs::current_actions_popup_theme();
    resolved_actions_popup_height(
        &tokens,
        (num_actions, section_header_count),
        hide_search,
        has_header,
        show_footer,
        max_height,
        row_height,
    )
}

/// Pure popup-height formula over an explicit token definition. Production
/// passes `current_actions_popup_theme()`; the design-contract exporter passes
/// `base_actions_popup_theme()` so checked-in artifacts always match the base
/// token definition.
pub(crate) fn resolved_actions_popup_height(
    tokens: &crate::designs::ActionsPopupThemeDef,
    row_counts: (usize, usize),
    hide_search: bool,
    has_header: bool,
    show_footer: bool,
    max_height: f32,
    row_height: f32,
) -> f32 {
    let (num_actions, section_header_count) = row_counts;
    const POPUP_FOOTER_HEIGHT: f32 = 32.0;
    let search_box_height = if hide_search {
        0.0
    } else {
        tokens.search.height
    };
    let header_height = if has_header {
        tokens.context_header.height
    } else {
        0.0
    };
    let footer_height = if show_footer {
        POPUP_FOOTER_HEIGHT
    } else {
        0.0
    };
    let section_headers_height = section_header_count as f32 * tokens.list.section_header_height;
    let min_items_height = if num_actions == 0 {
        tokens.list.empty_row_height
    } else {
        0.0
    };
    let list_padding_height = tokens.list.padding_top + tokens.list.padding_bottom;
    let max_height = max_height.min(tokens.shell.max_height);
    let items_height = ((num_actions as f32 * row_height + section_headers_height)
        .max(min_items_height)
        + list_padding_height)
        .min(max_height - search_box_height - header_height - footer_height);
    let border_height = tokens.shell.border_height;
    items_height + search_box_height + header_height + footer_height + border_height
}

/// Compute the origin point for the actions popup window.
///
/// Pure helper that encapsulates all position-dependent origin math so it can
/// be tested without standing up a real window.
fn actions_popup_origin(
    main_window_bounds: Bounds<Pixels>,
    window_width: Pixels,
    window_height: Pixels,
    position: WindowPosition,
) -> Point<Pixels> {
    let tokens = crate::designs::current_actions_popup_theme();
    let right_aligned_x = main_window_bounds.origin.x + main_window_bounds.size.width
        - window_width
        - px(tokens.shell.margin_x);

    let y = match position {
        WindowPosition::BottomRight => {
            main_window_bounds.origin.y + main_window_bounds.size.height
                - window_height
                - px(FOOTER_HEIGHT)
                - px(tokens.shell.margin_y)
        }
        WindowPosition::TopRight | WindowPosition::TopCenter => {
            main_window_bounds.origin.y
                + px(tokens.shell.titlebar_offset_y)
                + px(tokens.shell.margin_y)
        }
    };

    let x = match position {
        WindowPosition::TopCenter => {
            main_window_bounds.origin.x + (main_window_bounds.size.width - window_width) / 2.0
        }
        _ => right_aligned_x,
    };

    Point { x, y }
}

/// Full popup bounds (origin + size) for the actions window.
///
/// Wraps [`actions_popup_origin`] so callers get a single `Bounds` value
/// without reconstructing size separately.
fn actions_popup_bounds(
    main_window_bounds: Bounds<Pixels>,
    window_width: Pixels,
    window_height: Pixels,
    position: WindowPosition,
) -> Bounds<Pixels> {
    Bounds {
        origin: actions_popup_origin(main_window_bounds, window_width, window_height, position),
        size: Size {
            width: window_width,
            height: window_height,
        },
    }
}

/// Structured placement receipt for the actions popup.
///
/// Captures all inputs and computed outputs of a placement decision so that
/// agentic callers can verify geometry deterministically.
#[derive(Debug)]
struct ActionsPopupPlacementReceipt {
    position: WindowPosition,
    display_id: Option<DisplayId>,
    main_window_bounds: Bounds<Pixels>,
    popup_bounds: Bounds<Pixels>,
    anchor_x: Pixels,
    anchor_y: Pixels,
    pinned_edge: &'static str,
}

fn actions_popup_placement_receipt(
    main_window_bounds: Bounds<Pixels>,
    window_width: Pixels,
    window_height: Pixels,
    position: WindowPosition,
    display_id: Option<DisplayId>,
) -> ActionsPopupPlacementReceipt {
    let tokens = crate::designs::current_actions_popup_theme();
    let popup_bounds =
        actions_popup_bounds(main_window_bounds, window_width, window_height, position);

    let (anchor_x, anchor_y, pinned_edge) = match position {
        WindowPosition::BottomRight => (
            main_window_bounds.origin.x + main_window_bounds.size.width - px(tokens.shell.margin_x),
            main_window_bounds.origin.y + main_window_bounds.size.height
                - px(FOOTER_HEIGHT)
                - px(tokens.shell.margin_y),
            "bottom",
        ),
        WindowPosition::TopRight => (
            main_window_bounds.origin.x + main_window_bounds.size.width - px(tokens.shell.margin_x),
            main_window_bounds.origin.y
                + px(tokens.shell.titlebar_offset_y)
                + px(tokens.shell.margin_y),
            "top",
        ),
        WindowPosition::TopCenter => (
            main_window_bounds.origin.x + (main_window_bounds.size.width / 2.0),
            main_window_bounds.origin.y
                + px(tokens.shell.titlebar_offset_y)
                + px(tokens.shell.margin_y),
            "top",
        ),
    };

    ActionsPopupPlacementReceipt {
        position,
        display_id,
        main_window_bounds,
        popup_bounds,
        anchor_x,
        anchor_y,
        pinned_edge,
    }
}

fn log_actions_popup_placement(stage: &'static str, receipt: &ActionsPopupPlacementReceipt) {
    let main_origin_x_px: f32 = receipt.main_window_bounds.origin.x.into();
    let main_origin_y_px: f32 = receipt.main_window_bounds.origin.y.into();
    let main_width_px: f32 = receipt.main_window_bounds.size.width.into();
    let main_height_px: f32 = receipt.main_window_bounds.size.height.into();

    let popup_origin_x_px: f32 = receipt.popup_bounds.origin.x.into();
    let popup_origin_y_px: f32 = receipt.popup_bounds.origin.y.into();
    let popup_width_px: f32 = receipt.popup_bounds.size.width.into();
    let popup_height_px: f32 = receipt.popup_bounds.size.height.into();

    let anchor_x_px: f32 = receipt.anchor_x.into();
    let anchor_y_px: f32 = receipt.anchor_y.into();

    tracing::info!(
        target: "ACTIONS_POPUP",
        stage = stage,
        position = ?receipt.position,
        display_id = ?receipt.display_id,
        pinned_edge = receipt.pinned_edge,
        main_origin_x_px,
        main_origin_y_px,
        main_width_px,
        main_height_px,
        popup_origin_x_px,
        popup_origin_y_px,
        popup_width_px,
        popup_height_px,
        anchor_x_px,
        anchor_y_px,
        "actions popup placement receipt"
    );
}
