impl AgentChatTranscript {
    pub(crate) fn interaction_revision(&self) -> u64 {
        self.interaction_revision.get()
    }

    fn advance_interaction_revision(&self) {
        self.interaction_revision
            .set(self.interaction_revision.get().strict_add(1));
    }

    pub fn toggle_collapsed(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.collapsed_ids.contains(&id) {
            self.collapsed_ids.remove(&id);
        } else {
            self.collapsed_ids.insert(id);
        }
        self.advance_interaction_revision();
        cx.notify();
    }

    fn expand_heavy_markdown(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.expanded_heavy_markdown_ids.insert(id) {
            self.advance_interaction_revision();
            self.reconcile_message_views(cx);
            cx.notify();
        }
    }

    /// Whether a collapsible message renders expanded by default, before any
    /// user toggle. Edit diffs and failed tools surface their body immediately;
    /// everything else starts collapsed.
    fn default_expanded(msg: &AgentChatThreadMessage) -> bool {
        msg.tool_meta
            .as_ref()
            .is_some_and(|meta| meta.diff.is_some() || meta.is_error)
    }

    /// `collapsed_ids` records user toggles, so the effective state is the
    /// default expansion XOR a recorded toggle.
    fn is_collapsed_for(msg: &AgentChatThreadMessage, toggled: &HashSet<u64>) -> bool {
        let is_collapsible = matches!(
            msg.role,
            AgentChatThreadMessageRole::Thought | AgentChatThreadMessageRole::Tool
        );
        if !is_collapsible {
            return false;
        }
        let expanded = Self::default_expanded(msg) ^ toggled.contains(&msg.id);
        !expanded
    }

    pub fn clear_collapsed_ids(&mut self, cx: &mut Context<Self>) {
        if self.collapsed_ids.is_empty() {
            return;
        }
        self.collapsed_ids.clear();
        self.advance_interaction_revision();
        cx.notify();
    }

    pub fn expand_ids(&mut self, ids: Vec<u64>, cx: &mut Context<Self>) {
        let previous_len = self.collapsed_ids.len();
        self.collapsed_ids.extend(ids);
        if self.collapsed_ids.len() == previous_len {
            return;
        }
        self.advance_interaction_revision();
        cx.notify();
    }

    pub fn scroll_to_reveal_item(&self, index: usize) {
        // Revealing a specific item is a manual navigation: stop following the
        // tail so a later stream chunk does not snap the reader away.
        self.list_state.set_follow_tail(false);
        self.list_state.scroll_to_reveal_item(index);
        self.advance_interaction_revision();
    }

    pub fn logical_scroll_top(&self) -> ListOffset {
        self.list_state.logical_scroll_top()
    }

    pub fn scroll_to(&self, offset: ListOffset) {
        self.list_state.set_follow_tail(false);
        self.list_state.scroll_to(offset);
        self.advance_interaction_revision();
    }

    /// Explicitly resume tail-following. `set_follow_tail(true)` snaps the list
    /// to the end, so the next paint shows the newest message.
    pub fn scroll_to_end(&self) {
        if !self.list_state.is_following_tail() {
            self.advance_interaction_revision();
        }
        self.list_state.set_follow_tail(true);
    }
}
