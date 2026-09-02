impl AgentChatView {
    /// Whether the rendered body is the setup card, including live-thread recovery.
    pub(crate) fn shows_setup_card(&self, cx: &App) -> bool {
        match &self.session {
            AgentChatSession::Setup(_) => true,
            AgentChatSession::Live(thread) => thread.read(cx).setup_state().is_some(),
        }
    }
    pub(crate) fn applied_theme_revision(&self) -> Option<u64> {
        self.rendered_theme_revision
    }
    /// Authority comes from live mutation owners, including children whose
    /// notifications have not yet reached this view. Reads never advance it.
    pub(crate) fn semantic_revision(&self, cx: &App) -> u64 {
        let mut revision = self.semantic_revision;
        let active_thread = match &self.session {
            AgentChatSession::Live(thread) => Some(thread),
            AgentChatSession::Setup(_) => None,
        };
        for thread in self.retained_threads.iter().chain(active_thread) {
            revision = revision.strict_add(thread.read(cx).semantic_revision());
        }
        if let Some(transcript) = &self.transcript {
            revision = revision.strict_add(transcript.read(cx).interaction_revision());
        }
        if let Some(card) = &self.setup_card {
            revision = revision.strict_add(card.read(cx).interaction_revision());
        }
        revision
    }

    fn advance_semantic_revision(&mut self) {
        self.semantic_revision = self.semantic_revision.strict_add(1);
    }

    fn notify_semantic_change(&mut self, cx: &mut Context<Self>) {
        let token = self.semantic_state_token(cx);
        if self.last_notified_semantic_state != Some(token) {
            self.last_notified_semantic_state = Some(token);
            self.advance_semantic_revision();
        }
        cx.notify();
    }
    pub(crate) fn semantic_token(&self, cx: &App) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.semantic_revision(cx).hash(&mut hash);
        self.semantic_state_token(cx).hash(&mut hash);
        hash.finish()
    }

    fn semantic_state_token(&self, cx: &App) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.thread()
            .map(|thread| thread.read(cx).semantic_token())
            .hash(&mut hash);
        self.ui_variant.state_id().hash(&mut hash);
        self.command_status.hash(&mut hash);
        self.composer_spine.selected_index.hash(&mut hash);
        self.composer_spine.visible_start.hash(&mut hash);
        self.composer_spine.project_scope_cwd.hash(&mut hash);
        self.composer_spine
            .project_scope_cwd_recents
            .hash(&mut hash);
        for command in &self.cached_slash_commands {
            command.name.hash(&mut hash);
            command.description.hash(&mut hash);
            std::mem::discriminant(&command.source).hash(&mut hash);
        }
        self.permission_index.hash(&mut hash);
        self.permission_options_open.hash(&mut hash);
        self.attach_menu_open.hash(&mut hash);
        self.message_queue_expanded.hash(&mut hash);
        self.search_state.hash(&mut hash);
        self.expanded_composer.hash(&mut hash);
        self.scope_input.hash(&mut hash);
        self.scope_visible.hash(&mut hash);
        self.scope_focused.hash(&mut hash);
        self.context_capture_pending.hash(&mut hash);
        self.focused_text_selected_variation.hash(&mut hash);
        self.focused_text_editing_variation.hash(&mut hash);
        self.focused_text_variation_generation.hash(&mut hash);
        self.focused_text_variation_history_index.hash(&mut hash);
        for variation in &self.focused_text_variations {
            variation.text.hash(&mut hash);
            std::mem::discriminant(&variation.status).hash(&mut hash);
        }
        if let AgentChatSession::Setup(setup) = &self.session {
            setup.reason_code.hash(&mut hash);
            setup.title.hash(&mut hash);
            setup.body.hash(&mut hash);
            setup
                .selected_agent
                .as_ref()
                .map(|agent| &agent.id)
                .hash(&mut hash);
        }
        if let Some(session) = &self.composer_picker_session {
            session.trigger.label().hash(&mut hash);
            session.selected_index.hash(&mut hash);
            session.query.hash(&mut hash);
            session.trigger_range.hash(&mut hash);
            session.visible_start.hash(&mut hash);
            for item in &session.items {
                item.id.hash(&mut hash);
                item.label.hash(&mut hash);
            }
        }
        if let Some(menu) = &self.history_menu {
            menu.selected_index.hash(&mut hash);
            menu.query.hash(&mut hash);
            for hit in &menu.hits {
                hit.entry.session_id.hash(&mut hash);
                hit.entry.title.hash(&mut hash);
                hit.entry.custom_title.hash(&mut hash);
                hit.entry.preview.hash(&mut hash);
                hit.entry.message_count.hash(&mut hash);
            }
        }
        hash.finish()
    }

    /// Redacted per-part identity for automation: kind/label/source, plus the
    /// focused-target identity so probes can prove the RIGHT chip is staged
    /// (e.g. the notes→main handoff's note part). Never part content.
    fn context_part_identity_snapshot(
        item: &crate::ai::staged_context::StagedContextItem,
    ) -> crate::protocol::AgentChatContextPartSnapshot {
        use crate::ai::message_parts::AiContextPart;
        let part = &item.part;
        let kind = match part {
            AiContextPart::ResourceUri { .. } => "resourceUri",
            AiContextPart::FilePath { .. } => "filePath",
            AiContextPart::SkillFile { .. } => "skillFile",
            AiContextPart::FocusedTarget { .. } => "focusedTarget",
            AiContextPart::AmbientContext { .. } => "ambientContext",
            AiContextPart::TextBlock { .. } => "textBlock",
        };
        let (target_kind, target_source, target_semantic_id) = match part {
            AiContextPart::FocusedTarget { target, .. } => (
                Some(target.kind.clone()),
                Some(target.source.clone()),
                Some(target.semantic_id.clone()),
            ),
            _ => (None, None, None),
        };
        let failure = item.state.failure();
        let source_kind = format!("{:?}", item.source_kind());
        crate::protocol::AgentChatContextPartSnapshot {
            id: item.id.0.clone(),
            kind: kind.to_string(),
            label: item.display_label(),
            source: source_kind,
            source_fingerprint: crate::ai::reliability::redacted_fingerprint(part.source()),
            provenance: item.provenance.as_str().to_string(),
            role: item.role.as_str().to_string(),
            state: item.state.as_str().to_string(),
            lifetime: item.lifetime.as_str().to_string(),
            removable: item.can_remove(),
            generation: item.generation,
            failure_code: failure.map(|record| format!("{:?}", record.failure.code)),
            diagnostic_fingerprint: failure
                .and_then(|record| record.failure.diagnostic.as_ref())
                .map(|diagnostic| diagnostic.fingerprint.0.clone()),
            target_kind,
            target_source,
            target_semantic_id: target_semantic_id.map(|semantic_id| {
                format!(
                    "fingerprint:{}",
                    crate::ai::reliability::redacted_fingerprint(&semantic_id)
                )
            }),
        }
    }
}
