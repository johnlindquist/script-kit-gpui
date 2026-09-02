use gpui::{Context, Window};

use crate::{AppView, ScriptListApp};

impl ScriptListApp {
    pub(crate) fn menu_syntax_object_selector_owns_main_keyboard(&self) -> bool {
        matches!(self.current_view, AppView::ScriptList)
            && self.menu_syntax_object_selector_state.owns_main_list()
    }

    pub(crate) fn selected_menu_syntax_object_selector_row_id_from_main_list(
        &mut self,
    ) -> Option<String> {
        let crate::ResolvedMainMenuSelection::SearchResult {
            result: crate::scripts::SearchResult::SpineProjection(row),
            ..
        } = self.resolved_main_menu_selected_subject()?
        else {
            return None;
        };
        match &row.action {
            crate::spine::SpineListAction::AcceptMenuSyntaxObject { row_id } => {
                Some(row_id.to_string())
            }
            _ => None,
        }
    }

    pub(crate) fn accept_menu_syntax_object_selector_row(
        &mut self,
        row_id: &str,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.menu_syntax_object_selector_owns_main_keyboard() {
            return false;
        }
        let Some(crate::ResolvedMainMenuSelection::SearchResult {
            row,
            result: crate::scripts::SearchResult::SpineProjection(projection),
            ..
        }) = self.resolved_main_menu_selected_subject()
        else {
            return false;
        };
        if !row.eligibility.activatable
            || !matches!(&projection.action, crate::spine::SpineListAction::AcceptMenuSyntaxObject { row_id: owner_id } if owner_id.as_ref() == row_id)
            || projection.id.as_ref().strip_prefix("menu-syntax-object:") != Some(row_id)
        {
            return false;
        }
        let observation = crate::MainMenuDispatchObservation {
            query: self.root_search.query_stamp(),
            stable_key: row.stable_key.clone(),
            content_fingerprint: row.content_fingerprint.clone(),
            status: "dispatchRequested",
            reason: None,
        };
        let Some(snapshot) = self
            .menu_syntax_object_selector_state
            .snapshot
            .as_ref()
            .cloned()
        else {
            return false;
        };
        let Some(selected_index) = snapshot.rows.iter().position(|row| row.id == row_id) else {
            return false;
        };
        let raw_filter_text = self.filter_text.clone();
        let outcome = crate::menu_syntax::apply_object_selector_intent(
            crate::menu_syntax::InlinePickerKeyIntent::Accept,
            &snapshot,
            Some(selected_index),
            &raw_filter_text,
        );
        if matches!(
            outcome,
            crate::menu_syntax::ObjectSelectorIntentOutcome::Ignored
                | crate::menu_syntax::ObjectSelectorIntentOutcome::SelectionChanged { .. }
        ) {
            return false;
        }
        self.dispatch_menu_syntax_object_selector_outcome(outcome, window, cx);
        self.set_main_menu_dispatch_observation(Some(observation));
        true
    }

    fn dispatch_menu_syntax_object_selector_outcome(
        &mut self,
        outcome: crate::menu_syntax::ObjectSelectorIntentOutcome,
        _window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        match outcome {
            crate::menu_syntax::ObjectSelectorIntentOutcome::Ignored
            | crate::menu_syntax::ObjectSelectorIntentOutcome::SelectionChanged { .. } => {}
            crate::menu_syntax::ObjectSelectorIntentOutcome::ReplaceInput { text } => {
                self.filter_text = text;
                self.pending_filter_sync = true;
                self.menu_syntax_object_selector_state = Default::default();
                self.flush_pending_main_menu_query(cx);
                cx.notify();
            }
            crate::menu_syntax::ObjectSelectorIntentOutcome::Close => {
                self.menu_syntax_object_selector_state = Default::default();
                self.flush_pending_main_menu_query(cx);
                cx.notify();
            }
        }
    }

