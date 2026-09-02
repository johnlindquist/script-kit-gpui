impl ScriptListApp {
    pub(crate) fn resolved_main_menu_selected_subject(
        &self,
    ) -> Option<ResolvedMainMenuSelection<'_>> {
        if !matches!(self.current_view, AppView::ScriptList)
            || !self.root_search.query_is_current()
            || self.main_menu_committed_query_stamp() != self.root_search.computed_query_stamp()
            || self.spine_empty_subsearch_selection_suppressed()
            || self.menu_syntax_capture_form_owns_input()
        {
            return None;
        }
        let row = self
            .main_menu_committed_rows()
            .iter()
            .find(|row| row.grouped_index == self.selected_index && row.eligibility.selectable)?;
        match row.subject {
            MainMenuRowSubject::SearchResult { flat_index } => {
                Some(ResolvedMainMenuSelection::SearchResult {
                    row,
                    result: self.main_menu_committed_results().get(flat_index)?,
                })
            }
            MainMenuRowSubject::Calculator => Some(ResolvedMainMenuSelection::Calculator {
                row,
                result: self.main_menu_committed_calculator()?,
            }),
        }
    }

    pub(crate) fn resolve_main_menu_semantic_row(
        &self,
        semantic_id: &str,
    ) -> Option<&MainMenuRowProjection> {
        if !self.root_search.query_is_current()
            || self.main_menu_committed_query_stamp() != self.root_search.computed_query_stamp()
        {
            return None;
        }
        self.main_menu_committed_rows()
            .iter()
            .find(|row| row.semantic_id == semantic_id && row.eligibility.selectable)
    }
}

fn prepend_inline_calculator_group(
    grouped_items: Vec<GroupedListItem>,
    flat_results: Vec<scripts::SearchResult>,
    calculator: Option<&crate::calculator::CalculatorInlineResult>,
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let Some(_calculator) = calculator else {
        return (grouped_items, flat_results);
    };

    let mut merged_grouped_items = Vec::with_capacity(grouped_items.len() + 2);
    merged_grouped_items.push(GroupedListItem::SectionHeader(
        INLINE_CALCULATOR_SECTION_LABEL.to_string(),
        None,
    ));
    merged_grouped_items.push(GroupedListItem::Item(INLINE_CALCULATOR_RESULT_INDEX));
    merged_grouped_items.extend(grouped_items);

    (merged_grouped_items, flat_results)
}

fn build_menu_syntax_trigger_picker_main_list_results(
    snapshot: &crate::menu_syntax::TriggerPickerSnapshot,
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let section = snapshot.mode.main_list_section();
    let mut grouped_items = Vec::with_capacity(snapshot.rows.len() + 1);
    let mut flat_results = Vec::with_capacity(snapshot.rows.len());
    grouped_items.push(GroupedListItem::SectionHeader(
        section.0.to_string(),
        Some(section.1.to_string()),
    ));

    for row in &snapshot.rows {
        let flat_index = flat_results.len();
        flat_results.push(scripts::SearchResult::SpineProjection(
            crate::menu_syntax_trigger_picker::trigger_picker_row_to_main_list_row(row),
        ));
        grouped_items.push(GroupedListItem::Item(flat_index));
    }

    (grouped_items, flat_results)
}

fn build_menu_syntax_object_selector_main_list_results(
    snapshot: &crate::menu_syntax::ObjectSelectorSnapshot,
) -> (Vec<GroupedListItem>, Vec<scripts::SearchResult>) {
    let mut grouped_items = Vec::with_capacity(snapshot.rows.len() + 1);
    let mut flat_results = Vec::with_capacity(snapshot.rows.len());
    grouped_items.push(GroupedListItem::SectionHeader(
        "Objects".to_string(),
        Some("at-sign".to_string()),
    ));

    for row in &snapshot.rows {
        let flat_index = flat_results.len();
        flat_results.push(scripts::SearchResult::SpineProjection(
            crate::menu_syntax::object_selector_row_to_main_list_row(row),
        ));
        grouped_items.push(GroupedListItem::Item(flat_index));
    }

    (grouped_items, flat_results)
}
