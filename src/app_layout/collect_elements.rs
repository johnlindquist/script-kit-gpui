// Element collection for getElements protocol support.
// Returns a bounded list of visible UI elements with semantic IDs.

/// Outcome of collecting visible UI elements, carrying receipt metadata
/// for the `elementsResult` protocol response.
#[derive(Debug, Clone)]
pub(crate) struct ElementCollectionOutcome {
    pub elements: Vec<protocol::ElementInfo>,
    pub total_count: usize,
    pub warnings: Vec<String>,
}

impl ElementCollectionOutcome {
    pub fn new(elements: Vec<protocol::ElementInfo>, total_count: usize) -> Self {
        Self {
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

impl From<(Vec<protocol::ElementInfo>, usize)> for ElementCollectionOutcome {
    fn from((elements, total_count): (Vec<protocol::ElementInfo>, usize)) -> Self {
        Self::new(elements, total_count)
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
    }

    pub(crate) fn collect_visible_elements(
        &self,
        limit: usize,
        cx: &Context<Self>,
    ) -> ElementCollectionOutcome {
        let mut outcome = match &self.current_view {
            AppView::ScriptList => {
                let (elements, total_count) = self.collect_script_list_elements(limit);
                ElementCollectionOutcome::new(elements, total_count)
            }

            AppView::AgentChatView { entity } => {
                let focused_text_elements = entity
                    .read(cx)
                    .collect_focused_text_mini_elements(limit, cx);
                if !focused_text_elements.is_empty() {
                    ElementCollectionOutcome::new(
                        focused_text_elements.clone(),
                        focused_text_elements.len(),
                    )
                } else {
                    let state = entity.read(cx).collect_agent_chat_state_snapshot(cx);
                    let elements = vec![
                        protocol::ElementInfo::panel("agent_chat-chat"),
                        protocol::ElementInfo::input(
                            "agent_chat-composer",
                            Some(state.input_text.as_str()),
                            true,
                        ),
                        protocol::ElementInfo::list("agent_chat-messages", state.message_count),
                    ];
                    let total_count = elements.len();
                    ElementCollectionOutcome::new(
                        elements.into_iter().take(limit).collect(),
                        total_count,
                    )
                }
            }

            AppView::DayPage { entity } => {
                let (elements, total_count) =
                    entity.read(cx).collect_day_page_elements(limit, self, cx);
                ElementCollectionOutcome::new(elements, total_count)
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
                    let mut row = protocol::ElementInfo::choice(
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
                ElementCollectionOutcome::new(
                    elements.into_iter().take(limit).collect(),
                    total_count,
                )
            }

            AppView::ArgPrompt { choices, .. } => self
                .collect_choice_view_elements(
                    "filter",
                    self.arg_input.text().to_string(),
                    choices,
                    self.arg_selected_index,
                    limit,
                )
                .into(),

            AppView::MiniPrompt { choices, .. } => self
                .collect_choice_view_elements(
                    "filter",
                    self.arg_input.text().to_string(),
                    choices,
                    self.arg_selected_index,
                    limit,
                )
                .into(),

            AppView::MicroPrompt { choices, .. } => self
                .collect_choice_view_elements(
                    "filter",
                    self.arg_input.text().to_string(),
                    choices,
                    self.arg_selected_index,
                    limit,
                )
                .into(),

            AppView::ClipboardHistoryView {
                filter,
                selected_index,
            } => {
                let rows = self.clipboard_history_visible_row_labels(filter);
                self.collect_named_rows(
                    "clipboard-filter",
                    filter.clone(),
                    "clipboard-history",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
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
                self.collect_named_rows(
                    "app-filter",
                    filter.clone(),
                    "apps",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
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
                self.collect_named_rows(
                    "window-filter",
                    filter.clone(),
                    "windows",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::BrowserTabsView {
                filter,
                selected_index,
            } => {
                let rows = self.browser_tabs_visible_row_labels(filter);
                self.collect_named_rows(
                    "browser-tabs-filter",
                    filter.clone(),
                    "browser-tabs",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
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
                self.collect_named_rows(
                    "browser-history-filter",
                    filter.clone(),
                    "browser-history",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::DesignGalleryView {
                filter,
                selected_index,
            } => {
                let filter_lower = filter.to_lowercase();
                let rows: Vec<String> = crate::build_gallery_items()
                    .into_iter()
                    .filter(|item| {
                        filter.is_empty() || crate::gallery_item_matches(item, &filter_lower)
                    })
                    .map(|item| crate::design_gallery_item_label(&item))
                    .collect();
                self.collect_named_rows(
                    "design-gallery-filter",
                    filter.clone(),
                    "design-gallery",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::FooterGalleryView {
                filter,
                selected_index,
            } => {
                let rows = Self::footer_gallery_visible_row_labels(filter);
                self.collect_named_rows(
                    "footer-gallery-filter",
                    filter.clone(),
                    "footer-gallery",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::NonListStatesView { selected_index } => {
                let rows = [
                    ("empty", "Empty"),
                    ("help", "Help"),
                    ("form", "Form"),
                    ("setup", "Setup"),
                    ("permission", "Permission"),
                    ("recovery", "Recovery"),
                    ("about", "About"),
                    ("density", "Density"),
                ];
                let mut elements = vec![protocol::ElementInfo {
                    semantic_id: "non-list-states:surface".to_string(),
                    element_type: protocol::ElementType::Panel,
                    text: rows
                        .get(*selected_index)
                        .map(|(_, label)| (*label).to_string())
                        .or_else(|| Some("Non-list state language".to_string())),
                    value: Some("main-window-showcase".to_string()),
                    selected: None,
                    focused: Some(true),
                    index: None,
                    role: Some("region".to_string()),
                    kind: Some("designLanguageShowcase".to_string()),
                    source: Some("nonListStates".to_string()),
                    source_name: Some("Non-List States".to_string()),
                    selectable: Some(false),
                    status_kind: None,
                    action_disabled: None,
            style: None,
                }];

                for (index, (value, label)) in rows.iter().enumerate() {
                    if !Self::push_limited_element(
                        &mut elements,
                        limit,
                        protocol::ElementInfo {
                            semantic_id: format!("non-list-states:{value}"),
                            element_type: protocol::ElementType::Panel,
                            text: Some((*label).to_string()),
                            value: Some((*value).to_string()),
                            selected: Some(index == *selected_index),
                            focused: None,
                            index: Some(index),
                            role: Some("example".to_string()),
                            kind: Some("nonListState".to_string()),
                            source: Some("nonListStates".to_string()),
                            source_name: Some("Non-List States".to_string()),
                            selectable: Some(false),
                            status_kind: None,
                            action_disabled: None,
            style: None,
                        },
                    ) {
                        break;
                    }
                }

                ElementCollectionOutcome::new(elements, rows.len() + 1)
            }

            AppView::AgentChatHistoryView {
                filter,
                selected_index,
            } => {
                let rows = Self::agent_chat_history_visible_row_labels(filter);
                self.collect_named_rows(
                    "agent_chat-history-filter",
                    filter.clone(),
                    "agent_chat-history",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::DictationHistoryView {
                filter,
                selected_index,
            } => {
                let rows = Self::dictation_history_visible_row_labels(filter);
                self.collect_named_rows(
                    "dictation-history-filter",
                    filter.clone(),
                    "dictation-history",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::NotesBrowseView {
                filter,
                selected_index,
            } => {
                let rows = Self::notes_browse_visible_row_labels(filter);
                self.collect_named_rows(
                    "notes-browse-filter",
                    filter.clone(),
                    "notes",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

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
                self.collect_named_rows(
                    "file-search-input",
                    query.clone(),
                    "file-results",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::ProcessManagerView {
                filter,
                selected_index,
            } => {
                let rows = self.process_manager_visible_row_names(filter);
                self.collect_named_rows(
                    "process-filter",
                    filter.clone(),
                    "processes",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::FlowUxView {
                filter,
                selected_index,
                ..
            } => {
                let cwd = self.flow_ux_cwd();
                let roster = crate::flows::catalog::flow_catalog().roster_for(&cwd);
                let rows: Vec<String> =
                    crate::flows::catalog::filter_flows(&roster.flows, filter)
                        .iter()
                        .map(|flow| flow.name.clone())
                        .collect();
                self.collect_named_rows(
                    "flow-ux-filter",
                    filter.clone(),
                    "flows",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::SettingsView {
                filter,
                selected_index,
            } => {
                let rows = self.settings_visible_row_names(filter);
                self.collect_named_rows(
                    "settings-filter",
                    filter.clone(),
                    "settings",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::CurrentAppCommandsView {
                filter,
                selected_index,
            } => {
                let rows = self.current_app_commands_visible_row_names(filter);
                self.collect_named_rows(
                    "current-app-commands-filter",
                    filter.clone(),
                    "menu-commands",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::SearchAiPresetsView {
                filter,
                selected_index,
            } => {
                let rows = Self::ai_preset_search_visible_row_labels(filter);
                self.collect_generic_filterable_rows(
                    "ai-presets-filter",
                    filter.clone(),
                    "ai-presets",
                    "ai-presets-empty",
                    AiPresetSearchEmptyState::from_filter(filter).message(),
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::FavoritesBrowseView {
                filter,
                selected_index,
            } => {
                let rows = self.filtered_favorite_ids_for_filter(filter);
                self.collect_generic_filterable_rows(
                    "favorites-filter",
                    filter.clone(),
                    "favorites",
                    "favorites-empty",
                    FavoritesEmptyState::from_filter(filter).message(),
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::SdkReferenceView {
                filter,
                selected_index,
                entries,
            } => {
                let rows = crate::mcp_resources::sdk_reference_visible_row_names(entries, filter);
                self.collect_named_rows(
                    "sdk-reference-filter",
                    filter.clone(),
                    "sdk-functions",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::ScriptTemplateCatalogView {
                filter,
                selected_index,
                templates,
            } => {
                let rows = crate::mcp_resources::script_template_catalog_visible_row_names(
                    templates, filter,
                );
                self.collect_named_rows(
                    "script-template-filter",
                    filter.clone(),
                    "script-templates",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
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
                self.collect_named_rows(
                    "emoji-filter",
                    filter.clone(),
                    "emoji-results",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::BrowseKitsView {
                query,
                selected_index,
                results,
            } => {
                let rows = Self::kit_store_browse_visible_row_labels(results);
                self.collect_named_rows(
                    "kit-search",
                    query.clone(),
                    "kit-results",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
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
                self.collect_named_rows(
                    "migrate-v1-filter",
                    filter.clone(),
                    "migrate-v1-results",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
            }

            AppView::InstalledKitsView {
                filter,
                selected_index,
                kits,
            } => {
                let rows = Self::kit_store_installed_visible_row_labels(kits, filter);
                self.collect_named_rows(
                    "installed-kits-filter",
                    filter.clone(),
                    "installed-kits",
                    &rows,
                    *selected_index,
                    limit,
                )
                .into()
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
                ElementCollectionOutcome::new(elements, total_count)
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
                            elements.push(protocol::ElementInfo::choice(
                                filter_pos,
                                &action.title,
                                &action.id,
                                is_selected,
                            ));
                        }
                    }

                    let total_count = elements.len();
                    if elements.len() > limit {
                        elements.truncate(limit);
                    }
                    ElementCollectionOutcome::new(elements, total_count)
                } else {
                    let total_count = 1;
                    let elements: Vec<protocol::ElementInfo> =
                        vec![protocol::ElementInfo::panel("actions-dialog")]
                            .into_iter()
                            .take(limit)
                            .collect();
                    ElementCollectionOutcome::new(elements, total_count)
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
                ElementCollectionOutcome::new(elements, total_count)
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

            AppView::SelectPrompt { entity, .. } => entity.read(cx).collect_elements(limit).into(),

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
                ElementCollectionOutcome::new(elements, total_count)
            }

            AppView::ConfirmPrompt {
                options,
                focused_button,
                ..
            } => {
                let confirm_selected = matches!(focused_button, ConfirmFocusedButton::Confirm);
                let cancel_selected = matches!(focused_button, ConfirmFocusedButton::Cancel);
                let mut confirm_button =
                    protocol::ElementInfo::button(0, &options.confirm_text.to_string());
                confirm_button.selected = Some(confirm_selected);
                confirm_button.role = Some("footer".to_string());
                confirm_button.kind = Some("confirm".to_string());
                confirm_button.selectable = Some(true);
                let mut cancel_button =
                    protocol::ElementInfo::button(1, &options.cancel_text.to_string());
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
                ElementCollectionOutcome::new(elements, 3)
            }

            AppView::WebcamView { .. } => {
                let total_count = 1;
                let elements: Vec<protocol::ElementInfo> =
                    vec![protocol::ElementInfo::panel("webcam")]
                        .into_iter()
                        .take(limit)
                        .collect();
                ElementCollectionOutcome::new(elements, total_count)
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

            _ => {
                let total_count = 1;
                let elements: Vec<protocol::ElementInfo> =
                    vec![protocol::ElementInfo::panel("current-view")]
                        .into_iter()
                        .take(limit)
                        .collect();
                ElementCollectionOutcome::new(elements, total_count)
                    .with_warning("collector_used_current_view_fallback")
            }
        };

        self.append_footer_elements(&mut outcome, limit, cx);
        outcome
    }

    fn append_footer_elements(
        &self,
        outcome: &mut ElementCollectionOutcome,
        limit: usize,
        cx: &Context<Self>,
    ) {
        let footer = self.active_footer_snapshot(&**cx);
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
            outcome.elements.push(protocol::ElementInfo {
                semantic_id: format!("footer:{}:{}", footer.owner, button.action),
                element_type: protocol::ElementType::Button,
                text: Some(format!("{} {}", button.key, button.label)),
                value: Some(button.action.clone()),
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

        for (index, row) in rows.iter().enumerate() {
            if elements.len() >= limit {
                break;
            }
            elements.push(protocol::ElementInfo {
                semantic_id: protocol::generate_semantic_id("choice", index, row),
                element_type: protocol::ElementType::Choice,
                text: Some(row.clone()),
                value: Some(row.clone()),
                selected: Some(index == selected_index),
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
            });
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
            return ElementCollectionOutcome::new(elements, total_count)
                .with_warning("profile_search_elements_truncated_by_limit");
        }
        ElementCollectionOutcome::new(elements, total_count)
    }

    fn collect_generic_filterable_rows(
        &self,
        input_name: &str,
        input_value: String,
        list_name: &str,
        empty_panel_name: &str,
        empty_text: &str,
        rows: &[String],
        selected_index: usize,
        limit: usize,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let empty_panel_count = usize::from(rows.is_empty());
        let total_count = rows.len() + 2 + empty_panel_count;
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

        if rows.is_empty() {
            Self::push_limited_element(
                &mut elements,
                limit,
                protocol::ElementInfo {
                    semantic_id: protocol::generate_semantic_id_named("panel", empty_panel_name),
                    element_type: protocol::ElementType::Panel,
                    text: Some(empty_text.to_string()),
                    value: Some(list_name.to_string()),
                    selected: None,
                    focused: None,
                    index: None,
                    role: Some("empty-state".to_string()),
                    kind: Some("genericFilterableEmptyState".to_string()),
                    source: Some(list_name.to_string()),
                    source_name: None,
                    selectable: Some(false),
                    status_kind: None,
                    action_disabled: None,
            style: None,
                },
            );
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
            return ElementCollectionOutcome::new(elements, total_count);
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
        ElementCollectionOutcome::new(elements, total_count).with_warning(warning)
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

    include!("prompt_and_script_list_collectors.rs");

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

    fn collect_script_list_elements(&self, limit: usize) -> (Vec<protocol::ElementInfo>, usize) {
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
        let total_count = total_rows + source_statuses.len() + handler_form_field_count + 2;
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

            for (index, row) in snapshot.rows.iter().enumerate() {
                if elements.len() >= limit {
                    break;
                }
                elements.push(protocol::ElementInfo {
                    semantic_id: protocol::generate_semantic_id("choice", index, &row.id),
                    element_type: protocol::ElementType::Choice,
                    text: Some(row.title.clone()),
                    value: Some(row.token.clone().unwrap_or_else(|| row.id.clone())),
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

            return (elements, snapshot.rows.len() + 2);
        }

        if let Some(form) = handler_form.as_ref() {
            for (index, field) in form.fields.iter().enumerate() {
                if elements.len() >= limit {
                    break;
                }
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
                    semantic_id: format!("handler-form:{}:{}", form.target, field.id),
                    element_type,
                    text: Some(field.label.clone()),
                    value: Some(field.value.clone()),
                    selected: Some(false),
                    focused: Some(field.focused && self.menu_syntax_form_input_active),
                    index: Some(index),
                    role: Some(role.to_string()),
                    kind: Some(kind.to_string()),
                    source: Some("menuSyntaxMainHint.form".to_string()),
                    source_name: Some(form.target.clone()),
                    selectable: Some(selectable),
                    status_kind: None,
                    action_disabled: None,
            style: None,
                });
            }
        }

        let mut row_index = 0usize;
        for (grouped_index, item) in grouped_items.iter().enumerate() {
            if elements.len() >= limit {
                break;
            }
            match item {
                crate::list_item::GroupedListItem::SectionHeader(..) => {}
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
                    let mut element = protocol::ElementInfo {
                        semantic_id: protocol::generate_semantic_id("choice", row_index, &label),
                        element_type: protocol::ElementType::Choice,
                        text: Some(label.clone()),
                        value: subtitle.or_else(|| Some(label)),
                        selected: Some(Some(grouped_index) == selected_grouped_index),
                        focused: None,
                        index: Some(row_index),
                        role: Some("row".to_string()),
                        kind: Some(result.type_label().to_ascii_lowercase()),
                        source: source.map(|source| source.receipt_label().to_string()),
                        source_name: result.source_name().map(str::to_string),
                        selectable: Some(true),
                        status_kind: None,
                        action_disabled: None,
            style: None,
                    };
                    if matches!(result, scripts::SearchResult::File(_)) {
                        element.kind = Some("file".to_string());
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
