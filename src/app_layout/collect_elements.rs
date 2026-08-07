// Element collection for getElements protocol support.

fn root_file_semantic_kind(file_type: crate::file_search::FileType) -> &'static str {
    match file_type {
        crate::file_search::FileType::Directory => "directory",
        _ => "file",
    }
}
// Returns a bounded list of visible UI elements with semantic IDs.

/// Outcome of collecting visible UI elements, carrying receipt metadata
/// for the `elementsResult` protocol response.
#[derive(Debug, Clone)]
pub(crate) struct ElementCollectionOutcome {
    pub semantic_surface: String,
    pub version: u32,
    pub projection_quality: protocol::ProjectionQuality,
    pub reason_codes: Vec<protocol::ProjectionReason>,
    pub elements: Vec<protocol::ElementInfo>,
    pub total_count: usize,
    pub warnings: Vec<String>,
}

impl ElementCollectionOutcome {
    const VERSION: u32 = 1;

    pub fn complete(
        semantic_surface: impl Into<String>,
        elements: Vec<protocol::ElementInfo>,
        total_count: usize,
    ) -> Self {
        Self {
            semantic_surface: semantic_surface.into(),
            version: Self::VERSION,
            projection_quality: protocol::ProjectionQuality::Complete,
            reason_codes: Vec::new(),
            elements,
            total_count,
            warnings: Vec::new(),
        }
    }

    pub fn complete_from(
        semantic_surface: impl Into<String>,
        (elements, total_count): (Vec<protocol::ElementInfo>, usize),
    ) -> Self {
        Self::complete(semantic_surface, elements, total_count)
    }

    pub fn partial(
        semantic_surface: impl Into<String>,
        reason: protocol::ProjectionReason,
        elements: Vec<protocol::ElementInfo>,
        total_count: usize,
    ) -> Self {
        Self {
            semantic_surface: semantic_surface.into(),
            version: Self::VERSION,
            projection_quality: protocol::ProjectionQuality::Partial,
            reason_codes: vec![reason],
            elements,
            total_count,
            warnings: Vec::new(),
        }
    }

    pub fn unsupported(
        semantic_surface: impl Into<String>,
        reason: protocol::ProjectionReason,
        elements: Vec<protocol::ElementInfo>,
        total_count: usize,
    ) -> Self {
        Self {
            semantic_surface: semantic_surface.into(),
            version: Self::VERSION,
            projection_quality: protocol::ProjectionQuality::Unsupported,
            reason_codes: vec![reason],
            elements,
            total_count,
            warnings: Vec::new(),
        }
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    pub fn focused_semantic_id(&self) -> Option<String> {
        self.elements
            .iter()
            .find(|element| element.focused == Some(true))
            .map(|element| element.semantic_id.clone())
    }

    pub fn selected_semantic_id(&self) -> Option<String> {
        self.elements
            .iter()
            .find(|element| element.selected == Some(true))
            .map(|element| element.semantic_id.clone())
    }
}

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

    fn info_state_elements(
        snapshot: &crate::components::InfoStateSemanticSnapshot,
    ) -> Vec<protocol::ElementInfo> {
        let mut root = protocol::ElementInfo::panel(snapshot.id);
        root.semantic_id = format!("info-state:{}", snapshot.id);
        root.text = snapshot.accessible_prefix.map(str::to_string);
        root.value = snapshot.default_icon_hint.map(str::to_string);
        root.role = Some("info-state".to_string());
        root.kind = Some(snapshot.semantic_kind.to_string());
        root.source = Some("InfoState".to_string());
        root.source_name = Some(snapshot.id.to_string());
        root.selectable = Some(false);
        root.status_kind = Some(snapshot.semantic_kind.to_string());

        let mut elements = Vec::with_capacity(snapshot.cues.len() + 1);
        elements.push(root);
        elements.extend(snapshot.cues.iter().enumerate().map(|(index, cue)| {
            protocol::ElementInfo {
                semantic_id: format!("info-cue:{}", cue.semantic_id),
                element_type: protocol::ElementType::Panel,
                text: Some(cue.cue_text.clone()),
                value: cue.canonical_shortcut.clone(),
                content: None,
                selected: Some(false),
                focused: Some(false),
                index: Some(index),
                role: Some("guidance-cue".to_string()),
                kind: Some(cue.cue_kind.to_string()),
                source: Some("InfoState".to_string()),
                source_name: Some(snapshot.id.to_string()),
                selectable: Some(false),
                status_kind: Some(snapshot.semantic_kind.to_string()),
                action_disabled: None,
                style: None,
            }
        }));
        elements
    }

    fn menu_syntax_guidance_elements(
        snapshot: &crate::menu_syntax::MenuSyntaxMainHintSnapshot,
    ) -> Vec<protocol::ElementInfo> {
        let mut root = protocol::ElementInfo::panel("menu-syntax-main-hint");
        root.role = Some("guidance-state".to_string());
        root.kind = Some("syntax-guidance".to_string());
        root.source = Some("MenuSyntaxMainHint".to_string());
        root.source_name = Some(format!("{:?}", snapshot.kind));
        root.selectable = Some(false);

        let mut elements = vec![root];
        let mut syntax_tokens = Vec::new();
        if let Some(active_head) = snapshot.active_head.as_deref() {
            syntax_tokens.push(active_head.to_string());
        }
        for example in &snapshot.examples {
            if !syntax_tokens.contains(example) {
                syntax_tokens.push(example.clone());
            }
        }
        if syntax_tokens.is_empty() {
            if let Some(example) = snapshot.example.as_deref() {
                syntax_tokens.push(example.to_string());
            }
        }

        elements.extend(syntax_tokens.into_iter().enumerate().map(|(index, syntax)| {
            protocol::ElementInfo {
                semantic_id: format!("menu-syntax-cue:{index}"),
                element_type: protocol::ElementType::Panel,
                text: Some(syntax),
                value: None,
                content: None,
                selected: Some(false),
                focused: Some(false),
                index: Some(index),
                role: Some("guidance-cue".to_string()),
                kind: Some("syntax".to_string()),
                source: Some("MenuSyntaxMainHint".to_string()),
                source_name: Some(format!("{:?}", snapshot.kind)),
                selectable: Some(false),
                status_kind: None,
                action_disabled: None,
                style: None,
            }
        }));
        elements
    }

    fn main_view_context_elements(
        zone: &crate::components::main_view_chrome::MainViewContextZoneSpec,
    ) -> Vec<protocol::ElementInfo> {
        let mut chips = vec![&zone.leading_identity];
        if let Some(context) = zone.context_attachment.as_ref() {
            chips.push(context);
        }
        chips.push(&zone.trailing_identity);

        chips
            .into_iter()
            .flat_map(
                crate::windows::automation_surface_collector::collect_semantic_chip_elements,
            )
            .collect()
    }

    pub(crate) fn collect_visible_elements(
        &self,
        limit: usize,
        cx: &Context<Self>,
    ) -> ElementCollectionOutcome {
        self.collect_visible_elements_with_headers(limit, false, cx)
    }

