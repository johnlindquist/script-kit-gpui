use crate::ui_foundation::{is_key_down, is_key_enter, is_key_escape, is_key_space, is_key_up};

pub(crate) fn render_builtin_split_main_content_layout(
    list_pane: gpui::AnyElement,
    preview_pane: gpui::AnyElement,
) -> gpui::AnyElement {
    gpui::div()
        .flex()
        .flex_row()
        .flex_1()
        .min_h(gpui::px(0.))
        .w_full()
        .overflow_hidden()
        .child(
            gpui::div()
                .flex_1()
                .h_full()
                .min_w(gpui::px(0.))
                .min_h(gpui::px(0.))
                .child(list_pane),
        )
        .child(
            gpui::div()
                .flex_1()
                .h_full()
                .min_h(gpui::px(0.))
                .overflow_hidden()
                .child(preview_pane),
        )
        .into_any_element()
}

impl ScriptListApp {
    /// Available vibrancy material presets for the theme customizer
    const VIBRANCY_MATERIALS: &[(theme::VibrancyMaterial, &str)] = &[
        (theme::VibrancyMaterial::Hud, "HUD"),
        (theme::VibrancyMaterial::Popover, "Popover"),
        (theme::VibrancyMaterial::Menu, "Menu"),
        (theme::VibrancyMaterial::Sidebar, "Sidebar"),
        (theme::VibrancyMaterial::Content, "Content"),
    ];

    /// Available font size presets for the theme customizer
    const FONT_SIZE_PRESETS: &[(f32, &str)] = &[
        (12.0, "12"),
        (13.0, "13"),
        (14.0, "14"),
        (15.0, "15"),
        (16.0, "16"),
        (18.0, "18"),
        (20.0, "20"),
    ];

    /// Find the index of a vibrancy material in the presets array
    fn find_vibrancy_material_index(material: theme::VibrancyMaterial) -> usize {
        Self::VIBRANCY_MATERIALS
            .iter()
            .position(|(m, _)| *m == material)
            .unwrap_or(0)
    }

