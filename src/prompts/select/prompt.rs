use super::*;

/// Collect visible elements for a SelectPrompt's getElements protocol response.
///
/// Separated from the struct method so it can be tested without a GPUI context.
pub(crate) fn collect_select_prompt_elements(
    filter_text: &str,
    choices: &[Choice],
    filtered_choices: &[usize],
    selected: &HashSet<usize>,
    focused_index: usize,
    limit: usize,
) -> (Vec<crate::protocol::ElementInfo>, usize) {
    let total_count = filtered_choices.len() + 2; // input + list + choices
    let mut elements = Vec::with_capacity(limit.min(total_count));

    // Filter input element
    if elements.len() < limit {
        elements.push(crate::protocol::ElementInfo::input(
            "select-filter",
            Some(filter_text),
            true,
        ));
    }

    // Choices list container
    if elements.len() < limit {
        elements.push(crate::protocol::ElementInfo::list(
            "select-choices",
            filtered_choices.len(),
        ));
    }

    // Visible choice rows expose the same semantic IDs as rendered row elements.
    for (display_idx, &choice_idx) in filtered_choices.iter().enumerate() {
        if elements.len() >= limit {
            break;
        }

        let choice = &choices[choice_idx];
        let is_selected = selected.contains(&choice_idx);
        let is_focused = display_idx == focused_index;

        elements.push(crate::protocol::ElementInfo {
            semantic_id: select_choice_semantic_id(choice, choice_idx),
            element_type: crate::protocol::ElementType::Choice,
            text: Some(choice.name.clone()),
            value: Some(choice.value.clone()),
            content: None,
            selected: Some(is_selected),
            focused: Some(is_focused),
            index: Some(display_idx),
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

/// SelectPrompt - Multi-select from choices
///
/// Allows selecting multiple items from a list of choices.
/// Use Cmd/Ctrl+Space to toggle selection, Enter to submit selected items.
pub struct SelectPrompt {
    /// Unique ID for this prompt instance
    pub id: String,
    /// Placeholder text for the search input
    pub placeholder: Option<String>,
    /// Available choices
    pub choices: Vec<Choice>,
    /// Cached searchable/indexed choice data to reduce refilter work
    pub(super) choice_index: Vec<SelectChoiceIndex>,
    /// Indices of selected choices
    pub selected: HashSet<usize>,
    /// Visible guidance after a blocked zero-selection submit.
    pub submission_hint: Option<String>,
    /// Filtered choice indices (for display)
    pub filtered_choices: Vec<usize>,
    /// Currently focused index in filtered list
    pub focused_index: usize,
    /// Currently hovered index in filtered list
    pub hovered_index: Option<usize>,
    /// Filter text
    pub filter_text: String,
    /// Whether multiple selection is allowed
    pub multiple: bool,
    /// Focus handle for keyboard input
    pub focus_handle: FocusHandle,
    /// Callback when user submits
    pub on_submit: SubmitCallback,
    /// Theme for styling
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling
    pub design_variant: DesignVariant,
    /// Scroll handle for virtualized choices list
    pub list_scroll_handle: UniformListScrollHandle,
}
impl SelectPrompt {
    pub fn new(
        id: String,
        placeholder: Option<String>,
        choices: Vec<Choice>,
        multiple: bool,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        logging::log(
            "PROMPTS",
            &format!(
                "SelectPrompt::new with {} choices (multiple: {})",
                choices.len(),
                multiple
            ),
        );
        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::minimal_list("prompts::select", true),
        );

        let filtered_choices: Vec<usize> = (0..choices.len()).collect();
        let choice_index: Vec<SelectChoiceIndex> = choices
            .iter()
            .enumerate()
            .map(|(source_index, choice)| SelectChoiceIndex::from_choice(choice, source_index))
            .collect();

        SelectPrompt {
            id,
            placeholder,
            choices,
            choice_index,
            selected: HashSet::new(),
            submission_hint: None,
            filtered_choices,
            focused_index: 0,
            hovered_index: None,
            filter_text: String::new(),
            multiple,
            focus_handle,
            on_submit,
            theme,
            design_variant: DesignVariant::Default,
            list_scroll_handle: UniformListScrollHandle::new(),
        }
    }

    /// Refilter choices based on current filter_text
    fn refilter(&mut self) {
        let trimmed_filter = self.filter_text.trim();
        if trimmed_filter.is_empty() {
            self.filtered_choices = (0..self.choices.len()).collect();
            self.focused_index = 0;
            self.hovered_index = None;
            return;
        }

        let query_lower = trimmed_filter.to_lowercase();
        let mut nucleo = scripts::NucleoCtx::new(trimmed_filter);
        let mut scored_matches: Vec<(usize, u32)> = self
            .choices
            .iter()
            .enumerate()
            .filter_map(|(idx, choice)| {
                score_choice_for_filter(choice, &self.choice_index[idx], &query_lower, &mut nucleo)
                    .map(|score| (idx, score))
            })
            .collect();

        scored_matches.sort_by(|(a_idx, a_score), (b_idx, b_score)| {
            b_score.cmp(a_score).then_with(|| {
                self.choice_index[*a_idx]
                    .name_lower
                    .cmp(&self.choice_index[*b_idx].name_lower)
            })
        });

        self.filtered_choices = scored_matches.into_iter().map(|(idx, _)| idx).collect();
        self.focused_index = 0;
        self.hovered_index = None;
    }

    /// Set the filter text programmatically
    pub fn set_input(&mut self, text: String, cx: &mut Context<Self>) {
        if self.filter_text == text {
            return;
        }

        self.filter_text = text;
        self.refilter();
        self.list_scroll_handle
            .scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    /// Toggle selection of currently focused item
    pub(super) fn toggle_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(&choice_idx) = self.filtered_choices.get(self.focused_index) {
            if toggle_choice_selection(&mut self.selected, choice_idx, self.multiple) {
                self.submission_hint = None;
                cx.notify();
            }
        }
    }

    /// Submit selected items as JSON array
    pub(super) fn submit(&mut self, cx: &mut Context<Self>) -> bool {
        if !select_submission_is_allowed(self.multiple, self.selected.len()) {
            self.submission_hint = Some("Select at least one item".to_string());
            cx.notify();
            return false;
        }
        let mut selected_indices: Vec<usize> = self.selected.iter().copied().collect();
        selected_indices.sort_unstable();
        let focused_choice_index = self.filtered_choices.get(self.focused_index).copied();
        let resolved_indices =
            resolve_submission_indices(self.multiple, &selected_indices, focused_choice_index);

        let selected_values: Vec<String> = resolved_indices
            .iter()
            .filter_map(|&idx| self.choices.get(idx).map(|choice| choice.value.clone()))
            .collect();

        let json_str = serde_json::to_string(&selected_values).unwrap_or_else(|_| "[]".to_string());
        (self.on_submit)(self.id.clone(), Some(json_str));
        true
    }

    /// Cancel - submit None
    pub(super) fn submit_cancel(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }

    /// Move focus up
    pub(super) fn move_up(&mut self, cx: &mut Context<Self>) {
        if self.focused_index > 0 {
            self.focused_index -= 1;
            self.hovered_index = None;
            self.list_scroll_handle
                .scroll_to_item(self.focused_index, ScrollStrategy::Nearest);
            cx.notify();
        }
    }

    /// Move focus down
    pub(super) fn move_down(&mut self, cx: &mut Context<Self>) {
        if self.focused_index < self.filtered_choices.len().saturating_sub(1) {
            self.focused_index += 1;
            self.hovered_index = None;
            self.list_scroll_handle
                .scroll_to_item(self.focused_index, ScrollStrategy::Nearest);
            cx.notify();
        }
    }

    /// Handle character input
    pub(super) fn handle_char(&mut self, ch: char, cx: &mut Context<Self>) {
        if !should_append_to_filter(ch) {
            return;
        }
        self.filter_text.push(ch);
        self.refilter();
        self.list_scroll_handle
            .scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    /// Handle backspace
    pub(super) fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        if !self.filter_text.is_empty() {
            self.filter_text.pop();
            self.refilter();
            self.list_scroll_handle
                .scroll_to_item(0, ScrollStrategy::Top);
            cx.notify();
        }
    }

    /// Collect visible elements for getElements protocol introspection.
    pub(crate) fn collect_elements(
        &self,
        limit: usize,
    ) -> (Vec<crate::protocol::ElementInfo>, usize) {
        collect_select_prompt_elements(
            &self.filter_text,
            &self.choices,
            &self.filtered_choices,
            &self.selected,
            self.focused_index,
            limit,
        )
    }

    /// Select all choices (Ctrl+A)
    pub(super) fn toggle_select_all_filtered(&mut self, cx: &mut Context<Self>) {
        if !self.multiple {
            return;
        }

        toggle_filtered_selection(&mut self.selected, &self.filtered_choices);
        cx.notify();
    }
}
