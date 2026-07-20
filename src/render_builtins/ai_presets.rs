#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreateAiPresetFormAction {
    Submit,
}

impl CreateAiPresetFormAction {
    fn success_hud(self, preset_name: &str) -> String {
        match self {
            Self::Submit => format!("Preset '{preset_name}' created"),
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::Submit => format!("Failed to create preset: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AiPresetSearchEmptyState {
    NoPresetsAvailable,
    NoFilteredMatches,
}

impl AiPresetSearchEmptyState {
    pub(crate) fn from_filter(filter: &str) -> Self {
        if filter.is_empty() {
            Self::NoPresetsAvailable
        } else {
            Self::NoFilteredMatches
        }
    }

    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::NoPresetsAvailable => "No presets available",
            Self::NoFilteredMatches => "No presets match your filter",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiPresetModelSelection<'a> {
    ProviderDefault,
    Explicit(&'a str),
}

impl<'a> AiPresetModelSelection<'a> {
    fn from_input(model: &'a str) -> Self {
        if model.trim().is_empty() {
            Self::ProviderDefault
        } else {
            Self::Explicit(model)
        }
    }

    fn as_create_arg(self) -> Option<&'a str> {
        match self {
            Self::ProviderDefault => None,
            Self::Explicit(model) => Some(model),
        }
    }
}

impl ScriptListApp {
    pub(crate) fn ai_preset_search_visible_row_labels(filter: &str) -> Vec<String> {
        let all_presets = crate::ai::presets::load_presets().unwrap_or_default();
        let default_presets: Vec<(&str, &str, &str)> = vec![
            (
                "general",
                "General Assistant",
                "Helpful AI assistant for any task",
            ),
            ("coder", "Code Assistant", "Expert programmer and debugger"),
            (
                "writer",
                "Writing Assistant",
                "Help with writing and editing",
            ),
            (
                "researcher",
                "Research Assistant",
                "Deep analysis and research",
            ),
            (
                "creative",
                "Creative Partner",
                "Brainstorming and creative ideas",
            ),
        ];

        let mut rows: Vec<(String, String, String)> = default_presets
            .iter()
            .map(|(id, name, desc)| ((*id).to_string(), (*name).to_string(), (*desc).to_string()))
            .collect();
        for preset in &all_presets {
            if !default_presets.iter().any(|(did, _, _)| *did == preset.id) {
                rows.push((
                    preset.id.clone(),
                    preset.name.clone(),
                    preset.description.clone(),
                ));
            }
        }

        let filter_lower = filter.to_lowercase();
        rows.into_iter()
            .filter(|(id, name, desc)| {
                filter.is_empty()
                    || name.to_lowercase().contains(&filter_lower)
                    || desc.to_lowercase().contains(&filter_lower)
                    || id.to_lowercase().contains(&filter_lower)
            })
            .map(|(_, name, desc)| {
                if desc.is_empty() {
                    name
                } else {
                    format!("{name} — {desc}")
                }
            })
            .collect()
    }

    /// Render the searchable AI presets list view.
    fn render_search_ai_presets(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();

        let text_muted = self.theme.colors.text.muted;

        // Load all presets (defaults + user-saved)
        let all_presets = crate::ai::presets::load_presets().unwrap_or_default();
        let default_presets: Vec<(&str, &str, &str)> = vec![
            (
                "general",
                "General Assistant",
                "Helpful AI assistant for any task",
            ),
            ("coder", "Code Assistant", "Expert programmer and debugger"),
            (
                "writer",
                "Writing Assistant",
                "Help with writing and editing",
            ),
            (
                "researcher",
                "Research Assistant",
                "Deep analysis and research",
            ),
            (
                "creative",
                "Creative Partner",
                "Brainstorming and creative ideas",
            ),
        ];

        let mut items: Vec<(String, String, String, bool)> = Vec::new();
        for (id, name, desc) in &default_presets {
            items.push((id.to_string(), name.to_string(), desc.to_string(), true));
        }
        for preset in &all_presets {
            if !default_presets.iter().any(|(did, _, _)| *did == preset.id) {
                items.push((
                    preset.id.clone(),
                    preset.name.clone(),
                    preset.description.clone(),
                    false,
                ));
            }
        }

        let filter_lower = filter.to_lowercase();
        let filtered_items: Vec<_> = if filter.is_empty() {
            items.iter().enumerate().collect()
        } else {
            items
                .iter()
                .enumerate()
                .filter(|(_, (id, name, desc, _))| {
                    name.to_lowercase().contains(&filter_lower)
                        || desc.to_lowercase().contains(&filter_lower)
                        || id.to_lowercase().contains(&filter_lower)
                })
                .collect()
        };

        let count = filtered_items.len();
        let list_colors = ListItemColors::from_theme(&self.theme);
        let entity = cx.entity().downgrade();

        let list_items: Vec<AnyElement> = filtered_items
            .iter()
            .enumerate()
            .map(
                |(display_idx, (_original_idx, (id, name, desc, is_default)))| {
                    let is_selected = display_idx == selected_index;
                    let id_clone = id.clone();
                    let entity_clone = entity.clone();
                    let badge = if *is_default {
                        " (built-in)"
                    } else {
                        " (custom)"
                    };
                    let description = format!("{}{}", desc, badge);

                    div()
                        .id(display_idx)
                        .cursor_pointer()
                        .on_click(move |_event, window, cx| {
                            if let Some(app) = entity_clone.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.select_ai_preset(&id_clone, window, cx);
                                });
                            }
                        })
                        .child(
                            ListItem::new(name.clone(), list_colors)
                                .description_opt(Some(description))
                                .selected(is_selected),
                        )
                        .into_any_element()
                },
            )
            .collect();

        let list_element: AnyElement = if count == 0 {
            div()
                .w_full()
                .py(px(design_spacing.padding_xl))
                .text_center()
                .text_color(rgb(text_muted))
                .font_family(design_typography.font_family)
                .child(AiPresetSearchEmptyState::from_filter(&filter).message())
                .into_any_element()
        } else {
            crate::components::scrollbar::render_tracked_scroll_column(
                "search-ai-presets-row-stack",
                &self.builtin_row_stack_scroll_handle,
                list_items,
            )
        };
        // Every list leads with a persistent section separator (POLISH.md
        // layout-stability bar; same rule as the main menu's "Results"
        // header, 4d76327b8): the label may swap but the row never appears
        // or disappears, so filtering can't shift the rows below it.
        let list_element = div()
            .flex()
            .flex_col()
            .h_full()
            .min_h(px(0.))
            .child(crate::list_item::render_section_header(
                if filter.trim().is_empty() {
                    "Presets"
                } else {
                    "Results"
                },
                None,
                list_colors,
                true,
            ))
            .child(div().relative().flex_1().min_h(px(0.)).child(list_element))
            .into_any_element();

        let footer = self.main_window_footer_slot(crate::components::render_simple_hint_strip(
            vec![
                gpui::SharedString::from("↵ Select"),
                gpui::SharedString::from("Esc Back"),
            ],
            None,
        ));

        self.render_generic_filterable_search_surface(
            "search_ai_presets",
            format!("{} presets", count),
            list_element,
            footer,
            cx,
        )
    }