    /// Return a human-readable name for a hex accent color
    fn accent_color_name(color: u32) -> &'static str {
        theme::accent_color_name(color)
    }

    pub(crate) fn theme_font_family(&self) -> String {
        crate::theme::TypographyResolver::new_theme_first(&self.theme, self.current_design)
            .primary_font()
            .to_string()
    }

    pub(crate) fn theme_font_size_xl(&self) -> f32 {
        crate::theme::TypographyResolver::new_theme_first(&self.theme, self.current_design)
            .font_size_xl()
    }

    pub(crate) fn render_search_input(&self) -> gpui_component::input::Input {
        let search = self.current_main_menu_theme.def().search;
        let input_font_size = search.font_size;
        gpui_component::input::Input::new(&self.gpui_input_state)
            .w_full()
            .h(gpui::px(search.height))
            .line_height(gpui::px(search.height))
            .font_family(self.theme_font_family())
            .font_weight(search.font_weight)
            .px(gpui::px(0.))
            .py(gpui::px(0.))
            .with_size(gpui_component::Size::Size(gpui::px(input_font_size)))
            .appearance(false)
            .bordered(false)
            .focus_bordered(false)
    }

    pub(crate) fn render_search_input_with_ghost(&self, _cx: &gpui::Context<Self>) -> gpui::Div {
        gpui::div().w_full().child(self.render_search_input())
    }

    pub(crate) fn render_builtin_main_input_count_label(
        &self,
        label: impl Into<gpui::SharedString>,
    ) -> gpui::AnyElement {
        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        // Shared with the design-token exporter
        // (builtin_main_input_contract.rs): text_sm-sized, gpui default line
        // height and NORMAL weight (never the search body's 430), right
        // inset = search text_inset_x, color = chrome text hint.
        let style = resolved_builtin_main_input_count_label_style(
            self.current_main_menu_theme.def(),
            &chrome,
        );
        gpui::div()
            .flex_none()
            .whitespace_nowrap()
            .pr(gpui::px(style.inset_right))
            .text_size(gpui::px(style.font_size_px))
            .line_height(gpui::px(style.line_height_px))
            .font_weight(style.font_weight)
            .text_color(gpui::rgba(style.text_rgba))
            .child(label.into())
            .into_any_element()
    }

    pub(crate) fn render_builtin_main_input_shell(
        &self,
        trailing: Vec<gpui::AnyElement>,
    ) -> gpui::AnyElement {
        let menu_def = self.current_main_menu_theme.def();
        crate::components::main_view_chrome::render_main_view_input_shell(
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewInputChrome {
                body: self.render_search_input().into_any_element(),
                trailing,
            },
        )
    }

    pub(crate) fn render_builtin_main_input_header(
        &self,
        trailing: Vec<gpui::AnyElement>,
        cx: &mut gpui::Context<Self>,
    ) -> crate::components::main_view_chrome::MainViewHeaderChrome {
        let menu_def = self.current_main_menu_theme.def();
        crate::components::main_view_chrome::MainViewHeaderChrome::canonical(
            menu_def,
            self.render_clickable_main_view_context_zone(menu_def, cx),
            self.render_builtin_main_input_shell(trailing),
        )
    }

    pub(crate) fn render_builtin_main_input_surface(
        &self,
        key_context: &'static str,
        trailing: Vec<gpui::AnyElement>,
        main: gpui::AnyElement,
        footer: Option<gpui::AnyElement>,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;
        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(gpui::rgb(chrome.text_primary_hex))
                .font_family(self.theme_font_family())
                .key_context(key_context)
                .track_focus(&self.focus_handle),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(trailing, cx),
                divider: crate::components::main_view_chrome::MainViewDividerChrome {
                    margin_x: shell.divider_margin_x,
                    height: shell.divider_height,
                    visible: shell.divider_height > 0.0,
                },
                main,
                footer,
                overlays: Vec::new(),
            },
        )
    }

    pub(crate) fn render_generic_filterable_search_surface(
        &self,
        key_context: &'static str,
        count_label: String,
        list_element: gpui::AnyElement,
        footer: Option<gpui::AnyElement>,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let content = gpui::div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(gpui::px(0.))
            .w_full()
            .overflow_hidden()
            .child(list_element);

        self.render_builtin_main_input_surface(
            key_context,
            vec![self.render_builtin_main_input_count_label(count_label)],
            content.into_any_element(),
            footer,
            cx,
        )
    }

    pub(crate) fn render_builtin_split_main_content(
        &self,
        list_pane: gpui::AnyElement,
        preview_pane: gpui::AnyElement,
    ) -> gpui::AnyElement {
        render_builtin_split_main_content_layout(list_pane, preview_pane)
    }

    /// Emit a structured scroll log line for builtin views.
    #[allow(clippy::too_many_arguments)]
    fn log_builtin_scroll_event(
        view: &'static str,
        action: &'static str,
        reason: &'static str,
        item_count: usize,
        selected_index: Option<usize>,
        target_item: Option<usize>,
        filter: Option<&str>,
        input_mode: &'static str,
    ) {
        tracing::debug!(
            target: "script_kit::scroll",
            view = view,
            action = action,
            reason = reason,
            item_count = item_count,
            selected_index = selected_index
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".into())
                .as_str(),
            target_item = target_item
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".into())
                .as_str(),
            filter_len = filter
                .map(|v| v.chars().count().to_string())
                .unwrap_or_else(|| "none".into())
                .as_str(),
            input_mode = input_mode,
        );
    }

    /// Scroll a builtin uniform list to the top and emit a structured log.
    fn scroll_builtin_to_top_with_log(
        handle: &UniformListScrollHandle,
        view: &'static str,
        item_count: usize,
        filter: &str,
        input_mode: &'static str,
    ) {
        Self::log_builtin_scroll_event(
            view,
            "scroll_to_item",
            "filter_changed",
            item_count,
            Some(0),
            Some(0),
            Some(filter),
            input_mode,
        );
        handle.scroll_to_item(0, ScrollStrategy::Top);
    }

    /// Convert a wheel or trackpad event into whole-row builtin list steps.
    fn builtin_scroll_wheel_steps(&mut self, event: &gpui::ScrollWheelEvent) -> i32 {
        let avg_item_height = crate::list_item::effective_average_item_height_for_scroll();
        let (delta_kind, raw_delta_y, delta_lines): (&'static str, f32, f32) = match event.delta {
            gpui::ScrollDelta::Lines(point) => ("lines", point.y, point.y),
            gpui::ScrollDelta::Pixels(point) => {
                let pixels: f32 = point.y.into();
                ("pixels", pixels, pixels / avg_item_height)
            }
        };

        self.wheel_accum += -delta_lines;
        let steps = self.wheel_accum.trunc() as i32;
        if steps != 0 {
            self.wheel_accum -= steps as f32;
        }
        tracing::info!(
            target: "script_kit::scroll_trace",
            event = "SCROLL_TRACE wheel_steps",
            delta_kind,
            raw_delta_y,
            delta_lines,
            avg_item_height,
            steps,
            wheel_accum_after = self.wheel_accum,
            "SCROLL_TRACE wheel_steps"
        );
        steps
    }

    /// Resolve the next selected row for builtin browsers from a wheel event.
    fn builtin_scroll_target_from_wheel(
        &mut self,
        event: &gpui::ScrollWheelEvent,
        current_selected: usize,
        item_count: usize,
    ) -> Option<usize> {
        if item_count == 0 {
            self.wheel_accum = 0.0;
            tracing::info!(
                target: "script_kit::scroll_trace",
                event = "SCROLL_TRACE wheel_target.empty",
                current_selected,
                item_count,
                "SCROLL_TRACE wheel_target.empty"
            );
            return None;
        }

        let steps = self.builtin_scroll_wheel_steps(event);
        if steps == 0 {
            tracing::info!(
                target: "script_kit::scroll_trace",
                event = "SCROLL_TRACE wheel_target.no_whole_step",
                current_selected,
                item_count,
                wheel_accum = self.wheel_accum,
                "SCROLL_TRACE wheel_target.no_whole_step"
            );
            return None;
        }

        let max_index = item_count.saturating_sub(1) as i32;
        let target = (current_selected as i32 + steps).clamp(0, max_index) as usize;
        tracing::info!(
            target: "script_kit::scroll_trace",
            event = "SCROLL_TRACE wheel_target.result",
            current_selected,
            item_count,
            steps,
            target,
            "SCROLL_TRACE wheel_target.result"
        );
        Some(target)
    }

    /// Compute scrollbar metrics for a tracked uniform list.
    fn builtin_uniform_list_scrollbar_metrics(
        handle: &UniformListScrollHandle,
        total_items: usize,
        fallback_visible_items: usize,
    ) -> Option<(usize, usize, Option<f32>)> {
        if total_items == 0 {
            tracing::info!(
                target: "script_kit::scroll_trace",
                event = "SCROLL_TRACE metrics.empty",
                total_items,
                fallback_visible_items,
                "SCROLL_TRACE metrics.empty"
            );
            return None;
        }

        let state = handle.0.borrow();
        let live_scroll_top = state.base_handle.logical_scroll_top().0;
        let deferred_item_index = state
            .deferred_scroll_to_item
            .map(|deferred| deferred.item_index);
        let has_item_size = state.last_item_size.is_some();
        let scroll_offset = crate::components::scrollbar::preferred_scroll_offset(
            live_scroll_top,
            deferred_item_index,
            has_item_size,
            total_items,
        );

        let fallback_visible_items = fallback_visible_items.max(1).min(total_items);

        if let Some(item_size) = state.last_item_size {
            let viewport_height = item_size.item.height.as_f32().max(0.0);
            let content_height = item_size.contents.height.as_f32().max(0.0);
            let visible_items = if content_height > 0.0 {
                ((viewport_height / content_height) * total_items as f32)
                    .ceil()
                    .max(1.0) as usize
            } else {
                fallback_visible_items
            };
            let clamped_visible_items = visible_items.clamp(1, total_items);
            tracing::info!(
                target: "script_kit::scroll_trace",
                event = "SCROLL_TRACE metrics.measured",
                total_items,
                fallback_visible_items,
                live_scroll_top,
                deferred_item_index = ?deferred_item_index,
                has_item_size,
                scroll_offset,
                viewport_height,
                content_height,
                visible_items = clamped_visible_items,
                "SCROLL_TRACE metrics.measured"
            );

            Some((scroll_offset, clamped_visible_items, Some(viewport_height)))
        } else {
            tracing::info!(
                target: "script_kit::scroll_trace",
                event = "SCROLL_TRACE metrics.fallback",
                total_items,
                fallback_visible_items,
                live_scroll_top,
                deferred_item_index = ?deferred_item_index,
                has_item_size,
                scroll_offset,
                visible_items = fallback_visible_items,
                "SCROLL_TRACE metrics.fallback"
            );
            Some((scroll_offset, fallback_visible_items, None))
        }
    }

    fn note_builtin_selection_owned_wheel_scroll(&mut self, selected_index: usize) {
        self.builtin_wheel_owned_selected_index = Some(selected_index);
        tracing::info!(
            target: "script_kit::scroll_trace",
            event = "SCROLL_TRACE wheel_owned.note",
            selected_index,
            "SCROLL_TRACE wheel_owned.note"
        );
    }

    fn should_suppress_builtin_scroll_reanchor(&self, current_selected: usize) -> bool {
        self.builtin_wheel_owned_selected_index == Some(current_selected)
    }

    fn builtin_reanchor_selection_from_scroll(
        &self,
        current_selected: usize,
        handle: &UniformListScrollHandle,
        total_items: usize,
        fallback_visible_items: usize,
    ) -> Option<usize> {
        let suppress = self.should_suppress_builtin_scroll_reanchor(current_selected);
        tracing::info!(
            target: "script_kit::scroll_trace",
            event = "SCROLL_TRACE reanchor.check",
            current_selected,
            total_items,
            fallback_visible_items,
            wheel_owned_selected_index = ?self.builtin_wheel_owned_selected_index,
            suppress,
            "SCROLL_TRACE reanchor.check"
        );
        if suppress {
            return None;
        }

        let (first_visible, visible_items, _) = Self::builtin_uniform_list_scrollbar_metrics(
            handle,
            total_items,
            fallback_visible_items,
        )?;
        let reanchored = crate::scrolling::selection_owned::reanchor_uniform_selection(
            current_selected,
            first_visible,
            visible_items,
            total_items,
        );
        tracing::info!(
            target: "script_kit::scroll_trace",
            event = "SCROLL_TRACE reanchor.result",
            current_selected,
            first_visible,
            visible_items,
            total_items,
            reanchored = ?reanchored,
            "SCROLL_TRACE reanchor.result"
        );
        reanchored
    }

    fn builtin_reanchor_selection_from_scroll_handle(
        current_selected: usize,
        handle: &gpui::ScrollHandle,
        total_items: usize,
    ) -> Option<usize> {
        if total_items == 0 || handle.children_count() == 0 {
            return None;
        }

        let first_visible = handle
            .logical_scroll_top()
            .0
            .min(total_items.saturating_sub(1));
        let last_visible = handle.bottom_item().min(total_items.saturating_sub(1));
        let visible_items = last_visible.saturating_sub(first_visible) + 1;

        crate::scrolling::selection_owned::reanchor_uniform_selection(
            current_selected,
            first_visible,
            visible_items,
            total_items,
        )
    }

    /// Build a vendor scrollbar bound to the tracked uniform-list handle.
    fn builtin_uniform_list_scrollbar(
        &self,
        handle: &UniformListScrollHandle,
        total_items: usize,
        fallback_visible_items: usize,
    ) -> AnyElement {
        if Self::builtin_uniform_list_scrollbar_metrics(handle, total_items, fallback_visible_items)
            .is_none()
        {
            return div().into_any_element();
        }
        gpui_component::scroll::Scrollbar::vertical(handle)
            .scrollbar_show(gpui_component::scroll::ScrollbarShow::Always)
            .into_any_element()
    }
}

