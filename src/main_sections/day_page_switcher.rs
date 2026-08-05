// Day Page past-day switcher (Cmd+P): find and swap to past days.
//
// The switcher lists `brain/days/YYYY-MM-DD.md` files newest-first with a
// first-line preview, filters on typed query text, and rebinds the Day Page
// document session to the chosen date. While open, focus moves from the
// editor to the Day Page root focus handle so typed characters filter the
// switcher instead of editing the document.

// The former inline `DaySwitcherState`/`DaySwitcherEntry` machinery and its
// free helpers were dead (the field was never populated). The live past-day
// switcher is the `note_switcher` CommandBar popup below.

impl DayPageView {
    fn local_today(&self) -> chrono::NaiveDate {
        Utc::now()
            .with_timezone(&self.session.substrate().timezone())
            .date_naive()
    }

    pub(crate) fn is_day_switcher_open(&self) -> bool {
        self.note_switcher.is_open()
    }

    pub(crate) fn toggle_day_switcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.note_switcher.is_open() {
            self.close_day_switcher(window, cx);
        } else {
            self.open_note_switcher(window, cx);
        }
    }

    pub(crate) fn open_day_switcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_note_switcher(window, cx);
    }

    pub(crate) fn open_note_switcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = crate::notes::init_notes_db() {
            tracing::warn!(
                target: "script_kit::day_page",
                error = %error,
                "day_page_note_switcher_notes_init_failed"
            );
        }
        let destination = crate::notes::search_model::NoteSearchDestination::OpenHere;
        let search_state = crate::notes::search_model::load_note_search_state(
            "",
            &crate::notes::notes_brain_days_dir(),
            1,
            None,
        );
        let current_id = self
            .session
            .viewing_note_id()
            .and_then(crate::notes::NoteId::parse)
            .map(crate::notes::search_model::NoteSearchDocumentId::Note)
            .or_else(|| {
                self.session
                    .bound_date()
                    .map(crate::notes::search_model::NoteSearchDocumentId::Day)
            });
        let actions = crate::actions::get_canonical_note_search_actions(&search_state, current_id);
        self.note_switcher.set_actions(actions, cx);
        self.note_switcher.open_centered(window, cx);
        self.wire_note_switcher_activation(window, cx);
        if let Some(dialog) = self.note_switcher.dialog() {
            dialog.update(cx, |dialog, cx| {
                dialog.set_context_title(Some(destination.primary_verb().to_string()));
                cx.notify();
            });
        }
    }

    fn wire_note_switcher_activation(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.note_switcher.dialog().cloned() else {
            return;
        };
        let day_page = cx.entity().downgrade();
        let day_window = window.window_handle();
        let on_close_day_page = day_page.clone();
        dialog.update(cx, |dialog, _cx| {
            dialog.set_on_close(std::sync::Arc::new(move |cx| {
                let day_page = on_close_day_page.clone();
                cx.defer(move |cx| {
                    let _ = day_window.update(cx, |_root, window, cx| {
                        let Some(day_page) = day_page.upgrade() else {
                            return;
                        };
                        day_page.update(cx, |view, cx| {
                            view.note_switcher.mark_closed_externally();
                            view.focus_editor(window, cx);
                        });
                    });
                });
            }));
            dialog.set_on_activation(std::sync::Arc::new(move |activation, _window, cx| {
                match activation {
                    crate::actions::ActionsDialogActivation::Executed { action_id, .. } => {
                        let day_page = day_page.clone();
                        cx.defer(move |cx| {
                            let _ = day_window.update(cx, |_root, window, cx| {
                                let Some(day_page) = day_page.upgrade() else {
                                    return;
                                };
                                day_page.update(cx, |view, cx| {
                                    view.execute_note_switcher_action(&action_id, window, cx);
                                });
                            });
                        });
                    }
                    crate::actions::ActionsDialogActivation::DrillDownPushed { .. }
                    | crate::actions::ActionsDialogActivation::Blocked { .. }
                    | crate::actions::ActionsDialogActivation::NoSelection => {}
                }
            }));
        });
    }

    fn execute_note_switcher_action(
        &mut self,
        action_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let is_local_switch_target =
            action_id.starts_with("daypage_") || action_id.starts_with("note_");
        if is_local_switch_target
            && self.session.is_dirty()
            && !self.save_and_sync_footer(window, cx)
        {
            tracing::warn!(
                target: "script_kit::day_page",
                action_id,
                "day_page_note_switcher_dirty_save_failed"
            );
            cx.notify();
            return;
        }
        self.note_switcher.close(cx);
        if let Some(date) = action_id.strip_prefix("daypage_") {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
                self.bind_day(date, window, cx);
                self.focus_editor(window, cx);
            }
            return;
        }
        let Some(note_id_text) = action_id.strip_prefix("note_") else {
            return;
        };
        if let Some(date) = crate::notes::day_switcher::parse_day_note_action_id(note_id_text) {
            self.bind_day(date, window, cx);
            self.focus_editor(window, cx);
            return;
        }
        if let Some(note_id) = crate::notes::NoteId::parse(note_id_text) {
            let Ok(Some(note)) = crate::notes::get_note(note_id) else {
                return;
            };
            let path = crate::notes::note_file_path(note.id).ok().flatten();
            if let Err(error) = self.session.bind_note_content(
                note.id.as_str().to_string(),
                note.title.clone(),
                note.content.clone(),
                path,
                Utc::now(),
            ) {
                tracing::error!(
                    target: "script_kit::day_page",
                    error = %error,
                    "day_page_note_switcher_bind_note_failed"
                );
                return;
            }
            self.apply_loaded_content_to_editor(window, cx);
            self.focus_editor(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn close_day_switcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.note_switcher.close(cx);
        self.focus_editor(window, cx);
        cx.notify();
    }

    /// Rebind the Day Page session to an arbitrary existing day.
    pub(crate) fn bind_day(
        &mut self,
        date: chrono::NaiveDate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.session.bind_date(date, Utc::now()) {
            tracing::error!(
                target: "script_kit::day_page",
                event = "day_page_bind_day_failed",
                date = %date,
                error = %error,
            );
            return;
        }
        self.apply_loaded_content_to_editor(window, cx);
        cx.notify();
    }

    /// Handle a key while the switcher is open. Returns true when consumed.
    pub(crate) fn handle_day_switcher_key(
        &mut self,
        key: &str,
        cmd: bool,
        shift: bool,
        alt: bool,
        control: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.note_switcher.is_open() {
            return false;
        }
        let exact_plain = !cmd && !shift && !alt && !control;
        if exact_plain && crate::ui_foundation::is_key_escape(key) {
            self.close_day_switcher(window, cx);
            return true;
        }
        if cmd && !shift && !alt && !control && key == "p" {
            self.close_day_switcher(window, cx);
            return true;
        }
        if exact_plain && matches!(key, "down" | "arrowdown" | "up" | "arrowup") {
            if matches!(key, "down" | "arrowdown") {
                self.note_switcher.select_next(cx);
            } else {
                self.note_switcher.select_prev(cx);
            }
            return true;
        }
        if exact_plain && key == "enter" {
            if let Some(action_id) = self.note_switcher.execute_selected_action(cx) {
                self.execute_note_switcher_action(&action_id, window, cx);
            }
            return true;
        }
        if exact_plain && key == "backspace" {
            self.note_switcher.handle_backspace(window, cx);
            return true;
        }
        if alt && !cmd && !shift && !control && key == "backspace" {
            self.note_switcher.handle_backspace_word(window, cx);
            return true;
        }
        if cmd && !shift && !alt && !control && key == "v" {
            self.note_switcher.handle_paste(window, cx);
            return true;
        }
        if !cmd && !control && !alt {
            let ch = if key == "space" {
                Some(' ')
            } else if key.chars().count() == 1 {
                key.chars().next()
            } else {
                None
            };
            if let Some(ch) = ch {
                self.note_switcher.handle_char(ch, window, cx);
                return true;
            }
        }
        // Swallow everything else so stray keys cannot edit the document
        // while the switcher overlays it.
        true
    }
}