    /// Like [`Self::collect_visible_elements`], but optionally emits
    /// non-selectable section-header rows so layout-stability probes can audit
    /// the persistent leading separator contract (POLISH.md §2).
    pub(crate) fn collect_visible_elements_with_headers(
        &self,
        limit: usize,
        include_headers: bool,
        cx: &Context<Self>,
    ) -> ElementCollectionOutcome {
        let mut outcome = match &self.current_view {
            AppView::ScriptList => {
                let context = Self::main_view_context_elements(&self.main_view_context_zone_spec());
                let context_count = context.len();
                let (list_elements, list_total) = self.collect_script_list_elements(
                    limit.saturating_sub(context_count),
                    include_headers,
                );
                let elements = context
                    .into_iter()
                    .chain(list_elements)
                    .take(limit)
                    .collect();
                ElementCollectionOutcome::complete(
                    "scriptList",
                    elements,
                    context_count + list_total,
                )
            }

            AppView::AgentChatView { entity } => {
                let focused_text_elements = entity
                    .read(cx)
                    .collect_focused_text_mini_elements(limit, cx);
                if !focused_text_elements.is_empty() {
                    ElementCollectionOutcome::complete(
                        "focusedTextMini",
                        focused_text_elements.clone(),
                        focused_text_elements.len(),
                    )
                } else {
                    let state = entity.read(cx).collect_agent_chat_state_snapshot(cx);
                    let mut elements = vec![
                        protocol::ElementInfo::panel("agent_chat-chat"),
                        protocol::ElementInfo::input(
                            "agent_chat-composer",
                            Some(state.input_text.as_str()),
                            true,
                        ),
                        protocol::ElementInfo::list("agent_chat-messages", state.message_count),
                    ];
                    elements.extend(
                        entity
                            .read(cx)
                            .conversation_semantic_chip_specs(cx)
                            .iter()
                            .flat_map(
                                crate::windows::automation_surface_collector::collect_semantic_chip_elements,
                            ),
                    );
                    elements.extend(
                        crate::windows::automation_surface_collector::collect_conversation_command_elements(
                            &entity.read(cx).conversation_command_bindings(cx),
                        ),
                    );
                    elements.extend(
                        crate::windows::automation_surface_collector::collect_agent_chat_conversation_elements(
                            entity, cx,
                        ),
                    );
                    if state.message_count == 0 {
                        let snapshot =
                            crate::components::agent_chat_empty_guidance_spec().semantic_snapshot();
                        elements.extend(Self::info_state_elements(&snapshot));
                    }
                    // S12: the SHARED recovery card, reported with the same
                    // `ai-recovery-*` ids the flow session surface reports, so
                    // one probe can compare the two surfaces directly.
                    if let Some(spec) = entity.read(cx).active_recovery_card_spec(cx) {
                        elements.extend(Self::ai_recovery_elements(&spec));
                    }
                    let total_count = elements.len();
                    ElementCollectionOutcome::complete(
                        "agentChat",
                        elements.into_iter().take(limit).collect(),
                        total_count,
                    )
                }
            }

            AppView::DayPage { entity } => {
                let (elements, total_count) =
                    entity.read(cx).collect_day_page_elements(limit, self, cx);
                ElementCollectionOutcome::complete("dayPage", elements, total_count)
            }

            AppView::PermissionsWizardView { selected_index } => {
                let mut elements = Vec::new();
                let mut root = protocol::ElementInfo::panel("permissions-wizard");
                root.text = Some("Set Up Permissions".to_string());
                root.role = Some("wizard".to_string());
                root.source = Some("PermissionsWizard".to_string());
                elements.push(root);

                let mut intro = protocol::ElementInfo::panel("permissions-intro");
                intro.text = Some(
                    "Script Kit uses macOS permissions to read selected text, paste into other apps, run shortcuts, and capture context."
                        .to_string(),
                );
                intro.role = Some("intro".to_string());
                intro.source = Some("PermissionsWizard".to_string());
                elements.push(intro);

                let kinds = crate::permissions_wizard::PermissionKind::all();
                let mut list = protocol::ElementInfo::list("permissions", kinds.len());
                list.source = Some("PermissionsWizard".to_string());
                elements.push(list);

                for (index, kind) in kinds.iter().enumerate() {
                    let status = crate::permissions_wizard::detect_permission(*kind);
                    let mut row = protocol::ElementInfo::product_static_choice(
                        index,
                        kind.name(),
                        kind.name(),
                        index == *selected_index,
                    );
                    row.semantic_id = format!("permission-row:{:?}", kind);
                    row.role = Some("permission".to_string());
                    row.kind = Some(format!("{:?}", kind));
                    row.source = Some("PermissionsWizard".to_string());
                    row.status_kind = Some(format!("{:?}", status));
                    elements.push(row);
                }

                let total_count = elements.len();
                ElementCollectionOutcome::complete(
                    "permissionsWizard",
                    elements.into_iter().take(limit).collect(),
                    total_count,
                )
            }

            AppView::ArgPrompt { choices, .. } => ElementCollectionOutcome::complete_from(
                "argPrompt",
                self.collect_choice_view_elements(
                    "filter",
                    self.arg_input.text().to_string(),
                    choices,
                    self.arg_selected_index,
                    limit,
                ),
            ),

            AppView::MiniPrompt { choices, .. } => ElementCollectionOutcome::complete_from(
                "miniPrompt",
                self.collect_choice_view_elements(
                    "filter",
                    self.arg_input.text().to_string(),
                    choices,
                    self.arg_selected_index,
                    limit,
                ),
            ),

            AppView::MicroPrompt { choices, .. } => ElementCollectionOutcome::complete_from(
                "microPrompt",
                self.collect_choice_view_elements(
                    "filter",
                    self.arg_input.text().to_string(),
                    choices,
                    self.arg_selected_index,
                    limit,
                ),
            ),

            AppView::ClipboardHistoryView {
                filter,
                selected_index,
            } => {
                let rows = self.clipboard_history_visible_row_labels(filter);
                ElementCollectionOutcome::complete_from(
                    "clipboardHistory",
                    self.collect_named_rows(
                        "clipboard-filter",
                        filter.clone(),
                        "clipboard-history",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::ProfileSearchView {
                filter,
                selected_index,
            } => self.collect_profile_search_elements(filter, *selected_index, limit),

            AppView::AppLauncherView {
                filter,
                selected_index,
            } => {
                let rows = self.app_launcher_visible_row_names(filter);
                ElementCollectionOutcome::complete_from(
                    "appLauncher",
                    self.collect_named_rows(
                        "app-filter",
                        filter.clone(),
                        "apps",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::WindowSwitcherView {
                filter,
                selected_index,
            } => {
                let rows: Vec<String> = if filter.is_empty() {
                    self.cached_windows
                        .iter()
                        .map(|w| format!("{} — {}", w.app, w.title))
                        .collect()
                } else {
                    let filter_lower = filter.to_lowercase();
                    self.cached_windows
                        .iter()
                        .map(|w| format!("{} — {}", w.app, w.title))
                        .filter(|row| row.to_lowercase().contains(&filter_lower))
                        .collect()
                };
                ElementCollectionOutcome::complete_from(
                    "windowSwitcher",
                    self.collect_named_rows(
                        "window-filter",
                        filter.clone(),
                        "windows",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::BrowserTabsView {
                filter,
                selected_index,
            } => {
                let rows = self.browser_tabs_visible_row_labels(filter);
                ElementCollectionOutcome::complete_from(
                    "browserTabs",
                    self.collect_named_rows(
                        "browser-tabs-filter",
                        filter.clone(),
                        "browser-tabs",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::BrowserHistoryView {
                filter,
                selected_index,
            } => {
                let rows: Vec<String> = if filter.is_empty() {
                    self.cached_browser_history
                        .iter()
                        .map(|entry| entry.display_title().to_string())
                        .collect()
                } else {
                    crate::browser_history::fuzzy_search_browser_history(
                        &self.cached_browser_history,
                        filter,
                    )
                    .into_iter()
                    .map(|hit| hit.entry.display_title().to_string())
                    .collect()
                };
                ElementCollectionOutcome::complete_from(
                    "browserHistory",
                    self.collect_named_rows(
                        "browser-history-filter",
                        filter.clone(),
                        "browser-history",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::AgentChatHistoryView {
                filter,
                selected_index,
            } => {
                let rows = Self::agent_chat_history_visible_row_labels(filter);
                ElementCollectionOutcome::complete_from(
                    "agentChatHistory",
                    self.collect_named_rows(
                        "agent_chat-history-filter",
                        filter.clone(),
                        "agent_chat-history",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::DictationHistoryView {
                filter,
                selected_index,
                visible_limit,
            } => {
                let page_result =
                    crate::dictation::search_history_page(filter, 0, *visible_limit);
                let load_failed = page_result.is_err();
                let page = page_result
                    .ok()
                    .or_else(|| self.dictation_history_previous_page.clone());
                let rows = page
                    .as_ref()
                    .map(|page| {
                        page.rows
                            .iter()
                            .map(|entry| entry.preview.clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let (mut elements, mut total_count) = self.collect_named_rows(
                    "dictation-history-filter",
                    filter.clone(),
                    "dictation-history",
                    &rows,
                    *selected_index,
                    limit,
                );
                if load_failed {
                    total_count += 1;
                    Self::push_limited_element(
                        &mut elements,
                        limit,
                        protocol::ElementInfo {
                            semantic_id: "status:dictation-history-load-failed".to_string(),
                            element_type: protocol::ElementType::Panel,
                            text: Some("Dictation History could not be loaded".to_string()),
                            value: None,
                            content: None,
                            selected: Some(false),
                            focused: Some(false),
                            index: None,
                            role: Some("status".to_string()),
                            kind: Some("failed".to_string()),
                            source: Some("dictationHistory".to_string()),
                            source_name: None,
                            selectable: Some(false),
                            status_kind: Some("loadFailed".to_string()),
                            action_disabled: Some("Retry after the History file becomes available".to_string()),
                            style: None,
                        },
                    );
                }
                if page.as_ref().is_some_and(|page| page.has_more) {
                    total_count += 1;
                    Self::push_limited_element(
                        &mut elements,
                        limit,
                        protocol::ElementInfo {
                            semantic_id: "button:dictation-history-load-more".to_string(),
                            element_type: protocol::ElementType::Button,
                            text: Some("Load More".to_string()),
                            value: None,
                            content: None,
                            selected: Some(false),
                            focused: Some(false),
                            index: None,
                            role: Some("action".to_string()),
                            kind: Some("loadMore".to_string()),
                            source: Some("dictationHistory".to_string()),
                            source_name: None,
                            selectable: Some(true),
                            status_kind: None,
                            action_disabled: None,
                            style: None,
                        },
                    );
                }
                ElementCollectionOutcome::complete("dictationHistory", elements, total_count)
            }

            AppView::NotesBrowseView { search } => ElementCollectionOutcome::complete_from(
                "notesBrowse",
                self.collect_notes_browse_elements(search, limit),
            ),

            AppView::FileSearchView {
                ref query,
                selected_index,
                ..
            } => {
                let rows: Vec<String> = self
                    .file_search_display_indices
                    .iter()
                    .filter_map(|&result_index| self.cached_file_results.get(result_index))
                    .map(|entry| format!("{} — {}", entry.name, entry.path))
                    .collect();
                ElementCollectionOutcome::complete_from(
                    "fileSearch",
                    self.collect_named_rows(
                        "file-search-input",
                        query.clone(),
                        "file-results",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::ProcessManagerView {
                filter,
                selected_index,
            } => {
                let rows = self.process_manager_visible_row_names(filter);
                ElementCollectionOutcome::complete_from(
                    "processManager",
                    self.collect_named_rows(
                        "process-filter",
                        filter.clone(),
                        "processes",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::FlowUxView {
                filter,
                selected_index,
                ..
            } => {
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
                        Some(filter.as_str()),
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
                        selected: Some(index == *selected_index),
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

            AppView::SettingsView {
                filter,
                selected_index,
            } => {
                let rows = self.settings_visible_row_names(filter);
                ElementCollectionOutcome::complete_from(
                    "settings",
                    self.collect_named_rows(
                        "settings-filter",
                        filter.clone(),
                        "settings",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::CurrentAppCommandsView {
                filter,
                selected_index,
            } => {
                let rows = self.current_app_commands_visible_row_names(filter);
                ElementCollectionOutcome::complete_from(
                    "currentAppCommands",
                    self.collect_named_rows(
                        "current-app-commands-filter",
                        filter.clone(),
                        "menu-commands",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::SearchAiPresetsView {
                filter,
                selected_index,
            } => {
                let rows = Self::ai_preset_search_visible_row_labels(filter);
                ElementCollectionOutcome::complete_from(
                    "searchAiPresets",
                    self.collect_filterable_rows_with_info_empty(
                        "ai-presets-filter",
                        filter.clone(),
                        "ai-presets",
                        "ai-presets-empty",
                        AiPresetSearchEmptyState::from_filter(filter).message(),
                        "sparkles",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::FavoritesBrowseView {
                filter,
                selected_index,
            } => {
                let rows = self.filtered_favorite_ids_for_filter(filter);
                ElementCollectionOutcome::complete_from(
                    "favoritesBrowse",
                    self.collect_filterable_rows_with_info_empty(
                        "favorites-filter",
                        filter.clone(),
                        "favorites",
                        "favorites-empty",
                        FavoritesEmptyState::from_filter(filter).message(),
                        "star",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::SdkReferenceView {
                filter,
                selected_index,
                entries,
            } => {
                let rows = crate::mcp_resources::sdk_reference_visible_row_names(entries, filter);
                ElementCollectionOutcome::complete_from(
                    "sdkReference",
                    self.collect_named_rows(
                        "sdk-reference-filter",
                        filter.clone(),
                        "sdk-functions",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }
            AppView::TipsView {
                filter,
                selected_index,
                entries,
            } => {
                let query = filter.trim().to_lowercase();
                let rows: Vec<String> = entries
                    .iter()
                    .filter(|tip| {
                        query.is_empty()
                            || tip.title.to_lowercase().contains(&query)
                            || tip.hint.to_lowercase().contains(&query)
                            || tip.description.to_lowercase().contains(&query)
                            || tip
                                .keywords
                                .iter()
                                .any(|keyword| keyword.to_lowercase().contains(&query))
                    })
                    .map(|tip| tip.title.clone())
                    .collect();
                ElementCollectionOutcome::complete_from(
                    "tips",
                    self.collect_named_rows(
                        "tips-filter",
                        filter.clone(),
                        "tips",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::ScriptTemplateCatalogView {
                filter,
                selected_index,
                templates,
            } => {
                let rows = crate::mcp_resources::script_template_catalog_visible_row_names(
                    templates, filter,
                );
                ElementCollectionOutcome::complete_from(
                    "scriptTemplateCatalog",
                    self.collect_named_rows(
                        "script-template-filter",
                        filter.clone(),
                        "script-templates",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::EmojiPickerView {
                ref filter,
                selected_index,
                selected_category,
            } => {
                let rows: Vec<String> = crate::emoji::search_emojis(filter.as_str())
                    .into_iter()
                    .filter(|emoji| {
                        selected_category
                            .map(|category| emoji.category == category)
                            .unwrap_or(true)
                    })
                    .map(|emoji| emoji.name.to_string())
                    .collect();
                ElementCollectionOutcome::complete_from(
                    "emojiPicker",
                    self.collect_named_rows(
                        "emoji-filter",
                        filter.clone(),
                        "emoji-results",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::BrowseKitsView {
                query,
                selected_index,
                results,
            } => {
                let rows = Self::kit_store_browse_visible_row_labels(results);
                ElementCollectionOutcome::complete_from(
                    "browseKits",
                    self.collect_named_rows(
                        "kit-search",
                        query.clone(),
                        "kit-results",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }
            AppView::MigrateV1View {
                filter,
                selected_index,
                board,
            } => {
                let rows: Vec<String> = Self::migrate_visible_rows(&board.rows, filter)
                    .into_iter()
                    .filter_map(|ix| board.rows.get(ix).map(|row| row.file.clone()))
                    .collect();
                ElementCollectionOutcome::complete_from(
                    "migrateV1",
                    self.collect_named_rows(
                        "migrate-v1-filter",
                        filter.clone(),
                        "migrate-v1-results",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::InstalledKitsView {
                filter,
                selected_index,
                kits,
            } => {
                let rows = Self::kit_store_installed_visible_row_labels(kits, filter);
                ElementCollectionOutcome::complete_from(
                    "installedKits",
                    self.collect_named_rows(
                        "installed-kits-filter",
                        filter.clone(),
                        "installed-kits",
                        &rows,
                        *selected_index,
                        limit,
                    ),
                )
            }

            AppView::ThemeChooserView {
                filter,
                selected_index,
            } => {
                let catalog = Self::theme_chooser_catalog();
                let filtered = Self::theme_chooser_catalog_filtered_indices(filter, &catalog);
                let mut elements: Vec<protocol::ElementInfo> = vec![
                    protocol::ElementInfo::input("theme-filter", Some(filter.as_str()), true),
                    protocol::ElementInfo::panel("theme-chooser"),
                    // Keep the list name under the 20-char slug cap so the
                    // semantic id is not truncated ("theme-chooser-catalog"
                    // would collapse to "theme-chooser-catalo").
                    protocol::ElementInfo::list("theme-catalog", filtered.len()),
                ];
                let selected_entry =
                    Self::theme_chooser_selected_entry(&catalog, &filtered, *selected_index);
                let management = self.theme_chooser_management_snapshot(selected_entry);

                elements.push(protocol::ElementInfo {
                    semantic_id: "control:theme-chooser:panel-mode".to_string(),
                    element_type: protocol::ElementType::Toggle,
                    text: Some("Panel Mode".to_string()),
                    value: Some(self.theme_chooser_panel_mode.as_str().to_string()),
                    content: None,
                    selected: Some(matches!(
                        self.theme_chooser_panel_mode,
                        ThemeChooserPanelMode::Customize
                    )),
                    focused: None,
                    index: None,
                    role: Some("theme-control".to_string()),
                    kind: Some("panel-mode".to_string()),
                    source: None,
                    source_name: None,
                    selectable: Some(true),
                    status_kind: None,
                    action_disabled: None,
                    style: None,
                });

                elements.push(protocol::ElementInfo {
                    semantic_id: "status:theme-chooser-dirty-state".to_string(),
                    element_type: protocol::ElementType::Panel,
                    text: Some(management.status_label.clone()),
                    value: Some(management.status_value.clone()),
                    content: None,
                    selected: Some(management.is_dirty),
                    focused: None,
                    index: None,
                    role: Some("theme-management".to_string()),
                    kind: Some("dirty-state".to_string()),
                    source: management.base_slug.clone(),
                    source_name: management.base_name.clone(),
                    selectable: Some(false),
                    status_kind: Some(management.status_kind.clone()),
                    action_disabled: None,
                    style: None,
                });
                elements.push(protocol::ElementInfo {
                    semantic_id: "control:theme-chooser:save-name".to_string(),
                    element_type: protocol::ElementType::Input,
                    text: Some("Theme Name".to_string()),
                    value: Some(management.resolved_save_name.clone()),
                    content: None,
                    selected: None,
                    focused: None,
                    index: None,
                    role: Some("theme-management".to_string()),
                    kind: Some("save-name".to_string()),
                    source: None,
                    source_name: None,
                    selectable: Some(true),
                    status_kind: management.duplicate_status_kind.clone(),
                    action_disabled: None,
                    style: None,
                });

                elements.push(protocol::ElementInfo {
                    semantic_id: "button:theme-chooser-save-as-user-theme".to_string(),
                    element_type: protocol::ElementType::Button,
                    text: Some("Save Copy".to_string()),
                    value: Some("theme_chooser_save_as_user_theme".to_string()),
                    content: None,
                    selected: None,
                    focused: None,
                    index: None,
                    role: Some("theme-action".to_string()),
                    kind: Some("save-as-user-theme".to_string()),
                    source: None,
                    source_name: None,
                    selectable: Some(true),
                    status_kind: None,
                    action_disabled: None,
                    style: None,
                });
                elements.push(protocol::ElementInfo {
                    semantic_id: "button:theme-chooser-edit-theme-as-text".to_string(),
                    element_type: protocol::ElementType::Button,
                    text: Some("Edit Theme as Text".to_string()),
                    value: Some("theme_chooser_edit_theme_as_text".to_string()),
                    content: None,
                    selected: None,
                    focused: None,
                    index: None,
                    role: Some("theme-action".to_string()),
                    kind: Some("edit-theme-as-text".to_string()),
                    source: None,
                    source_name: None,
                    selectable: Some(true),
                    status_kind: None,
                    action_disabled: None,
                    style: None,
                });
                elements.push(protocol::ElementInfo {
                    semantic_id: "button:theme-chooser-update-user-theme".to_string(),
                    element_type: protocol::ElementType::Button,
                    text: Some("Update".to_string()),
                    value: Some("theme_chooser_update_user_theme".to_string()),
                    content: None,
                    selected: Some(management.can_update),
                    focused: None,
                    index: None,
                    role: Some("theme-action".to_string()),
                    kind: Some("update-user-theme".to_string()),
                    source: None,
                    source_name: None,
                    selectable: Some(management.update_disabled.is_none()),
                    status_kind: None,
                    action_disabled: management.update_disabled.clone(),
                    style: None,
                });
                elements.push(protocol::ElementInfo {
                    semantic_id: "button:theme-chooser-delete-user-theme".to_string(),
                    element_type: protocol::ElementType::Button,
                    text: Some("Delete".to_string()),
                    value: Some("theme_chooser_delete_user_theme".to_string()),
                    content: None,
                    selected: None,
                    focused: None,
                    index: None,
                    role: Some("theme-action".to_string()),
                    kind: Some("delete-user-theme".to_string()),
                    source: None,
                    source_name: None,
                    selectable: Some(management.delete_disabled.is_none()),
                    status_kind: None,
                    action_disabled: management.delete_disabled.clone(),
                    style: None,
                });
                elements.push(protocol::ElementInfo {
                    semantic_id: "button:theme-chooser-restore-deleted-user-theme".to_string(),
                    element_type: protocol::ElementType::Button,
                    text: Some("Restore".to_string()),
                    value: Some("theme_chooser_restore_deleted_user_theme".to_string()),
                    content: None,
                    selected: None,
                    focused: None,
                    index: None,
                    role: Some("theme-action".to_string()),
                    kind: Some("restore-deleted-user-theme".to_string()),
                    source: None,
                    source_name: None,
                    selectable: Some(management.restore_disabled.is_none()),
                    status_kind: None,
                    action_disabled: management.restore_disabled.clone(),
                    style: None,
                });
                elements.push(protocol::ElementInfo {
                    semantic_id: "button:theme-chooser-gradient-cycle".to_string(),
                    element_type: protocol::ElementType::Button,
                    text: Some("Gradient".to_string()),
                    value: Some("theme_chooser_gradient_cycle".to_string()),
                    content: None,
                    selected: self
                        .theme
                        .active_background_gradient()
                        .is_some()
                        .then_some(true),
                    focused: None,
                    index: None,
                    role: Some("theme-action".to_string()),
                    kind: Some("gradient-cycle".to_string()),
                    source: None,
                    source_name: None,
                    selectable: Some(true),
                    status_kind: None,
                    action_disabled: None,
                    style: None,
                });
                let gradient_layer_count = self
                    .theme
                    .background_gradient
                    .as_ref()
                    .map(|gradient| gradient.layers.len())
                    .unwrap_or(0);
                elements.push(protocol::ElementInfo {
                    semantic_id: "button:theme-chooser-gradient-layer-add".to_string(),
                    element_type: protocol::ElementType::Button,
                    text: Some("Add Layer".to_string()),
                    value: Some("theme_chooser_gradient_layer_add".to_string()),
                    content: None,
                    selected: None,
                    focused: None,
                    index: None,
                    role: Some("theme-action".to_string()),
                    kind: Some("gradient-layer-add".to_string()),
                    source: None,
                    source_name: None,
                    selectable: Some(true),
                    status_kind: None,
                    action_disabled: None,
                    style: None,
                });
                elements.push(protocol::ElementInfo {
                    semantic_id: "button:theme-chooser-gradient-layer-remove".to_string(),
                    element_type: protocol::ElementType::Button,
                    text: Some("Remove Layer".to_string()),
                    value: Some("theme_chooser_gradient_layer_remove".to_string()),
                    content: None,
                    selected: None,
                    focused: None,
                    index: None,
                    role: Some("theme-action".to_string()),
                    kind: Some("gradient-layer-remove".to_string()),
                    source: None,
                    source_name: None,
                    selectable: Some(gradient_layer_count > 0),
                    status_kind: None,
                    action_disabled: (gradient_layer_count == 0)
                        .then_some("no_gradient_layers".to_string()),
                    style: None,
                });

                let opacity = self.theme.get_opacity();
                let fonts = self.theme.get_fonts();
                let gradient = self.theme.background_gradient.clone().unwrap_or_default();
                let vibrancy_enabled = self
                    .theme
                    .vibrancy
                    .as_ref()
                    .map(|vibrancy| vibrancy.enabled)
                    .unwrap_or(false);
                let mut push_theme_control =
                    |semantic_id: String,
                     element_type: protocol::ElementType,
                     text: &str,
                     value: String,
                     kind: &str| {
                        elements.push(protocol::ElementInfo {
                            semantic_id,
                            element_type,
                            text: Some(text.to_string()),
                            value: Some(value),
                            content: None,
                            selected: None,
                            focused: None,
                            index: None,
                            role: Some("theme-control".to_string()),
                            kind: Some(kind.to_string()),
                            source: None,
                            source_name: None,
                            selectable: Some(true),
                            status_kind: None,
                            action_disabled: None,
                            style: None,
                        });
                    };
                push_theme_control(
                    "control:theme-chooser:accent-color".to_string(),
                    protocol::ElementType::ColorPicker,
                    "Accent Color",
                    format!("#{:06X}", self.theme.colors.accent.selected),
                    "accent-color",
                );
                push_theme_control(
                    "control:theme-chooser:accent-color-hex".to_string(),
                    protocol::ElementType::Input,
                    "Accent Color Hex",
                    format!("#{:06X}", self.theme.colors.accent.selected),
                    "accent-color-hex",
                );
                push_theme_control(
                    "control:theme-chooser:background-color".to_string(),
                    protocol::ElementType::ColorPicker,
                    "Background Color",
                    format!("#{:06X}", self.theme.colors.background.main),
                    "background-color",
                );
                push_theme_control(
                    "control:theme-chooser:background-color-hex".to_string(),
                    protocol::ElementType::Input,
                    "Background Color Hex",
                    format!("#{:06X}", self.theme.colors.background.main),
                    "background-color-hex",
                );
                push_theme_control(
                    "control:theme-chooser:surface-opacity".to_string(),
                    protocol::ElementType::Slider,
                    "Surface Opacity",
                    format!("{:.2}", opacity.main),
                    "surface-opacity",
                );
                if crate::platform::tahoe_liquid_glass_available() {
                    push_theme_control(
                        "control:theme-chooser:glass-veil-opacity".to_string(),
                        protocol::ElementType::Slider,
                        "Glass Veil Opacity",
                        format!(
                            "{:.2}",
                            opacity
                                .glass_veil_opacity
                                .unwrap_or(crate::theme::opacity::OPACITY_GLASS_MODE_VEIL_CAP)
                        ),
                        "glass-veil-opacity",
                    );
                    push_theme_control(
                        "control:theme-chooser:glass-tint-opacity".to_string(),
                        protocol::ElementType::Slider,
                        "Glass Tint Opacity",
                        format!("{:.2}", opacity.glass_tint_opacity.unwrap_or(0.0)),
                        "glass-tint-opacity",
                    );
                    push_theme_control(
                        "control:theme-chooser:glass-morph-duration".to_string(),
                        protocol::ElementType::Slider,
                        "Glass Morph Duration",
                        format!(
                            "{:.2}",
                            opacity
                                .glass_morph_duration
                                .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION)
                        ),
                        "glass-morph-duration",
                    );
                    push_theme_control(
                        "control:theme-chooser:glass-morph-inset".to_string(),
                        protocol::ElementType::Slider,
                        "Glass Morph Inset",
                        format!(
                            "{:.2}",
                            opacity
                                .glass_morph_inset
                                .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_INSET)
                        ),
                        "glass-morph-inset",
                    );
                }
                push_theme_control(
                    "control:theme-chooser:secondary-text-opacity".to_string(),
                    protocol::ElementType::Slider,
                    "Typography Hint Opacity",
                    format!("{:.2}", opacity.text_placeholder),
                    "secondary-text-opacity",
                );
                push_theme_control(
                    "control:theme-chooser:focused-background-opacity".to_string(),
                    protocol::ElementType::Slider,
                    "Focused Row Opacity",
                    format!("{:.2}", opacity.selected),
                    "focused-background-opacity",
                );
                push_theme_control(
                    "control:theme-chooser:vibrancy-enabled".to_string(),
                    protocol::ElementType::Toggle,
                    "Vibrancy",
                    vibrancy_enabled.to_string(),
                    "vibrancy-enabled",
                );
                push_theme_control(
                    "control:theme-chooser:vibrancy-material".to_string(),
                    protocol::ElementType::Input,
                    "Vibrancy Material",
                    format!("{:?}", self.theme.get_vibrancy().material).to_lowercase(),
                    "vibrancy-material",
                );
                push_theme_control(
                    "control:theme-chooser:appearance-mode".to_string(),
                    protocol::ElementType::Input,
                    "Appearance Mode",
                    format!("{:?}", self.theme.appearance).to_lowercase(),
                    "appearance-mode",
                );
                push_theme_control(
                    "control:theme-chooser:gradient-enabled".to_string(),
                    protocol::ElementType::Toggle,
                    "Backdrop Gradient",
                    gradient.enabled.to_string(),
                    "gradient-enabled",
                );
                push_theme_control(
                    "control:theme-chooser:gradient-base-from".to_string(),
                    protocol::ElementType::ColorPicker,
                    "Gradient Base From",
                    format!("#{:06X}", gradient.from),
                    "gradient-base-from",
                );
                push_theme_control(
                    "control:theme-chooser:gradient-base-from-hex".to_string(),
                    protocol::ElementType::Input,
                    "Gradient Base From Hex",
                    format!("#{:06X}", gradient.from),
                    "gradient-base-from-hex",
                );
                push_theme_control(
                    "control:theme-chooser:gradient-base-to".to_string(),
                    protocol::ElementType::ColorPicker,
                    "Gradient Base To",
                    format!("#{:06X}", gradient.to),
                    "gradient-base-to",
                );
                push_theme_control(
                    "control:theme-chooser:gradient-base-to-hex".to_string(),
                    protocol::ElementType::Input,
                    "Gradient Base To Hex",
                    format!("#{:06X}", gradient.to),
                    "gradient-base-to-hex",
                );
                push_theme_control(
                    "control:theme-chooser:gradient-base-angle".to_string(),
                    protocol::ElementType::Slider,
                    "Gradient Base Angle",
                    format!("{:.0}", gradient.angle),
                    "gradient-base-angle",
                );
                push_theme_control(
                    "control:theme-chooser:gradient-base-opacity".to_string(),
                    protocol::ElementType::Slider,
                    "Gradient Base Opacity",
                    format!("{:.2}", gradient.opacity),
                    "gradient-base-opacity",
                );
                push_theme_control(
                    "control:theme-chooser:ui-font-size".to_string(),
                    protocol::ElementType::Slider,
                    "UI Font Size",
                    format!("{:.1}", fonts.ui_size),
                    "ui-font-size",
                );
                for (layer_index, layer) in gradient.layers.iter().enumerate() {
                    let ordinal = layer_index + 1;
                    push_theme_control(
                        format!("control:theme-chooser:gradient-layer-{ordinal}-from"),
                        protocol::ElementType::ColorPicker,
                        &format!("Gradient Layer {ordinal} From"),
                        format!("#{:06X}", layer.from),
                        &format!("gradient-layer-{ordinal}-from"),
                    );
                    push_theme_control(
                        format!("control:theme-chooser:gradient-layer-{ordinal}-from-hex"),
                        protocol::ElementType::Input,
                        &format!("Gradient Layer {ordinal} From Hex"),
                        format!("#{:06X}", layer.from),
                        &format!("gradient-layer-{ordinal}-from-hex"),
                    );
                    push_theme_control(
                        format!("control:theme-chooser:gradient-layer-{ordinal}-to"),
                        protocol::ElementType::ColorPicker,
                        &format!("Gradient Layer {ordinal} To"),
                        format!("#{:06X}", layer.to),
                        &format!("gradient-layer-{ordinal}-to"),
                    );
                    push_theme_control(
                        format!("control:theme-chooser:gradient-layer-{ordinal}-to-hex"),
                        protocol::ElementType::Input,
                        &format!("Gradient Layer {ordinal} To Hex"),
                        format!("#{:06X}", layer.to),
                        &format!("gradient-layer-{ordinal}-to-hex"),
                    );
                    push_theme_control(
                        format!("control:theme-chooser:gradient-layer-{ordinal}-angle"),
                        protocol::ElementType::Slider,
                        &format!("Gradient Layer {ordinal} Angle"),
                        format!("{:.0}", layer.angle),
                        &format!("gradient-layer-{ordinal}-angle"),
                    );
                    push_theme_control(
                        format!("control:theme-chooser:gradient-layer-{ordinal}-opacity"),
                        protocol::ElementType::Slider,
                        &format!("Gradient Layer {ordinal} Opacity"),
                        format!("{:.2}", layer.opacity),
                        &format!("gradient-layer-{ordinal}-opacity"),
                    );
                }

                for (visible_index, catalog_index) in filtered.into_iter().enumerate() {
                    let Some(entry) = catalog.get(catalog_index) else {
                        continue;
                    };
                    let (semantic_id, source_kind, value) = match &entry.kind {
                        ThemeChooserCatalogKind::BuiltIn(index) => (
                            format!("theme-row-builtin:{index}"),
                            "built-in".to_string(),
                            index.to_string(),
                        ),
                        ThemeChooserCatalogKind::User { slug } => (
                            format!("theme-row-user:{slug}"),
                            "user".to_string(),
                            slug.clone(),
                        ),
                    };
                    elements.push(protocol::ElementInfo {
                        semantic_id,
                        element_type: protocol::ElementType::Choice,
                        text: Some(entry.name.clone()),
                        value: Some(value),
                        content: None,
                        selected: Some(visible_index == *selected_index),
                        focused: None,
                        index: Some(visible_index),
                        role: Some("theme-row".to_string()),
                        kind: Some(source_kind),
                        source: None,
                        source_name: None,
                        selectable: Some(true),
                        status_kind: None,
                        action_disabled: None,
                        style: None,
                    });
                }

                let total_count = elements.len();
                elements.truncate(limit);
                ElementCollectionOutcome::complete("themeChooser", elements, total_count)
            }

            AppView::ActionsDialog => {
                if let Some(ref dialog_entity) = self.actions_dialog {
                    let dialog = dialog_entity.read(cx);
                    let mut elements: Vec<protocol::ElementInfo> = Vec::new();

                    elements.push(protocol::ElementInfo::input(
                        "actions-search",
                        Some(&dialog.search_text),
                        !dialog.hide_search,
                    ));

                    let action_count = dialog.filtered_actions.len();
                    elements.push(protocol::ElementInfo::list("actions", action_count));

                    let selected_action_idx = dialog
                        .get_selected_filtered_index()
                        .and_then(|fi| dialog.filtered_actions.get(fi).copied());

                    for (filter_pos, &action_idx) in dialog.filtered_actions.iter().enumerate() {
                        if let Some(action) = dialog.actions.get(action_idx) {
                            let is_selected = selected_action_idx == Some(action_idx);
                            elements.push(protocol::ElementInfo::redacted_choice(
                                filter_pos,
                                &action.title,
                                &action.id,
                                is_selected,
                                protocol::ElementContentKind::ExternalContent,
                            ));
                        }
                    }

                    let total_count = elements.len();
                    if elements.len() > limit {
                        elements.truncate(limit);
                    }
                    ElementCollectionOutcome::complete("actionsDialog", elements, total_count)
                } else {
                    let total_count = 1;
                    let elements: Vec<protocol::ElementInfo> =
                        vec![protocol::ElementInfo::panel("actions-dialog")]
                            .into_iter()
                            .take(limit)
                            .collect();
                    ElementCollectionOutcome::partial(
                        "actionsDialog",
                        protocol::ProjectionReason::RuntimeEntityMissing,
                        elements,
                        total_count,
                    )
                    .with_warning("panel_only_actions_dialog")
                }
            }

            AppView::DivPrompt { .. } => {
                let total_count = 1;
                let elements: Vec<protocol::ElementInfo> =
                    vec![protocol::ElementInfo::panel("div-prompt")]
                        .into_iter()
                        .take(limit)
                        .collect();
                ElementCollectionOutcome::unsupported(
                    "divPrompt",
                    protocol::ProjectionReason::UnsupportedCustomDocument,
                    elements,
                    total_count,
                )
                .with_warning("panel_only_div_prompt")
            }

            AppView::FormPrompt { entity, .. } => {
                let form = entity.read(cx);
                let (elements, total_count) = self.collect_form_prompt_elements(form, limit, cx);
                let surface_id = format!("{}-prompt", form.semantic_prefix());
                Self::finalize_surface_outcome(
                    surface_id.as_str(),
                    surface_id.as_str(),
                    "panel_only_form_prompt",
                    limit,
                    elements,
                    total_count,
                )
            }

            AppView::TermPrompt { entity, .. } => {
                let term = entity.read(cx);
                let (elements, total_count) =
                    self.collect_term_prompt_elements(term, "term", limit);
                Self::finalize_surface_outcome(
                    "term-prompt",
                    "term-prompt",
                    "panel_only_term_prompt",
                    limit,
                    elements,
                    total_count,
                )
            }

            AppView::EditorPrompt { entity, .. } => {
                let editor = entity.read(cx);
                let (elements, total_count) =
                    self.collect_editor_prompt_elements(editor, "editor", limit);
                Self::finalize_surface_outcome(
                    "editor-prompt",
                    "editor-prompt",
                    "panel_only_editor_prompt",
                    limit,
                    elements,
                    total_count,
                )
            }

            AppView::SelectPrompt { entity, .. } => ElementCollectionOutcome::complete_from(
                "selectPrompt",
                entity.read(cx).collect_elements(limit),
            ),

            AppView::PathPrompt { entity, .. } => {
                let path_prompt = entity.read(cx);
                let (elements, total_count) = self.collect_path_prompt_elements(path_prompt, limit);
                Self::finalize_surface_outcome(
                    "path-prompt",
                    "path-prompt",
                    "panel_only_path_prompt",
                    limit,
                    elements,
                    total_count,
                )
            }

            AppView::ChatPrompt { entity, .. } => {
                let chat = entity.read(cx);
                let (elements, total_count) = self.collect_chat_prompt_elements(chat, limit);
                Self::finalize_surface_outcome(
                    "chat-prompt",
                    "chat-prompt",
                    "panel_only_chat_prompt",
                    limit,
                    elements,
                    total_count,
                )
            }

            AppView::EnvPrompt { entity, .. } => {
                let env_prompt = entity.read(cx);
                let (elements, total_count) = self.collect_env_prompt_elements(env_prompt, limit);
                Self::finalize_surface_outcome(
                    "env-prompt",
                    "env-prompt",
                    "panel_only_env_prompt",
                    limit,
                    elements,
                    total_count,
                )
            }

            AppView::DropPrompt { entity, .. } => {
                let drop_prompt = entity.read(cx);
                let (elements, total_count) = self.collect_drop_prompt_elements(drop_prompt, limit);
                Self::finalize_surface_outcome(
                    "drop-prompt",
                    "drop-prompt",
                    "panel_only_drop_prompt",
                    limit,
                    elements,
                    total_count,
                )
            }

            AppView::TemplatePrompt { entity, .. } => {
                let template_prompt = entity.read(cx);
                let (elements, total_count) =
                    self.collect_template_prompt_elements(template_prompt, limit);
                Self::finalize_surface_outcome(
                    "template-prompt",
                    "template-prompt",
                    "panel_only_template_prompt",
                    limit,
                    elements,
                    total_count,
                )
            }

            AppView::HotkeyPrompt { entity, .. } => {
                let hotkey_prompt = entity.read(cx);
                let (elements, total_count) =
                    self.collect_hotkey_prompt_elements(hotkey_prompt, limit);
                Self::finalize_surface_outcome(
                    "hotkey-prompt",
                    "hotkey-prompt",
                    "panel_only_hotkey_prompt",
                    limit,
                    elements,
                    total_count,
                )
            }

            AppView::NamingPrompt { entity, .. } => {
                let naming_prompt = entity.read(cx);
                let (elements, total_count) =
                    self.collect_naming_prompt_elements(naming_prompt, limit);
                Self::finalize_surface_outcome(
                    "naming-prompt",
                    "naming-prompt",
                    "panel_only_naming_prompt",
                    limit,
                    elements,
                    total_count,
                )
            }

            AppView::CreationFeedback { payload } => {
                let mut all_elements = Vec::new();
                let mut field = |semantic_id: &str,
                                 element_type: protocol::ElementType,
                                 text: Option<String>,
                                 value: Option<String>,
                                 role: Option<&str>,
                                 kind: Option<&str>,
                                 status_kind: Option<&str>,
                                 action_disabled: Option<&str>| {
                    all_elements.push(protocol::ElementInfo {
                        semantic_id: semantic_id.to_string(),
                        element_type,
                        text,
                        value,
                        content: None,
                        selected: None,
                        focused: None,
                        index: None,
                        role: role.map(str::to_string),
                        kind: kind.map(str::to_string),
                        source: None,
                        source_name: None,
                        selectable: Some(false),
                        status_kind: status_kind.map(str::to_string),
                        action_disabled: action_disabled.map(str::to_string),
                        style: None,
                    });
                };

                field(
                    "creation-feedback:panel",
                    protocol::ElementType::Panel,
                    Some("Creation feedback".to_string()),
                    None,
                    Some("panel"),
                    Some("creation_feedback"),
                    None,
                    None,
                );
                field(
                    "creation-feedback:artifact-kind",
                    protocol::ElementType::Panel,
                    Some(payload.artifact_kind_label().to_string()),
                    Some(payload.artifact_kind.kind().to_string()),
                    Some("status"),
                    Some("artifact_kind"),
                    Some(payload.artifact_kind.kind()),
                    None,
                );
                field(
                    "creation-feedback:artifact-path",
                    protocol::ElementType::Input,
                    Some("Artifact path".to_string()),
                    Some(payload.artifact_path.to_string_lossy().to_string()),
                    Some("readonly_path"),
                    Some("artifact_path"),
                    None,
                    None,
                );
                field(
                    "creation-feedback:verification-status",
                    protocol::ElementType::Panel,
                    Some(payload.verification_status_label().to_string()),
                    Some(payload.verification_status_kind().to_string()),
                    Some("status"),
                    Some("verification_status"),
                    Some(payload.verification_status_kind()),
                    None,
                );
                field(
                    "creation-feedback:receipt-status",
                    protocol::ElementType::Panel,
                    Some(payload.receipt_status_label().to_string()),
                    Some(payload.receipt_status_kind().to_string()),
                    Some("status"),
                    Some("receipt_status"),
                    Some(payload.receipt_status_kind()),
                    None,
                );
                field(
                    "creation-feedback:receipt-path",
                    protocol::ElementType::Input,
                    Some("Receipt path".to_string()),
                    Some(payload.receipt_path_text().to_string()),
                    Some("readonly_path"),
                    Some("receipt_path"),
                    Some(payload.receipt_status_kind()),
                    None,
                );

                for (index, semantic_id, label, disabled) in [
                    (
                        0,
                        "button:creation-feedback:reveal-artifact",
                        "Reveal in Finder",
                        None,
                    ),
                    (
                        1,
                        "button:creation-feedback:copy-artifact-path",
                        "Copy Path",
                        None,
                    ),
                    (2, "button:creation-feedback:edit-artifact", "Edit", None),
                    (
                        3,
                        "button:creation-feedback:run-artifact",
                        "Run",
                        Some(payload.run_disabled_reason()),
                    ),
                    (
                        4,
                        "button:creation-feedback:copy-receipt-path",
                        "Copy Receipt Path",
                        payload
                            .receipt_path()
                            .is_none()
                            .then_some("receipt_not_applicable"),
                    ),
                    (
                        5,
                        "button:creation-feedback:open-receipt",
                        "Open Receipt",
                        payload
                            .receipt_path()
                            .is_none()
                            .then_some("receipt_not_applicable"),
                    ),
                ] {
                    all_elements.push(protocol::ElementInfo {
                        semantic_id: semantic_id.to_string(),
                        element_type: protocol::ElementType::Button,
                        text: Some(label.to_string()),
                        value: None,
                        content: None,
                        selected: None,
                        focused: None,
                        index: Some(index),
                        role: Some("action".to_string()),
                        kind: Some("creation_feedback_action".to_string()),
                        source: None,
                        source_name: None,
                        selectable: Some(disabled.is_none()),
                        status_kind: disabled.map(str::to_string),
                        action_disabled: disabled.map(str::to_string),
                        style: None,
                    });
                }

                let total_count = all_elements.len();
                let elements = all_elements.into_iter().take(limit).collect();
                ElementCollectionOutcome::complete("creationFeedback", elements, total_count)
            }

            AppView::ConfirmPrompt {
                options,
                focused_button,
                ..
            } => {
                let confirm_selected = matches!(focused_button, ConfirmFocusedButton::Confirm);
                let cancel_selected = matches!(focused_button, ConfirmFocusedButton::Cancel);
                let mut confirm_button =
                    protocol::ElementInfo::button(0, options.confirm_text.as_ref());
                confirm_button.selected = Some(confirm_selected);
                confirm_button.role = Some("footer".to_string());
                confirm_button.kind = Some("confirm".to_string());
                confirm_button.selectable = Some(true);
                let mut cancel_button =
                    protocol::ElementInfo::button(1, options.cancel_text.as_ref());
                cancel_button.selected = Some(cancel_selected);
                cancel_button.role = Some("footer".to_string());
                cancel_button.kind = Some("cancel".to_string());
                cancel_button.selectable = Some(true);

                let elements: Vec<protocol::ElementInfo> = vec![
                    protocol::ElementInfo::panel("confirm-prompt"),
                    confirm_button,
                    cancel_button,
                ]
                .into_iter()
                .take(limit)
                .collect();
                ElementCollectionOutcome::complete("confirmPrompt", elements, 3)
            }

            AppView::WebcamView { .. } => {
                let total_count = 1;
                let elements: Vec<protocol::ElementInfo> =
                    vec![protocol::ElementInfo::panel("webcam")]
                        .into_iter()
                        .take(limit)
                        .collect();
                ElementCollectionOutcome::partial(
                    "webcam",
                    protocol::ProjectionReason::SemanticControlsUnavailable,
                    elements,
                    total_count,
                )
                .with_warning("panel_only_webcam")
            }

            AppView::ScratchPadView { entity, .. } => {
                let editor = entity.read(cx);
                let (elements, total_count) =
                    self.collect_editor_prompt_elements(editor, "scratch-pad", limit);
                Self::finalize_surface_outcome(
                    "scratch-pad",
                    "scratch-pad",
                    "panel_only_scratch_pad",
                    limit,
                    elements,
                    total_count,
                )
            }

            AppView::QuickTerminalView { entity } => {
                let term = entity.read(cx);
                let (elements, total_count) =
                    self.collect_term_prompt_elements(term, "quick-terminal", limit);
                Self::finalize_surface_outcome(
                    "quick-terminal",
                    "quick-terminal",
                    "panel_only_quick_terminal",
                    limit,
                    elements,
                    total_count,
                )
            }

            AppView::FlowSessionView { session_id } => {
                let entity = self
                    .conversations.flow_sessions
                    .iter()
                    .find(|(meta, _)| meta.id == *session_id)
                    .map(|(_, entity)| entity.clone());
                if let Some(entity) = entity {
                    let chat = entity.read(cx);
                    let (mut elements, mut total_count) =
                        self.collect_chat_prompt_elements(chat, limit);
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
                        .find(|(meta, _)| meta.id == *session_id)
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
                        .conversations.flow_sessions
                        .iter()
                        .map(|(meta, _)| meta)
                        .find(|meta| meta.id == *session_id)
                    {
                        let snapshot = crate::flows::session::FlowSessionIdentitySnapshot::from_meta(meta);
                        let identity = crate::components::main_view_chrome::SemanticChipSpec::enabled_identity(
                            "flow-session:identity",
                            format!(
                                "{} · {} · {}",
                                snapshot.friendly_name,
                                snapshot.engine,
                                if snapshot.read_only { "Archived" } else { "Active" }
                            ),
                            crate::components::main_view_chrome::SemanticChipAction::OpenDetails,
                            "⌘K",
                        );
                        elements.push(
                            crate::windows::automation_surface_collector::collect_semantic_chip_element(&identity),
                        );
                        let fact_specs = [
                            ("flow-session:friendly-name", snapshot.friendly_name.clone()),
                            ("flow-session:engine", snapshot.engine.clone()),
                            (
                                "flow-session:model",
                                snapshot.model.clone().unwrap_or_else(|| "Model unavailable".into()),
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
                            let spec = crate::components::main_view_chrome::SemanticChipSpec::disabled_identity(
                                semantic_id,
                                label,
                                "Read-only Flow session fact",
                            );
                            elements.push(
                                crate::windows::automation_surface_collector::collect_semantic_chip_element(&spec),
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

            AppView::About { .. } => ElementCollectionOutcome::partial(
                "about",
                protocol::ProjectionReason::CollectorUnavailable,
                vec![protocol::ElementInfo::panel("about")]
                    .into_iter()
                    .take(limit)
                    .collect(),
                1,
            )
            .with_warning("panel_only_about"),

            AppView::CreateAiPresetView { .. } => ElementCollectionOutcome::partial(
                "createAiPreset",
                protocol::ProjectionReason::SemanticControlsUnavailable,
                vec![protocol::ElementInfo::panel("create-ai-preset")]
                    .into_iter()
                    .take(limit)
                    .collect(),
                1,
            )
            .with_warning("panel_only_create_ai_preset"),

            AppView::ScriptIssuesView { .. } => ElementCollectionOutcome::partial(
                "scriptIssues",
                protocol::ProjectionReason::CollectorUnavailable,
                vec![protocol::ElementInfo::panel("script-issues")]
                    .into_iter()
                    .take(limit)
                    .collect(),
                1,
            )
            .with_warning("panel_only_script_issues"),
        };

        if include_headers {
            let leading = match &self.current_view {
                AppView::AppLauncherView { filter, .. } => Some((
                    "app-launcher",
                    if filter.trim().is_empty() {
                        "Apps"
                    } else {
                        "Results"
                    },
                    None,
                )),
                AppView::ClipboardHistoryView { filter, .. } => Some((
                    "clipboard-history",
                    if filter.trim().is_empty() {
                        "Clipboard"
                    } else {
                        "Results"
                    },
                    None,
                )),
                AppView::DictationHistoryView { filter, .. } => Some((
                    "dictation-history",
                    if filter.trim().is_empty() {
                        "Transcripts"
                    } else {
                        "Results"
                    },
                    None,
                )),
                AppView::EmojiPickerView { filter, .. } => Some((
                    "emoji-picker",
                    if filter.trim().is_empty() {
                        "Emoji"
                    } else {
                        "Results"
                    },
                    None,
                )),
                AppView::FileSearchView { query, .. } => Some((
                    "file-search",
                    if query.trim().is_empty() {
                        "Files"
                    } else {
                        "Results"
                    },
                    self.file_search_loading.then_some("Indexing files"),
                )),
                AppView::SettingsView { filter, .. } => Some((
                    "settings",
                    if filter.trim().is_empty() {
                        "Settings"
                    } else {
                        "Results"
                    },
                    None,
                )),
                AppView::ThemeChooserView { filter, .. } => Some((
                    "theme-chooser",
                    if filter.trim().is_empty() {
                        "Themes"
                    } else {
                        "Results"
                    },
                    None,
                )),
                _ => None,
            };
            if let Some((surface, label, status)) = leading {
                outcome.total_count += 1;
                if limit > 0 {
                    outcome.elements.insert(
                        0,
                        crate::components::builtin_leading_separator::builtin_leading_separator_element(
                            surface, label, status,
                        ),
                    );
                    outcome.elements.truncate(limit);
                }
            }
        }

        self.append_footer_elements(&mut outcome, limit, cx);
        outcome
    }

    fn append_footer_elements(
        &self,
        outcome: &mut ElementCollectionOutcome,
        limit: usize,
        cx: &Context<Self>,
    ) {
        let footer = self.active_footer_snapshot(cx);
        let row_kind = match footer.owner.as_str() {
            "native" => Some("nativeFooterRow"),
            "prompt" => Some("promptFooterRow"),
            "popup" => Some("popupFooterRow"),
            "content" => Some("contentFooterRow"),
            _ => None,
        };
        let Some(row_kind) = row_kind else {
            return;
        };

        outcome.total_count += 1 + footer.buttons.len();

        if outcome.elements.len() >= limit {
            outcome
                .warnings
                .push("footer_elements_truncated_by_limit".to_string());
            return;
        }

        outcome.elements.push(protocol::ElementInfo {
            semantic_id: format!("footer:{}:row", footer.owner),
            element_type: protocol::ElementType::Panel,
            text: Some(footer.owner.clone()),
            value: footer.expected_surface.clone(),
            content: None,
            selected: None,
            focused: None,
            index: None,
            role: Some("footer".to_string()),
            kind: Some(row_kind.to_string()),
            source: None,
            source_name: None,
            selectable: Some(false),
            status_kind: footer.mismatch.clone(),
            action_disabled: None,
            style: None,
        });

        for (index, button) in footer.buttons.iter().enumerate() {
            if outcome.elements.len() >= limit {
                outcome
                    .warnings
                    .push("footer_elements_truncated_by_limit".to_string());
                break;
            }

            let kind = match footer.owner.as_str() {
                "native" => "nativeFooterButton",
                "prompt" => "promptFooterButton",
                "popup" => "popupFooterButton",
                _ => "contentFooterButton",
            };
            let text = if button.shortcut_routable {
                format!("{} {}", button.key, button.label)
            } else {
                button.label.clone()
            };
            outcome.elements.push(protocol::ElementInfo {
                semantic_id: button.id.clone(),
                element_type: protocol::ElementType::Button,
                text: Some(text),
                value: Some(button.action.clone()),
                content: None,
                selected: Some(button.selected),
                focused: None,
                index: Some(index),
                role: Some("footer".to_string()),
                kind: Some(kind.to_string()),
                source: footer.expected_surface.clone(),
                source_name: footer.requested_surface.clone(),
                selectable: Some(button.enabled),
                status_kind: button.action_disabled.clone(),
                action_disabled: button.action_disabled.clone(),
                style: None,
            });
        }
    }

    fn collect_choice_view_elements(
        &self,
        input_name: &str,
        input_value: String,
        choices: &[Choice],
        selected_index: usize,
        limit: usize,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let filtered = self.get_filtered_arg_choices(choices);
        let total_count = filtered.len() + 2;

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
            protocol::ElementInfo::list("choices", filtered.len()),
        );

        for (display_index, choice) in filtered.iter().enumerate() {
            if elements.len() >= limit {
                break;
            }
            elements.push(Self::keyed_choice_element(
                display_index,
                choice,
                display_index == selected_index,
            ));
        }

        (elements, total_count)
    }

    fn collect_notes_browse_elements(
        &self,
        search: &crate::notes::search_model::NoteSearchHostState,
        limit: usize,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let rows = search.state.rows();
        let total_count = rows.len().saturating_mul(2) + 4;
        let mut elements = Vec::with_capacity(limit.min(total_count));

        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo::input(
                "notes-browse-filter",
                Some(search.query.as_str()),
                self.focused_input != FocusedInput::None,
            ),
        );
        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo::list("notes", rows.len()),
        );
        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo {
                semantic_id: "notes-search-state".to_string(),
                element_type: protocol::ElementType::Panel,
                text: Some(match search.state.kind() {
                    "failed" => "Notes couldn’t be loaded".to_string(),
                    "loading" => "Loading notes".to_string(),
                    "readyEmpty" => "No notes yet".to_string(),
                    "noMatch" => "No matching notes".to_string(),
                    _ => "Notes ready".to_string(),
                }),
                value: Some(search.state.generation().to_string()),
                content: None,
                selected: None,
                focused: None,
                index: None,
                role: Some("status".to_string()),
                kind: Some("noteSearchState".to_string()),
                source: Some("notes".to_string()),
                source_name: None,
                selectable: Some(false),
                status_kind: Some(search.state.kind().to_string()),
                action_disabled: None,
                style: None,
            },
        );
        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo {
                semantic_id: search.destination.semantic_action().to_string(),
                element_type: protocol::ElementType::Button,
                text: Some(search.destination.primary_verb().to_string()),
                value: search.selected_id.map(|id| id.stable_id()),
                content: None,
                selected: None,
                focused: None,
                index: None,
                role: Some("action".to_string()),
                kind: Some(search.destination.as_str().to_string()),
                source: Some("notes".to_string()),
                source_name: Some("Notes search".to_string()),
                selectable: Some(search.selected_row().is_some()),
                status_kind: None,
                action_disabled: search
                    .selected_row()
                    .is_none()
                    .then(|| "Select a note first.".to_string()),
                style: None,
            },
        );

        for (index, row) in rows.iter().enumerate() {
            if elements.len() >= limit {
                break;
            }
            elements.push(
                protocol::ElementInfo {
                    semantic_id: row.semantic_id(),
                    element_type: protocol::ElementType::Choice,
                    text: Some(row.title.clone()),
                    value: Some(row.stable_id()),
                    content: None,
                    selected: Some(search.selected_id == Some(row.id)),
                    focused: None,
                    index: Some(index),
                    role: Some("result".to_string()),
                    kind: Some(row.kind.as_str().to_string()),
                    source: Some("notes".to_string()),
                    source_name: Some(search.destination.primary_verb().to_string()),
                    selectable: Some(true),
                    status_kind: None,
                    action_disabled: None,
                    style: None,
                }
                .redact_text(protocol::ElementContentKind::UserContent),
            );
            Self::push_limited_element(
                &mut elements,
                limit,
                protocol::ElementInfo {
                    semantic_id: format!("{}:metadata", row.semantic_id()),
                    element_type: protocol::ElementType::Panel,
                    text: Some(row.preview.clone()),
                    value: Some(row.automation_metadata()),
                    content: None,
                    selected: None,
                    focused: None,
                    index: Some(index),
                    role: Some("resultMetadata".to_string()),
                    kind: Some(row.kind.as_str().to_string()),
                    source: Some("notes".to_string()),
                    source_name: None,
                    selectable: Some(false),
                    status_kind: None,
                    action_disabled: None,
                    style: None,
                }
                .redact_content(protocol::ElementContentKind::UserContent),
            );
        }

        (elements, total_count)
    }

    fn collect_named_rows(
        &self,
        input_name: &str,
        input_value: String,
        list_name: &str,
        rows: &[String],
        selected_index: usize,
        limit: usize,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let total_count = rows.len() + 2;

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

        let content_kind = match list_name {
            "clipboard-history" | "agent_chat-history" | "dictation-history" => {
                Some(protocol::ElementContentKind::UserContent)
            }
            "windows" | "browser-tabs" | "browser-history" | "processes"
            | "kit-results" | "installed-kits" => {
                Some(protocol::ElementContentKind::ExternalContent)
            }
            "file-results" | "migrate-v1-results" => {
                Some(protocol::ElementContentKind::FilePath)
            }
            "apps" | "settings" | "menu-commands" | "sdk-functions" | "tips"
            | "script-templates" | "emoji-results" => None,
            _ => Some(protocol::ElementContentKind::ExternalContent),
        };

        for (index, row) in rows.iter().enumerate() {
            if elements.len() >= limit {
                break;
            }
            let element = if let Some(content_kind) = content_kind {
                protocol::ElementInfo::redacted_choice(
                    index,
                    row,
                    row,
                    index == selected_index,
                    content_kind,
                )
            } else {
                protocol::ElementInfo::product_static_choice(index, row, row, index == selected_index)
            };
            elements.push(element);
        }

        (elements, total_count)
    }

    fn collect_profile_search_elements(
        &self,
        filter: &str,
        selected_index: usize,
        limit: usize,
    ) -> ElementCollectionOutcome {
        let results = self.profile_search_results_for_filter(filter);
        let selected_index = selected_index.min(results.len().saturating_sub(1));
        let selected_result = results.get(selected_index);
        let current_result = results
            .iter()
            .find(|result| result.selected)
            .cloned()
            .or_else(|| {
                self.profile_search_results_for_filter("")
                    .into_iter()
                    .find(|result| result.selected)
            });

        let preview_count = if selected_result.is_some() { 10 } else { 1 };
        let current_count = usize::from(current_result.is_some());
        let total_count = 2 + results.len() + current_count + preview_count;
        let mut elements = Vec::with_capacity(limit.min(total_count));
        let mut truncated = false;

        truncated |= !Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo {
                semantic_id: "input:profile-search-input".to_string(),
                element_type: protocol::ElementType::Input,
                text: Some("Search profiles".to_string()),
                value: Some(filter.to_string()),
                content: None,
                selected: None,
                focused: Some(matches!(self.focused_input, FocusedInput::MainFilter)),
                index: None,
                role: Some("searchbox".to_string()),
                kind: Some("profileSearchInput".to_string()),
                source: Some("profileSearch".to_string()),
                source_name: Some("Profile Search".to_string()),
                selectable: Some(true),
                status_kind: None,
                action_disabled: None,
                style: None,
            },
        );

        truncated |= !Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo {
                semantic_id: "list:profile-search-results".to_string(),
                element_type: protocol::ElementType::List,
                text: Some("Profile Search Results".to_string()),
                value: Some(results.len().to_string()),
                content: None,
                selected: None,
                focused: None,
                index: None,
                role: Some("listbox".to_string()),
                kind: Some("profileSearchResults".to_string()),
                source: Some("profileSearch".to_string()),
                source_name: Some("Profile Search".to_string()),
                selectable: Some(false),
                status_kind: None,
                action_disabled: None,
                style: None,
            },
        );

        if let Some(current) = current_result.as_ref() {
            truncated |= !Self::push_limited_element(
                &mut elements,
                limit,
                protocol::ElementInfo {
                    semantic_id: "status:profile-search-current".to_string(),
                    element_type: protocol::ElementType::Panel,
                    text: Some(current.profile.name.clone()),
                    value: Some(current.profile.id.clone()),
                    content: None,
                    selected: None,
                    focused: None,
                    index: None,
                    role: Some("status".to_string()),
                    kind: Some("profileSearchCurrent".to_string()),
                    source: Some("profileSearch".to_string()),
                    source_name: Some("Current Profile".to_string()),
                    selectable: Some(false),
                    status_kind: Some("current".to_string()),
                    action_disabled: None,
                    style: None,
                },
            );
        }

        for (index, result) in results.iter().enumerate() {
            truncated |= !Self::push_limited_element(
                &mut elements,
                limit,
                protocol::ElementInfo {
                    semantic_id: format!("profile-search-row:{}", result.profile.id),
                    element_type: protocol::ElementType::Choice,
                    text: Some(result.profile.name.clone()),
                    value: Some(result.profile.id.clone()),
                    content: None,
                    selected: Some(index == selected_index),
                    focused: None,
                    index: Some(index),
                    role: Some("option".to_string()),
                    kind: Some("profileSearchRow".to_string()),
                    source: Some("profileSearch".to_string()),
                    source_name: Some(
                        crate::profile_search::source_label(result.profile.source).to_string(),
                    ),
                    selectable: Some(true),
                    status_kind: match (result.selected, result.quick_ai) {
                        (true, true) => Some("current+quick-ai".to_string()),
                        (true, false) => Some("current".to_string()),
                        (false, true) => Some("quick-ai".to_string()),
                        (false, false) => None,
                    },
                    action_disabled: None,
                    style: None,
                },
            );
        }

        if let Some(result) = selected_result {
            let profile = &result.profile;
            let cwd_value = profile
                .cwd
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "Default".to_string());
            let preview_fields = [
                (
                    "profile-search-preview",
                    protocol::ElementType::Panel,
                    Some(profile.name.clone()),
                    Some(profile.id.clone()),
                    Some("region".to_string()),
                    Some("profileSearchPreview".to_string()),
                    Some("selected".to_string()),
                ),
                (
                    "profile-search-preview-title",
                    protocol::ElementType::Panel,
                    Some(profile.name.clone()),
                    Some(profile.id.clone()),
                    Some("heading".to_string()),
                    Some("profileSearchPreviewTitle".to_string()),
                    None,
                ),
                (
                    "profile-search-preview-explanation",
                    protocol::ElementType::Panel,
                    Some("What profiles do".to_string()),
                    Some(crate::profile_search::profile_preview_explanation().to_string()),
                    Some("document".to_string()),
                    Some("profileSearchPreviewExplanation".to_string()),
                    None,
                ),
                (
                    "profile-search-preview-overview",
                    protocol::ElementType::Panel,
                    Some("Overview".to_string()),
                    Some(format!(
                        "{} · {} · {}",
                        crate::profile_search::source_label(profile.source),
                        profile.id,
                        crate::profile_search::backend_label(profile.backend)
                    )),
                    Some("group".to_string()),
                    Some("profileSearchPreviewOverview".to_string()),
                    None,
                ),
                (
                    "profile-search-preview-runtime",
                    protocol::ElementType::Panel,
                    Some("Runtime Setup".to_string()),
                    Some("Model, tools, and working directory".to_string()),
                    Some("group".to_string()),
                    Some("profileSearchPreviewRuntime".to_string()),
                    None,
                ),
                (
                    "profile-search-preview-model",
                    protocol::ElementType::Panel,
                    Some("Model".to_string()),
                    Some(format!(
                        "{} ({})",
                        crate::profile_search::profile_model_label(profile),
                        crate::profile_search::backend_label(profile.backend)
                    )),
                    Some("metadata".to_string()),
                    Some("profileSearchPreviewModel".to_string()),
                    None,
                ),
                (
                    "profile-search-preview-cwd",
                    protocol::ElementType::Panel,
                    Some("CWD".to_string()),
                    Some(cwd_value),
                    Some("metadata".to_string()),
                    Some("profileSearchPreviewCwd".to_string()),
                    None,
                ),
                (
                    "profile-search-preview-tools",
                    protocol::ElementType::Panel,
                    Some("Tools".to_string()),
                    Some(crate::profile_search::profile_tools_label(profile)),
                    Some("metadata".to_string()),
                    Some("profileSearchPreviewTools".to_string()),
                    None,
                ),
                (
                    "profile-search-preview-instructions",
                    protocol::ElementType::Panel,
                    Some("Instructions".to_string()),
                    Some(crate::profile_search::profile_prompt_summary(profile)),
                    Some("group".to_string()),
                    Some("profileSearchPreviewInstructions".to_string()),
                    None,
                ),
                (
                    "profile-search-preview-prompt",
                    protocol::ElementType::Panel,
                    Some("Prompt".to_string()),
                    Some(crate::profile_search::profile_prompt_summary(profile)),
                    Some("document".to_string()),
                    Some("profileSearchPreviewPrompt".to_string()),
                    None,
                ),
            ];

            for (semantic_id, element_type, text, value, role, kind, status_kind) in preview_fields
            {
                truncated |= !Self::push_limited_element(
                    &mut elements,
                    limit,
                    protocol::ElementInfo {
                        semantic_id: semantic_id.to_string(),
                        element_type,
                        text,
                        value,
                        content: None,
                        selected: None,
                        focused: None,
                        index: None,
                        role,
                        kind,
                        source: Some("profileSearch".to_string()),
                        source_name: Some("Profile Search".to_string()),
                        selectable: Some(false),
                        status_kind,
                        action_disabled: None,
                        style: None,
                    },
                );
            }
        } else {
            truncated |= !Self::push_limited_element(
                &mut elements,
                limit,
                protocol::ElementInfo {
                    semantic_id: "panel:profile-search-empty".to_string(),
                    element_type: protocol::ElementType::Panel,
                    text: Some(if filter.trim().is_empty() {
                        "No profiles".to_string()
                    } else {
                        "No matching profiles".to_string()
                    }),
                    value: Some(filter.to_string()),
                    content: None,
                    selected: None,
                    focused: None,
                    index: None,
                    role: Some("empty-state".to_string()),
                    kind: Some("profileSearchEmpty".to_string()),
                    source: Some("profileSearch".to_string()),
                    source_name: Some("Profile Search".to_string()),
                    selectable: Some(false),
                    status_kind: Some("empty".to_string()),
                    action_disabled: None,
                    style: None,
                },
            );
        }

        if truncated {
            return ElementCollectionOutcome::complete("profileSearch", elements, total_count)
                .with_warning("profile_search_elements_truncated_by_limit");
        }
        ElementCollectionOutcome::complete("profileSearch", elements, total_count)
    }

    fn collect_filterable_rows_with_info_empty(
        &self,
        input_name: &str,
        input_value: String,
        list_name: &str,
        empty_state_id: &'static str,
        empty_text: &str,
        empty_icon_hint: &'static str,
        rows: &[String],
        selected_index: usize,
        limit: usize,
    ) -> (Vec<protocol::ElementInfo>, usize) {
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

    fn collect_form_prompt_elements(
        &self,
        form: &FormPromptState,
        limit: usize,
        cx: &Context<Self>,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let total_count = form.fields.len() + 1;
        let mut elements = Vec::with_capacity(limit.min(total_count));

        let semantic_prefix = form.semantic_prefix();
        let list_id = format!("{semantic_prefix}-fields");
        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo::list(list_id.as_str(), form.fields.len()),
        );

        for (index, (field, entity)) in form.fields.iter().enumerate() {
            if elements.len() >= limit {
                break;
            }

            let field_name = format!("{semantic_prefix}-{}", field.name);
            let field_label = field.label.clone().unwrap_or_else(|| field.name.clone());
            let focused = index == form.focused_index;

            let element = match entity {
                crate::form_prompt::FormFieldEntity::TextField(text_field) => {
                    let text_field = text_field.read(cx);
                    Self::input_element(
                        field_name.as_str(),
                        field_label,
                        Some(Self::preview_value(text_field.value(), 240)),
                        focused,
                        Some(index),
                    )
                }
                crate::form_prompt::FormFieldEntity::TextArea(text_area) => {
                    let text_area = text_area.read(cx);
                    Self::input_element(
                        field_name.as_str(),
                        field_label,
                        Some(Self::preview_value(text_area.value(), 240)),
                        focused,
                        Some(index),
                    )
                }
                crate::form_prompt::FormFieldEntity::Checkbox(checkbox) => {
                    let checkbox = checkbox.read(cx);
                    let value = if checkbox.is_checked() {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    };
                    protocol::ElementInfo {
                        semantic_id: protocol::generate_semantic_id_named(
                            "choice",
                            field_name.as_str(),
                        ),
                        element_type: protocol::ElementType::Choice,
                        text: Some(field_label),
                        value: Some(value),
                        content: None,
                        selected: Some(checkbox.is_checked()),
                        focused: Some(focused),
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
            };

            elements.push(element);
        }

        (elements, total_count)
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

    fn collect_path_prompt_elements(
        &self,
        path_prompt: &PathPrompt,
        limit: usize,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let total_count = path_prompt.filtered_entries.len() + 4;
        let mut elements = Vec::with_capacity(limit.min(total_count));

        Self::push_limited_element(
            &mut elements,
            limit,
            Self::input_element(
                "path-current-directory",
                "Current Directory",
                Some(path_prompt.current_path.clone()),
                false,
                Some(0),
            ),
        );
        Self::push_limited_element(
            &mut elements,
            limit,
            Self::input_element(
                "path-filter",
                "Filter",
                Some(path_prompt.filter_text.clone()),
                true,
                Some(1),
            ),
        );
        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo::list("path-entries", path_prompt.filtered_entries.len()),
        );
        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo {
                semantic_id: protocol::generate_semantic_id_named("panel", "path-status"),
                element_type: protocol::ElementType::Panel,
                text: Some(path_prompt.visible_status_message()),
                value: Some(path_prompt.automation_state()["status"].to_string()),
                content: None,
                selected: None,
                focused: None,
                index: Some(2),
                role: Some("status".to_string()),
                kind: Some("path_status".to_string()),
                source: None,
                source_name: None,
                selectable: Some(false),
                status_kind: Some(path_prompt.visible_status_kind().as_str().to_string()),
                action_disabled: None,
                style: None,
            },
        );

        for (index, entry) in path_prompt.filtered_entries.iter().enumerate() {
            if elements.len() >= limit {
                break;
            }
            let label = if entry.is_dir {
                format!("{}/", entry.name)
            } else {
                entry.name.clone()
            };
            let mut element = Self::choice_element(
                index,
                label,
                entry.path.clone(),
                index == path_prompt.selected_index,
            );
            element.kind = Some(if entry.is_symlink {
                "symlink".to_string()
            } else if entry.is_dir {
                "directory".to_string()
            } else {
                "file".to_string()
            });
            element.selectable = Some(true);
            elements.push(element);
        }

        (elements, total_count)
    }

    fn collect_env_prompt_elements(
        &self,
        env_prompt: &EnvPrompt,
        limit: usize,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let input_text = env_prompt.input_text();
        let display_value = if env_prompt.secret {
            if input_text.is_empty() {
                String::new()
            } else {
                "*".repeat(input_text.chars().count().clamp(1, 8))
            }
        } else {
            Self::preview_value(input_text, 240)
        };

        let mut total_count = 2;
        if env_prompt.exists_in_keyring {
            total_count += 1;
        }
        if env_prompt.secret_store_error.is_some() {
            total_count += 1;
        }

        let mut elements = Vec::with_capacity(limit.min(total_count));
        Self::push_limited_element(
            &mut elements,
            limit,
            Self::input_element(
                "env-key",
                env_prompt
                    .title
                    .clone()
                    .unwrap_or_else(|| env_prompt.key.clone()),
                Some(env_prompt.key.clone()),
                false,
                Some(0),
            ),
        );
        Self::push_limited_element(
            &mut elements,
            limit,
            Self::input_element(
                "env-value",
                env_prompt
                    .prompt
                    .clone()
                    .unwrap_or_else(|| "Value".to_string()),
                Some(display_value),
                true,
                Some(1),
            ),
        );

        if env_prompt.exists_in_keyring {
            Self::push_limited_element(
                &mut elements,
                limit,
                protocol::ElementInfo {
                    semantic_id: protocol::generate_semantic_id_named(
                        "choice",
                        "env-keyring-status",
                    ),
                    element_type: protocol::ElementType::Choice,
                    text: Some("Stored Secret".to_string()),
                    value: Some("present".to_string()),
                    content: None,
                    selected: Some(true),
                    focused: None,
                    index: Some(2),
                    role: None,
                    kind: None,
                    source: None,
                    source_name: None,
                    selectable: None,
                    status_kind: None,
                    action_disabled: None,
                    style: None,
                },
            );
        }

        if let Some(error) = &env_prompt.secret_store_error {
            Self::push_limited_element(
                &mut elements,
                limit,
                protocol::ElementInfo {
                    semantic_id: protocol::generate_semantic_id_named(
                        "status",
                        "env-secret-store-error",
                    ),
                    element_type: protocol::ElementType::Panel,
                    text: Some("Secret Store Error".to_string()),
                    value: Some(error.kind_str().to_string()),
                    content: None,
                    selected: None,
                    focused: None,
                    index: Some(total_count - 1),
                    role: Some("status".to_string()),
                    kind: Some("secret_store_error".to_string()),
                    source: None,
                    source_name: None,
                    selectable: Some(false),
                    status_kind: Some(error.kind_str().to_string()),
                    action_disabled: None,
                    style: None,
                },
            );
        }

        (elements, total_count)
    }

    fn collect_drop_prompt_elements(
        &self,
        drop_prompt: &DropPrompt,
        limit: usize,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        if drop_prompt.dropped_files.is_empty() {
            return (Vec::new(), 0);
        }

        let total_count = drop_prompt.dropped_files.len() + 1;
        let mut elements = Vec::with_capacity(limit.min(total_count));

        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo::list("dropped-files", drop_prompt.dropped_files.len()),
        );

        for (index, file) in drop_prompt.dropped_files.iter().enumerate() {
            if elements.len() >= limit {
                break;
            }
            elements.push(protocol::ElementInfo {
                semantic_id: protocol::generate_semantic_id_named(
                    "choice",
                    &format!("dropped-file-{index}"),
                ),
                element_type: protocol::ElementType::Choice,
                text: Some(file.name.clone()),
                value: Some(file.automation_metadata(index).to_string()),
                content: None,
                selected: Some(false),
                focused: None,
                index: Some(index),
                role: Some("file".to_string()),
                kind: Some("dropped_file".to_string()),
                source: None,
                source_name: Some(file.name.clone()),
                selectable: Some(false),
                status_kind: None,
                action_disabled: None,
                style: None,
            });
        }

        (elements, total_count)
    }

    fn collect_template_prompt_elements(
        &self,
        template_prompt: &TemplatePrompt,
        limit: usize,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let total_count = template_prompt.inputs.len() + 2;
        let mut elements = Vec::with_capacity(limit.min(total_count));

        Self::push_limited_element(
            &mut elements,
            limit,
            Self::input_element(
                "template-source",
                "Template",
                Some(Self::preview_value(template_prompt.template.as_str(), 240)),
                false,
                Some(0),
            ),
        );
        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo::list("template-inputs", template_prompt.inputs.len()),
        );

        for (index, input) in template_prompt.inputs.iter().enumerate() {
            if elements.len() >= limit {
                break;
            }
            let value = template_prompt
                .values
                .get(index)
                .cloned()
                .unwrap_or_default();
            elements.push(Self::input_element(
                format!("template-{}", input.name).as_str(),
                input.label.clone(),
                Some(Self::preview_value(value.as_str(), 180)),
                index == template_prompt.current_input,
                Some(index),
            ));
        }

        (elements, total_count)
    }

    fn collect_hotkey_prompt_elements(
        &self,
        hotkey_prompt: &crate::components::shortcut_recorder::ShortcutRecorder,
        limit: usize,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let total_count = 3;
        let shortcut = hotkey_prompt.shortcut.to_display_string();
        let status = if hotkey_prompt.shortcut.is_complete() {
            "captured"
        } else if hotkey_prompt.shortcut.has_only_modifiers()
            || hotkey_prompt.current_modifiers.platform
            || hotkey_prompt.current_modifiers.control
            || hotkey_prompt.current_modifiers.alt
            || hotkey_prompt.current_modifiers.shift
        {
            "modifiers"
        } else {
            "recording"
        };
        let mut elements = Vec::with_capacity(limit.min(total_count));

        let mut panel = protocol::ElementInfo::panel("hotkey-capture");
        panel.status_kind = Some(status.to_string());
        Self::push_limited_element(&mut elements, limit, panel);
        Self::push_limited_element(
            &mut elements,
            limit,
            Self::input_element("hotkey-shortcut", "Shortcut", Some(shortcut), true, Some(0)),
        );
        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo::button(0, "Cancel"),
        );

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

    pub(crate) fn script_list_visible_row_labels_from_cache(&self) -> (Vec<String>, Option<usize>) {
        let (grouped_items, flat_results) = self.cached_grouped_results_snapshot();
        let selected_grouped_index =
            crate::list_item::coerce_selection(&grouped_items, self.selected_index);
        let mut selected_row_index = None;
        let mut row_names = Vec::new();

        for (grouped_index, item) in grouped_items.iter().enumerate() {
            let crate::list_item::GroupedListItem::Item(result_idx) = item else {
                continue;
            };
            let Some(result) = flat_results.get(*result_idx) else {
                continue;
            };
            if Some(grouped_index) == selected_grouped_index {
                selected_row_index = Some(row_names.len());
            }
            row_names.push(Self::script_list_result_label(result));
        }

        (row_names, selected_row_index)
    }

    fn collect_script_list_elements(
        &self,
        limit: usize,
        include_headers: bool,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let (grouped_items, flat_results) = self.cached_grouped_results_snapshot();
        let source_statuses = self.cached_source_statuses_snapshot();
        let selected_grouped_index =
            crate::list_item::coerce_selection(&grouped_items, self.selected_index);
        let total_rows = grouped_items
            .iter()
            .filter(|item| matches!(item, crate::list_item::GroupedListItem::Item(_)))
            .count();
        let handler_form = self
            .menu_syntax_main_hint_snapshot(&self.filter_text, false)
            .and_then(|snapshot| snapshot.form);
        let handler_form_field_count = handler_form
            .as_ref()
            .map_or(0usize, |form| form.fields.len());
        let mut total_count = total_rows + source_statuses.len() + handler_form_field_count + 2;
        let mut elements = Vec::with_capacity(limit.min(total_count));

        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo::input(
                "filter",
                Some(self.filter_text.as_str()),
                self.focused_input != FocusedInput::None,
            ),
        );
        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo::list("results", total_rows),
        );

        if let Some(snapshot) = self
            .menu_syntax_object_selector_state
            .snapshot
            .as_ref()
            .filter(|_| self.menu_syntax_object_selector_state.owns_main_list())
        {
            if let Some(list) = elements
                .iter_mut()
                .find(|element| element.semantic_id == "list:results")
            {
                list.semantic_id = "list:menu-syntax-object-selector".to_string();
                list.text = Some(format!("{} rows", snapshot.rows.len()));
                list.value = Some("menuSyntaxObjectSelector".to_string());
                list.kind = Some("menuSyntaxObjectSelector".to_string());
                list.source = Some("ScriptList".to_string());
            }

            let selected_row_id = self
                .selected_index
                .checked_sub(1)
                .and_then(|index| snapshot.rows.get(index))
                .map(|row| row.id.as_str())
                .or(self
                    .menu_syntax_object_selector_state
                    .selected_row_id
                    .as_deref());

            for (index, row) in snapshot.rows.iter().enumerate() {
                if elements.len() >= limit {
                    break;
                }
                elements.push(protocol::ElementInfo {
                    semantic_id: protocol::generate_semantic_id("choice", index, &row.id),
                    element_type: protocol::ElementType::Choice,
                    text: Some(row.title.clone()),
                    value: Some(row.token.clone().unwrap_or_else(|| row.id.clone())),
                    content: None,
                    selected: Some(selected_row_id == Some(row.id.as_str())),
                    focused: None,
                    index: Some(index),
                    role: Some("menu-syntax-object-selector-row".to_string()),
                    kind: Some("menuSyntaxObjectSelector".to_string()),
                    source: Some("menuSyntaxObjectSelector".to_string()),
                    source_name: Some("ScriptList".to_string()),
                    selectable: Some(row.enabled),
                    status_kind: None,
                    action_disabled: (!row.enabled).then(|| "disabled".to_string()),
                    style: None,
                });
            }

            return (elements, snapshot.rows.len() + 2);
        }

        if let Some(snapshot) = self
            .menu_syntax_trigger_picker_state
            .snapshot
            .as_ref()
            .filter(|_| self.menu_syntax_trigger_picker_state.owns_main_list())
        {
            if let Some(list) = elements
                .iter_mut()
                .find(|element| element.semantic_id == "list:results")
            {
                list.semantic_id = "list:menu-syntax-trigger-picker".to_string();
                list.text = Some(format!("{} rows", snapshot.rows.len()));
                list.value = Some("menuSyntaxTriggerPicker".to_string());
                list.kind = Some("menuSyntaxTriggerPicker".to_string());
                list.source = Some("ScriptList".to_string());
            }

            // The rendered picker leads with a persistent section header
            // (filtering_cache pushes it from the same mode mapping); report
            // it so header-stability probes see what users see.
            if include_headers && elements.len() < limit {
                let (section_label, _icon) = snapshot.mode.main_list_section();
                elements.push(protocol::ElementInfo {
                    semantic_id: protocol::generate_semantic_id("section", 0, section_label),
                    element_type: protocol::ElementType::Panel,
                    text: Some(section_label.to_string()),
                    value: None,
                    content: None,
                    selected: Some(false),
                    focused: None,
                    index: None,
                    role: Some("sectionHeader".to_string()),
                    kind: Some("sectionHeader".to_string()),
                    source: None,
                    source_name: None,
                    selectable: Some(false),
                    status_kind: None,
                    action_disabled: None,
                    style: None,
                });
            }

            for (index, row) in snapshot.rows.iter().enumerate() {
                if elements.len() >= limit {
                    break;
                }
                elements.push(protocol::ElementInfo {
                    semantic_id: protocol::generate_semantic_id("choice", index, &row.id),
                    element_type: protocol::ElementType::Choice,
                    text: Some(row.title.clone()),
                    value: Some(row.token.clone().unwrap_or_else(|| row.id.clone())),
                    content: None,
                    selected: Some(
                        self.menu_syntax_trigger_picker_state
                            .selected_row_id
                            .as_deref()
                            == Some(row.id.as_str()),
                    ),
                    focused: None,
                    index: Some(index),
                    role: Some("menu-syntax-trigger-row".to_string()),
                    kind: Some("menuSyntaxTriggerPicker".to_string()),
                    source: Some("menuSyntaxTriggerPicker".to_string()),
                    source_name: Some("ScriptList".to_string()),
                    selectable: Some(row.enabled),
                    status_kind: None,
                    action_disabled: (!row.enabled).then(|| "disabled".to_string()),
                    style: None,
                });
            }

            return (
                elements,
                snapshot.rows.len() + 2 + usize::from(include_headers),
            );
        }

        if let Some(form) = handler_form.as_ref() {
            for (index, field) in form.fields.iter().enumerate() {
                if elements.len() >= limit {
                    break;
                }
                let shell_spec = crate::components::menu_syntax_form_field_shell_spec(
                    &form.target,
                    field,
                    crate::components::FormFieldMetrics::from_colors(
                        crate::components::FormFieldColors::default(),
                    ),
                );
                let (element_type, role, kind, selectable) = match field.kind {
                    crate::menu_syntax::MenuSyntaxFormFieldKind::Priority
                    | crate::menu_syntax::MenuSyntaxFormFieldKind::Tags
                    | crate::menu_syntax::MenuSyntaxFormFieldKind::Object => (
                        protocol::ElementType::Input,
                        "combobox",
                        "handlerFormAutocompleteField",
                        true,
                    ),
                    _ => (
                        protocol::ElementType::Input,
                        "textbox",
                        "handlerFormField",
                        false,
                    ),
                };
                elements.push(protocol::ElementInfo {
                    semantic_id: shell_spec.semantic_id.to_string(),
                    element_type,
                    text: Some(field.label.clone()),
                    value: Some(field.value.clone()),
                    content: None,
                    selected: Some(false),
                    focused: Some(
                        shell_spec.focused
                            && shell_spec.editable()
                            && self.menu_syntax_form_input_active,
                    ),
                    index: Some(index),
                    role: Some(role.to_string()),
                    kind: Some(kind.to_string()),
                    source: Some("menuSyntaxMainHint.form".to_string()),
                    source_name: Some(form.target.clone()),
                    selectable: Some(selectable && shell_spec.editable()),
                    status_kind: Some(shell_spec.validation.status_kind().to_string()),
                    action_disabled: shell_spec
                        .disabled_reason
                        .as_ref()
                        .map(ToString::to_string),
                    style: None,
                });
            }
        }

        let main_hint_snapshot = (total_rows == 0)
            .then(|| self.menu_syntax_main_hint_snapshot(&self.filter_text, true))
            .flatten();
        let guidance_elements = if let Some(snapshot) = main_hint_snapshot.as_ref() {
            Self::menu_syntax_guidance_elements(snapshot)
        } else if total_rows == 0 && handler_form.is_none() {
            let has_active_filter = self
                .menu_syntax_mode
                .advanced_query_for(&self.filter_text)
                .is_some_and(|query| query.has_source_filters() || query.has_predicates());
            let snapshot = crate::components::launcher_empty_or_no_results_spec(
                &self.filter_text,
                has_active_filter,
            )
            .semantic_snapshot();
            Self::info_state_elements(&snapshot)
        } else {
            Vec::new()
        };
        total_count += guidance_elements.len();
        for element in guidance_elements {
            if !Self::push_limited_element(&mut elements, limit, element) {
                break;
            }
        }

        let mut row_index = 0usize;
        for (grouped_index, item) in grouped_items.iter().enumerate() {
            if elements.len() >= limit {
                break;
            }
            match item {
                crate::list_item::GroupedListItem::SectionHeader(label, icon) => {
                    if include_headers {
                        let presentation =
                            crate::list_item::resolve_section_header_presentation(
                                label,
                                icon.as_deref(),
                                None,
                                crate::list_item::SectionPresentationFamily::Launcher,
                            );
                        elements.push(protocol::ElementInfo {
                            semantic_id: protocol::generate_semantic_id(
                                "section",
                                grouped_index,
                                label,
                            ),
                            element_type: protocol::ElementType::Panel,
                            text: Some(presentation.display_label.to_string()),
                            value: Some(presentation.semantic_label.to_string()),
                            content: None,
                            selected: Some(false),
                            focused: None,
                            index: None,
                            role: Some("sectionHeader".to_string()),
                            kind: Some("sectionHeader".to_string()),
                            source: None,
                            source_name: None,
                            selectable: Some(false),
                            status_kind: None,
                            action_disabled: None,
                            style: None,
                        });
                    }
                }
                crate::list_item::GroupedListItem::ReservedSectionSlot => {
                    // Visual rhythm only: never expose an empty accessibility heading.
                }
                crate::list_item::GroupedListItem::Item(result_idx) => {
                    let Some(result) = flat_results.get(*result_idx) else {
                        continue;
                    };
                    let label = Self::script_list_result_label(result);
                    let source = result.root_unified_source();
                    let subtitle = result.description().map(|d| {
                        if matches!(result, scripts::SearchResult::Scriptlet(_)) {
                            let vars = crate::context_templates::ContextTemplateVars::from_frontmost_tracker();
                            crate::context_templates::substitute_context_vars(d, &vars).into_owned()
                        } else {
                            d.to_string()
                        }
                    });
                    let (value, content_kind) = match result {
                        scripts::SearchResult::BuiltIn(_) => (subtitle.unwrap_or_else(|| label.clone()), None),
                        scripts::SearchResult::File(file_match) => (
                            file_match.file.path.clone(),
                            Some(protocol::ElementContentKind::FilePath),
                        ),
                        scripts::SearchResult::Fallback(_)
                        | scripts::SearchResult::Note(_)
                        | scripts::SearchResult::BrainHit(_)
                        | scripts::SearchResult::BrainInboxItem(_)
                        | scripts::SearchResult::Todo(_)
                        | scripts::SearchResult::AgentChatHistory(_)
                        | scripts::SearchResult::ClipboardHistory(_)
                        | scripts::SearchResult::DictationHistory(_) => (
                            subtitle.unwrap_or_else(|| label.clone()),
                            Some(protocol::ElementContentKind::UserContent),
                        ),
                        _ => (
                            subtitle.unwrap_or_else(|| label.clone()),
                            Some(protocol::ElementContentKind::ExternalContent),
                        ),
                    };
                    let mut element = if let Some(content_kind) = content_kind {
                        protocol::ElementInfo::redacted_choice(
                            row_index,
                            &label,
                            &value,
                            Some(grouped_index) == selected_grouped_index,
                            content_kind,
                        )
                    } else {
                        protocol::ElementInfo::product_static_choice(
                            row_index,
                            &label,
                            &value,
                            Some(grouped_index) == selected_grouped_index,
                        )
                    };
                    element.role = Some("row".to_string());
                    element.kind = Some(result.type_label().to_ascii_lowercase());
                    element.source = source.map(|source| source.receipt_label().to_string());
                    element.source_name = content_kind
                        .is_none()
                        .then(|| result.source_name().map(str::to_string))
                        .flatten();
                    element.selectable = Some(true);
                    if let scripts::SearchResult::File(file_match) = result {
                        element.kind =
                            Some(root_file_semantic_kind(file_match.file.file_type).to_string());
                    }
                    elements.push(element);
                    row_index += 1;
                }
                crate::list_item::GroupedListItem::Status(status) => {
                    elements.push(protocol::ElementInfo {
                        semantic_id: protocol::generate_semantic_id(
                            "status",
                            row_index,
                            status.source.receipt_label(),
                        ),
                        element_type: protocol::ElementType::Panel,
                        text: Some(status.label.clone()),
                        value: Some(status.label.clone()),
                        content: None,
                        selected: Some(false),
                        focused: None,
                        index: Some(row_index),
                        role: Some("status".to_string()),
                        kind: Some("sourceStatus".to_string()),
                        source: Some(status.source.receipt_label().to_string()),
                        source_name: Some(status.source_name.clone()),
                        selectable: Some(false),
                        status_kind: Some(status.status_kind.as_str().to_string()),
                        action_disabled: None,
                        style: None,
                    });
                    row_index += 1;
                }
            }
        }

        for status in source_statuses.iter() {
            if elements.len() >= limit {
                break;
            }
            elements.push(protocol::ElementInfo {
                semantic_id: protocol::generate_semantic_id(
                    "status",
                    row_index,
                    status.source.receipt_label(),
                ),
                element_type: protocol::ElementType::Panel,
                text: Some(status.label.clone()),
                value: Some(status.label.clone()),
                content: None,
                selected: Some(false),
                focused: None,
                index: None,
                role: Some("status".to_string()),
                kind: Some("sourceStatus".to_string()),
                source: Some(status.source.receipt_label().to_string()),
                source_name: Some(status.source_name.clone()),
                selectable: Some(false),
                status_kind: Some(status.status_kind.as_str().to_string()),
                action_disabled: None,
                style: None,
            });
            row_index += 1;
        }

        // Emit JSON snapshot of all collected semantic IDs for agent introspection
        let semantic_ids: Vec<&str> = elements.iter().map(|e| e.semantic_id.as_str()).collect();
        tracing::debug!(
            event = "collect_script_list_elements",
            total_count,
            returned = elements.len(),
            limit,
            truncated = total_count > elements.len(),
            semantic_ids = ?semantic_ids,
            "ScriptList element collection complete"
        );

        (elements, total_count)
    }
}
include!("prompt_and_script_list_collectors.rs");

#[cfg(test)]
mod info_state_semantic_tests {
    use super::*;

    #[test]
    fn info_state_elements_project_cue_kind_without_action_fiction() {
        let snapshot =
            crate::components::launcher_empty_or_no_results_spec("#work", false).semantic_snapshot();
        let elements = ScriptListApp::info_state_elements(&snapshot);
        let root = elements
            .iter()
            .find(|element| element.role.as_deref() == Some("info-state"))
            .expect("InfoState root");
        assert_eq!(root.kind.as_deref(), Some("help"));
        assert_eq!(root.status_kind.as_deref(), Some("help"));

        let syntax: Vec<_> = elements
            .iter()
            .filter(|element| element.role.as_deref() == Some("guidance-cue"))
            .collect();
        assert_eq!(syntax.len(), 3);
        assert!(syntax.iter().all(|element| element.kind.as_deref() == Some("syntax")));
        assert!(syntax.iter().all(|element| element.selectable == Some(false)));
        assert!(syntax.iter().all(|element| element.action_disabled.is_none()));
        assert!(syntax.iter().any(|element| element.text.as_deref() == Some(";todo")));
    }

    #[test]
    fn empty_agent_chat_elements_expose_trigger_and_shortcut_kinds() {
        let snapshot = crate::components::agent_chat_empty_guidance_spec().semantic_snapshot();
        let elements = ScriptListApp::info_state_elements(&snapshot);
        assert!(elements.iter().any(|element| {
            element.kind.as_deref() == Some("trigger")
                && element.text.as_deref() == Some("/")
                && element.value.is_none()
        }));
        assert!(elements.iter().any(|element| {
            element.kind.as_deref() == Some("shortcut")
                && element.text.as_deref() == Some("⌘K")
                && element.value.as_deref() == Some("cmd+k")
                && element.semantic_id == "info-cue:agent-chat-open-actions"
        }));
    }

    #[test]
    fn simple_builtin_empty_elements_expose_info_state_owner_and_icon() {
        let snapshot = crate::components::simple_empty_state_spec(
            "favorites-empty",
            "No favorites yet",
            "star",
            None,
        )
        .semantic_snapshot();
        let elements = ScriptListApp::info_state_elements(&snapshot);
        assert_eq!(elements.len(), 1);
        let root = &elements[0];
        assert_eq!(root.semantic_id, "info-state:favorites-empty");
        assert_eq!(root.role.as_deref(), Some("info-state"));
        assert_eq!(root.source.as_deref(), Some("InfoState"));
        assert_eq!(root.source_name.as_deref(), Some("favorites-empty"));
        assert_eq!(root.value.as_deref(), Some("star"));
        assert_eq!(root.kind.as_deref(), Some("neutral"));
    }
}

#[cfg(test)]
mod app_layout_projection_tests {
    use super::*;

    #[test]
    fn complete_projection_has_no_degradation_reasons() {
        let outcome = ElementCollectionOutcome::complete(
            "settings",
            vec![protocol::ElementInfo::panel("settings")],
            1,
        );
        assert_eq!(outcome.semantic_surface, "settings");
        assert_eq!(outcome.version, 1);
        assert_eq!(
            outcome.projection_quality,
            protocol::ProjectionQuality::Complete
        );
        assert!(outcome.reason_codes.is_empty());
    }

    #[test]
    fn partial_and_unsupported_projections_are_typed() {
        let partial = ElementCollectionOutcome::partial(
            "flowSession",
            protocol::ProjectionReason::RuntimeEntityMissing,
            vec![protocol::ElementInfo::panel("flow-session")],
            1,
        );
        assert_eq!(partial.projection_quality, protocol::ProjectionQuality::Partial);
        assert_eq!(
            partial.reason_codes,
            vec![protocol::ProjectionReason::RuntimeEntityMissing]
        );

        let unsupported = ElementCollectionOutcome::unsupported(
            "divPrompt",
            protocol::ProjectionReason::UnsupportedCustomDocument,
            vec![protocol::ElementInfo::panel("div-prompt")],
            1,
        );
        assert_eq!(
            unsupported.projection_quality,
            protocol::ProjectionQuality::Unsupported
        );
        assert_eq!(
            unsupported.reason_codes,
            vec![protocol::ProjectionReason::UnsupportedCustomDocument]
        );
    }

    #[test]
    fn empty_surface_finalizer_cannot_fabricate_completeness() {
        let outcome = ScriptListApp::finalize_surface_outcome(
            "fixture",
            "fixture",
            "panel_only_fixture",
            10,
            Vec::new(),
            0,
        );
        assert_eq!(outcome.projection_quality, protocol::ProjectionQuality::Partial);
        assert_eq!(outcome.reason_codes, vec![protocol::ProjectionReason::PanelOnly]);
        assert_eq!(outcome.warnings, vec!["panel_only_fixture"]);
    }
}

#[cfg(test)]
mod recent_files_semantic_tests {
    use super::*;

    #[test]
    fn recent_files_semantic_kind_distinguishes_directories() {
        assert_eq!(
            root_file_semantic_kind(crate::file_search::FileType::Directory),
            "directory"
        );
        assert_eq!(
            root_file_semantic_kind(crate::file_search::FileType::Document),
            "file"
        );
    }
}
