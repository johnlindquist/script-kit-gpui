impl ScriptListApp {
    fn collect_flow_desk_elements(
        &self,
        filter: &str,
        selected_index: usize,
        limit: usize,
    ) -> ElementCollectionOutcome {
        let desk_state = self.flow_desk_state(filter);
        let descriptors: Vec<FlowDeskRowDescriptor> = self
            .flow_desk_rows(filter)
            .iter()
            .map(|row| self.flow_desk_row_descriptor(row))
            .collect();
        let total_count = descriptors.len() + 3;
        let mut elements = Vec::with_capacity(limit.min(total_count));
        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo::input(
                "flow-ux-filter",
                Some(filter),
                self.focused_input != FocusedInput::None,
            ),
        );
        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo {
                semantic_id: "flow-desk:state".to_string(),
                element_type: protocol::ElementType::Panel,
                text: Some(desk_state.automation_label().to_string()),
                value: None,
                content: None,
                selected: None,
                focused: None,
                index: None,
                role: Some("flowDeskState".to_string()),
                kind: Some(desk_state.automation_label().to_string()),
                source: None,
                source_name: None,
                selectable: Some(false),
                status_kind: None,
                action_disabled: None,
                style: None,
            },
        );
        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo::list("flows", descriptors.len()),
        );
        for (index, descriptor) in descriptors.into_iter().enumerate() {
            if elements.len() >= limit {
                break;
            }
            elements.push(protocol::ElementInfo {
                semantic_id: descriptor.semantic_id,
                element_type: protocol::ElementType::Choice,
                text: Some(descriptor.title),
                value: Some(descriptor.detail),
                content: None,
                selected: Some(index == selected_index),
                focused: None,
                index: Some(index),
                role: Some("flowDeskRow".to_string()),
                kind: Some(descriptor.primary.label().to_string()),
                source: None,
                source_name: None,
                selectable: Some(true),
                status_kind: None,
                action_disabled: None,
                style: None,
            });
        }
        ElementCollectionOutcome::complete("flowDesk", elements, total_count)
    }

    fn collect_flow_session_elements(
        &self,
        session_id: u64,
        limit: usize,
        cx: &Context<Self>,
    ) -> ElementCollectionOutcome {
        let entity = self
            .conversations
            .flow_sessions
            .iter()
            .find(|(meta, _)| meta.id == session_id)
            .map(|(_, entity)| entity.clone());
        if let Some(entity) = entity {
            let chat = entity.read(cx);
            let (mut elements, mut total_count) = self.collect_chat_prompt_elements(chat, limit);
            // The chat's internal composer/model rows are suppressed
            // in this surface (`external_input`): the shared MAIN
            // input is the composer. Report that input — with its
            // real draft and real focus — instead of the hidden ones.
            let removed = elements.len();
            elements.retain(|el| {
                el.semantic_id != "input:chat-input"
                    && el.semantic_id != "input:chat-model"
                    && (el.role.as_deref() != Some("conversationCommand")
                        || el.semantic_id.starts_with("conversation.copyTurn:"))
            });
            total_count = total_count.saturating_sub(removed - elements.len());
            let placeholder = chat
                .placeholder
                .clone()
                .unwrap_or_else(|| "Message".to_string());
            let viewing_archive = self
                .conversations
                .flow_sessions
                .iter()
                .find(|(meta, _)| meta.id == session_id)
                .is_some_and(|(meta, _)| meta.selected_is_archived());
            if !viewing_archive {
                elements.insert(
                    0,
                    Self::input_element(
                        "flow-session-composer",
                        placeholder,
                        Some(Self::preview_value(&self.filter_text, 240)),
                        self.focused_input == FocusedInput::MainFilter,
                        Some(0),
                    ),
                );
                total_count += 1;
            }
            // S12: the shared recovery card, from the same projection
            // the flow session renderer uses.
            if let Some(meta) = self
                .conversations
                .flow_sessions
                .iter()
                .map(|(meta, _)| meta)
                .find(|meta| meta.id == session_id)
            {
                let snapshot = crate::flows::session::FlowSessionIdentitySnapshot::from_meta(meta);
                let identity =
                    crate::components::main_view_chrome::SemanticChipSpec::enabled_identity(
                        "flow-session:identity",
                        format!(
                            "{} · {} · {}",
                            snapshot.friendly_name,
                            snapshot.engine,
                            if snapshot.read_only {
                                "Archived"
                            } else {
                                "Active"
                            }
                        ),
                        crate::components::main_view_chrome::SemanticChipAction::OpenDetails,
                        "⌘K",
                    );
                elements.push(
                    crate::windows::automation_surface_collector::collect_semantic_chip_element(
                        &identity,
                    ),
                );
                let fact_specs = [
                    ("flow-session:friendly-name", snapshot.friendly_name.clone()),
                    ("flow-session:engine", snapshot.engine.clone()),
                    (
                        "flow-session:model",
                        snapshot
                            .model
                            .clone()
                            .unwrap_or_else(|| "Model unavailable".into()),
                    ),
                    ("flow-session:cwd", snapshot.cwd_display.clone()),
                    ("flow-session:origin", snapshot.origin_label.to_string()),
                    ("flow-session:selection", snapshot.selection.to_string()),
                    (
                        "flow-session:thread",
                        format!(
                            "active={} selected={}",
                            snapshot.active_thread_fingerprint,
                            snapshot.selected_thread_fingerprint
                        ),
                    ),
                    (
                        "flow-session:lineage",
                        format!(
                            "inherited={} parentRetained={}",
                            snapshot.inherited_turn_count,
                            snapshot
                                .parent_retained
                                .map(|retained| retained.to_string())
                                .unwrap_or_else(|| "unavailable".into())
                        ),
                    ),
                    ("flow-session:retention", snapshot.retention_text()),
                    (
                        "flow-session:rethread",
                        format!("needsRethread={}", snapshot.needs_rethread),
                    ),
                    (
                        "flow-session:draft",
                        format!(
                            "chars={} generation={}",
                            snapshot.draft_chars, snapshot.draft_generation
                        ),
                    ),
                    (
                        "flow-session:runtime",
                        format!(
                            "ready={} generation={} revision={}",
                            snapshot.thread_ready,
                            snapshot.runtime_generation,
                            snapshot.persistence_revision
                        ),
                    ),
                ];
                let fact_count = fact_specs.len();
                for (semantic_id, label) in fact_specs {
                    let spec =
                        crate::components::main_view_chrome::SemanticChipSpec::disabled_identity(
                            semantic_id,
                            label,
                            "Read-only Flow session fact",
                        );
                    elements.push(
                        crate::windows::automation_surface_collector::collect_semantic_chip_element(
                            &spec,
                        ),
                    );
                }
                let command_elements = crate::windows::automation_surface_collector::collect_conversation_command_elements(
                    &crate::components::conversation_actions::flow_conversation_commands_for_facts(
                        crate::components::conversation_actions::FlowConversationCommandFacts {
                            response_in_progress: meta.active_turn.is_some(),
                            viewing_archive: meta.selected_is_archived(),
                            has_archives: !meta.archived_threads.is_empty(),
                            selected_has_response: meta.selected_turns().iter().any(|turn| !turn.assistant.trim().is_empty()),
                            composer_has_text: !self.filter_text.trim().is_empty(),
                            hidden_draft_exists: meta.selected_is_archived() && !meta.active_draft.is_empty(),
                            runtime_attached: meta.thread_ready,
                        },
                    ),
                );
                total_count += 1 + fact_count + command_elements.len();
                elements.extend(command_elements);
                if let Some(spec) = (!meta.selected_is_archived())
                    .then(|| {
                        crate::ai::reliability::project_recovery(
                            &meta.reliability.state().identity,
                            meta.reliability.state(),
                            &crate::ai::reliability::flow_session_recovery_capabilities(),
                        )
                    })
                    .flatten()
                {
                    let recovery = Self::ai_recovery_elements(&spec);
                    total_count += recovery.len();
                    elements.extend(recovery);
                }
            }
            Self::finalize_surface_outcome(
                "flow-session",
                "flow-session",
                "panel_only_flow_session",
                limit,
                elements,
                total_count,
            )
        } else {
            ElementCollectionOutcome::partial(
                "flowSession",
                protocol::ProjectionReason::RuntimeEntityMissing,
                vec![protocol::ElementInfo::panel("flow-session")],
                1,
            )
            .with_warning("flow_session_entity_missing")
        }
    }
}
