// App navigation methods - extracted from app_impl.rs
// This file is included via include!() macro in main.rs
// Contains: move_selection_up, move_selection_down, scroll_to_selected, etc.

impl ScriptListApp {
    fn move_selection_up(&mut self, cx: &mut Context<Self>) {
        // Get grouped results to check for section headers (cached)
        let (grouped_items, _) = self.get_grouped_results_cached();
        // Clone to avoid borrow issues with self mutation below
        let grouped_items = grouped_items.clone();

        // Find the first selectable (non-header) item index
        let first_selectable = grouped_items
            .iter()
            .position(|item| matches!(item, GroupedListItem::Item(_)));

        // If already at or before first selectable, can't go further up
        if let Some(first) = first_selectable {
            if self.selected_index <= first {
                // Already at the first selectable item, stay here
                return;
            }
        }

        if self.selected_index > 0 {
            let mut new_index = self.selected_index - 1;

            // Skip section headers when moving up
            while new_index > 0 {
                if let Some(GroupedListItem::SectionHeader(_)) = grouped_items.get(new_index) {
                    new_index -= 1;
                } else {
                    break;
                }
            }

            // Make sure we didn't land on a section header at index 0
            if let Some(GroupedListItem::SectionHeader(_)) = grouped_items.get(new_index) {
                // Stay at current position if we can't find a valid item
                return;
            }

            self.selected_index = new_index;
            self.scroll_to_selected_if_needed("keyboard_up");
            self.trigger_scroll_activity(cx);
            cx.notify();
        }
    }

    fn move_selection_down(&mut self, cx: &mut Context<Self>) {
        // Get grouped results to check for section headers (cached)
        let (grouped_items, _) = self.get_grouped_results_cached();
        // Clone to avoid borrow issues with self mutation below
        let grouped_items = grouped_items.clone();

        let item_count = grouped_items.len();

        // Find the last selectable (non-header) item index
        let last_selectable = grouped_items
            .iter()
            .rposition(|item| matches!(item, GroupedListItem::Item(_)));

        // If already at or after last selectable, can't go further down
        if let Some(last) = last_selectable {
            if self.selected_index >= last {
                // Already at the last selectable item, stay here
                return;
            }
        }

        if self.selected_index < item_count.saturating_sub(1) {
            let mut new_index = self.selected_index + 1;

            // Skip section headers when moving down
            while new_index < item_count.saturating_sub(1) {
                if let Some(GroupedListItem::SectionHeader(_)) = grouped_items.get(new_index) {
                    new_index += 1;
                } else {
                    break;
                }
            }

            // Make sure we didn't land on a section header at the end
            if let Some(GroupedListItem::SectionHeader(_)) = grouped_items.get(new_index) {
                // Stay at current position if we can't find a valid item
                return;
            }

            self.selected_index = new_index;
            self.scroll_to_selected_if_needed("keyboard_down");
            self.trigger_scroll_activity(cx);
            cx.notify();
        }
    }

    /// Scroll stabilization helper: only call scroll_to_reveal_item if we haven't already scrolled to this index.
    /// This prevents scroll jitter from redundant scroll calls.
    ///
    /// NOTE: Uses main_list_state (ListState) for the variable-height list() component,
    /// not the legacy list_scroll_handle (UniformListScrollHandle).
    fn scroll_to_selected_if_needed(&mut self, _reason: &str) {
        let target = self.selected_index;

        // Check if we've already scrolled to this index
        if self.last_scrolled_index == Some(target) {
            return;
        }

        // Use perf guard for scroll timing
        let _scroll_perf = crate::perf::ScrollPerfGuard::new();

        // Perform the scroll using ListState for variable-height list
        // This scrolls the actual list() component used in render_script_list
        self.main_list_state.scroll_to_reveal_item(target);
        self.last_scrolled_index = Some(target);
    }

    /// Trigger scroll activity - shows the scrollbar and schedules fade-out
    ///
    /// This should be called whenever scroll-related activity occurs:
    /// - Keyboard up/down navigation
    /// - scroll_to_item calls
    /// - Mouse wheel scrolling (if tracked)
    fn trigger_scroll_activity(&mut self, cx: &mut Context<Self>) {
        self.is_scrolling = true;
        self.last_scroll_time = Some(std::time::Instant::now());

        // Schedule fade-out after 1000ms of inactivity
        cx.spawn(async move |this, cx| {
            Timer::after(std::time::Duration::from_millis(1000)).await;
            let _ = cx.update(|cx| {
                this.update(cx, |app, cx| {
                    // Only hide if no new scroll activity occurred
                    if let Some(last_time) = app.last_scroll_time {
                        if last_time.elapsed() >= std::time::Duration::from_millis(1000) {
                            app.is_scrolling = false;
                            cx.notify();
                        }
                    }
                })
            });
        })
        .detach();

        cx.notify();
    }

    /// Apply a coalesced navigation delta in the given direction
    fn apply_nav_delta(&mut self, dir: NavDirection, delta: i32, cx: &mut Context<Self>) {
        let signed = match dir {
            NavDirection::Up => -delta,
            NavDirection::Down => delta,
        };
        self.move_selection_by(signed, cx);
    }

    /// Move selection by a signed delta (positive = down, negative = up)
    /// Used by NavCoalescer for batched movements
    fn move_selection_by(&mut self, delta: i32, cx: &mut Context<Self>) {
        let len = self.get_filtered_results_cached().len();
        if len == 0 {
            self.selected_index = 0;
            return;
        }
        let new_index = (self.selected_index as i32 + delta).clamp(0, (len as i32) - 1) as usize;
        if new_index != self.selected_index {
            self.selected_index = new_index;
            self.scroll_to_selected_if_needed("coalesced_nav");
            self.trigger_scroll_activity(cx);
            cx.notify();
        }
    }

    /// Ensure the navigation flush task is running. Spawns a background task
    /// that periodically flushes pending navigation deltas.
    fn ensure_nav_flush_task(&mut self, cx: &mut Context<Self>) {
        if self.nav_coalescer.flush_task_running {
            return;
        }
        self.nav_coalescer.flush_task_running = true;
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(NavCoalescer::WINDOW).await;
                let keep_running = cx
                    .update(|cx| {
                        this.update(cx, |this, cx| {
                            // Flush any pending navigation delta
                            if let Some((dir, delta)) = this.nav_coalescer.flush_pending() {
                                this.apply_nav_delta(dir, delta, cx);
                            }
                            // Check if we should keep running
                            let now = std::time::Instant::now();
                            let recently_active = now.duration_since(this.nav_coalescer.last_event)
                                < NavCoalescer::WINDOW;
                            if !recently_active && this.nav_coalescer.pending_delta == 0 {
                                // No recent activity and no pending delta - stop the task
                                this.nav_coalescer.flush_task_running = false;
                                this.nav_coalescer.reset();
                                false
                            } else {
                                true
                            }
                        })
                    })
                    .unwrap_or(Ok(false))
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        })
        .detach();
    }
}