    pub(crate) fn run_menu_syntax_object_selector_state_machine(
        &mut self,
        raw_filter: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.menu_syntax_form_input_active && self.menu_syntax_capture_form_owns_input() {
            self.menu_syntax_object_selector_state = Default::default();
            self.invalidate_grouped_cache();
            cx.notify();
            return;
        }
        let capture_targets =
            crate::menu_syntax::registered_capture_targets_from_scripts(&self.scripts);
        let ctx = crate::menu_syntax::ObjectSelectorContext {
            candidates: self.menu_syntax_object_candidates_for_filter(raw_filter),
        };
        let transition = crate::menu_syntax::plan_object_selector_transition(
            &self.menu_syntax_object_selector_state,
            raw_filter,
            &capture_targets,
            &ctx,
        );
        match transition {
            crate::menu_syntax::ObjectSelectorTransition::NoChange => {}
            crate::menu_syntax::ObjectSelectorTransition::Close => {
                self.menu_syntax_object_selector_state = Default::default();
                self.invalidate_grouped_cache();
                cx.notify();
            }
            crate::menu_syntax::ObjectSelectorTransition::Open {
                snapshot,
                selected_row_id,
            } => {
                self.menu_syntax_object_selector_state =
                    crate::menu_syntax::MenuSyntaxObjectSelectorState {
                        snapshot: Some(snapshot),
                        selected_row_id,
                        visible_start: 0,
                    };
                self.menu_syntax_trigger_picker_state = Default::default();
                self.invalidate_grouped_cache();
                cx.notify();
            }
            crate::menu_syntax::ObjectSelectorTransition::Update {
                snapshot,
                selected_row_id,
            } => {
                let selected_index = selected_row_id
                    .as_deref()
                    .and_then(|id| snapshot.rows.iter().position(|row| row.id == id))
                    .unwrap_or(0);
                let visible_start = crate::menu_syntax::object_selector_visible_start_for_selection(
                    self.menu_syntax_object_selector_state.visible_start,
                    selected_index,
                    snapshot.rows.len(),
                );
                self.menu_syntax_object_selector_state =
                    crate::menu_syntax::MenuSyntaxObjectSelectorState {
                        snapshot: Some(snapshot),
                        selected_row_id,
                        visible_start,
                    };
                self.menu_syntax_trigger_picker_state = Default::default();
                self.invalidate_grouped_cache();
                cx.notify();
            }
        }
    }

    pub(crate) fn menu_syntax_object_candidates_for_filter(
        &self,
        raw_filter: &str,
    ) -> Vec<crate::menu_syntax::ObjectSelectorCandidate> {
        let capture_targets =
            crate::menu_syntax::registered_capture_targets_from_scripts(&self.scripts);
        let Some(selector) = crate::menu_syntax::capture::active_object_selector_for_input(
            raw_filter,
            &capture_targets,
        ) else {
            return Vec::new();
        };
        let query = selector.query.trim();
        match selector.kind {
            crate::menu_syntax::CaptureObjectKind::Note => {
                crate::notes::search_root_notes_meta_direct(
                    query,
                    crate::notes::RootNotesSectionOptions {
                        enabled: true,
                        max_results: 10,
                        min_query_chars: 0,
                        search_content: true,
                    },
                )
                .into_iter()
                .map(|hit| crate::menu_syntax::ObjectSelectorCandidate {
                    kind: crate::menu_syntax::CaptureObjectKind::Note,
                    id: hit.id.to_string(),
                    label: if hit.title.trim().is_empty() {
                        "Untitled Note".to_string()
                    } else {
                        hit.title
                    },
                    subtitle: format!(
                        "Updated {} - {} chars",
                        crate::formatting::format_relative_time_short_dt(hit.updated_at),
                        hit.char_count
                    ),
                })
                .collect()
            }
            kind => crate::menu_syntax::search_root_object_candidates_direct(kind, query, 10),
        }
    }