#[cfg(test)]
mod builtin_scroll_helpers_contract {
    const SOURCE: &str = include_str!("common.rs");

    #[test]
    fn builtin_helpers_include_wheel_delta_conversion() {
        assert!(
            SOURCE.contains("fn builtin_scroll_wheel_steps"),
            "builtin scroll helpers should convert raw wheel deltas into row steps"
        );
        assert!(
            SOURCE.contains("gpui::ScrollDelta::Pixels(point)"),
            "builtin wheel helpers should normalize pixel deltas for trackpads"
        );
        assert!(
            SOURCE.contains("fn builtin_scroll_target_from_wheel"),
            "builtin scroll helpers should expose a reusable wheel-to-selection target"
        );
        assert!(
            SOURCE.contains("fn builtin_reanchor_selection_from_scroll_handle"),
            "builtin scroll helpers should support ScrollHandle-based selection reanchor"
        );
    }

    #[test]
    fn builtin_uniform_list_scrollbar_uses_vendor_handle_path() {
        assert!(
            SOURCE.contains("gpui_component::scroll::Scrollbar::vertical(handle)"),
            "builtin uniform list scrollbars should be the GPUI vendor scrollbar bound to the real handle"
        );
        assert!(
            SOURCE.contains(".scrollbar_show(gpui_component::scroll::ScrollbarShow::Always)"),
            "builtin uniform list scrollbars should stay visible for launcher-family surfaces"
        );
    }
}

