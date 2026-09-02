impl ScriptListApp {
    #[inline]
    fn enter_keyboard_mode(&mut self, cx: &mut Context<Self>) {
        let mut policy = crate::scrolling::list_interaction::ListPointerPolicy {
            hovered_index: self.hovered_index,
            suppress_hover_until_pointer_move: self.list_suppress_hover_until_pointer_move,
        };
        policy.enter_keyboard_mode();
        self.input_mode = InputMode::Keyboard;
        self.last_list_interaction_source =
            crate::scrolling::list_interaction::ListViewportInputSource::Keyboard;
        self.list_suppress_hover_until_pointer_move = policy.suppress_hover_until_pointer_move;
        self.hovered_index = policy.hovered_index;
        if self.main_services.is_production() {
            self.hide_mouse_cursor(cx);
        }
    }

    #[inline]
    fn set_selected_index(&mut self, ix: usize, reason: &str, cx: &mut Context<Self>) {
        if !self.select_main_menu_row(ix, MainMenuSelectionOrigin::Keyboard, cx) {
            return;
        }
        self.reset_main_list_boundary_affordance(
            crate::scrolling::boundary_affordance::SettleReason::Reset,
        );
        self.maybe_expand_root_file_source_chip_page(cx);
        self.rebuild_main_window_preflight_if_needed();
        self.scroll_to_selected_if_needed(reason);
        self.trigger_scroll_activity(cx);
    }

    fn maybe_expand_root_file_source_chip_page(&mut self, cx: &mut Context<Self>) -> bool {
        const PRELOAD_THRESHOLD: usize = 3;

        if !matches!(self.current_view, AppView::ScriptList) {
            return false;
        }

        let raw_filter_text = self.computed_filter_text.clone();
        let Some((includes_files, advanced_predicate_active)) = self
            .menu_syntax_mode
            .advanced_query_for(&raw_filter_text)
            .map(|advanced_query| {
                (
                    advanced_query
                        .source_filters
                        .includes(crate::menu_syntax::RootUnifiedSourceFilter::Files),
                    advanced_query.has_predicates(),
                )
            })
        else {
            return false;
        };
        if !includes_files {
            return false;
        }

        let stripped_query =
            crate::menu_syntax::free_text_for_search(&self.menu_syntax_mode, &raw_filter_text)
                .to_string();
        let current_limit = self.root_file_source_chip_visible_limit_for(
            &raw_filter_text,
            stripped_query.as_str(),
            advanced_predicate_active,
            self.root_search.root_file_search_mode,
        );
        let max_visible = match self.root_search.root_file_search_mode {
            Some(crate::file_search::RootFileSectionMode::DirectoryBrowse) => {
                crate::file_search::ROOT_FILE_BROWSE_SOURCE_LIMIT
            }
            Some(crate::file_search::RootFileSectionMode::GlobalQuery) => {
                crate::file_search::ROOT_FILE_SOURCE_LIMIT
            }
            None => crate::file_search::ROOT_FILE_RECENT_SEED_LIMIT,
        };
        if current_limit >= max_visible {
            return false;
        }

        let (grouped_items, flat_results) = self.get_grouped_results_cached();
        let mut visible_file_rows = 0usize;
        let mut selected_file_rank = None;
        for (grouped_index, grouped_item) in grouped_items.iter().enumerate() {
            let GroupedListItem::Item(flat_index) = grouped_item else {
                continue;
            };
            let Some(result) = flat_results.get(*flat_index) else {
                continue;
            };
            if result.root_unified_source()
                != Some(crate::menu_syntax::RootUnifiedSourceFilter::Files)
            {
                continue;
            }
            if grouped_index == self.selected_index {
                selected_file_rank = Some(visible_file_rows);
            }
            visible_file_rows += 1;
        }

        let Some(selected_file_rank) = selected_file_rank else {
            return false;
        };
        if visible_file_rows < current_limit {
            return false;
        }
        if selected_file_rank + 1 + PRELOAD_THRESHOLD < visible_file_rows {
            return false;
        }

        self.commit_main_menu_results_refresh(
            "root_file_source_chip_page_expand",
            None,
            cx,
            |app, _cx| {
                app.root_search.root_file_source_chip_visible_limit = current_limit
                    .saturating_add(crate::file_search::ROOT_FILE_SOURCE_CHIP_PAGE_SIZE)
                    .min(max_visible);
                app.root_search.root_file_frame = None;
                true
            },
        )
    }

    fn move_selection_up(&mut self, cx: &mut Context<Self>) {
        self.move_selection_by(-1, cx);
    }

    fn move_selection_down(&mut self, cx: &mut Context<Self>) {
        self.move_selection_by(1, cx);
    }

    fn move_selection_to_first(&mut self, cx: &mut Context<Self>) {
        self.flush_pending_main_menu_query(cx);
        self.enter_keyboard_mode(cx);
        if let Some(first) = self.main_menu_result_caches.first_selectable_index() {
            self.set_selected_index(first, "jump_first", cx);
        }
    }

    fn move_selection_page_up(&mut self, cx: &mut Context<Self>) {
        self.move_selection_by(-10, cx);
    }

    fn move_selection_page_down(&mut self, cx: &mut Context<Self>) {
        self.move_selection_by(10, cx);
    }

    fn move_selection_to_last(&mut self, cx: &mut Context<Self>) {
        self.flush_pending_main_menu_query(cx);
        self.enter_keyboard_mode(cx);
        if self.spine_empty_subsearch_selection_suppressed() {
            return;
        }
        if let Some(last) = self.main_menu_result_caches.last_selectable_index() {
            self.set_selected_index(last, "jump_last", cx);
        }
    }
}
