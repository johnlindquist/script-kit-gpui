impl ScriptListApp {
    /// Push an element into the vec only if it hasn't reached the limit.
    /// Returns true if the element was added, false if capped.
    #[inline]
    fn push_limited_element(
        elements: &mut Vec<protocol::ElementInfo>,
        limit: usize,
        element: protocol::ElementInfo,
    ) -> bool {
        if elements.len() >= limit {
            return false;
        }
        elements.push(element);
        true
    }

    /// Build an ElementInfo for a Choice, preferring its stable key for the semantic ID.
    #[inline]
    fn keyed_choice_element(
        display_index: usize,
        choice: &Choice,
        selected: bool,
    ) -> protocol::ElementInfo {
        protocol::ElementInfo {
            semantic_id: choice.generate_id(display_index),
            element_type: protocol::ElementType::Choice,
            text: Some(choice.name.clone()),
            value: Some(choice.value.clone()),
            content: None,
            selected: Some(selected),
            focused: None,
            index: Some(display_index),
            role: None,
            kind: None,
            source: None,
            source_name: None,
            selectable: None,
            status_kind: None,
            action_disabled: None,
            style: None,
        }
        .redact_content(protocol::ElementContentKind::ExternalContent)
    }

    /// S12: project the SHARED AI recovery card into driver-visible elements.
    ///
    /// `collect_visible_elements` is a hand-written model of each surface, not
    /// a walk of the real GPUI tree. The recovery card had no node here, so
    /// `getElements` never reported it — and every probe assertion of the form
    /// "the recovery card is on screen" was unfalsifiable: it failed whether
    /// the card rendered or not. This projects the same
    /// [`crate::components::recovery_semantic_tree`] the renderer consumes, so
    /// runtime proof and render come from ONE source and cannot drift.
    fn ai_recovery_elements(
        spec: &crate::ai::reliability::AiRecoveryCardSpec,
    ) -> Vec<protocol::ElementInfo> {
        crate::components::recovery_semantic_tree(spec)
            .into_iter()
            .map(|node| {
                let is_action = node.role.ends_with("-action");
                let mut element = protocol::ElementInfo {
                    semantic_id: node.semantic_id.to_string(),
                    element_type: if is_action {
                        protocol::ElementType::Button
                    } else {
                        protocol::ElementType::Panel
                    },
                    text: None,
                    value: None,
                    content: None,
                    selected: None,
                    focused: None,
                    index: None,
                    role: Some(node.role.to_string()),
                    kind: None,
                    source: Some("AiRecoveryCard".to_string()),
                    source_name: None,
                    selectable: Some(node.enabled),
                    status_kind: None,
                    action_disabled: node
                        .disabled_reason
                        .as_ref()
                        .map(|reason| format!("{reason:?}")),
                    style: None,
                };
                match node.semantic_id {
                    crate::ai::reliability::AI_RECOVERY_TITLE_ID => {
                        element.text = Some(spec.title.to_string())
                    }
                    crate::ai::reliability::AI_RECOVERY_BODY_ID => {
                        element.text = Some(spec.body.to_string())
                    }
                    crate::ai::reliability::AI_RECOVERY_PROGRESS_ID => {
                        element.text = spec
                            .progress
                            .as_ref()
                            .map(|progress| progress.label.to_string())
                    }
                    _ => {
                        element.text = spec
                            .actions
                            .iter()
                            .find(|action| action.semantic_id == node.semantic_id)
                            .map(|action| action.label.to_string())
                    }
                }
                element
            })
            .collect()
    }

    fn collect_filterable_rows_with_info_empty(
        &self,
        collection: FilterableRowsCollection<'_>,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let FilterableRowsCollection {
            input_name,
            input_value,
            list_name,
            empty_state_id,
            empty_text,
            empty_icon_hint,
            rows,
            selected_index,
            limit,
        } = collection;
        let empty_state = rows.is_empty().then(|| {
            crate::components::simple_empty_state_spec(
                empty_state_id,
                empty_text.to_string(),
                empty_icon_hint,
                None,
            )
            .semantic_snapshot()
        });
        let empty_element_count = empty_state
            .as_ref()
            .map_or(0, |snapshot| Self::info_state_elements(snapshot).len());
        let total_count = rows.len() + 2 + empty_element_count;
        let mut elements = Vec::with_capacity(limit.min(total_count));

        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo::input(
                input_name,
                Some(input_value.as_str()),
                self.focused_input != FocusedInput::None,
            ),
        );

        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo::list(list_name, rows.len()),
        );

        if let Some(snapshot) = empty_state.as_ref() {
            for element in Self::info_state_elements(snapshot) {
                if !Self::push_limited_element(&mut elements, limit, element) {
                    break;
                }
            }
            return (elements, total_count);
        }

        for (index, row) in rows.iter().enumerate() {
            if elements.len() >= limit {
                break;
            }
            elements.push(protocol::ElementInfo {
                semantic_id: protocol::generate_semantic_id("choice", index, row),
                element_type: protocol::ElementType::Choice,
                text: Some(row.clone()),
                value: Some(row.clone()),
                content: None,
                selected: Some(index == selected_index),
                focused: None,
                index: Some(index),
                role: Some("generic-filterable-row".to_string()),
                kind: Some(list_name.to_string()),
                source: Some(list_name.to_string()),
                source_name: None,
                selectable: Some(true),
                status_kind: None,
                action_disabled: None,
                style: None,
            });
        }

        (elements, total_count)
    }

    fn finalize_surface_outcome(
        surface: &str,
        panel_name: &str,
        warning: &str,
        limit: usize,
        elements: Vec<protocol::ElementInfo>,
        total_count: usize,
    ) -> ElementCollectionOutcome {
        if !elements.is_empty() {
            let elements: Vec<protocol::ElementInfo> = elements.into_iter().take(limit).collect();
            tracing::info!(
                surface = surface,
                element_count = elements.len(),
                total_count,
                used_panel_fallback = false,
                "Collected semantic elements for inspectable surface"
            );
            return ElementCollectionOutcome::complete(surface, elements, total_count);
        }

        let total_count = 1;
        let elements: Vec<protocol::ElementInfo> = vec![protocol::ElementInfo::panel(panel_name)]
            .into_iter()
            .take(limit)
            .collect();
        tracing::info!(
            surface = surface,
            element_count = elements.len(),
            total_count,
            used_panel_fallback = true,
            "Collected semantic elements for inspectable surface"
        );
        ElementCollectionOutcome::partial(
            surface,
            protocol::ProjectionReason::PanelOnly,
            elements,
            total_count,
        )
        .with_warning(warning)
    }

    fn preview_value(value: &str, max_chars: usize) -> String {
        let char_count = value.chars().count();
        if char_count <= max_chars {
            return value.to_string();
        }

        let mut preview: String = value.chars().take(max_chars).collect();
        preview.push_str("...");
        preview
    }

    fn input_element(
        semantic_name: &str,
        label: impl Into<String>,
        value: Option<String>,
        focused: bool,
        index: Option<usize>,
    ) -> protocol::ElementInfo {
        protocol::ElementInfo {
            semantic_id: protocol::generate_semantic_id_named("input", semantic_name),
            element_type: protocol::ElementType::Input,
            text: Some(label.into()),
            value,
            content: None,
            selected: None,
            focused: Some(focused),
            index,
            role: None,
            kind: None,
            source: None,
            source_name: None,
            selectable: None,
            status_kind: None,
            action_disabled: None,
            style: None,
        }
        .redact_value(protocol::ElementContentKind::UserContent)
    }

    fn choice_element(
        index: usize,
        text: String,
        value: String,
        selected: bool,
    ) -> protocol::ElementInfo {
        protocol::ElementInfo {
            semantic_id: protocol::generate_semantic_id("choice", index, value.as_str()),
            element_type: protocol::ElementType::Choice,
            text: Some(text),
            value: Some(value),
            content: None,
            selected: Some(selected),
            focused: None,
            index: Some(index),
            role: None,
            kind: None,
            source: None,
            source_name: None,
            selectable: None,
            status_kind: None,
            action_disabled: None,
            style: None,
        }
    }

    fn collect_term_prompt_elements(
        &self,
        term: &term_prompt::TermPrompt,
        semantic_prefix: &str,
        limit: usize,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let content = term.terminal.content();
        let visible_lines: Vec<(usize, String)> = content
            .lines_plain()
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some((index, Self::preview_value(trimmed, 240)))
                }
            })
            .collect();

        let total_count = visible_lines.len() + 1;
        let mut elements = Vec::with_capacity(limit.min(total_count));

        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo::list(
                format!("{semantic_prefix}-lines").as_str(),
                visible_lines.len(),
            ),
        );

        for (index, (line_index, line)) in visible_lines.iter().enumerate() {
            if elements.len() >= limit {
                break;
            }
            elements.push(Self::choice_element(
                index,
                format!("Line {}", line_index + 1),
                line.clone(),
                *line_index == content.cursor_line,
            ));
        }

        (elements, total_count)
    }

    fn collect_editor_prompt_elements(
        &self,
        editor: &crate::editor::EditorPrompt,
        semantic_prefix: &str,
        limit: usize,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let mut total_count = 1;
        let mut elements = Vec::with_capacity(limit.min(8));

        Self::push_limited_element(
            &mut elements,
            limit,
            Self::input_element(
                format!("{semantic_prefix}-language").as_str(),
                "Language",
                Some(editor.language().to_string()),
                true,
                Some(0),
            ),
        );

        if let Some(snippet_state) = editor.snippet_state() {
            total_count += snippet_state.current_values.len() + 1;
            Self::push_limited_element(
                &mut elements,
                limit,
                protocol::ElementInfo::list(
                    format!("{semantic_prefix}-tabstops").as_str(),
                    snippet_state.current_values.len(),
                ),
            );

            for (index, value) in snippet_state.current_values.iter().enumerate() {
                if elements.len() >= limit {
                    break;
                }
                elements.push(Self::choice_element(
                    index,
                    format!("Tabstop {}", index + 1),
                    Self::preview_value(value.as_str(), 120),
                    index == snippet_state.current_tabstop_idx,
                ));
            }
        }

        (elements, total_count)
    }

    fn collect_naming_prompt_elements(
        &self,
        naming_prompt: &prompts::NamingPrompt,
        limit: usize,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let total_count = 2;
        let mut elements = Vec::with_capacity(limit.min(total_count));

        Self::push_limited_element(
            &mut elements,
            limit,
            Self::input_element(
                "naming-friendly-name",
                naming_prompt
                    .placeholder
                    .clone()
                    .unwrap_or_else(|| "Name".to_string()),
                Some(Self::preview_value(
                    naming_prompt.friendly_name.as_str(),
                    180,
                )),
                true,
                Some(0),
            ),
        );
        Self::push_limited_element(
            &mut elements,
            limit,
            Self::input_element(
                "naming-filename",
                "Filename",
                Some(Self::preview_value(naming_prompt.filename.as_str(), 180)),
                false,
                Some(1),
            ),
        );

        (elements, total_count)
    }
}