    pub(crate) fn apply_menu_syntax_object_selector_intent(
        &mut self,
        intent: crate::menu_syntax::InlinePickerKeyIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if matches!(self.current_view, AppView::ScriptList) {
            self.flush_pending_main_menu_query(cx);
        }
        let main_list_activation = self.menu_syntax_object_selector_owns_main_keyboard()
            && matches!(
                intent,
                crate::menu_syntax::InlinePickerKeyIntent::Accept
                    | crate::menu_syntax::InlinePickerKeyIntent::Apply
                    | crate::menu_syntax::InlinePickerKeyIntent::SecondaryAction
                    | crate::menu_syntax::InlinePickerKeyIntent::CreateAction
            );
        let observation = if main_list_activation {
            self.set_main_menu_dispatch_observation(None);
            let Some(subject) = self.resolved_main_menu_selected_subject() else {
                return false;
            };
            let crate::ResolvedMainMenuSelection::SearchResult { row, .. } = subject else {
                return false;
            };
            if !row.eligibility.activatable {
                return false;
            }
            Some(crate::MainMenuDispatchObservation {
                query: self.root_search.query_stamp(),
                stable_key: row.stable_key.clone(),
                content_fingerprint: row.content_fingerprint.clone(),
                status: "dispatchRequested",
                reason: None,
            })
        } else {
            None
        };
        let Some(snapshot) = self
            .menu_syntax_object_selector_state
            .snapshot
            .as_ref()
            .cloned()
        else {
            return false;
        };
        let selected_row_id = self.selected_menu_syntax_object_selector_row_id_from_main_list();
        if main_list_activation && selected_row_id.is_none() {
            return false;
        }
        let selected_row_id = selected_row_id.or_else(|| {
            self.menu_syntax_object_selector_state
                .selected_row_id
                .clone()
        });
        let selected_index = selected_row_id
            .as_deref()
            .and_then(|id| snapshot.rows.iter().position(|row| row.id == id));
        if main_list_activation && selected_index.is_none() {
            return false;
        }
        let raw_filter_text = self.filter_text.clone();
        let outcome = crate::menu_syntax::apply_object_selector_intent(
            intent,
            &snapshot,
            selected_index,
            &raw_filter_text,
        );
        match outcome {
            crate::menu_syntax::ObjectSelectorIntentOutcome::SelectionChanged { new_index } => {
                let next_row_id = snapshot.rows.get(new_index).map(|row| row.id.clone());
                if self.menu_syntax_object_selector_owns_main_keyboard() {
                    let Some(id) = next_row_id.as_deref() else {
                        return false;
                    };
                    let Some(index) = self.main_menu_committed_rows().iter().find_map(|row| {
                        (row.eligibility.selectable
                            && row.stable_key.strip_prefix("menu-syntax-object:") == Some(id))
                        .then_some(row.grouped_index)
                    }) else {
                        return false;
                    };
                    if !self.select_main_menu_row(
                        index,
                        crate::MainMenuSelectionOrigin::Keyboard,
                        cx,
                    ) {
                        return false;
                    }
                    self.reveal_main_list_selection_above_footer(
                        "menu_syntax_object_selector_selection",
                    );
                    self.schedule_main_list_selection_reveal_above_footer(
                        "menu_syntax_object_selector_selection",
                        cx,
                    );
                }
                self.menu_syntax_object_selector_state.visible_start =
                    crate::menu_syntax::object_selector_visible_start_for_selection(
                        self.menu_syntax_object_selector_state.visible_start,
                        new_index,
                        snapshot.rows.len(),
                    );
                self.menu_syntax_object_selector_state.selected_row_id = next_row_id;
                cx.notify();
                true
            }
            crate::menu_syntax::ObjectSelectorIntentOutcome::Ignored => false,
            other => {
                self.dispatch_menu_syntax_object_selector_outcome(other, Some(window), cx);
                if let Some(observation) = observation {
                    self.set_main_menu_dispatch_observation(Some(observation));
                }
                true
            }
        }
    }
}