    /// Render the create AI preset form.
    fn render_create_ai_preset(
        &mut self,
        name: String,
        system_prompt: String,
        model: String,
        active_field: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();

        let text_primary = self.theme.colors.text.primary;
        let text_dimmed = self.theme.colors.text.dimmed;
        let text_muted = self.theme.colors.text.muted;
        let accent = self.theme.colors.accent.selected;
        let border = self.theme.colors.ui.border;

        let entity = cx.entity().downgrade();

        let fields: Vec<(&str, &str, &str, usize)> = vec![
            ("Name", "Preset name (e.g., Code Reviewer)", &name, 0),
            (
                "System Prompt",
                "Instructions for the AI",
                &system_prompt,
                1,
            ),
            (
                "Model (optional)",
                "Model ID or leave empty for any",
                &model,
                2,
            ),
        ];

        let field_elements: Vec<AnyElement> = fields
            .iter()
            .map(|(label, placeholder, value, idx)| {
                let is_active = *idx == active_field;
                let label_color = if is_active { accent } else { text_dimmed };
                let border_color = if is_active { accent } else { border };
                let entity_for_click = entity.clone();
                let field_idx = *idx;

                div()
                    .w_full()
                    .mb(px(design_spacing.padding_md))
                    .child(
                        div()
                            .text_size(px(design_typography.font_size_sm))
                            .text_color(rgb(label_color))
                            .mb(px(4.))
                            .child(label.to_string()),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("preset-field-{}", idx)))
                            .w_full()
                            .px(px(design_spacing.padding_md))
                            .py(px(design_spacing.padding_sm))
                            .border_1()
                            .border_color(rgb(border_color))
                            .rounded(px(design_visual.radius_md))
                            .text_size(px(design_typography.font_size_md))
                            .text_color(rgb(text_primary))
                            .cursor_pointer()
                            .on_click(move |_event, _window, cx| {
                                if let Some(app) = entity_for_click.upgrade() {
                                    app.update(cx, |this, cx| {
                                        if let AppView::CreateAiPresetView {
                                            ref mut active_field,
                                            ..
                                        } = this.current_view
                                        {
                                            *active_field = field_idx;
                                            cx.notify();
                                        }
                                    });
                                }
                            })
                            .child(if value.is_empty() {
                                div()
                                    .text_color(rgb(text_muted))
                                    .child(placeholder.to_string())
                                    .into_any_element()
                            } else {
                                div().child(value.to_string()).into_any_element()
                            }),
                    )
                    .into_any_element()
            })
            .collect();

        div()
            .flex().flex_col().w_full().h_full()
            .rounded(px(design_visual.radius_lg))
            .text_color(rgb(text_primary))
            .font_family(design_typography.font_family)
            .track_focus(&self.focus_handle)
            .child(
                div().w_full()
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .text_size(px(design_typography.font_size_xl))
                    .child("Create AI Preset"),
            )
            .child(div().w_full().h(px(1.)).bg(rgb(border)))
            .child(
                div().flex_1().w_full().min_h(px(0.))
                    .px(px(design_spacing.padding_lg))
                    .py(px(design_spacing.padding_md))
                    .children(field_elements)
                    .child(
                        div().text_size(px(design_typography.font_size_xs))
                            .text_color(rgb(text_muted))
                            .child("Type to edit \u{00b7} Tab: next field \u{00b7} Enter: save \u{00b7} Esc: cancel"),
                    ),
            )
            .into_any_element()
    }

    /// Select a preset from the search view and apply it in AI chat.
    fn select_ai_preset(&mut self, preset_id: &str, _window: &mut Window, cx: &mut Context<Self>) {
        tracing::info!(preset_id = %preset_id, action = "select_ai_preset", "User selected AI preset from search");
        let trace_id = uuid::Uuid::new_v4().to_string();
        self.open_ai_window_after_main_hide(
            "select_ai_preset",
            &trace_id,
            DeferredAiWindowAction::ApplyPreset {
                preset_id: preset_id.to_string(),
            },
            cx,
        );
    }

    /// Handle keyboard input for the Create AI Preset form.
    #[allow(dead_code)] // Called from startup_new_actions.rs interceptor
    pub(crate) fn handle_create_ai_preset_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let AppView::CreateAiPresetView {
            ref mut name,
            ref mut system_prompt,
            ref mut model,
            ref mut active_field,
        } = self.current_view
        {
            let active = *active_field;
            if crate::ui_foundation::is_key_tab(key) {
                *active_field = (active + 1) % 3;
                cx.notify();
            } else if crate::ui_foundation::is_key_enter(key) {
                let form_action = CreateAiPresetFormAction::Submit;
                let name_val = name.clone();
                let prompt_val = system_prompt.clone();
                let model_val = AiPresetModelSelection::from_input(model.as_str()).as_create_arg();
                match crate::ai::presets::create_preset(&name_val, &prompt_val, model_val) {
                    Ok(preset) => {
                        tracing::info!(id = %preset.id, name = %preset.name, action = "create_preset_success", "AI preset created");
                        self.show_hud(
                            form_action.success_hud(&preset.name),
                            Some(HUD_SHORT_MS),
                            cx,
                        );
                        ai::reload_ai_presets(cx);
                        self.go_back_or_close(window, cx);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, action = "create_preset_failed", "Failed to create preset");
                        self.show_error_toast(form_action.failure_message(e), cx);
                    }
                }
            } else if crate::ui_foundation::is_key_escape(key) {
                self.go_back_or_close(window, cx);
            } else if crate::ui_foundation::is_key_backspace(key) {
                match active {
                    0 => {
                        name.pop();
                    }
                    1 => {
                        system_prompt.pop();
                    }
                    _ => {
                        model.pop();
                    }
                }
                cx.notify();
            } else if key.len() == 1 {
                match active {
                    0 => name.push_str(key),
                    1 => system_prompt.push_str(key),
                    _ => model.push_str(key),
                }
                cx.notify();
            }
        }
    }

    /// Handle keyboard input for the Search AI Presets view.
    #[allow(dead_code)] // Called from startup_new_actions.rs interceptor
    pub(crate) fn handle_search_ai_presets_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let AppView::SearchAiPresetsView {
            ref filter,
            ref mut selected_index,
        } = self.current_view
        {
            if crate::ui_foundation::is_key_enter(key) {
                // Build the same item list as render_search_ai_presets to ensure
                // selected_index maps to the correct preset.
                let all_presets = crate::ai::presets::load_presets().unwrap_or_default();
                let default_presets: Vec<(&str, &str, &str)> = vec![
                    (
                        "general",
                        "General Assistant",
                        "Helpful AI assistant for any task",
                    ),
                    ("coder", "Code Assistant", "Expert programmer and debugger"),
                    (
                        "writer",
                        "Writing Assistant",
                        "Help with writing and editing",
                    ),
                    (
                        "researcher",
                        "Research Assistant",
                        "Deep analysis and research",
                    ),
                    (
                        "creative",
                        "Creative Partner",
                        "Brainstorming and creative ideas",
                    ),
                ];
                let mut items: Vec<(String, String, String)> = Vec::new();
                for (id, name, desc) in &default_presets {
                    items.push((id.to_string(), name.to_string(), desc.to_string()));
                }
                for preset in &all_presets {
                    if !default_presets.iter().any(|(did, _, _)| *did == preset.id) {
                        items.push((
                            preset.id.clone(),
                            preset.name.clone(),
                            preset.description.clone(),
                        ));
                    }
                }
                let filter_lower = filter.to_lowercase();
                let filtered: Vec<_> = if filter.is_empty() {
                    items.iter().collect()
                } else {
                    items
                        .iter()
                        .filter(|(id, name, desc)| {
                            name.to_lowercase().contains(&filter_lower)
                                || desc.to_lowercase().contains(&filter_lower)
                                || id.to_lowercase().contains(&filter_lower)
                        })
                        .collect()
                };
                if let Some((id, _, _)) = filtered.get(*selected_index) {
                    let id_clone = id.to_string();
                    self.select_ai_preset(&id_clone, window, cx);
                }
            } else if crate::ui_foundation::is_key_escape(key) {
                self.go_back_or_close(window, cx);
            }
        }
    }
}

#[cfg(test)]
mod ai_presets_chrome_audit {
    #[test]
    fn ai_presets_search_uses_minimal_chrome_scaffold() {
        let source = include_str!("ai_presets.rs");
        assert!(
            source.contains("render_minimal_list_prompt_scaffold("),
            "ai_presets search should use render_minimal_list_prompt_scaffold"
        );
        let legacy = "Prompt".to_owned() + "Footer::new(";
        assert_eq!(
            source.matches(&legacy).count(),
            0,
            "ai_presets should not construct PromptFooter"
        );
    }
}