/// Corpus-wide consistency audit for every built-in browser renderer.
///
/// WHY (decision lock, 2026-07-11): the Tips browser shipped with two
/// consistency regressions the shared component contract exists to prevent —
/// a selectable list that never scrolled its keyboard selection into view,
/// and a footer that bypassed the persistent main-window footer. These are
/// architectural invariants of the builtin-browser family, not per-surface
/// styling choices, and no higher enforcement rung can currently express
/// them (renderers are `AnyElement` builders with no inspectable tree in
/// unit tests). The audit asserts the ABSENCE of the dangerous pattern per
/// file and enumerates the grandfathered offenders explicitly; both lists
/// are shrink-only.
#[cfg(test)]
mod builtin_browser_consistency_audit {
    /// Every builtin browser renderer, enumerated explicitly so a new file
    /// cannot join the corpus unaudited (`include!` chain in `mod.rs` and
    /// this table must move together).
    const BUILTIN_BROWSER_SOURCES: &[(&str, &str)] = &[
        ("actions.rs", include_str!("actions.rs")),
        (
            "agent_chat_history.rs",
            include_str!("agent_chat_history.rs"),
        ),
        ("ai_presets.rs", include_str!("ai_presets.rs")),
        ("app_launcher.rs", include_str!("app_launcher.rs")),
        ("browser_history.rs", include_str!("browser_history.rs")),
        ("browser_tabs.rs", include_str!("browser_tabs.rs")),
        ("clipboard.rs", include_str!("clipboard.rs")),
        ("clipboard_preview.rs", include_str!("clipboard_preview.rs")),
        (
            "current_app_commands.rs",
            include_str!("current_app_commands.rs"),
        ),
        ("design_gallery.rs", include_str!("design_gallery.rs")),
        ("design_picker.rs", include_str!("design_picker.rs")),
        ("dictation_history.rs", include_str!("dictation_history.rs")),
        ("emoji_picker.rs", include_str!("emoji_picker.rs")),
        ("favorites.rs", include_str!("favorites.rs")),
        ("file_search.rs", include_str!("file_search.rs")),
        ("flow_ux.rs", include_str!("flow_ux.rs")),
        ("footer_gallery.rs", include_str!("footer_gallery.rs")),
        ("kit_store.rs", include_str!("kit_store.rs")),
        ("migrate_v1.rs", include_str!("migrate_v1.rs")),
        ("non_list_states.rs", include_str!("non_list_states.rs")),
        ("notes_browse.rs", include_str!("notes_browse.rs")),
        (
            "permissions_wizard.rs",
            include_str!("permissions_wizard.rs"),
        ),
        ("process_manager.rs", include_str!("process_manager.rs")),
        ("profile_search.rs", include_str!("profile_search.rs")),
        ("script_templates.rs", include_str!("script_templates.rs")),
        ("sdk_reference.rs", include_str!("sdk_reference.rs")),
        ("settings.rs", include_str!("settings.rs")),
        ("theme_chooser.rs", include_str!("theme_chooser.rs")),
        ("tips.rs", include_str!("tips.rs")),
        ("window_actions.rs", include_str!("window_actions.rs")),
        ("window_switcher.rs", include_str!("window_switcher.rs")),
    ];

