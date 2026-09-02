impl AgentChatView {
    fn set_history_popup_query(&mut self, query: String, cx: &mut Context<Self>) {
        let hits = super::history::search_history(&query, HISTORY_POPUP_SEARCH_LIMIT);
        self.history_closed_at = None;
        self.history_menu = Some(AgentChatHistoryMenuState {
            selected_index: 0,
            query,
            hits,
        });
        self.sync_history_popup_window_from_cached_parent(cx);
        self.notify_semantic_change(cx);
    }

    fn navigate_history_popup_selection(&mut self, delta: i32, cx: &mut Context<Self>) {
        let Some(menu) = self.history_menu.as_mut() else {
            return;
        };
        if menu.hits.is_empty() {
            return;
        }

        let len = menu.hits.len();
        let current = menu.selected_index;
        menu.selected_index = if delta < 0 {
            current.saturating_sub((-delta) as usize)
        } else {
            (current + delta as usize).min(len.saturating_sub(1))
        };
        self.history_closed_at = None;
        self.sync_history_popup_window_from_cached_parent(cx);
        self.notify_semantic_change(cx);
    }

    fn jump_history_popup_selection(&mut self, end: bool, cx: &mut Context<Self>) {
        let Some(menu) = self.history_menu.as_mut() else {
            return;
        };
        if menu.hits.is_empty() {
            return;
        }

        menu.selected_index = if end {
            menu.hits.len().saturating_sub(1)
        } else {
            0
        };
        self.history_closed_at = None;
        self.sync_history_popup_window_from_cached_parent(cx);
        self.notify_semantic_change(cx);
    }

    fn page_history_popup_selection(&mut self, delta: i32, cx: &mut Context<Self>) {
        let Some(menu) = self.history_menu.as_mut() else {
            return;
        };
        if menu.hits.is_empty() {
            return;
        }

        let len = menu.hits.len();
        menu.selected_index = if delta < 0 {
            menu.selected_index.saturating_sub(HISTORY_POPUP_PAGE_JUMP)
        } else {
            (menu.selected_index + HISTORY_POPUP_PAGE_JUMP).min(len.saturating_sub(1))
        };
        self.history_closed_at = None;
        self.sync_history_popup_window_from_cached_parent(cx);
        self.notify_semantic_change(cx);
    }

    fn execute_history_popup_selection(
        &mut self,
        modifiers: &gpui::Modifiers,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self
            .history_menu
            .as_ref()
            .and_then(|menu| menu.hits.get(menu.selected_index))
            .map(|hit| hit.entry.clone())
        else {
            return;
        };

        self.close_history_popup_for_owner_transition("committed_selection", true, cx);
        self.history_closed_at = None;

        if modifiers.platform {
            self.select_history_from_popup(&entry, cx);
            return;
        }

        let mode = if modifiers.shift {
            super::history_attachment::AgentChatHistoryAttachMode::Transcript
        } else {
            super::history_attachment::AgentChatHistoryAttachMode::Summary
        };

        if let Err(error) = self.attach_history_session(&entry.session_id, mode, cx) {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_history_popup_attach_failed",
                session_id = %entry.session_id,
                mode = ?mode,
                error = %error,
            );
        }
        self.notify_semantic_change(cx);
    }
}
