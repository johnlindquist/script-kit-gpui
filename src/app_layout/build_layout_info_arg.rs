impl ScriptListApp {
    fn append_arg_prompt_layout_components(
        &self,
        geometry: &ArgPromptGeometry,
        window_width: f32,
        window_height: f32,
        header_metrics: &crate::components::main_view_chrome::MainViewHeaderMetrics,
        components: &mut Vec<protocol::LayoutComponentInfo>,
    ) {
        use protocol::{LayoutComponentInfo, LayoutComponentType};

        let main_x = f32::from(geometry.shell_bounds.origin.x);
        let main_y = f32::from(geometry.shell_bounds.origin.y);
        let main_width = f32::from(geometry.shell_bounds.size.width);
        use crate::window_resize::arg_layout::{
            ArgPresentationMode, ARG_LIST_VIEWPORT_MEASUREMENT_ID,
            ARG_ROW_MEASUREMENT_ID_PREFIX, MINI_LIST_VIEWPORT_MEASUREMENT_ID,
            MINI_ROW_MEASUREMENT_ID_PREFIX,
        };

        let mode = if matches!(self.current_view, AppView::MiniPrompt { .. }) {
            ArgPresentationMode::Mini
        } else {
            ArgPresentationMode::Full
        };
        let filtered_len = self.filtered_arg_choices().len();
        let resolved = geometry.resolved;
        let (viewport_id, row_id_prefix) = match mode {
            ArgPresentationMode::Mini => (
                MINI_LIST_VIEWPORT_MEASUREMENT_ID,
                MINI_ROW_MEASUREMENT_ID_PREFIX,
            ),
            ArgPresentationMode::Full => (
                ARG_LIST_VIEWPORT_MEASUREMENT_ID,
                ARG_ROW_MEASUREMENT_ID_PREFIX,
            ),
        };

        let header_h = resolved.header_chrome_height;
        let viewport_top = main_y + header_h;
        let viewport_h = resolved.viewport_height;
        let footer_top = window_height - resolved.footer_reservation_height;

        components.push(
            LayoutComponentInfo::new("ArgPromptHeader", LayoutComponentType::Header)
                .with_geometry_identity(
                    "layout:arg-prompt-header",
                    None,
                    crate::list_item::geometry_roles::GeometryRole::MainHeaderChrome
                        .to_protocol(),
                )
                .with_bounds(main_x, main_y, main_width, header_h)
                .with_depth(1)
                .with_parent("Window")
                .with_explanation(format!(
                    "Canonical context/input header: context({}) + input({}) + gap({}) + padding({}) * 2 = {}px; origin is the production shell allocation.",
                    header_metrics.context_height,
                    header_metrics.input_height.unwrap_or(0.0),
                    header_metrics.gap,
                    header_metrics.padding_y,
                    header_h,
                )),
        );
        components.push(
            LayoutComponentInfo::new(viewport_id, LayoutComponentType::List)
                .with_geometry_identity(
                    format!("layout:{viewport_id}"),
                    None,
                    crate::list_item::geometry_roles::GeometryRole::ContentViewport
                        .to_protocol(),
                )
                .with_bounds(main_x, viewport_top, main_width, viewport_h)
                .with_depth(1)
                .with_parent("Window")
                .with_explanation(format!(
                    "Choice-list viewport excludes the rendered footer reservation. \
                     rowSlotHeight={} visibleRowCapacity={} intendedVisibleRows={} \
                     listPaddingTop={} listPaddingBottom={} choiceCount={}",
                    resolved.row_slot_height,
                    resolved.visible_row_capacity,
                    resolved.intended_visible_rows,
                    resolved.list_padding_top,
                    resolved.list_padding_bottom,
                    filtered_len
                )),
        );
        // The derived footer reservation is its own measurement. Its
        // protocol role is pending IR-01 (renderedFooterReservation); it
        // must NOT borrow a painted footer owner's role.
        components.push(
            LayoutComponentInfo::new("ArgFooterReservation", LayoutComponentType::Panel)
                .with_geometry_identity(
                    "layout:arg-footer-reservation",
                    None,
                    crate::list_item::geometry_roles::GeometryRole::RenderedFooterReservation
                        .to_protocol(),
                )
                .with_bounds(
                    0.0,
                    footer_top,
                    window_width,
                    resolved.footer_reservation_height,
                )
                .with_depth(1)
                .with_parent("Window")
                .with_explanation(format!(
                    "Derived safe-viewport exclusion sourced from the native footer owner \
                     (height {}px). Distinct measurement; not an alias of the painted footer.",
                    resolved.footer_reservation_height
                )),
        );

        // Visible modeled rows at the current scroll offset.
        let scroll_offset_y = {
            let state = self.arg_list_scroll_handle.0.borrow();
            (-state.base_handle.offset().y.as_f32()).max(0.0)
        };
        let first_visible = (scroll_offset_y / resolved.row_slot_height).floor() as usize;
        let last_visible =
            ((scroll_offset_y + viewport_h) / resolved.row_slot_height).ceil() as usize;
        for ix in first_visible..last_visible.min(filtered_len) {
            let row_y = viewport_top + (ix as f32 * resolved.row_slot_height) - scroll_offset_y;
            let selected = ix == self.arg_selected_index;
            components.push(
                LayoutComponentInfo::new(
                    format!("{row_id_prefix}:{ix}"),
                    LayoutComponentType::ListItem,
                )
                .with_geometry_identity(
                    format!("layout:{row_id_prefix}:{ix}"),
                    None,
                    crate::list_item::geometry_roles::GeometryRole::RowSlot.to_protocol(),
                )
                .with_bounds(main_x, row_y, main_width, resolved.row_slot_height)
                .with_depth(2)
                .with_parent(viewport_id)
                .with_explanation(format!(
                    "Modeled {} row {} at the resolved {}px row slot{}.",
                    mode.as_str(),
                    ix,
                    resolved.row_slot_height,
                    if selected { " (selected)" } else { "" }
                )),
            );
        }
    }
}