    /// Shrink-only. These files render selectable rows but still never move
    /// their scroll container when the selection moves (the Tips bug class).
    /// Fixing one means DELETING it here — never add a new entry; new
    /// browsers must scroll their selection into view from day one via a
    /// tracked `uniform_list` + `scroll_to_item` (see `window_switcher.rs`)
    /// or a `ListState`/`ScrollHandle` navigation scroll.
    const GRANDFATHERED_NON_SCROLLING_SELECTABLE_LISTS: &[&str] = &[
        "ai_presets.rs",
        "favorites.rs",
        "permissions_wizard.rs",
        "script_templates.rs",
        "sdk_reference.rs",
    ];

    fn renders_selectable_list(source: &str) -> bool {
        source.contains("ListItem::new") && source.contains(".selected(")
    }

    fn scrolls_selection(source: &str) -> bool {
        source.contains("scroll_to_item")
            || source.contains(".track_scroll(")
            || source.contains("_list_state")
    }

    #[test]
    fn selectable_builtin_lists_scroll_selection_into_view() {
        for (name, source) in BUILTIN_BROWSER_SOURCES {
            if !renders_selectable_list(source) {
                continue;
            }
            let grandfathered = GRANDFATHERED_NON_SCROLLING_SELECTABLE_LISTS.contains(name);
            let scrolls = scrolls_selection(source);
            if grandfathered {
                assert!(
                    !scrolls,
                    "{name} now scrolls its selection — delete it from \
                     GRANDFATHERED_NON_SCROLLING_SELECTABLE_LISTS (shrink-only)"
                );
                continue;
            }
            assert!(
                scrolls,
                "{name} renders a selectable list but never scrolls the selection into \
                 view. Use a tracked uniform_list + scroll_to_item on every selection \
                 move (keyboard, wheel, click) — see window_switcher.rs / tips.rs — \
                 instead of a free-scrolling div. Do NOT add this file to the \
                 grandfather list; it is shrink-only."
            );
        }
    }

    #[test]
    fn builtin_footers_route_through_the_persistent_main_window_footer() {
        for (name, source) in BUILTIN_BROWSER_SOURCES {
            if source.contains("render_simple_hint_strip(") {
                assert!(
                    source.contains("main_window_footer_slot("),
                    "{name} renders a GPUI hint strip without offering it to \
                     main_window_footer_slot. Builtin browsers must reuse the \
                     persistent native footer (native_footer_surface + \
                     FooterButtonConfig) and pass the hint strip only as its \
                     GPUI fallback — never render standalone footer chrome."
                );
            }
            // The gallery browser legitimately renders PromptFooter previews
            // as CONTENT; every other browser must not instantiate footer
            // chrome directly.
            if *name != "footer_gallery.rs" {
                assert!(
                    !source.contains("PromptFooter::new(") && !source.contains("HintStrip::new("),
                    "{name} builds footer chrome directly. Route through \
                     main_window_footer_slot(render_simple_hint_strip(...)) so the \
                     surface inherits the shared footer components and native footer."
                );
            }
        }
    }
}
