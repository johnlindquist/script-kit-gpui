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
        self.hide_mouse_cursor(cx);
    }

    #[inline]
    fn set_selected_index(&mut self, ix: usize, reason: &str, cx: &mut Context<Self>) {
        if ix == self.selected_index {
            return;
        }

        self.reset_main_list_boundary_affordance(
            crate::scrolling::boundary_affordance::SettleReason::Reset,
        );
        self.clear_menu_syntax_filter_accept_hint();
        self.selected_index = ix;
        self.maybe_expand_root_file_source_chip_page(cx);
        self.rebuild_main_window_preflight_if_needed();
        self.scroll_to_selected_if_needed(reason);
        self.trigger_scroll_activity(cx);
        cx.notify();
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

        let snapshot = self.main_menu_selection_snapshot();
        self.root_search.root_file_source_chip_visible_limit = current_limit
            .saturating_add(crate::file_search::ROOT_FILE_SOURCE_CHIP_PAGE_SIZE)
            .min(max_visible);
        self.root_search.root_file_frame = None;
        self.invalidate_grouped_cache();
        self.get_grouped_results_cached();
        self.sync_list_state();
        self.restore_main_menu_selection_from_snapshot(snapshot);
        self.validate_selection_bounds(cx);
        self.reveal_main_list_selection_above_footer("root_file_source_chip_page_expand");
        self.schedule_main_list_selection_reveal_above_footer(
            "root_file_source_chip_page_expand_deferred",
            cx,
        );
        self.invalidate_main_window_preflight();
        cx.notify();
        true
    }

    fn move_selection_up(&mut self, cx: &mut Context<Self>) {
        self.enter_keyboard_mode(cx);

        // Empty `@source:` colon mode renders unarmed: Up is not a choose
        // gesture, so it neither arms nor moves the (invisible) selection.
        if self.spine_empty_subsearch_selection_suppressed() {
            return;
        }
        self.mark_main_menu_selection_user_moved();

        let (target_index, reason) = {
            let (grouped_items, _) = self.get_grouped_results_cached();
            if grouped_items.is_empty() {
                return;
            }

            let clamped_index = self
                .selected_index
                .min(grouped_items.len().saturating_sub(1));
            let first_selectable = self.main_menu_result_caches.first_selectable_index();

            if let Some(first) = first_selectable {
                if clamped_index <= first {
                    (first, "keyboard_up_clamp")
                } else if clamped_index > 0 {
                    let mut new_index = clamped_index - 1;
                    while new_index > 0 {
                        if let Some(GroupedListItem::SectionHeader(..)) =
                            grouped_items.get(new_index)
                        {
                            new_index -= 1;
                        } else {
                            break;
                        }
                    }

                    if matches!(
                        grouped_items.get(new_index),
                        Some(GroupedListItem::SectionHeader(..))
                    ) {
                        (clamped_index, "keyboard_up_clamp")
                    } else {
                        (new_index, "keyboard_up")
                    }
                } else {
                    (clamped_index, "keyboard_up_clamp")
                }
            } else {
                (clamped_index, "keyboard_up_clamp")
            }
        };

        self.set_selected_index(target_index, reason, cx);
    }

    fn move_selection_down(&mut self, cx: &mut Context<Self>) {
        self.enter_keyboard_mode(cx);
        self.mark_main_menu_selection_user_moved();

        // Empty `@source:` colon mode renders unarmed: the first Down is the
        // explicit choose gesture and lands on the FIRST recent row (which
        // was visible but unselected), not the second.
        if self.spine_empty_subsearch_selection_suppressed() {
            self.arm_spine_empty_subsearch_selection();
            let _ = self.get_grouped_results_cached();
            if let Some(first) = self.main_menu_result_caches.first_selectable_index() {
                self.set_selected_index(first, "spine_empty_subsearch_arm", cx);
            }
            // set_selected_index early-exits when the index is unchanged;
            // arming alone changes what renders, so always repaint.
            cx.notify();
            return;
        }

        let (target_index, reason) = {
            let (grouped_items, _) = self.get_grouped_results_cached();
            if grouped_items.is_empty() {
                return;
            }

            let clamped_index = self
                .selected_index
                .min(grouped_items.len().saturating_sub(1));
            let item_count = grouped_items.len();
            let last_selectable = self.main_menu_result_caches.last_selectable_index();

            if let Some(last) = last_selectable {
                if clamped_index >= last {
                    (last, "keyboard_down_clamp")
                } else if clamped_index < item_count.saturating_sub(1) {
                    let mut new_index = clamped_index + 1;
                    while new_index < item_count.saturating_sub(1) {
                        if let Some(GroupedListItem::SectionHeader(..)) =
                            grouped_items.get(new_index)
                        {
                            new_index += 1;
                        } else {
                            break;
                        }
                    }

                    if matches!(
                        grouped_items.get(new_index),
                        Some(GroupedListItem::SectionHeader(..))
                    ) {
                        (clamped_index, "keyboard_down_clamp")
                    } else {
                        (new_index, "keyboard_down")
                    }
                } else {
                    (clamped_index, "keyboard_down_clamp")
                }
            } else {
                (clamped_index, "keyboard_down_clamp")
            }
        };

        self.set_selected_index(target_index, reason, cx);
    }

    /// Jump to the first selectable (non-header) item in the list
    fn move_selection_to_first(&mut self, cx: &mut Context<Self>) {
        self.enter_keyboard_mode(cx);
        self.mark_main_menu_selection_user_moved();

        let (target_index, reason) = {
            let (grouped_items, _) = self.get_grouped_results_cached();
            if grouped_items.is_empty() {
                return;
            }

            let clamped_index = self
                .selected_index
                .min(grouped_items.len().saturating_sub(1));
            let first_selectable = self.main_menu_result_caches.first_selectable_index();

            if let Some(first) = first_selectable {
                (first, "jump_first")
            } else {
                (clamped_index, "jump_first_clamp")
            }
        };

        self.set_selected_index(target_index, reason, cx);
    }

    /// Move selection up by approximately one page (~10 selectable items)
    /// Skips section headers and clamps to the first selectable item
    fn move_selection_page_up(&mut self, cx: &mut Context<Self>) {
        self.enter_keyboard_mode(cx);
        self.mark_main_menu_selection_user_moved();

        let (target_index, reason) = {
            let (grouped_items, _) = self.get_grouped_results_cached();
            if grouped_items.is_empty() {
                return;
            }

            let clamped_index = self
                .selected_index
                .min(grouped_items.len().saturating_sub(1));
            let first_selectable = self.main_menu_result_caches.first_selectable_index();

            if let Some(first) = first_selectable {
                if clamped_index <= first {
                    (clamped_index, "page_up_clamp")
                } else {
                    const PAGE_SIZE: usize = 10;
                    let mut remaining = PAGE_SIZE;
                    let mut target = clamped_index;
                    for i in (first..clamped_index).rev() {
                        if matches!(grouped_items.get(i), Some(GroupedListItem::Item(_))) {
                            target = i;
                            remaining -= 1;
                            if remaining == 0 {
                                break;
                            }
                        }
                    }

                    if target != clamped_index {
                        (target, "page_up")
                    } else {
                        (clamped_index, "page_up_clamp")
                    }
                }
            } else {
                (clamped_index, "page_up_clamp")
            }
        };

        self.set_selected_index(target_index, reason, cx);
    }

    /// Move selection down by approximately one page (~10 selectable items)
    /// Skips section headers and clamps to the last selectable item
    fn move_selection_page_down(&mut self, cx: &mut Context<Self>) {
        self.enter_keyboard_mode(cx);
        self.mark_main_menu_selection_user_moved();

        let (target_index, reason) = {
            let (grouped_items, _) = self.get_grouped_results_cached();
            if grouped_items.is_empty() {
                return;
            }

            let clamped_index = self
                .selected_index
                .min(grouped_items.len().saturating_sub(1));
            let last_selectable = self.main_menu_result_caches.last_selectable_index();

            if let Some(last) = last_selectable {
                if clamped_index >= last {
                    (clamped_index, "page_down_clamp")
                } else {
                    const PAGE_SIZE: usize = 10;
                    let target = page_down_target_index(
                        &grouped_items,
                        clamped_index,
                        PAGE_SIZE,
                        last_selectable,
                    );
                    if target != clamped_index {
                        (target, "page_down")
                    } else {
                        (clamped_index, "page_down_clamp")
                    }
                }
            } else {
                (clamped_index, "page_down_clamp")
            }
        };

        self.set_selected_index(target_index, reason, cx);
    }

    /// Jump to the last selectable (non-header) item in the list
    fn move_selection_to_last(&mut self, cx: &mut Context<Self>) {
        self.enter_keyboard_mode(cx);
        self.mark_main_menu_selection_user_moved();

        let (target_index, reason) = {
            let (grouped_items, _) = self.get_grouped_results_cached();
            if grouped_items.is_empty() {
                return;
            }

            let clamped_index = self
                .selected_index
                .min(grouped_items.len().saturating_sub(1));
            let last_selectable = self.main_menu_result_caches.last_selectable_index();

            if let Some(last) = last_selectable {
                (last, "jump_last")
            } else {
                (clamped_index, "jump_last_clamp")
            }
        };

        self.set_selected_index(target_index, reason, cx);
    }
}
