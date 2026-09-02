impl AgentChatView {
    fn focused_text_variation_angles() -> [crate::ai::focused_text::FocusedTextPromptAngle; 3] {
        use crate::ai::focused_text::FocusedTextPromptAngle;
        [
            FocusedTextPromptAngle::Conservative,
            FocusedTextPromptAngle::Balanced,
            FocusedTextPromptAngle::Creative,
        ]
    }

    fn cancel_isolated_variation_processes(&mut self) {
        for flag in &self.focused_text_variation_cancel_flags {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.focused_text_variation_cancel_flags.clear();
    }

    fn reset_focused_text_variations_for_submit(&mut self) {
        self.cancel_isolated_variation_processes();
        self.focused_text_variation_tasks.clear();
        self.focused_text_variation_generation += 1;
        self.focused_text_selected_variation = None;
        self.focused_text_editing_variation = None;
        self.focused_text_variations = Self::focused_text_variation_angles()
            .iter()
            .copied()
            .map(FocusedTextVariationState::streaming)
            .collect();
    }

    fn clear_focused_text_variations(&mut self) {
        self.cancel_isolated_variation_processes();
        self.focused_text_variation_tasks.clear();
        self.focused_text_variations.clear();
        self.focused_text_variation_history.clear();
        self.focused_text_variation_history_index = None;
        self.focused_text_selected_variation = None;
        self.focused_text_editing_variation = None;
    }

    fn select_first_completed_focused_text_variation(&mut self) {
        if self.focused_text_selected_variation.is_some() {
            return;
        }
        let Some(index) = self.focused_text_variations.iter().position(|variation| {
            variation.status == FocusedTextVariationStatus::Complete
                && !variation.text.trim().is_empty()
        }) else {
            return;
        };
        self.focused_text_selected_variation = Some(index);
        tracing::info!(
            target: "script_kit::focused_text",
            event = "focused_text_variation_auto_selected",
            index,
            angle = self.focused_text_variations[index].angle.id(),
            text_len = self.focused_text_variations[index].text.chars().count(),
        );
    }

    fn mark_focused_text_variation_failed(
        &mut self,
        index: usize,
        error: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(variation) = self.focused_text_variations.get_mut(index) {
            variation.status = FocusedTextVariationStatus::Error;
            variation.error = Some(error);
        }
        self.notify_semantic_change(cx);
    }

    fn sync_balanced_focused_text_variation(
        &mut self,
        messages: &[AgentChatThreadMessage],
        status: AgentChatThreadStatus,
        cx: &mut Context<Self>,
    ) {
        if self.focused_text.is_none()
            || self.focused_text_variations.len() <= FOCUSED_TEXT_BALANCED_VARIATION_INDEX
        {
            return;
        }

        let latest_text = Self::latest_assistant_response_after_latest_user_in_messages(messages)
            .unwrap_or_default();
        {
            let editing_balanced =
                self.focused_text_editing_variation == Some(FOCUSED_TEXT_BALANCED_VARIATION_INDEX);
            let variation =
                &mut self.focused_text_variations[FOCUSED_TEXT_BALANCED_VARIATION_INDEX];
            if editing_balanced {
                variation.status = FocusedTextVariationStatus::Complete;
                variation.error = None;
            } else {
                if !latest_text.trim().is_empty() {
                    variation.text = latest_text;
                }
                variation.status = match status {
                    AgentChatThreadStatus::Streaming
                    | AgentChatThreadStatus::WaitingForPermission => {
                        FocusedTextVariationStatus::Streaming
                    }
                    AgentChatThreadStatus::Idle if !variation.text.trim().is_empty() => {
                        FocusedTextVariationStatus::Complete
                    }
                    AgentChatThreadStatus::Error => {
                        if variation.error.is_none() {
                            variation.error = Some("balanced_turn_failed".to_string());
                        }
                        FocusedTextVariationStatus::Error
                    }
                    AgentChatThreadStatus::Idle => FocusedTextVariationStatus::Idle,
                };
            }
        }

        self.select_first_completed_focused_text_variation();
        self.notify_semantic_change(cx);
    }

    fn apply_focused_text_variation_event(
        &mut self,
        index: usize,
        generation: u64,
        event: AgentChatEvent,
        cx: &mut Context<Self>,
    ) {
        if generation != self.focused_text_variation_generation {
            return;
        }
        if index >= self.focused_text_variations.len() {
            return;
        }

        if self.focused_text_editing_variation == Some(index) {
            if matches!(
                event,
                AgentChatEvent::TurnCompleted { .. }
                    | AgentChatEvent::TurnFailed { .. }
                    | AgentChatEvent::SetupRequired { .. }
            ) {
                if let Some(variation) = self.focused_text_variations.get_mut(index) {
                    variation.status = FocusedTextVariationStatus::Complete;
                    variation.error = None;
                }
                self.notify_semantic_change(cx);
            }
            return;
        }

        match event {
            AgentChatEvent::AgentMessageDelta(chunk) => {
                let variation = &mut self.focused_text_variations[index];
                variation.text.push_str(&chunk);
                variation.status = FocusedTextVariationStatus::Streaming;
                variation.error = None;
            }
            AgentChatEvent::TurnCompleted { .. } => {
                let variation = &mut self.focused_text_variations[index];
                if variation.status != FocusedTextVariationStatus::Error {
                    variation.status = FocusedTextVariationStatus::Complete;
                }
            }
            AgentChatEvent::TurnFailed { failure } => {
                let variation = &mut self.focused_text_variations[index];
                variation.status = FocusedTextVariationStatus::Error;
                variation.error = Some(failure.primary_message().to_string());
            }
            AgentChatEvent::SetupRequired {
                reason,
                auth_methods,
            } => {
                // S11: `setup_required:<reason>` was an internal marker string
                // rendered straight into the variation card. Classify it, then
                // show the same safe copy every other AI surface shows.
                let failure = crate::ai::reliability::setup_required_failure(
                    sk_protocol::ai_reliability::ProtocolComponent::Pi,
                    &reason,
                    &auth_methods,
                );
                let variation = &mut self.focused_text_variations[index];
                variation.status = FocusedTextVariationStatus::Error;
                variation.error = Some(failure.primary_message().to_string());
            }
            AgentChatEvent::UserMessageDelta(_)
            | AgentChatEvent::AgentThoughtDelta(_)
            | AgentChatEvent::ToolCallStarted { .. }
            | AgentChatEvent::ToolCallUpdated { .. }
            | AgentChatEvent::PlanUpdated { .. }
            | AgentChatEvent::AvailableCommandsUpdated { .. }
            | AgentChatEvent::ModeChanged { .. }
            | AgentChatEvent::UsageUpdated { .. }
            | AgentChatEvent::ModelsAvailable { .. }
            | AgentChatEvent::ForkPointsAvailable { .. }
            | AgentChatEvent::ForkCompleted { .. } => {}
        }

        self.select_first_completed_focused_text_variation();
        self.notify_semantic_change(cx);
    }

    fn spawn_focused_text_variation_task(
        &mut self,
        index: usize,
        rx: AgentChatEventRx,
        cx: &mut Context<Self>,
    ) {
        let view = cx.entity().downgrade();
        let generation = self.focused_text_variation_generation;
        let task = cx.spawn(async move |_this, cx| {
            while let Ok(event) = rx.recv().await {
                let terminal = matches!(
                    event,
                    AgentChatEvent::TurnCompleted { .. }
                        | AgentChatEvent::TurnFailed { .. }
                        | AgentChatEvent::SetupRequired { .. }
                );
                let view_ref = view.clone();
                cx.update(|cx| {
                    if let Some(entity) = view_ref.upgrade() {
                        entity.update(cx, |this, cx| {
                            this.apply_focused_text_variation_event(index, generation, event, cx);
                        });
                    }
                });
                if terminal {
                    break;
                }
            }
        });
        self.focused_text_variation_tasks.push(task);
    }

    /// Text to apply or paste back into the host app. Prefers the selected
    /// focused-text variation when variations exist; otherwise the latest
    /// assistant message from the thread.
    pub(crate) fn pastable_response_text(&self, cx: &App) -> Option<String> {
        if self.is_setup_mode() {
            return None;
        }
        let thread = self.live_thread().read(cx);
        self.selected_focused_text_output(thread)
    }

    fn selected_focused_text_output(&self, thread: &AgentChatThread) -> Option<String> {
        if self.focused_text.is_some() {
            if let Some(text) = self
                .focused_text_selected_variation
                .and_then(|index| self.focused_text_variations.get(index))
                .filter(|variation| !variation.text.trim().is_empty())
                .map(|variation| variation.text.clone())
            {
                return Some(text);
            }

            if let Some(text) = self
                .focused_text_variations
                .iter()
                .find(|variation| {
                    variation.status == FocusedTextVariationStatus::Complete
                        && !variation.text.trim().is_empty()
                })
                .map(|variation| variation.text.clone())
            {
                return Some(text);
            }

            return Self::latest_assistant_response_after_latest_user(thread);
        }

        Self::latest_assistant_response_text(thread)
    }

    pub(crate) fn focused_text_variation_snapshots(&self) -> Vec<FocusedTextVariationSnapshot> {
        self.focused_text_variations
            .iter()
            .enumerate()
            .map(|(index, variation)| {
                variation.snapshot(index, self.focused_text_selected_variation == Some(index))
            })
            .collect()
    }

    pub(crate) fn select_focused_text_variation(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        if index >= self.focused_text_variations.len() {
            return false;
        }
        if self.focused_text_selected_variation == Some(index) {
            return true;
        }
        self.focused_text_editing_variation = None;
        self.focused_text_selected_variation = Some(index);
        self.scope_focused = false;
        self.cursor_visible = true;
        tracing::info!(
            target: "script_kit::focused_text",
            event = "focused_text_variation_selected",
            index,
            angle = self.focused_text_variations[index].angle.id(),
            status = self.focused_text_variations[index].status.state_id(),
            text_len = self.focused_text_variations[index].text.chars().count(),
        );
        self.notify_semantic_change(cx);
        true
    }

    /// Tab advances through the variation cards and wraps back to the first,
    /// unlike Up/Down which saturate at the edges.
    fn cycle_focused_text_variation_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(next) = Self::next_variation_index_wrapping(
            self.focused_text_selected_variation,
            self.focused_text_variations.len(),
        ) else {
            return false;
        };
        self.select_focused_text_variation(next, cx)
    }

    fn next_variation_index_wrapping(current: Option<usize>, count: usize) -> Option<usize> {
        if count == 0 {
            return None;
        }
        Some(match current.filter(|index| *index < count) {
            Some(index) => (index + 1) % count,
            None => 0,
        })
    }

    /// Plain Tab cycles the focused-text variation cards so the user can pick
    /// the rewrite to paste. Shift+Tab stays reserved for the profile picker;
    /// the scope toggle keeps Tab while no variations exist (ask phase).
    fn handle_focused_text_variation_tab(
        &mut self,
        has_shift: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if has_shift
            || self.ui_variant != AgentChatUiVariant::FocusedTextMini
            || self.focused_text.is_none()
            || self.focused_text_variations.is_empty()
            || self.focused_text_editing_variation.is_some()
            || self.scope_focused
            || self.composer_picker_session.is_some()
        {
            return false;
        }
        self.cycle_focused_text_variation_selection(cx)
    }

    fn move_focused_text_variation_selection(
        &mut self,
        direction: i32,
        cx: &mut Context<Self>,
    ) -> bool {
        let count = self.focused_text_variations.len();
        if count == 0 {
            return false;
        }
        let current = self
            .focused_text_selected_variation
            .filter(|index| *index < count);
        let next = match (current, direction < 0) {
            (Some(index), true) => index.saturating_sub(1),
            (Some(index), false) => (index + 1).min(count.saturating_sub(1)),
            (None, true) => count.saturating_sub(1),
            (None, false) => 0,
        };
        self.select_focused_text_variation(next, cx)
    }

    fn save_focused_text_variation_history_slot(&mut self, index: usize) {
        if let Some(entry) = self.focused_text_variation_history.get_mut(index) {
            *entry = self.focused_text_variations.clone();
        }
    }

    fn navigate_focused_text_variation_history(
        &mut self,
        delta: i32,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.focused_text_variation_history.is_empty() {
            return false;
        }

        if self.focused_text_variation_history_index.is_none() && delta < 0 {
            let should_push =
                self.focused_text_variation_history.last() != Some(&self.focused_text_variations);
            if should_push {
                self.focused_text_variation_history
                    .push(self.focused_text_variations.clone());
            }
        }

        let len = self.focused_text_variation_history.len();
        let current = self
            .focused_text_variation_history_index
            .unwrap_or(len.saturating_sub(1));
        let target = current as i32 + delta;
        if target < 0 {
            return false;
        }
        let target = target as usize;

        if target >= len {
            if delta <= 0 {
                return false;
            }
            self.save_focused_text_variation_history_slot(current);
            self.focused_text_variation_history_index = None;
            self.focused_text_selected_variation = None;
            self.focused_text_editing_variation = None;
            self.select_first_completed_focused_text_variation();
            self.notify_semantic_change(cx);
            return true;
        }

        self.save_focused_text_variation_history_slot(current);
        self.focused_text_variations = self.focused_text_variation_history[target].clone();
        self.focused_text_variation_history_index = Some(target);
        self.focused_text_selected_variation = None;
        self.focused_text_editing_variation = None;
        self.select_first_completed_focused_text_variation();
        self.notify_semantic_change(cx);
        true
    }

    fn regenerate_focused_text_variations(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.focused_text_selected_variation else {
            return;
        };
        let source_text = self
            .focused_text_variations
            .get(index)
            .map(|variation| variation.text.clone())
            .unwrap_or_default();
        if source_text.trim().is_empty() {
            return;
        }

        if !self.focused_text_variations.is_empty() {
            self.focused_text_variation_history
                .push(self.focused_text_variations.clone());
            self.focused_text_variation_history_index = None;
        }

        let semantics = {
            let thread = self.live_thread().read(cx);
            self.focused_text_enter_semantics_for_thread(thread)
        };

        tracing::info!(
            target: "script_kit::focused_text",
            event = "focused_text_variations_regenerated",
            source_index = index,
            source_text_len = source_text.chars().count(),
            history_len = self.focused_text_variation_history.len(),
        );

        if let Err(error) = self.submit_focused_text_turn(semantics, cx, Some(source_text)) {
            tracing::warn!(
                target: "script_kit::focused_text",
                event = "focused_text_regenerate_failed",
                error = %error,
            );
        }
    }
}
