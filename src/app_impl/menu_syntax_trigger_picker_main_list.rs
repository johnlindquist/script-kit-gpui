use std::io;
use std::path::Path;

use gpui::{Context, Window};

use crate::ScriptListApp;

struct AppCaptureHandlerScaffoldEffects<'a> {
    config: &'a crate::config::Config,
}
impl crate::menu_syntax::CaptureHandlerScaffoldEffects for AppCaptureHandlerScaffoldEffects<'_> {
    fn path_exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn write_file(&self, path: &Path, contents: &str) -> io::Result<()> {
        std::fs::write(path, contents)
    }

    fn open_in_editor(&self, path: &Path) -> io::Result<()> {
        crate::script_creation::open_in_editor(path, self.config)
            .map_err(io::Error::other)
            .or_else(|_| {
                let _child = std::process::Command::new("open").arg(path).spawn()?;
                Ok(())
            })
    }
}

impl ScriptListApp {
    fn menu_syntax_type_filter_accept_label(filter: &str) -> Option<&'static str> {
        match filter.trim() {
            "type:script" => Some("Showing Scripts"),
            "type:scriptlet" => Some("Showing Scriptlets"),
            "type:skill" => Some("Showing Skills"),
            "type:builtin" => Some("Showing Built-ins"),
            "type:app" => Some("Showing Apps"),
            "type:window" => Some("Showing Windows"),
            "type:agent" => Some("Showing Agents"),
            "type:issue" => Some("Showing Script Issues"),
            _ => None,
        }
    }

    fn arm_menu_syntax_filter_accept_hint(&mut self, filter: &str) {
        let Some(label) = Self::menu_syntax_type_filter_accept_label(filter) else {
            self.clear_menu_syntax_filter_accept_hint();
            return;
        };

        self.menu_syntax_filter_accept_hint_label = Some(label.to_string());
        self.menu_syntax_filter_accept_hint_filter = Some(filter.to_string());
        self.menu_syntax_filter_accept_hint_selected_index = Some(self.selected_index);
        tracing::info!(
            target: "script_kit::menu_syntax_popup",
            event = "menu_syntax_filter_accept_hint_armed",
            filter = %filter,
            selected_index = self.selected_index,
            label,
            "menu-syntax filter accept hint armed"
        );
    }

    pub(crate) fn clear_menu_syntax_filter_accept_hint(&mut self) {
        self.menu_syntax_filter_accept_hint_label = None;
        self.menu_syntax_filter_accept_hint_filter = None;
        self.menu_syntax_filter_accept_hint_selected_index = None;
    }

    pub(crate) fn menu_syntax_filter_accept_primary_label(&self) -> Option<&str> {
        if !matches!(self.current_view, crate::AppView::ScriptList) {
            return None;
        }

        let label = self.menu_syntax_filter_accept_hint_label.as_deref()?;
        let filter = self.menu_syntax_filter_accept_hint_filter.as_deref()?;
        let selected_index = self.menu_syntax_filter_accept_hint_selected_index?;
        (self.filter_text == filter && self.selected_index == selected_index).then_some(label)
    }

    pub(crate) fn should_consume_menu_syntax_filter_accept_enter(
        &mut self,
        route: &'static str,
    ) -> bool {
        let Some(label) = self
            .menu_syntax_filter_accept_primary_label()
            .map(str::to_string)
        else {
            return false;
        };

        tracing::info!(
            target: "script_kit::menu_syntax_popup",
            event = "menu_syntax_filter_accept_enter_consumed",
            route,
            filter = %self.filter_text,
            computed_filter = %self.computed_filter_text,
            selected_index = self.selected_index,
            label,
            "menu-syntax accepted filter consumed Enter before auto-selected row execution"
        );
        true
    }

    pub(crate) fn should_consume_menu_syntax_filter_accept_submit(
        &mut self,
        route: &'static str,
    ) -> bool {
        self.should_consume_menu_syntax_filter_accept_enter(route)
    }

    pub(crate) fn menu_syntax_trigger_picker_owns_main_keyboard(&self) -> bool {
        matches!(self.current_view, crate::AppView::ScriptList)
            && self.menu_syntax_trigger_picker_state.owns_main_list()
    }

    pub(crate) fn arm_menu_syntax_trigger_picker_enter_guard(&mut self, route: &'static str) {
        self.menu_syntax_trigger_picker_enter_guard = Some(std::time::Instant::now());
        tracing::info!(
            target: "script_kit::menu_syntax_popup",
            event = "menu_syntax_trigger_picker_enter_guard_armed",
            route,
            filter = %self.filter_text,
            computed_filter = %self.computed_filter_text,
            selected_index = self.selected_index,
            "menu-syntax trigger picker Enter guard armed"
        );
    }

    pub(crate) fn should_consume_menu_syntax_trigger_picker_press_enter(
        &mut self,
        route: &'static str,
    ) -> bool {
        const ENTER_ECHO_GUARD_MS: u128 = 250;
        let Some(armed_at) = self.menu_syntax_trigger_picker_enter_guard.take() else {
            return false;
        };

        let age_ms = armed_at.elapsed().as_millis();
        let consume = age_ms <= ENTER_ECHO_GUARD_MS;
        tracing::info!(
            target: "script_kit::menu_syntax_popup",
            event = if consume {
                "menu_syntax_trigger_picker_press_enter_consumed"
            } else {
                "menu_syntax_trigger_picker_press_enter_guard_expired"
            },
            route,
            age_ms,
            guard_ms = ENTER_ECHO_GUARD_MS,
            filter = %self.filter_text,
            computed_filter = %self.computed_filter_text,
            selected_index = self.selected_index,
            "menu-syntax trigger picker PressEnter guard checked"
        );

        consume
    }

    /// Accept the current canonical picker row for pointer or semantic callers.
    /// Returns whether an actionable outcome was dispatched; keep-open state
    /// is settled before the replacement query is published.
    pub(crate) fn accept_menu_syntax_trigger_picker_row(
        &mut self,
        row_id: &str,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some((field_id, suggestion_index)) =
            Self::parse_trigger_picker_form_suggestion_row_id(row_id)
        {
            let Some(window) = window else {
                return false;
            };
            return self.accept_menu_syntax_form_trigger_picker_suggestion(
                field_id,
                suggestion_index,
                window,
                cx,
            );
        }
        if !self.menu_syntax_trigger_picker_owns_main_keyboard() {
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
            || !matches!(&projection.action, crate::spine::SpineListAction::AcceptMenuSyntaxTrigger { row_id: owner_id } if owner_id.as_ref() == row_id)
            || projection.id.as_ref().strip_prefix("menu-syntax-trigger:") != Some(row_id)
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
            .menu_syntax_trigger_picker_state
            .snapshot
            .as_ref()
            .cloned()
        else {
            tracing::warn!(
                target: "script_kit::menu_syntax_popup",
                event = "menu_syntax_trigger_picker_accept_missing_snapshot",
                row_id,
                filter = %self.filter_text,
                computed_filter = %self.computed_filter_text,
                selected_index = self.selected_index,
                "menu-syntax trigger picker accept missing snapshot"
            );
            return false;
        };
        let Some(selected_index) = snapshot.rows.iter().position(|row| row.id == row_id) else {
            tracing::warn!(
                target: "script_kit::menu_syntax_popup",
                event = "menu_syntax_trigger_picker_accept_missing_row",
                row_id,
                filter = %self.filter_text,
                computed_filter = %self.computed_filter_text,
                selected_index = self.selected_index,
                "menu-syntax trigger picker accept missing row"
            );
            return false;
        };
        let raw_filter_text = self.filter_text.clone();
        tracing::info!(
            target: "script_kit::menu_syntax_popup",
            event = "menu_syntax_trigger_picker_accept_start",
            row_id,
            trigger_selected_index = selected_index,
            filter = %self.filter_text,
            computed_filter = %self.computed_filter_text,
            selected_index = self.selected_index,
            "menu-syntax trigger picker accept started"
        );
        let outcome = crate::menu_syntax::apply_intent(
            crate::menu_syntax::InlinePickerKeyIntent::Accept,
            &snapshot,
            Some(selected_index),
            &raw_filter_text,
        );
        if matches!(
            outcome,
            crate::menu_syntax::TriggerPickerIntentOutcome::Ignored
                | crate::menu_syntax::TriggerPickerIntentOutcome::SelectionChanged { .. }
        ) {
            return false;
        }
        self.dispatch_menu_syntax_trigger_picker_outcome(Some(row_id), outcome, window, cx);
        self.set_main_menu_dispatch_observation(Some(observation));
        true
    }

    fn dispatch_menu_syntax_trigger_picker_outcome(
        &mut self,
        row_id: Option<&str>,
        outcome: crate::menu_syntax::TriggerPickerIntentOutcome,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        use crate::menu_syntax::TriggerPickerIntentOutcome;
        match outcome {
            TriggerPickerIntentOutcome::Ignored
            | TriggerPickerIntentOutcome::SelectionChanged { .. } => {}
            TriggerPickerIntentOutcome::ReplaceInput { text, keep_open } => {
                tracing::info!(
                    target: "script_kit::menu_syntax_popup",
                    event = "menu_syntax_trigger_picker_accept_outcome",
                    row_id = row_id.unwrap_or(""),
                    outcome = "replace_input",
                    replacement = %text,
                    keep_open,
                    filter_before = %self.filter_text,
                    computed_before = %self.computed_filter_text,
                    selected_index = self.selected_index,
                    "menu-syntax trigger picker accept outcome"
                );
                let filter_accept_hint_filter = if keep_open {
                    None
                } else {
                    Self::menu_syntax_type_filter_accept_label(&text).map(|_| text.clone())
                };
                // Stage the replacement; the input widget synchronizes on its
                // next frame, after the final picker owner is committed below.
                self.filter_text = text.clone();
                self.pending_filter_sync = true;

                if keep_open {
                    // Re-run the picker state machine against the new filter
                    // before rebuilding grouped rows so the cache stores rows
                    // for the next picker snapshot, not the stale one.
                    if let Some(window) = window {
                        self.run_menu_syntax_trigger_picker_state_machine(&text, window, cx);
                    } else {
                        let picker_ctx = self.menu_syntax_trigger_picker_context(&text);
                        let transition =
                            crate::menu_syntax_trigger_picker::plan_trigger_picker_transition(
                                &self.menu_syntax_trigger_picker_state,
                                &text,
                                &picker_ctx,
                            );
                        use crate::menu_syntax_trigger_picker::TriggerPickerTransition;
                        match transition {
                            TriggerPickerTransition::NoChange => {}
                            TriggerPickerTransition::Close => {
                                self.menu_syntax_trigger_picker_state = Default::default();
                            }
                            TriggerPickerTransition::Open {
                                snapshot,
                                selected_row_id,
                            }
                            | TriggerPickerTransition::Update {
                                snapshot,
                                selected_row_id,
                            } => {
                                self.menu_syntax_trigger_picker_state =
                                    crate::menu_syntax_trigger_picker::MenuSyntaxTriggerPickerState {
                                        snapshot: Some(snapshot), selected_row_id, visible_start: 0,
                                    };
                            }
                        }
                    }
                } else {
                    self.menu_syntax_trigger_picker_state = Default::default();
                    // Mark this exact filter text as "user just accepted,
                    // do not re-open the picker". Without this, pressing
                    // Enter on `;` selects `;todo`, sets the filter to
                    // `;todo ` which parses to
                    // `Incomplete(MissingCaptureBody)`, and the next
                    // `handle_filter_input_change` re-runs
                    // `plan_trigger_picker_transition` -> `Open` with the
                    // handler snapshot - the picker flickers back open
                    // immediately after the user dismissed it. The
                    // suppression is cleared as soon as the filter text
                    // changes (user types a body character or deletes).
                    self.menu_syntax_trigger_picker_suppressed_filter = Some(text.clone());
                }

                self.flush_pending_main_menu_query(cx);
                if let Some(filter) = filter_accept_hint_filter {
                    self.arm_menu_syntax_filter_accept_hint(&filter);
                    self.invalidate_main_window_preflight();
                    self.rebuild_main_window_preflight_if_needed();
                }
                cx.notify();
            }
            TriggerPickerIntentOutcome::Close => {
                self.menu_syntax_trigger_picker_state = Default::default();
                self.flush_pending_main_menu_query(cx);
                cx.notify();
            }
            TriggerPickerIntentOutcome::OpenCaptures { .. }
            | TriggerPickerIntentOutcome::OpenHelp => {
                // Deferred — these routes wire through in follow-up work.
                // For now, treat as a close so the picker dismisses instead
                // of lingering with a stale snapshot.
                self.menu_syntax_trigger_picker_state = Default::default();
                self.flush_pending_main_menu_query(cx);
                cx.notify();
            }
            TriggerPickerIntentOutcome::CreateHandler { target } => {
                if let Some(slug) = target {
                    let effects = AppCaptureHandlerScaffoldEffects {
                        config: &self.config,
                    };
                    let scripts_dir = crate::script_creation::scripts_dir();
                    match crate::menu_syntax::create_capture_handler_scaffold(
                        &effects,
                        &scripts_dir,
                        &slug,
                        true,
                    ) {
                        Ok(created) => {
                            self.filter_text.clear();
                            self.pending_filter_sync = true;
                            self.computed_filter_text.clear();
                            self.set_menu_syntax_mode_from_filter("");
                            self.invalidate_grouped_cache();
                            self.show_hud(
                                format!("Created {}", created.filename),
                                Some(crate::HUD_SHORT_MS),
                                cx,
                            );
                        }
                        Err(error) => {
                            tracing::warn!(
                                target: "script_kit::menu_syntax",
                                event = "create_capture_handler_failed",
                                slug = %slug,
                                error = %error,
                            );
                            self.show_error_toast(format!("Create handler failed: {error}"), cx);
                        }
                    }
                }
                self.menu_syntax_trigger_picker_state = Default::default();
                self.flush_pending_main_menu_query(cx);
                cx.notify();
            }
            TriggerPickerIntentOutcome::AiScaffoldHandler {
                slug,
                nearest_targets,
            } => {
                let nearest = if nearest_targets.is_empty() {
                    "none".to_string()
                } else {
                    nearest_targets.join(", ")
                };
                let mut chars = slug.chars();
                let capitalized = match chars.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                };
                let prompt = format!(
                    "You are a helpful assistant guiding the user through creating a new Script Kit capture handler.\n\
                     The user typed `;{slug}` in the launcher, but does not have a capture handler for it yet.\n\
                     Nearest existing targets: {nearest}\n\n\
                     Existing capture handler examples in Script Kit:\n\
                     - `todo` (targets: [\"todo\"], accepts: [\"tags\", \"date\", \"priority\", \"url\", \"kv\"]) -> Appends a task line to `$SK_PATH/brain/days/YYYY-MM-DD.md`\n\
                     - `cal` (targets: [\"cal\"], accepts: [\"date\", \"duration\", \"tags\", \"kv\"]) -> Appends to `$SK_PATH/menu-syntax/events.jsonl`\n\
                     - `note` (targets: [\"note\"], accepts: [\"tags\", \"date\", \"kv\"]) -> Appends to `$SK_PATH/menu-syntax/notes.jsonl`\n\
                     - `social` (targets: [\"social\"], accepts: [\"tags\", \"url\", \"kv\"]) -> Appends to `$SK_PATH/menu-syntax/drafts.jsonl`\n\
                     - `link` (targets: [\"link\"], accepts: [\"url\", \"tags\", \"kv\"]) -> Appends to `$SK_PATH/menu-syntax/bookmarks.jsonl`\n\n\
                     Your task is to walk the user through scaffolding a capture handler for target \"{slug}\".\n\n\
                     Do NOT generate the final code immediately. Instead, start by introducing yourself, explain that you will help them build their ;{slug} capture handler, and ask them a series of questions to understand their needs:\n\
                     1. What human-readable name/label should this handler have? (e.g. \"Capture {capitalized}\")\n\
                     2. What fields/parameters should it accept from the captured text? (e.g. tags, dates, priority, URLs, custom key-values)\n\
                     3. What should the handler do when it executes? (e.g. append to a local JSONL file, call a webhook/API, run a shell command, etc.)"
                );
                self.menu_syntax_trigger_picker_state = Default::default();
                self.open_tab_ai_agent_chat_with_entry_intent_preserving_return(Some(prompt), cx);
                cx.notify();
            }
        }
    }

    fn parse_trigger_picker_form_suggestion_row_id(row_id: &str) -> Option<(&str, usize)> {
        let mut parts = row_id.split(':');
        match (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) {
            (Some("form-suggestion"), Some(_target), Some(field_id), Some(index), None) => {
                index.parse::<usize>().ok().map(|index| (field_id, index))
            }
            _ => None,
        }
    }

    fn menu_syntax_trigger_picker_state_is_form_suggestion(&self) -> bool {
        self.menu_syntax_trigger_picker_state
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.target.as_deref())
            .is_some_and(|target| target.starts_with("form:"))
    }

    pub(crate) fn selected_menu_syntax_trigger_row_id_from_main_list(&mut self) -> Option<String> {
        let crate::ResolvedMainMenuSelection::SearchResult {
            result: crate::scripts::SearchResult::SpineProjection(row),
            ..
        } = self.resolved_main_menu_selected_subject()?
        else {
            return None;
        };
        match &row.action {
            crate::spine::SpineListAction::AcceptMenuSyntaxTrigger { row_id } => {
                Some(row_id.to_string())
            }
            _ => None,
        }
    }

    fn sync_menu_syntax_form_selection_from_trigger_row(&mut self, row_id: Option<&str>) {
        if let Some((field_id, suggestion_index)) =
            row_id.and_then(Self::parse_trigger_picker_form_suggestion_row_id)
        {
            self.menu_syntax_form_suggestion_field_id = Some(field_id.to_string());
            self.menu_syntax_form_suggestion_selected_index = Some(suggestion_index);
        }
    }

    fn close_menu_syntax_form_trigger_picker(&mut self, cx: &mut Context<Self>) {
        self.menu_syntax_form_suggestion_field_id = None;
        self.menu_syntax_form_suggestion_selected_index = None;
        self.menu_syntax_trigger_picker_state = Default::default();
        self.flush_pending_main_menu_query(cx);
        cx.notify();
    }

    fn accept_menu_syntax_form_trigger_picker_suggestion(
        &mut self,
        field_id: &str,
        suggestion_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(snapshot) = self.menu_syntax_main_hint_snapshot(&self.filter_text, false) else {
            return false;
        };
        let Some(form) = snapshot.form else {
            return false;
        };
        let Some(field) = form.fields.iter().find(|field| field.id == field_id) else {
            return false;
        };
        let Some(suggestion) = field.suggestions.get(suggestion_index) else {
            return false;
        };
        let Some(application) =
            crate::menu_syntax::apply_menu_syntax_form_suggestion(field, suggestion)
        else {
            return false;
        };

        self.menu_syntax_form_draft_field_id = Some(field.id.clone());
        self.menu_syntax_form_draft_value = application.next_field_value.clone();
        let updated = self.update_menu_syntax_form_field(
            Some(&field.id),
            application.next_field_value,
            window,
            cx,
        );
        if updated {
            self.close_menu_syntax_form_trigger_picker(cx);
        }
        updated
    }

    /// Re-run the picker state machine against a (possibly new) filter text
    /// and dispatch the resulting transition to the GPUI window. Extracted
    /// here so both `apply_menu_syntax_trigger_picker_intent` (keyboard
    /// Tab-apply path) and `handle_filter_input_change` can share the
    /// state-machine invocation.
    pub(crate) fn run_menu_syntax_trigger_picker_state_machine(
        &mut self,
        raw_filter: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        let picker_ctx = self.menu_syntax_trigger_picker_context(raw_filter);
        let transition = crate::menu_syntax_trigger_picker::plan_trigger_picker_transition(
            &self.menu_syntax_trigger_picker_state,
            raw_filter,
            &picker_ctx,
        );
        use crate::menu_syntax_trigger_picker::TriggerPickerTransition;
        match transition {
            TriggerPickerTransition::NoChange => {}
            TriggerPickerTransition::Close => {
                self.menu_syntax_trigger_picker_state = Default::default();
            }
            TriggerPickerTransition::Open {
                snapshot,
                selected_row_id,
            } => {
                self.menu_syntax_trigger_picker_state =
                    crate::menu_syntax_trigger_picker::MenuSyntaxTriggerPickerState {
                        snapshot: Some(snapshot),
                        selected_row_id,
                        visible_start: 0,
                    };
            }
            TriggerPickerTransition::Update {
                snapshot,
                selected_row_id,
            } => {
                let selected_index = selected_row_id
                    .as_deref()
                    .and_then(|id| snapshot.rows.iter().position(|row| row.id == id))
                    .unwrap_or(0);
                let visible_start =
                    crate::menu_syntax_trigger_picker::trigger_picker_visible_start_for_selection(
                        self.menu_syntax_trigger_picker_state.visible_start,
                        selected_index,
                        snapshot.rows.len(),
                    );
                self.menu_syntax_trigger_picker_state =
                    crate::menu_syntax_trigger_picker::MenuSyntaxTriggerPickerState {
                        snapshot: Some(snapshot),
                        selected_row_id,
                        visible_start,
                    };
            }
        }
    }

    pub(crate) fn menu_syntax_trigger_picker_context(
        &self,
        _raw_filter: &str,
    ) -> crate::menu_syntax::TriggerPickerContext {
        crate::menu_syntax::TriggerPickerContext {
            recent_queries: self.input_history.recent_entries(8),
            scripts: self.scripts.clone(),
            scriptlets: self.scriptlets.clone(),
        }
    }

    /// Keyboard entry point for the menu-syntax trigger picker. Keyboard
    /// interceptors in `startup.rs` (arrow keys), `startup_new_tab.rs`
    /// (Tab / Enter), and `render_script_list/mod.rs` (Escape) call this
    /// when the picker is active. Returns `true` when the intent was consumed
    /// and the caller should NOT route the keystroke anywhere else.
    pub(crate) fn apply_menu_syntax_trigger_picker_intent(
        &mut self,
        intent: crate::menu_syntax::InlinePickerKeyIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.menu_syntax_trigger_picker_state_is_form_suggestion() {
            match intent {
                crate::menu_syntax::InlinePickerKeyIntent::Close => {
                    self.close_menu_syntax_form_trigger_picker(cx);
                    return true;
                }
                crate::menu_syntax::InlinePickerKeyIntent::Accept
                | crate::menu_syntax::InlinePickerKeyIntent::Apply => {
                    let selected_row_id = self
                        .menu_syntax_trigger_picker_state
                        .selected_row_id
                        .clone()
                        .or_else(|| {
                            self.menu_syntax_trigger_picker_state
                                .snapshot
                                .as_ref()
                                .and_then(|snapshot| {
                                    snapshot.rows.first().map(|row| row.id.clone())
                                })
                        });
                    let Some(row_id) = selected_row_id else {
                        return false;
                    };
                    if let Some((field_id, suggestion_index)) =
                        Self::parse_trigger_picker_form_suggestion_row_id(&row_id)
                    {
                        return self.accept_menu_syntax_form_trigger_picker_suggestion(
                            field_id,
                            suggestion_index,
                            window,
                            cx,
                        );
                    }
                    return false;
                }
                _ => {}
            }
        }
        if matches!(self.current_view, crate::AppView::ScriptList)
            && !self.menu_syntax_trigger_picker_state_is_form_suggestion()
        {
            self.flush_pending_main_menu_query(cx);
        }
        let main_list_activation = self.menu_syntax_trigger_picker_owns_main_keyboard()
            && !self.menu_syntax_trigger_picker_state_is_form_suggestion()
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
            .menu_syntax_trigger_picker_state
            .snapshot
            .as_ref()
            .cloned()
        else {
            return false;
        };

        let selected_row_id = self.selected_menu_syntax_trigger_row_id_from_main_list();
        if main_list_activation && selected_row_id.is_none() {
            return false;
        }
        let selected_row_id = selected_row_id.or_else(|| {
            self.menu_syntax_trigger_picker_state
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
        let outcome =
            crate::menu_syntax::apply_intent(intent, &snapshot, selected_index, &raw_filter_text);

        match outcome {
            crate::menu_syntax::TriggerPickerIntentOutcome::SelectionChanged { new_index } => {
                let next_row_id = snapshot.rows.get(new_index).map(|row| row.id.clone());
                if self.menu_syntax_trigger_picker_owns_main_keyboard()
                    && !self.menu_syntax_trigger_picker_state_is_form_suggestion()
                {
                    let Some(id) = next_row_id.as_deref() else {
                        return false;
                    };
                    let Some(index) = self.main_menu_committed_rows().iter().find_map(|row| {
                        (row.eligibility.selectable
                            && row.stable_key.strip_prefix("menu-syntax-trigger:") == Some(id))
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
                        "menu_syntax_trigger_picker_selection",
                    );
                    self.schedule_main_list_selection_reveal_above_footer(
                        "menu_syntax_trigger_picker_selection",
                        cx,
                    );
                }
                self.menu_syntax_trigger_picker_state.visible_start =
                    crate::menu_syntax_trigger_picker::trigger_picker_visible_start_for_selection(
                        self.menu_syntax_trigger_picker_state.visible_start,
                        new_index,
                        snapshot.rows.len(),
                    );
                self.menu_syntax_trigger_picker_state.selected_row_id = next_row_id;
                let selected_row_id = self
                    .menu_syntax_trigger_picker_state
                    .selected_row_id
                    .clone();
                self.sync_menu_syntax_form_selection_from_trigger_row(selected_row_id.as_deref());
                cx.notify();
                true
            }
            crate::menu_syntax::TriggerPickerIntentOutcome::Ignored => false,
            other => {
                self.dispatch_menu_syntax_trigger_picker_outcome(None, other, Some(window), cx);
                if let Some(observation) = observation {
                    self.set_main_menu_dispatch_observation(Some(observation));
                }
                true
            }
        }
    }
}
